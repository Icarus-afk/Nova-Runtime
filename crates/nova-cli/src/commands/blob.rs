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
        output: Option<String>,
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
