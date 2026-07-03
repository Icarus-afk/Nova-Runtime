use crate::types::*;
use std::collections::{HashMap, HashSet};

/// Role-Based Access Control engine.
pub struct RbacEngine {
    roles: HashMap<String, Role>,
    user_roles: HashMap<uuid::Uuid, Vec<String>>,
}

impl RbacEngine {
    pub fn new() -> Self {
        RbacEngine {
            roles: HashMap::new(),
            user_roles: HashMap::new(),
        }
    }

    /// Define a new role.
    pub fn define_role(&mut self, role: Role) {
        self.roles.insert(role.name.clone(), role);
    }

    /// Remove a role definition.
    pub fn remove_role(&mut self, name: &str) {
        self.roles.remove(name);
        for user_roles in self.user_roles.values_mut() {
            user_roles.retain(|r| r != name);
        }
    }

    /// Assign a role to a user.
    pub fn assign_role(&mut self, user_id: uuid::Uuid, role_name: &str) -> Result<(), String> {
        if !self.roles.contains_key(role_name) {
            return Err(format!("Role '{}' not defined", role_name));
        }
        self.user_roles.entry(user_id).or_default().push(role_name.to_string());
        Ok(())
    }

    /// Remove a role from a user.
    pub fn unassign_role(&mut self, user_id: uuid::Uuid, role_name: &str) {
        if let Some(roles) = self.user_roles.get_mut(&user_id) {
            roles.retain(|r| r != role_name);
        }
    }

    /// Check if a permission string matches a pattern (supports wildcards).
    fn permission_matches(pattern: &str, target: &str) -> bool {
        if pattern == "*:*" {
            return true;
        }
        if pattern == target {
            return true;
        }
        if let Some((p_action, p_resource)) = pattern.split_once(':') {
            if let Some((t_action, t_resource)) = target.split_once(':') {
                if p_action == "*" && p_resource == t_resource {
                    return true;
                }
                if p_resource == "*" && p_action == t_action {
                    return true;
                }
            }
        }
        false
    }

    /// Get all permissions for a user.
    pub fn user_permissions(&self, user_id: &uuid::Uuid) -> HashSet<String> {
        let mut perms = HashSet::new();
        if let Some(role_names) = self.user_roles.get(user_id) {
            for role_name in role_names {
                if let Some(role) = self.roles.get(role_name) {
                    perms.extend(role.permissions.clone());
                }
            }
        }
        perms
    }

    /// Check if a user has a specific permission (supports wildcard matching).
    pub fn has_permission(&self, user_id: &uuid::Uuid, permission: &str) -> bool {
        let user_perms = self.user_permissions(user_id);
        user_perms.iter().any(|p| Self::permission_matches(p, permission))
    }

    /// Check if a user has any of the given permissions.
    pub fn has_any_permission(&self, user_id: &uuid::Uuid, permissions: &[&str]) -> bool {
        let user_perms = self.user_permissions(user_id);
        permissions.iter().any(|p| user_perms.iter().any(|up| Self::permission_matches(up, p)))
    }

    /// Check if a user has all of the given permissions.
    pub fn has_all_permissions(&self, user_id: &uuid::Uuid, permissions: &[&str]) -> bool {
        let user_perms = self.user_permissions(user_id);
        permissions.iter().all(|p| user_perms.iter().any(|up| Self::permission_matches(up, p)))
    }

    /// Evaluate a permission request against the RBAC policy.
    pub fn evaluate(&self, request: &PermissionRequest) -> bool {
        let user_perms = self.user_permissions(&request.user_id);
        let required = format!("{}:{}", request.action, request.resource);
        user_perms.iter().any(|p| Self::permission_matches(p, &required))
    }

    pub fn role_names(&self) -> Vec<String> {
        self.roles.keys().cloned().collect()
    }

    pub fn user_role_names(&self, user_id: &uuid::Uuid) -> Vec<String> {
        self.user_roles.get(user_id).cloned().unwrap_or_default()
    }
}

