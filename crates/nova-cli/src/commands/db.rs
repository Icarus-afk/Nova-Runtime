use clap::Subcommand;

#[derive(Subcommand)]
pub enum DbCommands {
    /// List databases
    List,
    /// Create a database
    Create {
        name: String,
    },
    /// Drop a database
    Drop {
        name: String,
    },
    /// List collections in a database
    Collections {
        database: String,
    },
    /// Create a collection
    CreateCollection {
        database: String,
        collection: String,
    },
    /// Drop a collection
    DropCollection {
        database: String,
        collection: String,
    },
    /// Get database stats
    Stats {
        database: Option<String>,
    },
}

impl DbCommands {
    pub fn execute(&self) -> anyhow::Result<()> {
        println!("Database command not yet implemented");
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

    #[test]
    fn test_execute_returns_ok() {
        assert!(DbCommands::List.execute().is_ok());
        assert!(DbCommands::Create { name: "d".into() }.execute().is_ok());
        assert!(DbCommands::Drop { name: "d".into() }.execute().is_ok());
        assert!(DbCommands::Collections { database: "d".into() }.execute().is_ok());
        assert!(DbCommands::CreateCollection { database: "d".into(), collection: "c".into() }.execute().is_ok());
        assert!(DbCommands::DropCollection { database: "d".into(), collection: "c".into() }.execute().is_ok());
        assert!(DbCommands::Stats { database: None }.execute().is_ok());
    }
}
