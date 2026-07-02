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
        #[arg(short, long)]
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
