use std::sync::Arc;
use std::time::Duration;

use nova_cache::backend::{CacheBackend, HashMapBackend};
use nova_cache::metrics::CacheMetrics;

fn make_backend() -> HashMapBackend {
    HashMapBackend::new(1024 * 1024, Arc::new(CacheMetrics::default())).unwrap()
}

fn make_small_backend() -> HashMapBackend {
    HashMapBackend::new(18, Arc::new(CacheMetrics::default())).unwrap()
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
    backend.set("aaaa".into(), b"11111".to_vec(), None).await.unwrap();
    backend.set("bbbb".into(), b"22222".to_vec(), None).await.unwrap();
    backend.set("cccc".into(), b"33333".to_vec(), None).await.unwrap();
    let result = backend.get(&"aaaa".into()).await.unwrap();
    assert_eq!(result, None);
    assert!(backend.get(&"bbbb".into()).await.unwrap().is_some());
    assert!(backend.get(&"cccc".into()).await.unwrap().is_some());
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
    let backend = HashMapBackend::new(1024 * 1024, Arc::clone(&metrics)).unwrap();
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
    let backend = HashMapBackend::new(60, metrics.clone()).unwrap();
    let big = b"xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx".to_vec();
    backend.set("a".into(), big.clone(), None).await.unwrap();
    backend.set("b".into(), big.clone(), None).await.unwrap();
    assert!(metrics.evictions() > 0);
}

#[tokio::test]
async fn test_entry_size_accounting() {
    let metrics = Arc::new(CacheMetrics::default());
    let backend = HashMapBackend::new(1024 * 1024, metrics).unwrap();
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
    let backend = HashMapBackend::new(10_000, metrics).unwrap();
    let large_val = vec![0u8; 5_000];
    backend.set("large".into(), large_val, None).await.unwrap();
    let result = backend.get(&"large".into()).await.unwrap();
    assert_eq!(result.unwrap().len(), 5_000);
}

// ============================================================
// CACHE-008: Batch operations + get_or_insert_with
// ============================================================

#[tokio::test]
async fn test_get_or_insert_with_miss_calls_factory() {
    let backend = Arc::new(make_backend());
    let factory_called = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let factory_flag = Arc::clone(&factory_called);

    let val = backend
        .get_or_insert_with(
            "computed".into(),
            Box::new(move || {
                let flag = Arc::clone(&factory_flag);
                Box::pin(async move {
                    flag.store(true, std::sync::atomic::Ordering::SeqCst);
                    Ok(b"factory_value".to_vec())
                })
            }),
            None,
        )
        .await
        .unwrap();

    assert_eq!(val, b"factory_value".to_vec());
    assert!(factory_called.load(std::sync::atomic::Ordering::SeqCst));

    let cached = backend.get(&"computed".into()).await.unwrap();
    assert_eq!(cached, Some(b"factory_value".to_vec()));
}

#[tokio::test]
async fn test_get_or_insert_with_hit_returns_cached() {
    let backend = Arc::new(make_backend());
    backend.set("hit".into(), b"cached".to_vec(), None).await.unwrap();

    let factory_called = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let factory_flag = Arc::clone(&factory_called);

    let val = backend
        .get_or_insert_with(
            "hit".into(),
            Box::new(move || {
                let flag = Arc::clone(&factory_flag);
                Box::pin(async move {
                    flag.store(true, std::sync::atomic::Ordering::SeqCst);
                    Ok(b"factory_value".to_vec())
                })
            }),
            None,
        )
        .await
        .unwrap();

    assert_eq!(val, b"cached".to_vec());
    assert!(!factory_called.load(std::sync::atomic::Ordering::SeqCst));
}

#[tokio::test]
async fn test_get_many() {
    let backend = make_backend();
    backend.set("a".into(), b"1".to_vec(), None).await.unwrap();
    backend.set("b".into(), b"2".to_vec(), None).await.unwrap();
    backend.set("c".into(), b"3".to_vec(), None).await.unwrap();

    let results = backend
        .get_many(&["a".into(), "b".into(), "missing".into(), "c".into()])
        .await
        .unwrap();

    assert_eq!(results.len(), 3);
    assert!(results.contains(&("a".into(), b"1".to_vec())));
    assert!(results.contains(&("b".into(), b"2".to_vec())));
    assert!(results.contains(&("c".into(), b"3".to_vec())));
}

