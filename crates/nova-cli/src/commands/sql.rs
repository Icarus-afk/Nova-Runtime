use clap::Subcommand;

#[derive(Subcommand)]
pub enum SqlCommands {
    /// Execute a SQL query
    Query {
        sql: String,
        #[arg(short, long)]
        format: Option<String>,
    },
    /// Execute a SQL script file
    Execute {
        file: String,
    },
    /// Show SQL schema
    Schema {
        table: Option<String>,
    },
}

impl SqlCommands {
    pub fn execute(&self) -> anyhow::Result<()> {
        println!("SQL command not yet implemented");
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

    #[test]
    fn test_execute_returns_ok() {
        assert!(SqlCommands::Query { sql: "SELECT 1".into(), format: None }.execute().is_ok());
        assert!(SqlCommands::Execute { file: "f.sql".into() }.execute().is_ok());
        assert!(SqlCommands::Schema { table: None }.execute().is_ok());
    }
}
