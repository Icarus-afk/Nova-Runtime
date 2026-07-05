use std::sync::Arc;
use std::time::Duration;

fn setup() -> (tempfile::TempDir, Arc<nova_storage::Store>, Arc<nova_executor::PipelineExecutor>, Arc<parking_lot::RwLock<nova_config::Config>>) {
    let tmp = tempfile::tempdir().unwrap();
    let data_dir = tmp.path().join("data");
    let wal_dir = data_dir.join("wal");

    let storage_config = nova_storage::StorageConfig {
        data_dir: data_dir.clone(),
        wal_dir,
        page_cache_size: 64,
        memtable_size: 1024 * 1024,
        fsync_policy: nova_core::FsyncPolicy::Async,
        btree_order: 16,
    };
    let store = Arc::new(nova_storage::Store::open(&storage_config).unwrap());

    let pipeline = Arc::new(nova_executor::PipelineExecutor::new(
        nova_executor::PipelineConfig::default(),
    ));

    let config = Arc::new(parking_lot::RwLock::new(
        nova_config::Config::default(),
    ));

    (tmp, store, pipeline, config)
}

fn start_server(addr: &str, pipeline: Arc<nova_executor::PipelineExecutor>, config: Arc<parking_lot::RwLock<nova_config::Config>>, shutdown_rx: tokio::sync::watch::Receiver<bool>) -> tokio::task::JoinHandle<()> {
    let admin_state = Arc::new(nova_api::admin::AdminState {
        started_at: std::time::Instant::now(),
        pipeline,
        config,
        memory_mgr: None,
        sql_engine: None,
        cache_mgr: None,
        queue_mgr: None,
        scheduler_mgr: None,
        search_mgr: None,
        blob_mgr: None,
        auth_mgr: None,
        storage_ok: true,
    });
    let addr = addr.to_string();
    tokio::spawn(async move {
        nova_api::server::start_server(&addr, admin_state, shutdown_rx, None)
            .await
            .unwrap();
    })
}

fn bind_random() -> (String, u16) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    (format!("127.0.0.1:{}", port), port)
}

#[tokio::test]
async fn full_startup_health_check_and_shutdown() {
    let (_tmp, store, pipeline, config) = setup();
    let (addr, _port) = bind_random();
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    let handle = start_server(&addr, pipeline.clone(), config.clone(), shutdown_rx);
    tokio::time::sleep(Duration::from_millis(200)).await;

    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    let resp = client.get(format!("{}/health", base)).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "healthy");
    assert_eq!(body["checks"]["storage"], true);
    assert_eq!(body["checks"]["memory"], true);

    let resp = client.get(format!("{}/ready", base)).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "ready");

    let resp = client.get(format!("{}/live", base)).send().await.unwrap();
    assert_eq!(resp.status(), 200);
    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "alive");

    shutdown_tx.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(5), handle)
        .await
        .expect("server did not shut down within 5s")
        .expect("server panicked");

    store.close().unwrap();
}

#[tokio::test]
async fn metrics_endpoint_returns_prometheus_format() {
    let (_tmp, store, pipeline, config) = setup();
    let (addr, _port) = bind_random();
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    let handle = start_server(&addr, pipeline.clone(), config.clone(), shutdown_rx);
    tokio::time::sleep(Duration::from_millis(200)).await;

    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    let resp = client.get(format!("{}/metrics", base)).send().await.unwrap();
    assert_eq!(resp.status(), 200);

    let text = resp.text().await.unwrap();
    assert!(text.contains("nova_uptime_secs"));
    assert!(text.contains("nova_operations_total"));
    assert!(text.contains("# HELP"));
    assert!(text.contains("# TYPE"));

    shutdown_tx.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(5), handle)
        .await
        .expect("server did not shut down within 5s")
        .expect("server panicked");

    store.close().unwrap();
}

#[tokio::test]
async fn admin_config_endpoint_returns_config() {
    let (_tmp, store, pipeline, config) = setup();
    let (addr, _port) = bind_random();
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    let handle = start_server(&addr, pipeline.clone(), config.clone(), shutdown_rx);
    tokio::time::sleep(Duration::from_millis(200)).await;

    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    let resp = client.get(format!("{}/admin/config", base)).send().await.unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["version"], 1);
    assert!(body.get("general").is_some());
    assert!(body.get("storage").is_some());
    assert!(body.get("memory").is_some());
    assert!(body.get("networking").is_some());
    assert!(body.get("execution").is_some());
    assert_eq!(body["networking"]["listen_address"], "127.0.0.1");
    assert_eq!(body["networking"]["tls_enabled"], false);

    shutdown_tx.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(5), handle)
        .await
        .expect("server did not shut down within 5s")
        .expect("server panicked");

    store.close().unwrap();
}

#[tokio::test]
async fn pipeline_status_endpoint_returns_correct_values() {
    let (_tmp, store, pipeline, config) = setup();
    let (addr, _port) = bind_random();
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    let handle = start_server(&addr, pipeline.clone(), config.clone(), shutdown_rx);
    tokio::time::sleep(Duration::from_millis(200)).await;

    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);

    let resp = client.get(format!("{}/admin/status", base)).send().await.unwrap();
    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["is_running"], true);
    assert_eq!(body["is_draining"], false);
    assert!(body["active_operations"].as_u64().is_some());
    assert!(body["total_operations"].as_u64().is_some());
    assert!(body.get("metrics").is_some());
    assert_eq!(body["metrics"]["operations_total"], 0);
    assert_eq!(body["metrics"]["queue_depth"], 0);

    shutdown_tx.send(true).unwrap();
    tokio::time::timeout(Duration::from_secs(5), handle)
        .await
        .expect("server did not shut down within 5s")
        .expect("server panicked");

    store.close().unwrap();
}

#[tokio::test]
async fn shutdown_channel_triggers_clean_shutdown() {
    let (_tmp, store, pipeline, config) = setup();
    let (addr, _port) = bind_random();
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    let handle = start_server(&addr, pipeline.clone(), config.clone(), shutdown_rx);
    tokio::time::sleep(Duration::from_millis(200)).await;

    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);
    let resp = client.get(format!("{}/health", base)).send().await.unwrap();
    assert_eq!(resp.status(), 200, "server should be reachable before shutdown");

    shutdown_tx.send(true).unwrap();

    tokio::time::timeout(Duration::from_secs(5), handle)
        .await
        .expect("server did not shut down within 5s")
        .expect("server panicked");

    let err = client.get(format!("{}/health", base)).send().await;
    assert!(err.is_err(), "server should no longer accept requests after shutdown");

    pipeline.drain(Duration::from_secs(5)).await.unwrap();
    store.close().unwrap();
}

#[tokio::test]
async fn shutdown_tx_drop_triggers_shutdown() {
    let (_tmp, store, pipeline, config) = setup();
    let (addr, _port) = bind_random();
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    let handle = start_server(&addr, pipeline.clone(), config.clone(), shutdown_rx);
    tokio::time::sleep(Duration::from_millis(200)).await;

    let client = reqwest::Client::new();
    let base = format!("http://{}", addr);
    let resp = client.get(format!("{}/health", base)).send().await.unwrap();
    assert_eq!(resp.status(), 200, "server should be reachable before shutdown");

    drop(shutdown_tx);

    tokio::time::timeout(Duration::from_secs(5), handle)
        .await
        .expect("server did not shut down within 5s when sender was dropped")
        .expect("server panicked");

    store.close().unwrap();
}
