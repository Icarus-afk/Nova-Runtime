use std::sync::Arc;
use std::time::Duration;

use nova_cache::backend::{CacheBackend, HashMapBackend};
use nova_cache::metrics::CacheMetrics;

fn make_backend() -> HashMapBackend {
    HashMapBackend::new(1024 * 1024, Arc::new(CacheMetrics::default()))
}

fn make_small_backend() -> HashMapBackend {
    HashMapBackend::new(190, Arc::new(CacheMetrics::default()))
}

#[tokio::test]
async fn test_basic_put_get() {
    let backend = make_backend();
    backend.set("hello".into(), b"world".to_vec(), None).await.unwrap();
    let result = backend.get(&"hello".into()).await.unwrap();
    assert_eq!(result, Some(b"world".to_vec()));
}

#[tokio::test]
async fn test_get_missing() {
    let backend = make_backend();
    let result = backend.get(&"nonexistent".into()).await.unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn test_delete() {
    let backend = make_backend();
    backend.set("tmp".into(), b"data".to_vec(), None).await.unwrap();
    assert!(backend.delete(&"tmp".into()).await.unwrap());
    assert!(!backend.delete(&"tmp".into()).await.unwrap());
    let result = backend.get(&"tmp".into()).await.unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn test_ttl_expiry() {
    let backend = make_backend();
    backend
        .set("short".into(), b"lived".to_vec(), Some(Duration::from_millis(10)))
        .await
        .unwrap();
    assert!(backend.get(&"short".into()).await.unwrap().is_some());
    tokio::time::sleep(Duration::from_millis(50)).await;
    let result = backend.get(&"short".into()).await.unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn test_lru_eviction() {
    let backend = make_small_backend();
    backend.set("a".into(), b"1".to_vec(), None).await.unwrap();
    backend.set("b".into(), b"2".to_vec(), None).await.unwrap();
    backend.set("c".into(), b"3".to_vec(), None).await.unwrap();
    let result = backend.get(&"a".into()).await.unwrap();
    assert_eq!(result, None);
    assert!(backend.get(&"b".into()).await.unwrap().is_some());
    assert!(backend.get(&"c".into()).await.unwrap().is_some());
}

#[tokio::test]
async fn test_concurrent_access() {
    let backend = Arc::new(make_backend());
    let mut handles = Vec::new();
    for i in 0..10 {
        let b = Arc::clone(&backend);
        handles.push(tokio::spawn(async move {
            let key = format!("concurrent_key_{}", i);
            let val = format!("concurrent_val_{}", i);
            b.set(key.clone(), val.into_bytes(), None).await.unwrap();
            let result = b.get(&key).await.unwrap();
            assert!(result.is_some());
        }));
    }
    for h in handles {
        h.await.unwrap();
    }
}

#[tokio::test]
async fn test_flush() {
    let backend = make_backend();
    backend.set("x".into(), b"1".to_vec(), None).await.unwrap();
    backend.set("y".into(), b"2".to_vec(), None).await.unwrap();
    assert_eq!(backend.len().await.unwrap(), 2);
    backend.flush().await.unwrap();
    assert_eq!(backend.len().await.unwrap(), 0);
    assert!(backend.is_empty().await.unwrap());
}

#[tokio::test]
async fn test_overwrite() {
    let backend = make_backend();
    backend.set("key".into(), b"first".to_vec(), None).await.unwrap();
    backend.set("key".into(), b"second".to_vec(), None).await.unwrap();
    let result = backend.get(&"key".into()).await.unwrap();
    assert_eq!(result, Some(b"second".to_vec()));
}

#[tokio::test]
async fn test_metrics_tracking() {
    let metrics = Arc::new(CacheMetrics::default());
    let backend = HashMapBackend::new(1024 * 1024, Arc::clone(&metrics));
    backend.get(&"miss".into()).await.unwrap();
    assert_eq!(metrics.misses(), 1);
    backend.set("hit".into(), b"v".to_vec(), None).await.unwrap();
    backend.get(&"hit".into()).await.unwrap();
    assert_eq!(metrics.hits(), 1);
    assert_eq!(metrics.sets(), 1);
    backend.delete(&"hit".into()).await.unwrap();
    assert_eq!(metrics.deletes(), 1);
}

#[tokio::test]
async fn test_eviction_metrics() {
    let metrics = Arc::new(CacheMetrics::default());
    let backend = HashMapBackend::new(128, metrics.clone());
    backend.set("a".into(), b"1".to_vec(), None).await.unwrap();
    backend.set("b".into(), b"2".to_vec(), None).await.unwrap();
    backend.set("c".into(), b"3".to_vec(), None).await.unwrap();
    assert!(metrics.evictions() > 0);
}

#[tokio::test]
async fn test_entry_size_accounting() {
    let metrics = Arc::new(CacheMetrics::default());
    let backend = HashMapBackend::new(1024 * 1024, metrics);
    assert_eq!(backend.len().await.unwrap(), 0);
    backend.set("k".into(), b"v".to_vec(), None).await.unwrap();
    assert_eq!(backend.len().await.unwrap(), 1);
}

#[tokio::test]
async fn test_delete_nonexistent() {
    let backend = make_backend();
    assert!(!backend.delete(&"nothing".into()).await.unwrap());
}

#[tokio::test]
async fn test_exists() {
    let backend = make_backend();
    assert!(!backend.exists(&"missing".into()).await.unwrap());
    backend.set("present".into(), b"!".to_vec(), None).await.unwrap();
    assert!(backend.exists(&"present".into()).await.unwrap());
}

#[tokio::test]
async fn test_exists_with_ttl() {
    let backend = make_backend();
    backend
        .set("ephemeral".into(), b"x".to_vec(), Some(Duration::from_millis(10)))
        .await
        .unwrap();
    assert!(backend.exists(&"ephemeral".into()).await.unwrap());
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(!backend.exists(&"ephemeral".into()).await.unwrap());
}

#[tokio::test]
async fn test_is_empty() {
    let backend = make_backend();
    assert!(backend.is_empty().await.unwrap());
    backend.set("item".into(), b"1".to_vec(), None).await.unwrap();
    assert!(!backend.is_empty().await.unwrap());
}

#[tokio::test]
async fn test_large_values() {
    let metrics = Arc::new(CacheMetrics::default());
    let backend = HashMapBackend::new(10_000, metrics);
    let large_val = vec![0u8; 5_000];
    backend.set("large".into(), large_val, None).await.unwrap();
    let result = backend.get(&"large".into()).await.unwrap();
    assert_eq!(result.unwrap().len(), 5_000);
}
