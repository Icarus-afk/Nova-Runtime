use clap::Subcommand;

#[derive(Subcommand)]
pub enum QueueCommands {
    /// List all queues
    List,
    /// Create a new queue
    Create {
        name: String,
        #[arg(short, long)]
        durable: bool,
    },
    /// Delete a queue
    Delete {
        name: String,
    },
    /// Publish a message to a queue
    Publish {
        queue: String,
        message: String,
    },
    /// Consume messages from a queue
    Consume {
        queue: String,
        #[arg(long)]
        count: Option<u32>,
    },
    /// Get queue stats
    Stats {
        name: String,
    },
}

impl QueueCommands {
    pub fn execute(&self) -> anyhow::Result<()> {
        println!("Queue command not yet implemented");
        Ok(())
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

    #[test]
    fn test_execute_returns_ok() {
        assert!(QueueCommands::List.execute().is_ok());
        assert!(QueueCommands::Create { name: "q".into(), durable: true }.execute().is_ok());
        assert!(QueueCommands::Delete { name: "q".into() }.execute().is_ok());
        assert!(QueueCommands::Publish { queue: "q".into(), message: "m".into() }.execute().is_ok());
        assert!(QueueCommands::Consume { queue: "q".into(), count: None }.execute().is_ok());
        assert!(QueueCommands::Stats { name: "q".into() }.execute().is_ok());
    }
}
