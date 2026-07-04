use clap::Subcommand;
use crate::app::CommandContext;
use crate::client::ApiClient;
use crate::output;

#[derive(Subcommand)]
pub enum SearchCommands {
    Query {
        query: String,
        #[arg(long)]
        collection: Option<String>,
        #[arg(long)]
        limit: Option<u32>,
    },
    CreateIndex {
        name: String,
        collection: String,
        fields: Vec<String>,
    },
    DropIndex {
        name: String,
    },
    ListIndexes,
}

impl SearchCommands {
    pub fn execute(&self, ctx: &CommandContext) -> anyhow::Result<()> {
        let client = ApiClient::new(&ctx.address, ctx.api_key.as_deref());
        match self {
            SearchCommands::ListIndexes => {
                match client.get("/v1/search/indexes") {
                    Ok(body) => {
                        let indexes = if body.is_array() {
                            body.clone()
                        } else {
                            body.get("indexes").cloned().unwrap_or(body.clone())
                        };
                        if let Some(arr) = indexes.as_array() {
                            output::print_table_from_json(
                                &["Name", "Collection", "Fields", "Documents"],
                                arr,
                                |i| vec![
                                    i["name"].as_str().unwrap_or("-").to_string(),
                                    i["collection"].as_str().unwrap_or("-").to_string(),
                                    i["fields"].as_array().map(|f| f.len().to_string()).unwrap_or_else(|| "-".to_string()),
                                    i["documents"].as_u64().map(|v| v.to_string()).unwrap_or_else(|| "-".to_string()),
                                ],
                                &ctx.output,
                            )?;
                        } else {
                            output::print_value(&body, &ctx.output)?;
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to list indexes: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            SearchCommands::CreateIndex { name, collection, fields } => {
                let body = serde_json::json!({
                    "name": name,
                    "collection": collection,
                    "fields": fields,
                });
                match client.post("/v1/search/indexes", Some(&body)) {
                    Ok(resp) => output::print_value(&resp, &ctx.output)?,
                    Err(e) => {
                        eprintln!("Failed to create index: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            SearchCommands::DropIndex { name } => {
                match client.delete(&format!("/v1/search/indexes/{name}")) {
                    Ok(resp) => output::print_value(&resp, &ctx.output)?,
                    Err(e) => {
                        eprintln!("Failed to drop index: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            SearchCommands::Query { query, collection, limit } => {
                let mut params: Vec<(String, String)> = vec![("q".into(), query.clone())];
                if let Some(c) = collection {
                    params.push(("collection".into(), c.clone()));
                }
                if let Some(l) = limit {
                    params.push(("limit".into(), l.to_string()));
                }
                let str_params: Vec<(&str, &str)> = params.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
                match client.get_with_query("/v1/search/query", &str_params) {
                    Ok(body) => output::print_value(&body, &ctx.output)?,
                    Err(e) => {
                        eprintln!("Search failed: {e}");
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
        cmd: SearchCommands,
    }

    fn parse(args: &[&str]) -> SearchCommands {
        Cli::try_parse_from(args).unwrap().cmd
    }

    #[test]
    fn test_query() {
        assert!(matches!(parse(&["test", "query", "find"]), SearchCommands::Query { .. }));
        assert!(matches!(parse(&["test", "query", "find", "--collection", "docs"]), SearchCommands::Query { .. }));
        assert!(matches!(parse(&["test", "query", "find", "--limit", "10"]), SearchCommands::Query { .. }));
    }

    #[test]
    fn test_create_index() {
        let cmd = parse(&["test", "create-index", "idx", "coll", "f1", "f2"]);
        assert!(matches!(cmd, SearchCommands::CreateIndex { .. }));
    }

    #[test]
    fn test_drop_index() {
        assert!(matches!(parse(&["test", "drop-index", "idx"]), SearchCommands::DropIndex { .. }));
    }

    #[test]
    fn test_list_indexes() {
        assert!(matches!(parse(&["test", "list-indexes"]), SearchCommands::ListIndexes));
    }
}
