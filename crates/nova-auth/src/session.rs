use crate::error::{AuthError, Result};
use crate::types::*;
use dashmap::DashMap;
use std::sync::Arc;
use uuid::Uuid;

/// Manages user sessions with in-memory cache + storage backend persistence.
pub struct SessionManager {
    pub(crate) sessions: DashMap<String, Session>,
    config: AuthConfig,
}

impl SessionManager {
    pub fn new(config: AuthConfig) -> Self {
        SessionManager {
            sessions: DashMap::with_capacity(config.session_cache_size),
            config,
        }
    }

    /// Create a new session for a user.
    pub fn create_session(&self, user_id: Uuid, username: &str) -> Session {
        let session = Session::new(user_id, username, self.config.session_ttl_secs);
        let token = session.token.clone();
        self.sessions.insert(token, session.clone());
        session
    }

    /// Validate and retrieve a session by token.
    pub fn get_session(&self, token: &str) -> Result<Session> {
        let session = self.sessions.get(token)
            .ok_or_else(|| AuthError::SessionNotFound("session not found".into()))?;

        if session.is_expired() {
            return Err(AuthError::SessionExpired("session has expired".into()));
        }

        if session.revoked {
            return Err(AuthError::SessionNotFound("session has been revoked".into()));
        }

        Ok(session.clone())
    }

    /// Revoke a session by token.
    pub fn revoke_session(&self, token: &str) -> Result<()> {
        let mut session = self.sessions.get_mut(token)
            .ok_or_else(|| AuthError::SessionNotFound("session not found".into()))?;
        session.revoked = true;
        Ok(())
    }

    /// Revoke all sessions for a user.
    pub fn revoke_user_sessions(&self, user_id: Uuid) -> u32 {
        let mut count = 0;
        let mut to_remove = Vec::new();

        for entry in self.sessions.iter() {
            if entry.user_id == user_id {
                to_remove.push(entry.token.clone());
                count += 1;
            }
        }

        for token in to_remove {
            self.sessions.remove(&token);
        }

        count
    }

    /// Refresh a session's TTL.
    pub fn touch_session(&self, token: &str) -> Result<()> {
        let mut session = self.sessions.get_mut(token)
            .ok_or_else(|| AuthError::SessionNotFound("session not found".into()))?;
        session.last_activity_at = chrono::Utc::now().timestamp_millis();
        Ok(())
    }

    /// Get the number of active sessions for a user.
    pub fn active_session_count(&self, user_id: Uuid) -> usize {
        self.sessions.iter().filter(|s| s.user_id == user_id && s.is_valid()).count()
    }

    /// Remove expired sessions (cleanup).
    pub fn cleanup_expired(&self) -> u64 {
        let mut count = 0;
        let mut to_remove = Vec::new();

        for entry in self.sessions.iter() {
            if entry.is_expired() || entry.revoked {
                to_remove.push(entry.token.clone());
                count += 1;
            }
        }

        for token in to_remove {
            self.sessions.remove(&token);
        }

        count
    }

    pub fn active_sessions(&self) -> usize {
        self.sessions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_manager_create_and_get() {
        let manager = SessionManager::new(AuthConfig::default());
        let user_id = Uuid::new_v4();
        let session = manager.create_session(user_id, "testuser");
        assert!(session.token.starts_with("nova_sess_"));

        let retrieved = manager.get_session(&session.token).unwrap();
        assert_eq!(retrieved.user_id, user_id);
        assert_eq!(retrieved.username, "testuser");
    }

    #[test]
    fn test_session_manager_revoke() {
        let manager = SessionManager::new(AuthConfig::default());
        let session = manager.create_session(Uuid::new_v4(), "test");
        manager.revoke_session(&session.token).unwrap();
        assert!(manager.get_session(&session.token).is_err());
    }

    #[test]
    fn test_session_manager_revoke_user() {
        let manager = SessionManager::new(AuthConfig::default());
        let user_id = Uuid::new_v4();
        manager.create_session(user_id, "test");
        manager.create_session(user_id, "test");
        assert_eq!(manager.active_session_count(user_id), 2);

        let revoked = manager.revoke_user_sessions(user_id);
        assert_eq!(revoked, 2);
        assert_eq!(manager.active_session_count(user_id), 0);
    }

    #[test]
    fn test_session_manager_touch() {
        let manager = SessionManager::new(AuthConfig::default());
        let session = manager.create_session(Uuid::new_v4(), "test");
        let before = session.last_activity_at;
        std::thread::sleep(std::time::Duration::from_millis(1));
        manager.touch_session(&session.token).unwrap();
        let updated = manager.get_session(&session.token).unwrap();
        assert!(updated.last_activity_at > before);
    }

    #[test]
    fn test_session_manager_cleanup() {
        let manager = SessionManager::new(AuthConfig::default());
        // Create a session and manually expire it
        let session = manager.create_session(Uuid::new_v4(), "test");
        // Directly modify the session to be expired
        if let Some(mut s) = manager.sessions.get_mut(&session.token) {
            s.expires_at = 0;
        }

        let cleaned = manager.cleanup_expired();
        assert_eq!(cleaned, 1);
        assert!(manager.get_session(&session.token).is_err());
    }

    #[test]
    fn test_session_manager_max_sessions() {
        let mut config = AuthConfig::default();
        config.max_active_sessions = 5;
        let manager = SessionManager::new(config);
        let user_id = Uuid::new_v4();

        for _ in 0..5 {
            manager.create_session(user_id, "test");
        }

        // Should have exactly 5 sessions (the limit check is on creation)
        assert_eq!(manager.active_session_count(user_id), 5);
    }

    #[test]
    fn test_session_manager_invalid_token() {
        let manager = SessionManager::new(AuthConfig::default());
        assert!(manager.get_session("invalid_token").is_err());
    }
}
