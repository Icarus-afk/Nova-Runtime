use std::collections::HashMap;

use nova_blob::chunk::ChunkManager;
use nova_blob::config::BlobConfig;
use nova_blob::manager::BlobManager;

fn test_config(tmp_dir: &tempfile::TempDir) -> BlobConfig {
    BlobConfig {
        data_dir: tmp_dir.path().to_str().unwrap().to_string(),
        ..Default::default()
    }
}

fn test_config_small_chunks(tmp_dir: &tempfile::TempDir, chunk_size: usize) -> BlobConfig {
    BlobConfig {
        data_dir: tmp_dir.path().to_str().unwrap().to_string(),
        chunk_size,
        ..Default::default()
    }
}

#[tokio::test]
async fn test_create_and_get() {
    let dir = tempfile::tempdir().unwrap();
    let config = test_config(&dir);
    let mgr = BlobManager::new(config).await.unwrap();

    let data = b"Hello, Nova Blob Storage!";
    let meta = mgr
        .create_blob("test-ns", data, "text/plain", HashMap::new())
        .await
        .unwrap();

    assert_eq!(meta.size, data.len() as u64);
    assert!(!meta.id.is_empty());

    let retrieved = mgr.get_blob(&meta.id).await.unwrap();
    assert_eq!(retrieved, data);
}

#[tokio::test]
async fn test_metadata_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let config = test_config(&dir);
    let mgr = BlobManager::new(config).await.unwrap();

    let mut metadata = HashMap::new();
    metadata.insert("key1".to_string(), "val1".to_string());
    metadata.insert("key2".to_string(), "val2".to_string());

    let data = b"metadata test data";
    let meta = mgr
        .create_blob("meta-ns", data, "application/json", metadata.clone())
        .await
        .unwrap();

    let retrieved_meta = mgr.get_metadata(&meta.id).await.unwrap();
    assert_eq!(retrieved_meta.id, meta.id);
    assert_eq!(retrieved_meta.namespace, "meta-ns");
    assert_eq!(retrieved_meta.size, data.len() as u64);
    assert_eq!(retrieved_meta.content_type, "application/json");
    assert_eq!(retrieved_meta.sha256, meta.sha256);
    assert_eq!(retrieved_meta.chunk_count, meta.chunk_count);
    assert_eq!(retrieved_meta.metadata["key1"], "val1");
    assert_eq!(retrieved_meta.metadata["key2"], "val2");
}

#[tokio::test]
async fn test_delete() {
    let dir = tempfile::tempdir().unwrap();
    let config = test_config(&dir);
    let mgr = BlobManager::new(config).await.unwrap();

    let data = b"delete me";
    let meta = mgr
        .create_blob("del-ns", data, "text/plain", HashMap::new())
        .await
        .unwrap();

    mgr.delete_blob(&meta.id).await.unwrap();

    let result = mgr.get_blob(&meta.id).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_namespace_isolation() {
    let dir = tempfile::tempdir().unwrap();
    let config = test_config(&dir);
    let mgr = BlobManager::new(config).await.unwrap();

    mgr.create_blob("ns1", b"data for ns1", "text/plain", HashMap::new())
        .await
        .unwrap();
    mgr.create_blob("ns2", b"data for ns2", "text/plain", HashMap::new())
        .await
        .unwrap();

    let ns1_blobs = mgr.list_blobs("ns1").await.unwrap();
    let ns2_blobs = mgr.list_blobs("ns2").await.unwrap();

    assert_eq!(ns1_blobs.len(), 1);
    assert_eq!(ns2_blobs.len(), 1);

    // Blob from ns1 should not appear in ns2
    if !ns1_blobs.is_empty() && !ns2_blobs.is_empty() {
        assert_ne!(ns1_blobs[0], ns2_blobs[0]);
    }
}

#[tokio::test]
async fn test_range_download() {
    let dir = tempfile::tempdir().unwrap();
    let config = test_config(&dir);
    let mgr = BlobManager::new(config).await.unwrap();

    let data = b"Hello, World! This is a test blob for range download verification.";
    let meta = mgr
        .create_blob("range-ns", data, "text/plain", HashMap::new())
        .await
        .unwrap();

    let range = mgr.get_blob_range(&meta.id, 7, 5).await.unwrap();
    assert_eq!(range, b"World");

    let prefix = mgr.get_blob_range(&meta.id, 0, 5).await.unwrap();
    assert_eq!(prefix, b"Hello");

    let suffix = mgr
        .get_blob_range(&meta.id, data.len() as u64 - 13, 13)
        .await
        .unwrap();
    assert_eq!(suffix, b"verification.");
}

#[tokio::test]
async fn test_chunk_dedup() {
    let dir = tempfile::tempdir().unwrap();
    // Use a 4-byte chunk size so each 4-byte block is a separate chunk
    let config = test_config_small_chunks(&dir, 4);
    let mgr = BlobManager::new(config).await.unwrap();

    let shared = b"AAAA";
    let data1 = [shared.as_slice(), b"BBBB"].concat();
    let data2 = [shared.as_slice(), b"CCCC"].concat();

    let meta1 = mgr
        .create_blob("dedup-ns", &data1, "text/plain", HashMap::new())
        .await
        .unwrap();
    let meta2 = mgr
        .create_blob("dedup-ns", &data2, "text/plain", HashMap::new())
        .await
        .unwrap();

    let dedup = mgr.dedup();

    // The shared chunk "AAAA" should be referenced twice
    let shared_hash = ChunkManager::hash(shared);
    let ref_count = dedup.get_ref_count(&shared_hash);
    assert_eq!(ref_count, 2, "shared chunk should have ref_count=2");

    // Each blob can be fully retrieved
    assert_eq!(mgr.get_blob(&meta1.id).await.unwrap(), data1);
    assert_eq!(mgr.get_blob(&meta2.id).await.unwrap(), data2);
}

#[tokio::test]
async fn test_empty_blob() {
    let dir = tempfile::tempdir().unwrap();
    let config = test_config(&dir);
    let mgr = BlobManager::new(config).await.unwrap();

    let meta = mgr
        .create_blob("empty-ns", b"", "application/octet-stream", HashMap::new())
        .await
        .unwrap();

    assert_eq!(meta.size, 0);
    assert_eq!(meta.chunk_count, 1);

    let retrieved = mgr.get_blob(&meta.id).await.unwrap();
    assert!(retrieved.is_empty());
}

#[tokio::test]
async fn test_merkle_integrity() {
    use nova_blob::chunk::ChunkManager;
    use nova_blob::merkle::MerkleTree;

    let hashes: Vec<String> = vec![
        ChunkManager::hash(b"chunk1"),
        ChunkManager::hash(b"chunk2"),
        ChunkManager::hash(b"chunk3"),
        ChunkManager::hash(b"chunk4"),
    ];

    let root = MerkleTree::build(&hashes);

    // Verify each chunk with its proof
    for (i, hash) in hashes.iter().enumerate() {
        let proof = MerkleTree::generate_proof(&hashes, i);
        assert!(MerkleTree::verify(hash, &proof, &root));
    }

    // Verify a wrong hash fails
    let wrong_hash = ChunkManager::hash(b"wrong");
    let proof = MerkleTree::generate_proof(&hashes, 0);
    assert!(!MerkleTree::verify(&wrong_hash, &proof, &root));
}
