use clap::Subcommand;

#[derive(Subcommand)]
pub enum CacheCommands {
    /// Show cache statistics
    Stats,
    /// Clear the entire cache
    Clear,
    /// Flush dirty cache entries to disk
    Flush,
    /// List cache entries
    List {
        #[arg(long)]
        pattern: Option<String>,
    },
}

impl CacheCommands {
    pub fn execute(&self) -> anyhow::Result<()> {
        match self {
            CacheCommands::Stats => println!("cache stats"),
            CacheCommands::Clear => println!("cache clear"),
            CacheCommands::Flush => println!("cache flush"),
            CacheCommands::List { pattern } => {
                match pattern {
                    Some(p) => println!("cache list (pattern: {p})"),
                    None => println!("cache list"),
                }
            }
        }
        Ok(())
    }
}
