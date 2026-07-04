use clap::Subcommand;
use crate::app::CommandContext;
use crate::client::ApiClient;
use crate::output;

#[derive(Subcommand)]
pub enum RuntimeCommands {
    Status,
    Start {
        #[arg(short, long)]
        daemonize: bool,
    },
    Stop {
        #[arg(short, long)]
        force: bool,
    },
    Restart,
    Reload,
}

impl RuntimeCommands {
    pub fn execute(&self, ctx: &CommandContext) -> anyhow::Result<()> {
        let client = ApiClient::new(&ctx.address, ctx.api_key.as_deref());
        match self {
            RuntimeCommands::Status => {
                match client.get("/admin/status") {
                    Ok(body) => {
                        let is_running = body["is_running"].as_bool().unwrap_or(false);
                        let uptime = body["uptime_secs"].as_u64().unwrap_or(0);
                        let active = body["active_operations"].as_u64().unwrap_or(0);
                        let total = body["total_operations"].as_u64().unwrap_or(0);
                        output::print_value(&serde_json::json!({
                            "status": if is_running { "running" } else { "stopped" },
                            "uptime_secs": uptime,
                            "active_operations": active,
                            "total_operations": total,
                        }), &ctx.output)?;
                    }
                    Err(e) => match &ctx.output {
                        crate::app::OutputFormat::Json => {
                            output::print_json(&serde_json::json!({
                                "status": "unknown",
                                "error": e,
                            }))?;
                        }
                        _ => {
                            println!("Nova Runtime");
                            println!("  Status:       unknown (server unreachable)");
                            println!("  Error:        {e}");
                        }
                    },
                }
                Ok(())
            }
            RuntimeCommands::Start { daemonize } => {
                if *daemonize {
                    println!("Use `novad` to start the daemon as a background process.");
                } else {
                    println!("Use `nova run` to start the daemon in-process, or `novad` for the standalone daemon.");
                }
                Ok(())
            }
            RuntimeCommands::Stop { force: _ } => {
                println!("Use `kill` or SIGTERM to stop the novad process.");
                println!("Example: pkill novad");
                Ok(())
            }
            RuntimeCommands::Restart => {
                println!("To restart, stop novad and start it again.");
                Ok(())
            }
            RuntimeCommands::Reload => {
                match client.post("/admin/reload", None) {
                    Ok(body) => {
                        output::print_value(&body, &ctx.output)?;
                    }
                    Err(e) => {
                        eprintln!("Reload failed: {e}");
                        eprintln!("Suggestion: send SIGHUP to the novad process");
                    }
                }
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
