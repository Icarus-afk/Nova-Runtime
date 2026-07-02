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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct Cli {
        #[command(subcommand)]
        cmd: CacheCommands,
    }

    fn parse(args: &[&str]) -> CacheCommands {
        Cli::try_parse_from(args).unwrap().cmd
    }

    #[test]
    fn test_stats() {
        assert!(matches!(parse(&["test", "stats"]), CacheCommands::Stats));
    }

    #[test]
    fn test_clear() {
        assert!(matches!(parse(&["test", "clear"]), CacheCommands::Clear));
    }

    #[test]
    fn test_flush() {
        assert!(matches!(parse(&["test", "flush"]), CacheCommands::Flush));
    }

    #[test]
    fn test_list() {
        assert!(matches!(parse(&["test", "list"]), CacheCommands::List { pattern: None }));
        assert!(matches!(parse(&["test", "list", "--pattern", "user:*"]), CacheCommands::List { pattern: Some(_) }));
    }

    #[test]
    fn test_execute_stats() {
        assert!(CacheCommands::Stats.execute().is_ok());
        assert!(CacheCommands::Clear.execute().is_ok());
        assert!(CacheCommands::Flush.execute().is_ok());
        assert!(CacheCommands::List { pattern: None }.execute().is_ok());
        assert!(CacheCommands::List { pattern: Some("user:*".into()) }.execute().is_ok());
    }
}
