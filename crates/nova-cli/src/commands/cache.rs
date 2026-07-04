use clap::Subcommand;
use crate::app::CommandContext;
use crate::client::ApiClient;
use crate::output;

#[derive(Subcommand)]
pub enum CacheCommands {
    Stats,
    Clear,
    Flush,
    List {
        #[arg(long)]
        pattern: Option<String>,
    },
}

impl CacheCommands {
    pub fn execute(&self, ctx: &CommandContext) -> anyhow::Result<()> {
        let client = ApiClient::new(&ctx.address, ctx.api_key.as_deref());
        match self {
            CacheCommands::Stats => {
                match client.get("/v1/cache/stats") {
                    Ok(body) => output::print_value(&body, &ctx.output)?,
                    Err(e) => {
                        output::print_value(&serde_json::json!({
                            "error": e,
                            "message": "Cache endpoint not available"
                        }), &ctx.output)?;
                    }
                }
                Ok(())
            }
            CacheCommands::Clear => {
                match client.post("/v1/cache/clear", None) {
                    Ok(body) => output::print_value(&body, &ctx.output)?,
                    Err(e) => {
                        eprintln!("Failed to clear cache: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            CacheCommands::Flush => {
                match client.post("/v1/cache/flush", None) {
                    Ok(body) => output::print_value(&body, &ctx.output)?,
                    Err(e) => {
                        eprintln!("Failed to flush cache: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            CacheCommands::List { pattern } => {
                let params: Vec<(&str, &str)> = pattern.as_ref().map(|p| vec![("pattern", p.as_str())]).unwrap_or_default();
                match if params.is_empty() { client.get("/v1/cache/keys") } else { client.get_with_query("/v1/cache/keys", &params) } {
                    Ok(body) => output::print_value(&body, &ctx.output)?,
                    Err(e) => {
                        eprintln!("Failed to list cache keys: {e}");
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
}
