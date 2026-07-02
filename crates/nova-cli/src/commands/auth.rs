use clap::Subcommand;

#[derive(Subcommand)]
pub enum AuthCommands {
    /// Create a new user
    CreateUser {
        username: String,
        role: Option<String>,
    },
    /// Delete a user
    DeleteUser {
        username: String,
    },
    /// List all users
    ListUsers,
    /// Create a new API key
    CreateApiKey {
        name: String,
    },
    /// Revoke an API key
    RevokeApiKey {
        key_id: String,
    },
}

impl AuthCommands {
    pub fn execute(&self) -> anyhow::Result<()> {
        println!("Auth command not yet implemented");
        Ok(())
    }
}