impl Default for RbacEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn setup_rbac() -> RbacEngine {
        let mut rbac = RbacEngine::new();
        rbac.define_role(Role {
            name: "admin".into(),
            description: "Administrator".into(),
            permissions: vec!["*:*".into()],
            created_at: 0,
        });
        rbac.define_role(Role {
            name: "editor".into(),
            description: "Can read and write documents".into(),
            permissions: vec!["read:document".into(), "write:document".into()],
            created_at: 0,
        });
        rbac.define_role(Role {
            name: "viewer".into(),
            description: "Can only read".into(),
            permissions: vec!["read:*".into()],
            created_at: 0,
        });
        rbac
    }

    #[test]
    fn test_rbac_define_role() {
        let rbac = setup_rbac();
        assert_eq!(rbac.role_names().len(), 3);
    }

    #[test]
    fn test_rbac_assign_and_check_permission() {
        let mut rbac = setup_rbac();
        let user_id = Uuid::new_v4();
        assert!(rbac.assign_role(user_id, "editor").is_ok());
        assert!(rbac.has_permission(&user_id, "read:document"));
        assert!(rbac.has_permission(&user_id, "write:document"));
        assert!(!rbac.has_permission(&user_id, "delete:document"));
    }

    #[test]
    fn test_rbac_admin_has_all_permissions() {
        let mut rbac = setup_rbac();
        let user_id = Uuid::new_v4();
        rbac.assign_role(user_id, "admin").unwrap();
        assert!(rbac.has_permission(&user_id, "anything:anything"));
        assert!(rbac.has_permission(&user_id, "*:*"));
    }

    #[test]
    fn test_rbac_viewer_read_access() {
        let mut rbac = setup_rbac();
        let user_id = Uuid::new_v4();
        rbac.assign_role(user_id, "viewer").unwrap();
        assert!(rbac.has_permission(&user_id, "read:document"));
        assert!(rbac.has_permission(&user_id, "read:report"));
        assert!(!rbac.has_permission(&user_id, "write:document"));
    }

    #[test]
    fn test_rbac_unassign_role() {
        let mut rbac = setup_rbac();
        let user_id = Uuid::new_v4();
        rbac.assign_role(user_id, "editor").unwrap();
        assert!(rbac.has_permission(&user_id, "read:document"));
        rbac.unassign_role(user_id, "editor");
        assert!(!rbac.has_permission(&user_id, "read:document"));
    }

    #[test]
    fn test_rbac_evaluate_request() {
        let mut rbac = setup_rbac();
        let user_id = Uuid::new_v4();
        rbac.assign_role(user_id, "editor").unwrap();

        let request = PermissionRequest {
            user_id,
            action: "read".into(),
            resource: "document".into(),
            context: std::collections::HashMap::new(),
        };
        assert!(rbac.evaluate(&request));

        let deny_request = PermissionRequest {
            user_id,
            action: "delete".into(),
            resource: "document".into(),
            context: std::collections::HashMap::new(),
        };
        assert!(!rbac.evaluate(&deny_request));
    }

    #[test]
    fn test_rbac_has_any_permission() {
        let mut rbac = setup_rbac();
        let user_id = Uuid::new_v4();
        rbac.assign_role(user_id, "viewer").unwrap();
        assert!(rbac.has_any_permission(&user_id, &["read:document", "write:document"]));
        assert!(!rbac.has_any_permission(&user_id, &["write:document", "delete:document"]));
    }

    #[test]
    fn test_rbac_has_all_permissions() {
        let mut rbac = setup_rbac();
        let user_id = Uuid::new_v4();
        rbac.assign_role(user_id, "editor").unwrap();
        assert!(rbac.has_all_permissions(&user_id, &["read:document", "write:document"]));
        assert!(!rbac.has_all_permissions(&user_id, &["read:document", "delete:document"]));
    }

    #[test]
    fn test_rbac_remove_role() {
        let mut rbac = setup_rbac();
        let user_id = Uuid::new_v4();
        rbac.assign_role(user_id, "editor").unwrap();
        rbac.remove_role("editor");
        assert!(!rbac.has_permission(&user_id, "read:document"));
    }

    #[test]
    fn test_rbac_assign_nonexistent_role() {
        let mut rbac = RbacEngine::new();
        let user_id = Uuid::new_v4();
        assert!(rbac.assign_role(user_id, "nonexistent").is_err());
    }
}
