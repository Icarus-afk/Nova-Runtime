use clap::{Parser, Subcommand};
use crate::commands::runtime::RuntimeCommands;
use crate::commands::config_cmd::ConfigCommands;
use crate::commands::auth::AuthCommands;
use crate::commands::queue::QueueCommands;
use crate::commands::scheduler::SchedulerCommands;
use crate::commands::search::SearchCommands;
use crate::commands::blob::BlobCommands;
use crate::commands::sql::SqlCommands;
use crate::commands::db::DbCommands;
use crate::commands::cache::CacheCommands;

#[derive(Parser)]
#[command(name = "novactl", version, about = "Nova Runtime CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,

    /// Path to config file
    #[arg(short, long, global = true)]
    pub config: Option<String>,

    /// Output format (table, json, yaml)
    #[arg(short, long, global = true, default_value_t = OutputFormat::Table)]
    pub output: OutputFormat,

    /// Nova daemon address
    #[arg(short, long, global = true, default_value = "http://127.0.0.1:8642")]
    pub address: String,

    /// API key for authentication
    #[arg(long, global = true)]
    pub api_key: Option<String>,
}

#[derive(clap::ValueEnum, Clone)]
pub enum OutputFormat {
    Table,
    Json,
    Yaml,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Table => write!(f, "table"),
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::Yaml => write!(f, "yaml"),
        }
    }
}

#[derive(Subcommand)]
pub enum CompletionCommands {
    /// Generate bash completion script
    Bash,
    /// Generate zsh completion script
    Zsh,
    /// Generate fish completion script
    Fish,
    /// Generate PowerShell completion script
    PowerShell,
}

impl CompletionCommands {
    pub fn execute(&self) {
        match self {
            CompletionCommands::Bash => {
                println!("#!/usr/bin/env bash");
                println!("# Nova CLI bash completion");
                println!("# Install: source <(nova completion bash)");
                println!("# Completion not yet implemented. Use `nova --help` for available commands.");
            }
            CompletionCommands::Zsh => {
                println!("# Nova CLI zsh completion");
                println!("# Install: source <(nova completion zsh)");
                println!("# Completion not yet implemented. Use `nova --help` for available commands.");
            }
            CompletionCommands::Fish => {
                println!("# Nova CLI fish completion");
                println!("# Install: source <(nova completion fish)");
                println!("# Completion not yet implemented. Use `nova --help` for available commands.");
            }
            CompletionCommands::PowerShell => {
                println!("# Nova CLI PowerShell completion");
                println!("# Install: . (nova completion powershell) | Out-String | Invoke-Expression");
                println!("# Completion not yet implemented. Use `nova --help` for available commands.");
            }
        }
    }
}

#[derive(clap::Subcommand)]
pub enum Command {
    /// Runtime management
    #[command(subcommand)]
    Runtime(RuntimeCommands),
    /// Configuration management
    #[command(subcommand)]
    Config(ConfigCommands),
    /// Authentication management
    #[command(subcommand)]
    Auth(AuthCommands),
    /// Queue management
    #[command(subcommand)]
    Queue(QueueCommands),
    /// Scheduler management
    #[command(subcommand)]
    Scheduler(SchedulerCommands),
    /// Search management
    #[command(subcommand)]
    Search(SearchCommands),
    /// Blob storage management
    #[command(subcommand)]
    Blob(BlobCommands),
    /// SQL query execution
    #[command(subcommand)]
    Sql(SqlCommands),
    /// Database management
    #[command(subcommand)]
    Db(DbCommands),
    /// Cache management
    #[command(subcommand)]
    Cache(CacheCommands),
    /// Generate shell completion scripts
    #[command(subcommand)]
    Completion(CompletionCommands),
    /// Start the Nova Runtime daemon (runs novad in-process)
    Run {
        /// Path to config file
        #[arg(short, long)]
        config: Option<String>,
        /// Data directory
        #[arg(short, long)]
        data_dir: Option<String>,
    },
}

