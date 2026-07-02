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
