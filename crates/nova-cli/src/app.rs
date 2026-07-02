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
    #[arg(short, long, global = true, default_value = "table")]
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
