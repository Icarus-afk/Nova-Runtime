use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use nova_blob::chunk::ChunkManager;
use nova_blob::config::BlobConfig;
use nova_blob::gc::GarbageCollector;
use nova_blob::manager::BlobManager;
use nova_blob::namespace::validate_namespace;
use nova_blob::upload::UploadSession;

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

    let shared_hash = ChunkManager::hash(shared);
    let ref_count = dedup.get_ref_count(&shared_hash);
    assert_eq!(ref_count, 2, "shared chunk should have ref_count=2");

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

    for (i, hash) in hashes.iter().enumerate() {
        let proof = MerkleTree::generate_proof(&hashes, i);
        assert!(MerkleTree::verify(hash, &proof, &root));
    }

    let wrong_hash = ChunkManager::hash(b"wrong");
    let proof = MerkleTree::generate_proof(&hashes, 0);
    assert!(!MerkleTree::verify(&wrong_hash, &proof, &root));
}

#[tokio::test]
async fn test_namespace_validation() {
    let dir = tempfile::tempdir().unwrap();
    let config = test_config(&dir);
    let mgr = BlobManager::new(config).await.unwrap();

    let err = mgr.create_blob("", b"data", "text/plain", HashMap::new()).await;
    assert!(err.is_err(), "empty namespace should be rejected");

    let err = mgr.create_blob("/slash", b"data", "text/plain", HashMap::new()).await;
    assert!(err.is_err(), "namespace with '/' should be rejected");

    let err = mgr.create_blob("\\backslash", b"data", "text/plain", HashMap::new()).await;
    assert!(err.is_err(), "namespace with '\\' should be rejected");

    let err = mgr.create_blob("..", b"data", "text/plain", HashMap::new()).await;
    assert!(err.is_err(), "namespace with '..' should be rejected");

    let err = mgr.create_blob("a\0b", b"data", "text/plain", HashMap::new()).await;
    assert!(err.is_err(), "namespace with null byte should be rejected");

    let long_name = "a".repeat(256);
    let err = mgr.create_blob(&long_name, b"data", "text/plain", HashMap::new()).await;
    assert!(err.is_err(), "namespace >255 chars should be rejected");

    let result = mgr.create_blob("valid-ns_1.0", b"data", "text/plain", HashMap::new()).await;
    assert!(result.is_ok(), "valid namespace should be accepted");

    validate_namespace("").unwrap_err();
    validate_namespace("/bad").unwrap_err();
    validate_namespace("..").unwrap_err();
    validate_namespace("valid-name.1").unwrap();
}

#[tokio::test]
async fn test_blob_listing_pagination() {
    let dir = tempfile::tempdir().unwrap();
    let config = test_config(&dir);
    let mgr = BlobManager::new(config).await.unwrap();

    for i in 0..25 {
        let data = format!("blob-{}", i);
        mgr.create_blob("page-ns", data.as_bytes(), "text/plain", HashMap::new())
            .await
            .unwrap();
    }

    let (page1, total) = mgr.list_blobs_paginated("page-ns", 0, 10).await.unwrap();
    assert_eq!(page1.len(), 10, "first page should have 10 items");
    assert_eq!(total, 25, "total should be 25");

    let (page2, total2) = mgr.list_blobs_paginated("page-ns", 10, 10).await.unwrap();
    assert_eq!(page2.len(), 10, "second page should have 10 items");
    assert_eq!(total2, 25, "total should still be 25");

    let (page3, total3) = mgr.list_blobs_paginated("page-ns", 20, 10).await.unwrap();
    assert_eq!(page3.len(), 5, "third page should have 5 items");
    assert_eq!(total3, 25, "total should still be 25");

    let ids1: std::collections::HashSet<_> = page1.into_iter().collect();
    let ids2: std::collections::HashSet<_> = page2.into_iter().collect();
    let ids3: std::collections::HashSet<_> = page3.into_iter().collect();
    assert!(ids1.is_disjoint(&ids2), "pages should not overlap");
    assert!(ids1.is_disjoint(&ids3), "pages should not overlap");
    assert!(ids2.is_disjoint(&ids3), "pages should not overlap");
}

