use std::process::{Command, Stdio};
use std::time::Duration;
use tempfile::tempdir;
use nix::sys::signal::{self, Signal};
use nix::unistd::Pid;
use reqwest::blocking::Client;
use std::io::BufRead;
use std::fs;

fn spawn_novad(config_path: &str) -> std::process::Child {
    Command::new(env!("CARGO_BIN_EXE_novad"))
        .arg("--config")
        .arg(config_path)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start novad")
}

fn wait_for_server_ready(child: &mut std::process::Child) -> Option<u16> {
    let client = Client::builder()
        .timeout(Duration::from_secs(1))
        .build()
        .unwrap();
    let start = std::time::Instant::now();
    let timeout = Duration::from_secs(5);
    
    // Get the port from child's stdout
    let stdout = child.stdout.as_mut().unwrap();
    let reader = std::io::BufReader::new(stdout);
    
    for line in reader.lines() {
        if let Ok(line) = line {
            if line.contains("Listen:") {
                if let Some(port_str) = line.split("Listen: 127.0.0.1:").last() {
                    if let Ok(port) = port_str.trim().parse::<u16>() {
                        // Wait for server to be ready
                        while start.elapsed() < timeout {
                            if let Ok(resp) = client.get(format!("http://localhost:{}/health", port)).send() {
                                if resp.status().is_success() {
                                    return Some(port);
                                }
                            }
                            std::thread::sleep(Duration::from_millis(200));
                        }
                        return Some(port);
                    }
                }
            }
        }
    }
    None
}

#[test]
fn test_startup_and_health_check() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let data_dir = dir.path().join("data");
    let wal_dir = dir.path().join("wal");
    fs::create_dir(&data_dir).unwrap();
    fs::create_dir(&wal_dir).unwrap();
    fs::write(
        &config_path,
        format!(r#"
        [server]
        host = "127.0.0.1"
        port = 0
        
        [general]
        data_dir = "{}"
        
        [storage]
        path = "{}/nova-test"
        wal_dir = "{}"
        "#, data_dir.display(), dir.path().display(), wal_dir.display())
    ).unwrap();
    
    let mut child = spawn_novad(config_path.to_str().unwrap());
    let port = wait_for_server_ready(&mut child).expect("Failed to get server port");
    let client = reqwest::blocking::Client::new();
    let resp = client.get(format!("http://localhost:{}/health", port)).send().unwrap();
    assert_eq!(resp.status(), 200);
    
    let resp = client.get(format!("http://localhost:{}/ready", port)).send().unwrap();
    assert_eq!(resp.status(), 200);
    
    let resp = client.get(format!("http://localhost:{}/live", port)).send().unwrap();
    assert_eq!(resp.status(), 200);
    
    child.kill().unwrap();
}

#[test]
fn test_signal_shutdown() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let data_dir = dir.path().join("data");
    fs::create_dir(&data_dir).unwrap();
    let wal_dir = dir.path().join("wal");
    fs::create_dir(&wal_dir).unwrap();
    fs::write(
        &config_path,
        format!(r#"
        [server]
        host = "127.0.0.1"
        port = 0
        
        [general]
        data_dir = "{}"
        
        [storage]
        path = "{}/nova-test"
        wal_dir = "{}"
        "#, data_dir.display(), dir.path().display(), wal_dir.display())
    ).unwrap();
    
    let mut child = spawn_novad(config_path.to_str().unwrap());
    
    let _port = wait_for_server_ready(&mut child).expect("Failed to get server port");
    signal::kill(Pid::from_raw(child.id() as i32), Signal::SIGTERM).unwrap();
    
    let status = child.wait().unwrap();
    assert!(status.success());
}

#[test]
fn test_sighup_reload() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    fs::write(
        &config_path,
        r#"
        [server]
        host = "127.0.0.1"
        port = 3003
        "#,
    ).unwrap();
    

    let mut child = spawn_novad(config_path.to_str().unwrap());

    let port = wait_for_server_ready(&mut child).expect("Failed to get server port");
        // Modify config
        let data_dir = dir.path().join("data-reload");
        fs::create_dir(&data_dir).unwrap();
        let wal_dir = dir.path().join("wal-reload");
        fs::create_dir(&wal_dir).unwrap();
        fs::write(
            &config_path,
            format!(r#"
            [server]
            host = "127.0.0.1"
        port = 0
            
            [general]
            data_dir = "{}/data"
            
            [storage]
            path = "{}/nova-reload-test"
            wal_dir = "{}"
            "#, dir.path().display(), dir.path().display(), wal_dir.display())
        ).unwrap();
        
        signal::kill(Pid::from_raw(child.id() as i32), Signal::SIGHUP).unwrap();
        
        // Give time for reload
        std::thread::sleep(Duration::from_secs(1));
        
    let client = Client::builder()
        .timeout(Duration::from_secs(1))
        .build()
        .unwrap();
        let resp = client.get(format!("http://localhost:{}/health", port)).send().unwrap();
        assert_eq!(resp.status(), 200);
    child.kill().unwrap();
}

#[test]
fn test_graceful_shutdown() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    let data_dir = dir.path().join("data");
    fs::create_dir(&data_dir).unwrap();
    let wal_dir = dir.path().join("wal");
    fs::create_dir(&wal_dir).unwrap();
    fs::write(
        &config_path,
        format!(r#"
        [server]
        host = "127.0.0.1"
        port = 0
        
        [general]
        data_dir = "{}"
        
        [storage]
        path = "{}/nova-test"
        wal_dir = "{}"
        "#, data_dir.display(), dir.path().display(), wal_dir.display())
    ).unwrap();
    
    let mut child = spawn_novad(config_path.to_str().unwrap());
    
    let _port = wait_for_server_ready(&mut child).expect("Failed to get server port");
        signal::kill(Pid::from_raw(child.id() as i32), Signal::SIGINT).unwrap();
        
        let status = child.wait().unwrap();
        assert!(status.success());
}

#[test]
fn test_invalid_config() {
    let dir = tempdir().unwrap();
    let config_path = dir.path().join("config.toml");
    fs::write(
        &config_path,
        r#"
        [invalid_section]
        key = "value"
        "#,
    ).unwrap();
    
    let child = spawn_novad(config_path.to_str().unwrap());
    let status = child.wait_with_output().unwrap();
    
    assert!(!status.status.success());
}