#[tokio::test]
async fn test_set_many() {
    let backend = make_backend();
    let items = vec![
        ("x".into(), b"10".to_vec(), None),
        ("y".into(), b"20".to_vec(), None),
        ("z".into(), b"30".to_vec(), None),
    ];

    backend.set_many(items).await.unwrap();

    assert_eq!(backend.get(&"x".into()).await.unwrap(), Some(b"10".to_vec()));
    assert_eq!(backend.get(&"y".into()).await.unwrap(), Some(b"20".to_vec()));
    assert_eq!(backend.get(&"z".into()).await.unwrap(), Some(b"30".to_vec()));
}

#[tokio::test]
async fn test_delete_many() {
    let backend = make_backend();
    backend.set("a".into(), b"1".to_vec(), None).await.unwrap();
    backend.set("b".into(), b"2".to_vec(), None).await.unwrap();
    backend.set("c".into(), b"3".to_vec(), None).await.unwrap();

    let count = backend
        .delete_many(&["a".into(), "c".into(), "missing".into()])
        .await
        .unwrap();

    assert_eq!(count, 2);
    assert!(backend.get(&"a".into()).await.unwrap().is_none());
    assert!(backend.get(&"b".into()).await.unwrap().is_some());
    assert!(backend.get(&"c".into()).await.unwrap().is_none());
}

// ============================================================
// CACHE-009: Background TTL sweeper
// ============================================================

#[tokio::test]
async fn test_ttl_sweeper() {
    let backend = Arc::new(HashMapBackend::new(1024 * 1024, Arc::new(CacheMetrics::default())).unwrap());

    backend
        .set("ephemeral".into(), b"x".to_vec(), Some(Duration::from_millis(20)))
        .await
        .unwrap();
    backend.set("permanent".into(), b"y".to_vec(), None).await.unwrap();

    let handle = Arc::clone(&backend).start_ttl_sweeper(Duration::from_millis(50));

    tokio::time::sleep(Duration::from_millis(150)).await;

    assert!(backend.get(&"ephemeral".into()).await.unwrap().is_none());
    assert!(backend.get(&"permanent".into()).await.unwrap().is_some());

    handle.abort();
}

// ============================================================
// CACHE-010: Entry size accounting
// ============================================================

#[tokio::test]
async fn test_size_accounting() {
    let metrics = Arc::new(CacheMetrics::default());
    let backend = HashMapBackend::new(10_000, Arc::clone(&metrics)).unwrap();

    let key = "test_key".to_string();
    let val = vec![0u8; 500];
    backend.set(key.clone(), val, None).await.unwrap();

    backend
        .set("other".into(), vec![0u8; 100], None)
        .await
        .unwrap();

    let tiny_backend = HashMapBackend::new(50, Arc::new(CacheMetrics::default())).unwrap();
    tiny_backend
        .set("aaaa".into(), vec![0u8; 30], None)
        .await
        .unwrap();
    tiny_backend
        .set("bbbb".into(), vec![0u8; 30], None)
        .await
        .unwrap();
    let result = tiny_backend.get(&"aaaa".into()).await.unwrap();
    assert_eq!(result, None);
    let result = tiny_backend.get(&"bbbb".into()).await.unwrap();
    assert_eq!(result.unwrap().len(), 30);
}

// ============================================================
// CACHE-007: Event invalidation
// ============================================================

