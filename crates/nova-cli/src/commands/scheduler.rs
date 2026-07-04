use clap::Subcommand;
use crate::app::CommandContext;
use crate::client::ApiClient;
use crate::output;

#[derive(Subcommand)]
pub enum SchedulerCommands {
    List,
    Create {
        name: String,
        schedule: String,
        command: String,
    },
    Delete {
        name: String,
    },
    Pause {
        name: String,
    },
    Resume {
        name: String,
    },
}

impl SchedulerCommands {
    pub fn execute(&self, ctx: &CommandContext) -> anyhow::Result<()> {
        let client = ApiClient::new(&ctx.address, ctx.api_key.as_deref());
        match self {
            SchedulerCommands::List => {
                match client.get("/v1/scheduler/jobs") {
                    Ok(body) => {
                        let jobs = if body.is_array() {
                            body.clone()
                        } else {
                            body.get("jobs").cloned().unwrap_or(body.clone())
                        };
                        if let Some(arr) = jobs.as_array() {
                            output::print_table_from_json(
                                &["Name", "Schedule", "Command", "Status"],
                                arr,
                                |j| vec![
                                    j["name"].as_str().unwrap_or("-").to_string(),
                                    j["schedule"].as_str().unwrap_or("-").to_string(),
                                    j["command"].as_str().unwrap_or("-").to_string(),
                                    j["status"].as_str().unwrap_or("active").to_string(),
                                ],
                                &ctx.output,
                            )?;
                        } else {
                            output::print_value(&body, &ctx.output)?;
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to list jobs: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            SchedulerCommands::Create { name, schedule, command } => {
                let body = serde_json::json!({
                    "name": name,
                    "schedule": schedule,
                    "command": command,
                });
                match client.post("/v1/scheduler/jobs", Some(&body)) {
                    Ok(resp) => output::print_value(&resp, &ctx.output)?,
                    Err(e) => {
                        eprintln!("Failed to create job: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            SchedulerCommands::Delete { name } => {
                match client.delete(&format!("/v1/scheduler/jobs/{name}")) {
                    Ok(resp) => output::print_value(&resp, &ctx.output)?,
                    Err(e) => {
                        eprintln!("Failed to delete job: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            SchedulerCommands::Pause { name } => {
                match client.post(&format!("/v1/scheduler/jobs/{name}/pause"), None) {
                    Ok(resp) => output::print_value(&resp, &ctx.output)?,
                    Err(e) => {
                        eprintln!("Failed to pause job: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            SchedulerCommands::Resume { name } => {
                match client.post(&format!("/v1/scheduler/jobs/{name}/resume"), None) {
                    Ok(resp) => output::print_value(&resp, &ctx.output)?,
                    Err(e) => {
                        eprintln!("Failed to resume job: {e}");
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
}
