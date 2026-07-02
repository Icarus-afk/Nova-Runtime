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