impl Command {
    pub fn execute(&self, cli_config: &Option<String>) -> anyhow::Result<()> {
        match self {
            Command::Runtime(cmd) => {
                let loader = nova_config::ConfigLoader::new();
                let config = match cli_config {
                    Some(path) => nova_config::ConfigLoader::parse_file(std::path::Path::new(path))?,
                    None => loader.load(None)?,
                };
                cmd.execute(&config)
            }
            Command::Config(cmd) => cmd.execute(cli_config),
            Command::Auth(cmd) => cmd.execute(),
            Command::Queue(cmd) => cmd.execute(),
            Command::Scheduler(cmd) => cmd.execute(),
            Command::Search(cmd) => cmd.execute(),
            Command::Blob(cmd) => cmd.execute(),
            Command::Sql(cmd) => cmd.execute(),
            Command::Db(cmd) => cmd.execute(),
            Command::Cache(cmd) => cmd.execute(),
            Command::Completion(cmd) => {
                cmd.execute();
                Ok(())
            }
            Command::Run { config, data_dir } => {
                println!("Nova Runtime starting...");
                if let Some(path) = config {
                    println!("  Config: {}", path);
                }
                if let Some(dir) = data_dir {
                    println!("  Data dir: {}", dir);
                }
                println!("  (run via `novad` binary for the full daemon)");
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(args).unwrap()
    }

    #[test]
    fn test_default_output_format() {
        let cli = parse(&["novactl", "runtime", "status"]);
        assert!(matches!(cli.output, OutputFormat::Table));
    }

    #[test]
    fn test_global_flags() {
        let cli = parse(&[
            "novactl",
            "--config", "/tmp/test.toml",
            "--output", "json",
            "--address", "http://localhost:9999",
            "--api-key", "key123",
            "runtime", "status",
        ]);
        assert_eq!(cli.config, Some("/tmp/test.toml".to_string()));
        assert!(matches!(cli.output, OutputFormat::Json));
        assert_eq!(cli.address, "http://localhost:9999");
        assert_eq!(cli.api_key, Some("key123".to_string()));
    }

    #[test]
    fn test_default_address() {
        let cli = parse(&["novactl", "runtime", "status"]);
        assert_eq!(cli.address, "http://127.0.0.1:8642");
    }

    #[test]
    fn test_output_format_values() {
        let cli_table = parse(&["novactl", "--output", "table", "runtime", "status"]);
        assert!(matches!(cli_table.output, OutputFormat::Table));
        let cli_json = parse(&["novactl", "--output", "json", "runtime", "status"]);
        assert!(matches!(cli_json.output, OutputFormat::Json));
        let cli_yaml = parse(&["novactl", "--output", "yaml", "runtime", "status"]);
        assert!(matches!(cli_yaml.output, OutputFormat::Yaml));
    }

    #[test]
    fn test_runtime_commands() {
        assert!(matches!(parse(&["novactl", "runtime", "status"]).command, Command::Runtime(RuntimeCommands::Status)));
        assert!(matches!(parse(&["novactl", "runtime", "start"]).command, Command::Runtime(RuntimeCommands::Start { daemonize: false })));
        assert!(matches!(parse(&["novactl", "runtime", "start", "--daemonize"]).command, Command::Runtime(RuntimeCommands::Start { daemonize: true })));
        assert!(matches!(parse(&["novactl", "runtime", "stop"]).command, Command::Runtime(RuntimeCommands::Stop { force: false })));
        assert!(matches!(parse(&["novactl", "runtime", "stop", "--force"]).command, Command::Runtime(RuntimeCommands::Stop { force: true })));
        assert!(matches!(parse(&["novactl", "runtime", "restart"]).command, Command::Runtime(RuntimeCommands::Restart)));
        assert!(matches!(parse(&["novactl", "runtime", "reload"]).command, Command::Runtime(RuntimeCommands::Reload)));
    }

    #[test]
    fn test_config_commands() {
        assert!(matches!(parse(&["novactl", "config", "show"]).command, Command::Config(ConfigCommands::Show { section: None })));
        assert!(matches!(parse(&["novactl", "config", "show", "storage"]).command, Command::Config(ConfigCommands::Show { section: Some(_) })));
        assert!(matches!(parse(&["novactl", "config", "get", "storage.page_size"]).command, Command::Config(ConfigCommands::Get { .. })));
        assert!(matches!(parse(&["novactl", "config", "set", "key", "val"]).command, Command::Config(ConfigCommands::Set { .. })));
        assert!(matches!(parse(&["novactl", "config", "validate", "path.toml"]).command, Command::Config(ConfigCommands::Validate { .. })));
        assert!(matches!(parse(&["novactl", "config", "default"]).command, Command::Config(ConfigCommands::Default)));
    }

    #[test]
    fn test_auth_commands() {
        assert!(matches!(parse(&["novactl", "auth", "create-user", "admin"]).command, Command::Auth(AuthCommands::CreateUser { username: _, role: None })));
        assert!(matches!(parse(&["novactl", "auth", "create-user", "admin", "readonly"]).command, Command::Auth(AuthCommands::CreateUser { username: _, role: Some(_) })));
        assert!(matches!(parse(&["novactl", "auth", "delete-user", "admin"]).command, Command::Auth(AuthCommands::DeleteUser { .. })));
        assert!(matches!(parse(&["novactl", "auth", "list-users"]).command, Command::Auth(AuthCommands::ListUsers)));
        assert!(matches!(parse(&["novactl", "auth", "create-api-key", "my-key"]).command, Command::Auth(AuthCommands::CreateApiKey { .. })));
        assert!(matches!(parse(&["novactl", "auth", "revoke-api-key", "key-123"]).command, Command::Auth(AuthCommands::RevokeApiKey { .. })));
    }

    #[test]
    fn test_queue_commands() {
        assert!(matches!(parse(&["novactl", "queue", "list"]).command, Command::Queue(QueueCommands::List)));
        assert!(matches!(parse(&["novactl", "queue", "create", "q"]).command, Command::Queue(QueueCommands::Create { name: _, durable: false })));
        assert!(matches!(parse(&["novactl", "queue", "create", "q", "--durable"]).command, Command::Queue(QueueCommands::Create { name: _, durable: true })));
        assert!(matches!(parse(&["novactl", "queue", "delete", "q"]).command, Command::Queue(QueueCommands::Delete { .. })));
        assert!(matches!(parse(&["novactl", "queue", "publish", "q", "msg"]).command, Command::Queue(QueueCommands::Publish { .. })));
        assert!(matches!(parse(&["novactl", "queue", "consume", "q"]).command, Command::Queue(QueueCommands::Consume { queue: _, count: None })));
        assert!(matches!(parse(&["novactl", "queue", "consume", "q", "--count", "5"]).command, Command::Queue(QueueCommands::Consume { queue: _, count: Some(5) })));
        assert!(matches!(parse(&["novactl", "queue", "stats", "q"]).command, Command::Queue(QueueCommands::Stats { .. })));
    }

    #[test]
    fn test_scheduler_commands() {
        assert!(matches!(parse(&["novactl", "scheduler", "list"]).command, Command::Scheduler(SchedulerCommands::List)));
        assert!(matches!(parse(&["novactl", "scheduler", "create", "job", "* * * * *", "cmd"]).command, Command::Scheduler(SchedulerCommands::Create { .. })));
        assert!(matches!(parse(&["novactl", "scheduler", "delete", "job"]).command, Command::Scheduler(SchedulerCommands::Delete { .. })));
        assert!(matches!(parse(&["novactl", "scheduler", "pause", "job"]).command, Command::Scheduler(SchedulerCommands::Pause { .. })));
        assert!(matches!(parse(&["novactl", "scheduler", "resume", "job"]).command, Command::Scheduler(SchedulerCommands::Resume { .. })));
    }

    #[test]
    fn test_search_commands() {
        assert!(matches!(parse(&["novactl", "search", "query", "find"]).command, Command::Search(SearchCommands::Query { .. })));
        assert!(matches!(parse(&["novactl", "search", "query", "find", "--collection", "docs"]).command, Command::Search(SearchCommands::Query { .. })));
        assert!(matches!(parse(&["novactl", "search", "query", "find", "--limit", "10"]).command, Command::Search(SearchCommands::Query { .. })));
        assert!(matches!(parse(&["novactl", "search", "create-index", "idx", "coll", "f1", "f2"]).command, Command::Search(SearchCommands::CreateIndex { .. })));
        assert!(matches!(parse(&["novactl", "search", "drop-index", "idx"]).command, Command::Search(SearchCommands::DropIndex { .. })));
        assert!(matches!(parse(&["novactl", "search", "list-indexes"]).command, Command::Search(SearchCommands::ListIndexes)));
    }

    #[test]
    fn test_blob_commands() {
        assert!(matches!(parse(&["novactl", "blob", "list"]).command, Command::Blob(BlobCommands::List { prefix: None })));
        assert!(matches!(parse(&["novactl", "blob", "list", "--prefix", "img/"]).command, Command::Blob(BlobCommands::List { prefix: Some(_) })));
        assert!(matches!(parse(&["novactl", "blob", "put", "k", "f.txt"]).command, Command::Blob(BlobCommands::Put { .. })));
        assert!(matches!(parse(&["novactl", "blob", "get", "k"]).command, Command::Blob(BlobCommands::Get { .. })));
        assert!(matches!(parse(&["novactl", "blob", "get", "k", "out.txt"]).command, Command::Blob(BlobCommands::Get { .. })));
        assert!(matches!(parse(&["novactl", "blob", "delete", "k"]).command, Command::Blob(BlobCommands::Delete { .. })));
    }

    #[test]
    fn test_sql_commands() {
        assert!(matches!(parse(&["novactl", "sql", "query", "SELECT 1"]).command, Command::Sql(SqlCommands::Query { .. })));
        assert!(matches!(parse(&["novactl", "sql", "query", "SELECT 1", "--format", "json"]).command, Command::Sql(SqlCommands::Query { .. })));
        assert!(matches!(parse(&["novactl", "sql", "execute", "script.sql"]).command, Command::Sql(SqlCommands::Execute { .. })));
        assert!(matches!(parse(&["novactl", "sql", "schema"]).command, Command::Sql(SqlCommands::Schema { table: None })));
        assert!(matches!(parse(&["novactl", "sql", "schema", "users"]).command, Command::Sql(SqlCommands::Schema { table: Some(_) })));
    }

    #[test]
    fn test_db_commands() {
        assert!(matches!(parse(&["novactl", "db", "list"]).command, Command::Db(DbCommands::List)));
        assert!(matches!(parse(&["novactl", "db", "create", "mydb"]).command, Command::Db(DbCommands::Create { .. })));
        assert!(matches!(parse(&["novactl", "db", "drop", "mydb"]).command, Command::Db(DbCommands::Drop { .. })));
        assert!(matches!(parse(&["novactl", "db", "collections", "mydb"]).command, Command::Db(DbCommands::Collections { .. })));
        assert!(matches!(parse(&["novactl", "db", "create-collection", "mydb", "coll"]).command, Command::Db(DbCommands::CreateCollection { .. })));
        assert!(matches!(parse(&["novactl", "db", "drop-collection", "mydb", "coll"]).command, Command::Db(DbCommands::DropCollection { .. })));
        assert!(matches!(parse(&["novactl", "db", "stats"]).command, Command::Db(DbCommands::Stats { database: None })));
        assert!(matches!(parse(&["novactl", "db", "stats", "mydb"]).command, Command::Db(DbCommands::Stats { database: Some(_) })));
    }

    #[test]
    fn test_cache_commands() {
        assert!(matches!(parse(&["novactl", "cache", "stats"]).command, Command::Cache(CacheCommands::Stats)));
        assert!(matches!(parse(&["novactl", "cache", "clear"]).command, Command::Cache(CacheCommands::Clear)));
        assert!(matches!(parse(&["novactl", "cache", "flush"]).command, Command::Cache(CacheCommands::Flush)));
        assert!(matches!(parse(&["novactl", "cache", "list"]).command, Command::Cache(CacheCommands::List { pattern: None })));
        assert!(matches!(parse(&["novactl", "cache", "list", "--pattern", "user:*"]).command, Command::Cache(CacheCommands::List { pattern: Some(_) })));
    }

    #[test]
    fn test_completion_commands() {
        assert!(matches!(parse(&["novactl", "completion", "bash"]).command, Command::Completion(CompletionCommands::Bash)));
        assert!(matches!(parse(&["novactl", "completion", "zsh"]).command, Command::Completion(CompletionCommands::Zsh)));
        assert!(matches!(parse(&["novactl", "completion", "fish"]).command, Command::Completion(CompletionCommands::Fish)));
        assert!(matches!(parse(&["novactl", "completion", "power-shell"]).command, Command::Completion(CompletionCommands::PowerShell)));
    }

    #[test]
    fn test_run_command() {
        assert!(matches!(parse(&["novactl", "run"]).command, Command::Run { config: None, data_dir: None }));
        assert!(matches!(parse(&["novactl", "run", "--config", "c.toml", "--data-dir", "/d"]).command, Command::Run { config: Some(_), data_dir: Some(_) }));
    }

    #[test]
    fn test_run_execute_returns_ok() {
        let cmd = Command::Run { config: None, data_dir: None };
        assert!(cmd.execute(&None).is_ok());
    }

    #[test]
    fn test_completion_execute_does_not_panic() {
        for cmd in &[CompletionCommands::Bash, CompletionCommands::Zsh, CompletionCommands::Fish, CompletionCommands::PowerShell] {
            cmd.execute();
        }
    }

    #[test]
    fn test_invalid_command_fails() {
        assert!(Cli::try_parse_from(&["novactl", "nonexistent"]).is_err());
    }

    #[test]
    fn test_invalid_output_format_fails() {
        assert!(Cli::try_parse_from(&["novactl", "--output", "csv", "runtime", "status"]).is_err());
    }
}