#[tokio::test]
async fn test_ttl_expiry() {
    let dir = tempfile::tempdir().unwrap();
    let config = test_config(&dir);
    let mgr = BlobManager::new(config).await.unwrap();

    let meta = mgr
        .create_blob("ttl-ns", b"expiring data", "text/plain", HashMap::new())
        .await
        .unwrap();

    let store = mgr.store().clone();
    store.delete_metadata(&meta.id).await.unwrap();
    let mut expired_meta = meta.clone();
    expired_meta.expires_at = Some(0);
    expired_meta.created_at = 0;
    store.put_metadata(&expired_meta).await.unwrap();

    mgr.run_gc().await.unwrap();

    let result = mgr.get_blob(&meta.id).await;
    assert!(result.is_err(), "expired blob should be deleted after GC");
}

#[tokio::test]
async fn test_gc_shutdown() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = test_config(&dir);
    config.gc_interval_secs = 1;
    let config_for_store = config.clone();

    let store = Arc::new(nova_blob::backend::filesystem::FilesystemBackend::new(&config_for_store));
    store.init().await.unwrap();
    let dedup = Arc::new(nova_blob::dedup::DeduplicationEngine::new());
    let gc = Arc::new(GarbageCollector::new(store, dedup, &config_for_store));

    let cancel = CancellationToken::new();
    let handle = GarbageCollector::start_background(gc.clone(), Duration::from_millis(100), cancel.clone());

    cancel.cancel();

    handle.await.unwrap();
}

#[tokio::test]
async fn test_dedup_persistence() {
    let dir = tempfile::tempdir().unwrap();
    let config = test_config(&dir);
    let shared = b"AAAA";
    let data1 = [shared.as_slice(), b"BBBB"].concat();
    let data2 = [shared.as_slice(), b"CCCC"].concat();

    let config2 = BlobConfig {
        chunk_size: 4,
        ..test_config(&dir)
    };

    let mgr2 = BlobManager::new(config2).await.unwrap();
    mgr2.create_blob("dedup-persist-ns", &data1, "text/plain", HashMap::new())
        .await
        .unwrap();
    mgr2.create_blob("dedup-persist-ns", &data2, "text/plain", HashMap::new())
        .await
        .unwrap();

    mgr2.save_dedup_state().await.unwrap();

    let shared_hash = ChunkManager::hash(shared);
    assert_eq!(mgr2.dedup().get_ref_count(&shared_hash), 2);

    let config3 = BlobConfig {
        data_dir: dir.path().to_str().unwrap().to_string(),
        ..Default::default()
    };
    let mgr3 = BlobManager::new(config3).await.unwrap();
    assert_eq!(mgr3.dedup().get_ref_count(&shared_hash), 2, "dedup state should persist across manager instances");
}

#[tokio::test]
async fn test_upload_part_listing() {
    let dir = tempfile::tempdir().unwrap();
    let config = test_config(&dir);
    let mgr = BlobManager::new(config).await.unwrap();

    let session: UploadSession = mgr
        .initiate_upload("list-parts-ns", "text/plain", HashMap::new(), 30)
        .await
        .unwrap();

    mgr.upload_part(&session.upload_id, b"hello ".to_vec()).await.unwrap();
    mgr.upload_part(&session.upload_id, b"world ".to_vec()).await.unwrap();
    mgr.upload_part(&session.upload_id, b"parts!".to_vec()).await.unwrap();

    let parts = mgr.list_parts(&session.upload_id).unwrap();
    assert_eq!(parts.len(), 3, "should have 3 parts");
    assert_eq!(parts[0].size, 6, "first part size should be 6");
    assert_eq!(parts[1].size, 6, "second part size should be 6");
    assert_eq!(parts[2].size, 6, "third part size should be 6");
    assert_eq!(parts[0].part_number, 1);
    assert_eq!(parts[1].part_number, 2);
    assert_eq!(parts[2].part_number, 3);
}

#[tokio::test]
async fn test_upload_size_validation() {
    let dir = tempfile::tempdir().unwrap();
    let config = test_config(&dir);
    let mgr = BlobManager::new(config).await.unwrap();

    let session: UploadSession = mgr
        .initiate_upload("size-val-ns", "text/plain", HashMap::new(), 20)
        .await
        .unwrap();

    mgr.upload_part(&session.upload_id, b"hello world".to_vec()).await.unwrap();

    let result = mgr.complete_upload(&session.upload_id).await;
    assert!(result.is_err(), "should reject when declared total_size (20) != actual uploaded (11)");

    let session2: UploadSession = mgr
        .initiate_upload("size-val-ns2", "text/plain", HashMap::new(), 11)
        .await
        .unwrap();
    mgr.upload_part(&session2.upload_id, b"hello world".to_vec()).await.unwrap();
    let result2 = mgr.complete_upload(&session2.upload_id).await;
    assert!(result2.is_ok(), "should accept when declared total matches actual");
}
