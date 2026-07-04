use clap::Subcommand;
use crate::app::CommandContext;
use crate::client::ApiClient;
use crate::output;
use std::fs;
use std::io::Write;

#[derive(Subcommand)]
pub enum BlobCommands {
    List {
        #[arg(short, long)]
        prefix: Option<String>,
    },
    Put {
        key: String,
        file: String,
    },
    Get {
        key: String,
        output_file: Option<String>,
    },
    Delete {
        key: String,
    },
}

impl BlobCommands {
    pub fn execute(&self, ctx: &CommandContext) -> anyhow::Result<()> {
        let client = ApiClient::new(&ctx.address, ctx.api_key.as_deref());
        match self {
            BlobCommands::List { prefix } => {
                let params: Vec<(&str, &str)> = prefix.as_ref().map(|p| vec![("prefix", p.as_str())]).unwrap_or_default();
                match if params.is_empty() { client.get("/v1/blob") } else { client.get_with_query("/v1/blob", &params) } {
                    Ok(body) => {
                        let blobs = if body.is_array() {
                            body.clone()
                        } else {
                            body.get("blobs").cloned().unwrap_or(body.clone())
                        };
                        if let Some(arr) = blobs.as_array() {
                            output::print_table_from_json(
                                &["Key", "Size", "Content-Type"],
                                arr,
                                |b| vec![
                                    b["key"].as_str().unwrap_or("-").to_string(),
                                    b["size"].as_u64().map(|v| v.to_string()).unwrap_or_else(|| "-".to_string()),
                                    b["content_type"].as_str().unwrap_or("-").to_string(),
                                ],
                                &ctx.output,
                            )?;
                        } else {
                            output::print_value(&body, &ctx.output)?;
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to list blobs: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            BlobCommands::Put { key, file } => {
                let data = fs::read(file).map_err(|e| anyhow::anyhow!("Failed to read file '{file}': {e}"))?;
                let body = serde_json::json!({
                    "key": key,
                    "data": base64_encode(&data),
                    "filename": file,
                });
                match client.post("/v1/blob", Some(&body)) {
                    Ok(resp) => output::print_value(&resp, &ctx.output)?,
                    Err(e) => {
                        eprintln!("Failed to upload blob: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            BlobCommands::Get { key, output_file } => {
                match client.get(&format!("/v1/blob/{key}")) {
                    Ok(body) => {
                        let data_str = body.get("data").and_then(|v| v.as_str()).unwrap_or("");
                        let decoded = base64_decode(data_str).unwrap_or_default();
                        match output_file {
                            Some(path) => {
                                let mut f = fs::File::create(path).map_err(|e| anyhow::anyhow!("Failed to create file: {e}"))?;
                                f.write_all(&decoded).map_err(|e| anyhow::anyhow!("Failed to write file: {e}"))?;
                                println!("Downloaded {key} to {path}");
                            }
                            None => {
                                let meta = serde_json::json!({
                                    "key": key,
                                    "size": decoded.len(),
                                    "content_type": body.get("content_type"),
                                });
                                output::print_value(&meta, &ctx.output)?;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to download blob: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            BlobCommands::Delete { key } => {
                match client.delete(&format!("/v1/blob/{key}")) {
                    Ok(resp) => output::print_value(&resp, &ctx.output)?,
                    Err(e) => {
                        eprintln!("Failed to delete blob: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
        }
    }
}

fn base64_encode(data: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(data)
}

fn base64_decode(data: &str) -> Option<Vec<u8>> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.decode(data).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct Cli {
        #[command(subcommand)]
        cmd: BlobCommands,
    }

    fn parse(args: &[&str]) -> BlobCommands {
        Cli::try_parse_from(args).unwrap().cmd
    }

    #[test]
    fn test_list() {
        assert!(matches!(parse(&["test", "list"]), BlobCommands::List { prefix: None }));
        assert!(matches!(parse(&["test", "list", "--prefix", "img/"]), BlobCommands::List { prefix: Some(_) }));
    }

    #[test]
    fn test_put() {
        assert!(matches!(parse(&["test", "put", "k", "f.txt"]), BlobCommands::Put { .. }));
    }

    #[test]
    fn test_get() {
        assert!(matches!(parse(&["test", "get", "k"]), BlobCommands::Get { .. }));
        assert!(matches!(parse(&["test", "get", "k", "out.txt"]), BlobCommands::Get { .. }));
    }

    #[test]
    fn test_delete() {
        assert!(matches!(parse(&["test", "delete", "k"]), BlobCommands::Delete { .. }));
    }

    #[test]
    fn test_base64_roundtrip() {
        let data = b"hello world";
        let encoded = base64_encode(data);
        let decoded = base64_decode(&encoded).unwrap();
        assert_eq!(decoded, data);
    }

    #[test]
    fn test_base64_empty() {
        assert_eq!(base64_encode(b""), "");
        assert_eq!(base64_decode(""), Some(vec![]));
    }

    #[test]
    fn test_base64_invalid() {
        assert!(base64_decode("!!!").is_none());
    }
}
