use clap::Subcommand;
use crate::app::CommandContext;
use crate::client::ApiClient;
use crate::output;

#[derive(Subcommand)]
pub enum DbCommands {
    List,
    Create {
        name: String,
    },
    Drop {
        name: String,
    },
    Collections {
        database: String,
    },
    CreateCollection {
        database: String,
        collection: String,
    },
    DropCollection {
        database: String,
        collection: String,
    },
    Stats {
        database: Option<String>,
    },
}

impl DbCommands {
    pub fn execute(&self, ctx: &CommandContext) -> anyhow::Result<()> {
        let client = ApiClient::new(&ctx.address, ctx.api_key.as_deref());
        match self {
            DbCommands::List => {
                match client.get("/v1/databases") {
                    Ok(body) => {
                        let dbs = if body.is_array() {
                            body.clone()
                        } else {
                            body.get("databases").cloned().unwrap_or(body.clone())
                        };
                        if let Some(arr) = dbs.as_array() {
                            output::print_table_from_json(
                                &["Name", "Collections"],
                                arr,
                                |d| vec![
                                    d["name"].as_str().unwrap_or("-").to_string(),
                                    d["collections"].as_u64().map(|v| v.to_string()).unwrap_or_else(|| "-".to_string()),
                                ],
                                &ctx.output,
                            )?;
                        } else {
                            output::print_value(&body, &ctx.output)?;
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to list databases: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            DbCommands::Create { name } => {
                let body = serde_json::json!({"name": name});
                match client.post("/v1/databases", Some(&body)) {
                    Ok(resp) => output::print_value(&resp, &ctx.output)?,
                    Err(e) => {
                        eprintln!("Failed to create database: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            DbCommands::Drop { name } => {
                match client.delete(&format!("/v1/databases/{name}")) {
                    Ok(resp) => output::print_value(&resp, &ctx.output)?,
                    Err(e) => {
                        eprintln!("Failed to drop database: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            DbCommands::Collections { database } => {
                match client.get(&format!("/v1/databases/{database}/collections")) {
                    Ok(body) => output::print_value(&body, &ctx.output)?,
                    Err(e) => {
                        eprintln!("Failed to list collections: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            DbCommands::CreateCollection { database, collection } => {
                let body = serde_json::json!({"name": collection, "database": database});
                match client.post(&format!("/v1/databases/{database}/collections"), Some(&body)) {
                    Ok(resp) => output::print_value(&resp, &ctx.output)?,
                    Err(e) => {
                        eprintln!("Failed to create collection: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            DbCommands::DropCollection { database, collection } => {
                match client.delete(&format!("/v1/databases/{database}/collections/{collection}")) {
                    Ok(resp) => output::print_value(&resp, &ctx.output)?,
                    Err(e) => {
                        eprintln!("Failed to drop collection: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            DbCommands::Stats { database } => {
                let path = match database {
                    Some(d) => format!("/v1/databases/{d}/stats"),
                    None => "/v1/databases/stats".to_string(),
                };
                match client.get(&path) {
                    Ok(resp) => output::print_value(&resp, &ctx.output)?,
                    Err(e) => {
                        eprintln!("Failed to get database stats: {e}");
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
        cmd: DbCommands,
    }

    fn parse(args: &[&str]) -> DbCommands {
        Cli::try_parse_from(args).unwrap().cmd
    }

    #[test]
    fn test_list() {
        assert!(matches!(parse(&["test", "list"]), DbCommands::List));
    }

    #[test]
    fn test_create() {
        assert!(matches!(parse(&["test", "create", "mydb"]), DbCommands::Create { .. }));
    }

    #[test]
    fn test_drop() {
        assert!(matches!(parse(&["test", "drop", "mydb"]), DbCommands::Drop { .. }));
    }

    #[test]
    fn test_collections() {
        assert!(matches!(parse(&["test", "collections", "mydb"]), DbCommands::Collections { .. }));
    }

    #[test]
    fn test_create_collection() {
        assert!(matches!(parse(&["test", "create-collection", "mydb", "coll"]), DbCommands::CreateCollection { .. }));
    }

    #[test]
    fn test_drop_collection() {
        assert!(matches!(parse(&["test", "drop-collection", "mydb", "coll"]), DbCommands::DropCollection { .. }));
    }

    #[test]
    fn test_stats() {
        assert!(matches!(parse(&["test", "stats"]), DbCommands::Stats { database: None }));
        assert!(matches!(parse(&["test", "stats", "mydb"]), DbCommands::Stats { database: Some(_) }));
    }
}
