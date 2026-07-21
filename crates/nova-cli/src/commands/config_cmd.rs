use clap::Subcommand;
use crate::app::CommandContext;
use crate::client::ApiClient;
use crate::output;

#[derive(Subcommand)]
pub enum ConfigCommands {
    Show {
        section: Option<String>,
    },
    Get {
        key: String,
    },
    Set {
        key: String,
        value: String,
    },
    Validate {
        path: String,
    },
    Default,
}

impl ConfigCommands {
    pub fn execute(&self, ctx: &CommandContext) -> anyhow::Result<()> {
        match self {
            ConfigCommands::Show { section } => {
                let client = ApiClient::new(&ctx.address, ctx.api_key.as_deref());
                match client.get("/admin/config") {
                    Ok(body) => {
                        let result = match section {
                            Some(s) => body.get(s).cloned().unwrap_or(serde_json::json!({"error": format!("section '{s}' not found")})),
                            None => body,
                        };
                        output::print_value(&result, &ctx.output)?;
                    }
                    Err(_e) => {
                        let loader = nova_config::ConfigLoader::new();
                        let config = loader.load(None)?;
                        let cfg_value = serde_json::to_value(&config)?;
                        let result = match section {
                            Some(s) => cfg_value.get(s).cloned().unwrap_or(serde_json::json!({"error": format!("section '{s}' not found")})),
                            None => cfg_value,
                        };
                        output::print_value(&result, &ctx.output)?;
                    }
                }
                Ok(())
            }
            ConfigCommands::Validate { path } => {
                match nova_config::ConfigLoader::parse_file(std::path::Path::new(path)) {
                    Ok(config) => {
                        output::print_value(&serde_json::json!({
                            "valid": true,
                            "listen": format!("{}:{}", config.networking.listen_address, config.networking.listen_port),
                            "data_dir": config.general.data_dir,
                        }), &ctx.output)?;
                    }
                    Err(e) => {
                        eprintln!("Config invalid: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            ConfigCommands::Get { key } => {
                let client = ApiClient::new(&ctx.address, ctx.api_key.as_deref());
                match client.get("/admin/config") {
                    Ok(body) => {
                        let parts: Vec<&str> = key.split('.').collect();
                        let mut current = &body;
                        for part in &parts {
                            current = &current[part];
                        }
                        output::print_value(current, &ctx.output)?;
                    }
                    Err(_e) => {
                        let loader = nova_config::ConfigLoader::new();
                        let config = loader.load(None)?;
                        let value = serde_json::to_value(&config)?;
                        let parts: Vec<&str> = key.split('.').collect();
                        let mut current = &value;
                        for part in &parts {
                            current = &current[part];
                        }
                        output::print_value(current, &ctx.output)?;
                    }
                }
                Ok(())
            }
            ConfigCommands::Set { key, value } => {
                let client = ApiClient::new(&ctx.address, ctx.api_key.as_deref());
                let parsed: serde_json::Value = if let Ok(n) = value.parse::<i64>() {
                    serde_json::json!(n)
                } else if let Ok(n) = value.parse::<f64>() {
                    serde_json::json!(n)
                } else if let Ok(b) = value.parse::<bool>() {
                    serde_json::json!(b)
                } else {
                    serde_json::json!(value)
                };
                let parts: Vec<&str> = key.split('.').collect();
                let mut patch = serde_json::json!({});
                let mut current = &mut patch;
                for (i, part) in parts.iter().enumerate() {
                    if i == parts.len() - 1 {
                        current[part] = parsed.clone();
                    } else {
                        if !current[part].is_object() {
                            current[part] = serde_json::json!({});
                        }
                        current = &mut current[part];
                    }
                }
                match client.put("/admin/config", Some(&patch)) {
                    Ok(_body) => {
                        output::print_value(&serde_json::json!({
                            "status": "updated",
                            "key": key,
                            "value": value,
                        }), &ctx.output)?;
                    }
                    Err(e) => {
                        eprintln!("Failed to set config: {e}");
                        std::process::exit(1);
                    }
                }
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
