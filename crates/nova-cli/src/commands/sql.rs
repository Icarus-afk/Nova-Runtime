use clap::Subcommand;
use crate::app::CommandContext;
use crate::client::ApiClient;
use crate::output;

#[derive(Subcommand)]
pub enum SqlCommands {
    Query {
        sql: String,
        #[arg(short, long)]
        format: Option<String>,
    },
    Execute {
        file: String,
    },
    Schema {
        table: Option<String>,
    },
}

impl SqlCommands {
    pub fn execute(&self, ctx: &CommandContext) -> anyhow::Result<()> {
        let client = ApiClient::new(&ctx.address, ctx.api_key.as_deref());
        match self {
            SqlCommands::Query { sql, format: _ } => {
                let body = serde_json::json!({"sql": sql});
                match client.post("/v1/sql/query", Some(&body)) {
                    Ok(resp) => output::print_value(&resp, &ctx.output)?,
                    Err(e) => {
                        eprintln!("Query failed: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            SqlCommands::Execute { file } => {
                let content = std::fs::read_to_string(file)
                    .map_err(|e| anyhow::anyhow!("Failed to read file '{file}': {e}"))?;
                let body = serde_json::json!({"sql": content, "file": file});
                match client.post("/v1/sql/execute", Some(&body)) {
                    Ok(resp) => output::print_value(&resp, &ctx.output)?,
                    Err(e) => {
                        eprintln!("Execute failed: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            SqlCommands::Schema { table } => {
                let path = match table {
                    Some(t) => format!("/v1/sql/schema/{t}"),
                    None => "/v1/sql/schema".to_string(),
                };
                match client.get(&path) {
                    Ok(resp) => output::print_value(&resp, &ctx.output)?,
                    Err(e) => {
                        eprintln!("Failed to get schema: {e}");
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
        cmd: SqlCommands,
    }

    fn parse(args: &[&str]) -> SqlCommands {
        Cli::try_parse_from(args).unwrap().cmd
    }

    #[test]
    fn test_query() {
        assert!(matches!(parse(&["test", "query", "SELECT 1"]), SqlCommands::Query { .. }));
        assert!(matches!(parse(&["test", "query", "SELECT 1", "--format", "json"]), SqlCommands::Query { .. }));
    }

    #[test]
    fn test_execute() {
        assert!(matches!(parse(&["test", "execute", "script.sql"]), SqlCommands::Execute { .. }));
    }

    #[test]
    fn test_schema() {
        assert!(matches!(parse(&["test", "schema"]), SqlCommands::Schema { table: None }));
        assert!(matches!(parse(&["test", "schema", "users"]), SqlCommands::Schema { table: Some(_) }));
    }
}
