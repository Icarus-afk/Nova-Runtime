use clap::Subcommand;

#[derive(Subcommand)]
pub enum BlobCommands {
    /// List blobs
    List {
        #[arg(short, long)]
        prefix: Option<String>,
    },
    /// Upload a blob
    Put {
        key: String,
        file: String,
    },
    /// Download a blob
    Get {
        key: String,
        /// Output file path
        output_file: Option<String>,
    },
    /// Delete a blob
    Delete {
        key: String,
    },
}

impl BlobCommands {
    pub fn execute(&self) -> anyhow::Result<()> {
        println!("Blob command not yet implemented");
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
    fn test_execute_returns_ok() {
        assert!(BlobCommands::List { prefix: None }.execute().is_ok());
        assert!(BlobCommands::Put { key: "k".into(), file: "f".into() }.execute().is_ok());
        assert!(BlobCommands::Get { key: "k".into(), output_file: None }.execute().is_ok());
        assert!(BlobCommands::Delete { key: "k".into() }.execute().is_ok());
    }
}
