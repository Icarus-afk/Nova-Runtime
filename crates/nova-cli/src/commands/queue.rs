use clap::Subcommand;
use crate::app::CommandContext;
use crate::client::ApiClient;
use crate::output;

#[derive(Subcommand)]
pub enum QueueCommands {
    List,
    Create {
        name: String,
        #[arg(short, long)]
        durable: bool,
    },
    Delete {
        name: String,
    },
    Publish {
        queue: String,
        message: String,
    },
    Consume {
        queue: String,
        #[arg(long)]
        count: Option<u32>,
    },
    Stats {
        name: String,
    },
}

impl QueueCommands {
    pub fn execute(&self, ctx: &CommandContext) -> anyhow::Result<()> {
        let client = ApiClient::new(&ctx.address, ctx.api_key.as_deref());
        match self {
            QueueCommands::List => {
                match client.get("/v1/queues") {
                    Ok(body) => {
                        let queues = if body.is_array() {
                            body.clone()
                        } else {
                            body.get("queues").cloned().unwrap_or(body.clone())
                        };
                        if let Some(arr) = queues.as_array() {
                            output::print_table_from_json(
                                &["Name", "Messages", "Durable"],
                                arr,
                                |q| vec![
                                    q["name"].as_str().unwrap_or("-").to_string(),
                                    q["messages"].as_u64().map(|v| v.to_string()).unwrap_or_else(|| "-".to_string()),
                                    q["durable"].as_bool().map(|v| v.to_string()).unwrap_or_else(|| "-".to_string()),
                                ],
                                &ctx.output,
                            )?;
                        } else {
                            output::print_value(&body, &ctx.output)?;
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to list queues: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            QueueCommands::Create { name, durable } => {
                let body = serde_json::json!({"name": name, "durable": durable});
                match client.post("/v1/queues", Some(&body)) {
                    Ok(resp) => output::print_value(&resp, &ctx.output)?,
                    Err(e) => {
                        eprintln!("Failed to create queue: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            QueueCommands::Delete { name } => {
                match client.delete(&format!("/v1/queues/{name}")) {
                    Ok(resp) => output::print_value(&resp, &ctx.output)?,
                    Err(e) => {
                        eprintln!("Failed to delete queue: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            QueueCommands::Publish { queue, message } => {
                let body = serde_json::json!({"message": message});
                match client.post(&format!("/v1/queues/{queue}/messages"), Some(&body)) {
                    Ok(resp) => output::print_value(&resp, &ctx.output)?,
                    Err(e) => {
                        eprintln!("Failed to publish message: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            QueueCommands::Consume { queue, count } => {
                let path = format!("/v1/queues/{queue}/messages");
                match count {
                    Some(n) => {
                        match client.get_with_query(&path, &[("count", &n.to_string())]) {
                            Ok(body) => output::print_value(&body, &ctx.output)?,
                            Err(e) => {
                                eprintln!("Failed to consume messages: {e}");
                                std::process::exit(1);
                            }
                        }
                    }
                    None => {
                        match client.get(&path) {
                            Ok(body) => output::print_value(&body, &ctx.output)?,
                            Err(e) => {
                                eprintln!("Failed to consume messages: {e}");
                                std::process::exit(1);
                            }
                        }
                    }
                }
                Ok(())
            }
            QueueCommands::Stats { name } => {
                match client.get(&format!("/v1/queues/{name}/stats")) {
                    Ok(body) => output::print_value(&body, &ctx.output)?,
                    Err(e) => {
                        eprintln!("Failed to get queue stats: {e}");
                        std::process::exit(1);
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
        cmd: QueueCommands,
    }

    fn parse(args: &[&str]) -> QueueCommands {
        Cli::try_parse_from(args).unwrap().cmd
    }

    #[test]
    fn test_list() {
        assert!(matches!(parse(&["test", "list"]), QueueCommands::List));
    }

    #[test]
    fn test_create() {
        assert!(matches!(parse(&["test", "create", "q"]), QueueCommands::Create { name: _, durable: false }));
        assert!(matches!(parse(&["test", "create", "q", "--durable"]), QueueCommands::Create { name: _, durable: true }));
    }

    #[test]
    fn test_delete() {
        assert!(matches!(parse(&["test", "delete", "q"]), QueueCommands::Delete { .. }));
    }

    #[test]
    fn test_publish() {
        assert!(matches!(parse(&["test", "publish", "q", "msg"]), QueueCommands::Publish { .. }));
    }

    #[test]
    fn test_consume() {
        assert!(matches!(parse(&["test", "consume", "q"]), QueueCommands::Consume { queue: _, count: None }));
        assert!(matches!(parse(&["test", "consume", "q", "--count", "10"]), QueueCommands::Consume { queue: _, count: Some(10) }));
    }

    #[test]
    fn test_stats() {
        assert!(matches!(parse(&["test", "stats", "q"]), QueueCommands::Stats { .. }));
    }
}
