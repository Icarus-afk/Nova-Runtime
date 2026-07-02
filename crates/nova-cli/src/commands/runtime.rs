use clap::Subcommand;

#[derive(Subcommand)]
pub enum RuntimeCommands {
    /// Show runtime status
    Status,
    /// Start the Nova daemon
    Start {
        #[arg(short, long)]
        daemonize: bool,
    },
    /// Stop the Nova daemon
    Stop {
        #[arg(short, long)]
        force: bool,
    },
    /// Restart the Nova daemon
    Restart,
    /// Reload configuration
    Reload,
}

impl RuntimeCommands {
    pub fn execute(&self, config: &nova_config::Config) -> anyhow::Result<()> {
        match self {
            RuntimeCommands::Status => {
                println!("Nova Runtime");
                println!("  Version:     {}", env!("CARGO_PKG_VERSION"));
                println!("  Data Dir:    {}", config.general.data_dir.display());
                println!(
                    "  Listen:      {}:{}",
                    config.networking.listen_address, config.networking.listen_port
                );
                println!(
                    "  TLS:         {}",
                    if config.networking.tls_enabled {
                        "enabled"
                    } else {
                        "disabled"
                    }
                );
                println!(
                    "  Max Memory:  {} MB",
                    config.memory.max_memory / 1024 / 1024
                );
                Ok(())
            }
            _ => {
                println!("Command not yet implemented. Use `novad` to start the daemon.");
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct Cli {
        #[command(subcommand)]
        cmd: RuntimeCommands,
    }

    fn parse(args: &[&str]) -> RuntimeCommands {
        Cli::try_parse_from(args).unwrap().cmd
    }

    #[test]
    fn test_status() {
        assert!(matches!(parse(&["test", "status"]), RuntimeCommands::Status));
    }

    #[test]
    fn test_start() {
        assert!(matches!(parse(&["test", "start"]), RuntimeCommands::Start { daemonize: false }));
        assert!(matches!(parse(&["test", "start", "--daemonize"]), RuntimeCommands::Start { daemonize: true }));
    }

    #[test]
    fn test_stop() {
        assert!(matches!(parse(&["test", "stop"]), RuntimeCommands::Stop { force: false }));
        assert!(matches!(parse(&["test", "stop", "--force"]), RuntimeCommands::Stop { force: true }));
    }

    #[test]
    fn test_restart() {
        assert!(matches!(parse(&["test", "restart"]), RuntimeCommands::Restart));
    }

    #[test]
    fn test_reload() {
        assert!(matches!(parse(&["test", "reload"]), RuntimeCommands::Reload));
    }
}
