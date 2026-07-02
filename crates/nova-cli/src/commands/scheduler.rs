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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct Cli {
        #[command(subcommand)]
        cmd: SchedulerCommands,
    }

    fn parse(args: &[&str]) -> SchedulerCommands {
        Cli::try_parse_from(args).unwrap().cmd
    }

    #[test]
    fn test_list() {
        assert!(matches!(parse(&["test", "list"]), SchedulerCommands::List));
    }

    #[test]
    fn test_create() {
        assert!(matches!(parse(&["test", "create", "job", "* * * * *", "backup.sh"]), SchedulerCommands::Create { .. }));
    }

    #[test]
    fn test_delete() {
        assert!(matches!(parse(&["test", "delete", "job"]), SchedulerCommands::Delete { .. }));
    }

    #[test]
    fn test_pause() {
        assert!(matches!(parse(&["test", "pause", "job"]), SchedulerCommands::Pause { .. }));
    }

    #[test]
    fn test_resume() {
        assert!(matches!(parse(&["test", "resume", "job"]), SchedulerCommands::Resume { .. }));
    }

    #[test]
    fn test_execute_returns_ok() {
        assert!(SchedulerCommands::List.execute().is_ok());
        assert!(SchedulerCommands::Create { name: "j".into(), schedule: "* * * * *".into(), command: "c".into() }.execute().is_ok());
        assert!(SchedulerCommands::Delete { name: "j".into() }.execute().is_ok());
        assert!(SchedulerCommands::Pause { name: "j".into() }.execute().is_ok());
        assert!(SchedulerCommands::Resume { name: "j".into() }.execute().is_ok());
    }
}
