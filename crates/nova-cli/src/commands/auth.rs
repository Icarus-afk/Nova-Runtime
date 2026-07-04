use clap::Subcommand;
use crate::app::CommandContext;
use crate::client::ApiClient;
use crate::output;

#[derive(Subcommand)]
pub enum AuthCommands {
    CreateUser {
        username: String,
        role: Option<String>,
    },
    DeleteUser {
        username: String,
    },
    ListUsers,
    CreateApiKey {
        name: String,
    },
    RevokeApiKey {
        key_id: String,
    },
}

impl AuthCommands {
    pub fn execute(&self, ctx: &CommandContext) -> anyhow::Result<()> {
        let client = ApiClient::new(&ctx.address, ctx.api_key.as_deref());
        match self {
            AuthCommands::ListUsers => {
                match client.get("/v1/auth/users") {
                    Ok(body) => {
                        let users = if body.is_array() {
                            body.clone()
                        } else {
                            body.get("users").cloned().unwrap_or(body.clone())
                        };
                        if let Some(arr) = users.as_array() {
                            output::print_table_from_json(
                                &["Username", "Role", "Status"],
                                arr,
                                |u| vec![
                                    u["username"].as_str().unwrap_or("-").to_string(),
                                    u["role"].as_str().unwrap_or("-").to_string(),
                                    u["status"].as_str().unwrap_or("active").to_string(),
                                ],
                                &ctx.output,
                            )?;
                        } else {
                            output::print_value(&body, &ctx.output)?;
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to list users: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            AuthCommands::CreateUser { username, role } => {
                let mut body = serde_json::json!({"username": username});
                if let Some(r) = role {
                    body["role"] = serde_json::json!(r);
                }
                match client.post("/v1/auth/users", Some(&body)) {
                    Ok(resp) => {
                        output::print_value(&resp, &ctx.output)?;
                    }
                    Err(e) => {
                        eprintln!("Failed to create user: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            AuthCommands::DeleteUser { username } => {
                match client.delete(&format!("/v1/auth/users/{username}")) {
                    Ok(resp) => {
                        output::print_value(&resp, &ctx.output)?;
                    }
                    Err(e) => {
                        eprintln!("Failed to delete user: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            AuthCommands::CreateApiKey { name } => {
                let body = serde_json::json!({"name": name});
                match client.post("/v1/auth/api-keys", Some(&body)) {
                    Ok(resp) => {
                        output::print_value(&resp, &ctx.output)?;
                    }
                    Err(e) => {
                        eprintln!("Failed to create API key: {e}");
                        std::process::exit(1);
                    }
                }
                Ok(())
            }
            AuthCommands::RevokeApiKey { key_id } => {
                match client.delete(&format!("/v1/auth/api-keys/{key_id}")) {
                    Ok(resp) => {
                        output::print_value(&resp, &ctx.output)?;
                    }
                    Err(e) => {
                        eprintln!("Failed to revoke API key: {e}");
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
}
