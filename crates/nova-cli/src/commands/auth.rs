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

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct Cli {
        #[command(subcommand)]
        cmd: AuthCommands,
    }

    fn parse(args: &[&str]) -> AuthCommands {
        Cli::try_parse_from(args).unwrap().cmd
    }

    #[test]
    fn test_create_user() {
        assert!(matches!(parse(&["test", "create-user", "admin"]), AuthCommands::CreateUser { username: _, role: None }));
        assert!(matches!(parse(&["test", "create-user", "admin", "readonly"]), AuthCommands::CreateUser { username: _, role: Some(_) }));
    }

    #[test]
    fn test_delete_user() {
        assert!(matches!(parse(&["test", "delete-user", "admin"]), AuthCommands::DeleteUser { .. }));
    }

    #[test]
    fn test_list_users() {
        assert!(matches!(parse(&["test", "list-users"]), AuthCommands::ListUsers));
    }

    #[test]
    fn test_create_api_key() {
        assert!(matches!(parse(&["test", "create-api-key", "my-key"]), AuthCommands::CreateApiKey { .. }));
    }

    #[test]
    fn test_revoke_api_key() {
        assert!(matches!(parse(&["test", "revoke-api-key", "key-123"]), AuthCommands::RevokeApiKey { .. }));
    }

    #[test]
    fn test_execute_returns_ok() {
        for cmd in &[
            AuthCommands::ListUsers,
            AuthCommands::CreateUser { username: "u".into(), role: None },
            AuthCommands::DeleteUser { username: "u".into() },
            AuthCommands::CreateApiKey { name: "k".into() },
            AuthCommands::RevokeApiKey { key_id: "id".into() },
        ] {
            assert!(cmd.execute().is_ok());
        }
    }
}
