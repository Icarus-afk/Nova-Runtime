use clap::Subcommand;

#[derive(Subcommand)]
pub enum SchedulerCommands {
    /// List scheduled jobs
    List,
    /// Schedule a new job
    Create {
        name: String,
        schedule: String,
        command: String,
    },
    /// Delete a scheduled job
    Delete {
        name: String,
    },
    /// Pause a scheduled job
    Pause {
        name: String,
    },
    /// Resume a scheduled job
    Resume {
        name: String,
    },
}

impl SchedulerCommands {
    pub fn execute(&self) -> anyhow::Result<()> {
        println!("Scheduler command not yet implemented");
        Ok(())
    }
}
