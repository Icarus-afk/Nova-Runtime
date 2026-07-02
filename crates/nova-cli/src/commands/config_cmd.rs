use clap::Subcommand;

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Show current configuration
    Show {
        /// Only show specific section
        section: Option<String>,
    },
    /// Get a specific config value by path (e.g., "storage.page_size")
    Get {
        /// Config key path (e.g., "storage.page_size")
        key: String,
    },
    /// Set a config value temporarily
    Set {
        /// Config key path
        key: String,
        /// Value to set
        value: String,
    },
    /// Validate configuration file
    Validate {
        /// Config file path to validate
        path: String,
    },
    /// Print default configuration
    Default,
}

impl ConfigCommands {
    pub fn execute(&self, cli_config: &Option<String>) -> anyhow::Result<()> {
        match self {
            ConfigCommands::Show { section } => {
                let loader = nova_config::ConfigLoader::new();
                let config = match cli_config {
                    Some(path) => nova_config::ConfigLoader::parse_file(std::path::Path::new(path))?,
                    None => loader.load(None)?,
                };
                let value = serde_json::to_value(&config)?;
                match section {
                    Some(s) => {
                        let val = &value[s.as_str()];
                        println!("{}", serde_json::to_string_pretty(val)?);
                    }
                    None => println!("{}", serde_json::to_string_pretty(&value)?),
                }
                Ok(())
            }
            ConfigCommands::Validate { path } => {
                match nova_config::ConfigLoader::parse_file(std::path::Path::new(path)) {
                    Ok(config) => {
                        println!("Config valid:");
                        println!(
                            "  Listen: {}:{}",
                            config.networking.listen_address, config.networking.listen_port
                        );
                        println!("  Data Dir: {}", config.general.data_dir.display());
                        Ok(())
                    }
                    Err(e) => {
                        eprintln!("Config invalid: {e}");
                        std::process::exit(1);
                    }
                }
            }
            ConfigCommands::Get { key } => {
                let loader = nova_config::ConfigLoader::new();
                let config = match cli_config {
                    Some(path) => nova_config::ConfigLoader::parse_file(std::path::Path::new(path))?,
                    None => loader.load(None)?,
                };
                let value = serde_json::to_value(&config)?;
                let parts: Vec<&str> = key.split('.').collect();
                let mut current = &value;
                for part in &parts {
                    current = &current[part];
                }
                match current {
                    serde_json::Value::String(s) => println!("{}", s),
                    serde_json::Value::Number(n) => println!("{}", n),
                    serde_json::Value::Bool(b) => println!("{}", b),
                    other => println!("{}", serde_json::to_string_pretty(other)?),
                }
                Ok(())
            }
            ConfigCommands::Set { key: _, value: _ } => {
                println!("Runtime config changes are not yet supported");
                println!("Modify the config file directly instead.");
                Ok(())
            }
            ConfigCommands::Default => {
                println!("{}", nova_config::DEFAULT_TOML);
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
        cmd: ConfigCommands,
    }

    fn parse(args: &[&str]) -> ConfigCommands {
        Cli::try_parse_from(args).unwrap().cmd
    }

    #[test]
    fn test_show() {
        assert!(matches!(parse(&["test", "show"]), ConfigCommands::Show { section: None }));
        assert!(matches!(parse(&["test", "show", "storage"]), ConfigCommands::Show { section: Some(_) }));
    }

    #[test]
    fn test_get() {
        assert!(matches!(parse(&["test", "get", "storage.page_size"]), ConfigCommands::Get { .. }));
    }

    #[test]
    fn test_set() {
        assert!(matches!(parse(&["test", "set", "key", "value"]), ConfigCommands::Set { .. }));
    }

    #[test]
    fn test_validate() {
        assert!(matches!(parse(&["test", "validate", "/tmp/cfg.toml"]), ConfigCommands::Validate { .. }));
    }

    #[test]
    fn test_default() {
        assert!(matches!(parse(&["test", "default"]), ConfigCommands::Default));
    }
}