#[tokio::test]
async fn test_event_invalidation() {
    use nova_cache::CacheManager;
    use nova_cache::config::CacheConfig;

    let backend = Arc::new(make_backend());
    let config = CacheConfig::default();
    let manager = CacheManager::new(Arc::clone(&backend) as Arc<dyn CacheBackend>, config);

    let bus = nova_event::EventBus::new(1, nova_event::OverflowPolicy::DropNewest, 1024 * 1024, 1000);
    manager.attach_event_bus(&bus).unwrap();

    backend.set("invalidate_me".into(), b"data".to_vec(), None).await.unwrap();
    backend.set("keep_me".into(), b"data".to_vec(), None).await.unwrap();

    let event = nova_event::EventBuilder::new("cache.invalidate.invalidate_me")
        .unwrap()
        .build(b"{}".to_vec());
    bus.publish(event).unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    assert!(backend.get(&"invalidate_me".into()).await.unwrap().is_none());
    assert_eq!(backend.get(&"keep_me".into()).await.unwrap(), Some(b"data".to_vec()));
}

#[tokio::test]
async fn test_event_pattern_invalidation() {
    use nova_cache::CacheManager;
    use nova_cache::config::CacheConfig;

    let backend = Arc::new(make_backend());
    let config = CacheConfig::default();
    let manager = CacheManager::new(Arc::clone(&backend) as Arc<dyn CacheBackend>, config);

    let bus = nova_event::EventBus::new(1, nova_event::OverflowPolicy::DropNewest, 1024 * 1024, 1000);
    manager.attach_event_bus(&bus).unwrap();

    backend.set("user:alice".into(), b"data".to_vec(), None).await.unwrap();
    backend.set("user:bob".into(), b"data".to_vec(), None).await.unwrap();
    backend.set("config:app".into(), b"data".to_vec(), None).await.unwrap();

    let event = nova_event::EventBuilder::new("cache.invalidate.pattern.user:*")
        .unwrap()
        .build(b"{}".to_vec());
    bus.publish(event).unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    assert!(backend.get(&"user:alice".into()).await.unwrap().is_none());
    assert!(backend.get(&"user:bob".into()).await.unwrap().is_none());
    assert_eq!(backend.get(&"config:app".into()).await.unwrap(), Some(b"data".to_vec()));
}

// ============================================================
// CACHE-002: Concurrent high contention
// ============================================================

#[tokio::test]
async fn test_concurrent_high_contention() {
    let backend = Arc::new(make_backend());

    let key = "contended".to_string();
    backend
        .set(key.clone(), b"initial".to_vec(), None)
        .await
        .unwrap();

    let mut handles = Vec::new();
    for _i in 0..50 {
        let b = Arc::clone(&backend);
        let k = key.clone();
        handles.push(tokio::spawn(async move {
            for _ in 0..10 {
                let _ = b.get(&k).await;
                let _ = b.exists(&k).await;
            }
        }));
    }
    for h in handles {
        h.await.unwrap();
    }

    let final_val = backend.get(&key).await.unwrap();
    assert_eq!(final_val, Some(b"initial".to_vec()));
}

// ============================================================
// CACHE-004: TtlBackend
// ============================================================

#[tokio::test]
async fn test_ttl_backend_expiry() {
    let inner = Box::new(make_backend());
    let backend = Arc::new(nova_cache::backend::TtlBackend::new(inner));

    backend
        .set("ephemeral".into(), b"x".to_vec(), Some(Duration::from_millis(10)))
        .await
        .unwrap();
    backend.set("permanent".into(), b"y".to_vec(), None).await.unwrap();

    assert!(backend.get(&"ephemeral".into()).await.unwrap().is_some());
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(backend.get(&"ephemeral".into()).await.unwrap().is_none());
    assert!(backend.get(&"permanent".into()).await.unwrap().is_some());
}

#[tokio::test]
async fn test_ttl_backend_sweeper() {
    let inner = Box::new(make_backend());
    let backend = Arc::new(nova_cache::backend::TtlBackend::new(inner));

    backend
        .set("ephemeral".into(), b"x".to_vec(), Some(Duration::from_millis(10)))
        .await
        .unwrap();
    backend.set("permanent".into(), b"y".to_vec(), None).await.unwrap();

    let handle = Arc::clone(&backend).start_ttl_sweeper(Duration::from_millis(30));

    tokio::time::sleep(Duration::from_millis(100)).await;

    assert!(backend.get(&"ephemeral".into()).await.unwrap().is_none());
    assert!(backend.get(&"permanent".into()).await.unwrap().is_some());

    handle.abort();
}
