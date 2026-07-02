use crossbeam::channel;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditCategory {
    Authentication,
    Authorization,
    DataMutation,
    SchemaChange,
    ConfigurationChange,
    SecuritySetting,
    EncryptionEvent,
    NetworkEvent,
    AdminAction,
    SystemEvent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditAction {
    Login,
    LoginFailed,
    Logout,
    TokenRefresh,
    TokenRevoke,
    DocumentCreate,
    DocumentRead,
    DocumentUpdate,
    DocumentDelete,
    SchemaRegister,
    SchemaUpdate,
    SchemaDelete,
    ConfigReload,
    ConfigChange,
    KeyCreated,
    KeyRotated,
    KeyDeleted,
    ConnectionRejected,
    RateLimited,
    TlsHandshakeFailed,
    Startup,
    Shutdown,
    AdminCommand,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ActorType {
    User,
    Admin,
    Service,
    System,
    Anonymous,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditActor {
    pub actor_type: ActorType,
    pub id: String,
    pub name: Option<String>,
    pub ip_address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceType {
    Document,
    Collection,
    Schema,
    Config,
    Key,
    User,
    Session,
    Connection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditResource {
    pub resource_type: ResourceType,
    pub id: Option<String>,
    pub collection: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AuditStatus {
    Success,
    Denied,
    Failure,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditResult {
    pub status: AuditStatus,
    pub duration_ms: u64,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditEvent {
    pub id: Uuid,
    pub timestamp: u64,
    pub category: AuditCategory,
    pub action: AuditAction,
    pub actor: AuditActor,
    pub resource: AuditResource,
    pub result: AuditResult,
    pub context: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub enum LogRotation {
    None,
    Size { max_bytes: u64, max_files: u32 },
    Daily,
}

#[derive(Debug, Clone)]
pub enum AuditOutput {
    Stdout,
    File { path: PathBuf, rotation: LogRotation },
}

pub struct AuditLogger {
    sender: channel::Sender<AuditEvent>,
    outputs: Arc<Vec<AuditOutput>>,
    _handle: std::thread::JoinHandle<()>,
}

fn write_event_to_output(output: &AuditOutput, event: &AuditEvent) {
    let json = match serde_json::to_string(event) {
        Ok(j) => j,
        Err(e) => {
            tracing::error!("Failed to serialize audit event: {}", e);
            return;
        }
    };

    match output {
        AuditOutput::Stdout => {
            tracing::info!("AUDIT: {}", json);
        }
        AuditOutput::File { path, rotation } => {
            if let Err(e) = write_to_file_with_rotation(path, rotation, &json) {
                tracing::error!("Failed to write audit log: {}", e);
            }
        }
    }
}

fn write_to_file_with_rotation(path: &PathBuf, rotation: &LogRotation, line: &str) -> std::io::Result<()> {
    match rotation {
        LogRotation::None => {
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)?;
            writeln!(file, "{}", line)?;
            file.flush()?;
        }
        LogRotation::Size { max_bytes, max_files } => {
            if path.exists() && fs::metadata(path)?.len() >= *max_bytes {
                rotate_files(path, *max_files)?;
            }
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)?;
            writeln!(file, "{}", line)?;
            file.flush()?;
        }
        LogRotation::Daily => {
            if should_rotate_daily(path) {
                rotate_files(path, 7)?;
            }
            let mut file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)?;
            writeln!(file, "{}", line)?;
            file.flush()?;
        }
    }
    Ok(())
}

fn rotate_files(base_path: &PathBuf, max_files: u32) -> std::io::Result<()> {
    for i in (1..max_files).rev() {
        let src = format!("{}.{}", base_path.display(), i);
        let dst = format!("{}.{}", base_path.display(), i + 1);
        let src_path = PathBuf::from(&src);
        let dst_path = PathBuf::from(&dst);
        if src_path.exists() {
            fs::rename(&src_path, &dst_path)?;
        }
    }
    let first = PathBuf::from(format!("{}.1", base_path.display()));
    if base_path.exists() {
        fs::rename(base_path, &first)?;
    }
    Ok(())
}

fn should_rotate_daily(path: &PathBuf) -> bool {
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return false,
    };
    let modified = match metadata.modified() {
        Ok(t) => t,
        Err(_) => return false,
    };
    let modified_secs = modified
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let now_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let modified_day = modified_secs / 86400;
    let today = now_secs / 86400;
    modified_day != today
}

impl AuditLogger {
    pub fn new(outputs: Vec<AuditOutput>) -> Self {
        let (sender, receiver) = channel::bounded(4096);
        let outputs = Arc::new(outputs);
        let outputs_clone = Arc::clone(&outputs);

        let _handle = std::thread::Builder::new()
            .name("audit-logger".into())
            .spawn(move || {
                for event in receiver.iter() {
                    for output in outputs_clone.iter() {
                        write_event_to_output(output, &event);
                    }
                }
            })
            .expect("Failed to spawn audit logger thread");

        AuditLogger {
            sender,
            outputs,
            _handle,
        }
    }

    pub fn log(&self, event: AuditEvent) {
        if self.sender.try_send(event).is_err() {
            tracing::warn!("Audit log channel full, dropping event");
        }
    }

    pub fn log_sync(&self, event: AuditEvent) -> crate::Result<()> {
        for output in self.outputs.iter() {
            let json = serde_json::to_string(&event)
                .map_err(|e| crate::SecurityError::Internal(e.to_string()))?;
            match output {
                AuditOutput::Stdout => {
                    tracing::info!("AUDIT: {}", json);
                }
                AuditOutput::File { path, rotation } => {
                    write_to_file_with_rotation(path, rotation, &json)
                        .map_err(|e| crate::SecurityError::Internal(e.to_string()))?;
                }
            }
        }
        Ok(())
    }
}
