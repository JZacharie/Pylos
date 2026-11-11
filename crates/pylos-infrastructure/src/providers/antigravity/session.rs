use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct SessionEntry {
    pub agy_conversation_id: String,
    pub account: Option<String>,
    pub last_accessed: Instant,
}

/// Gestionnaire de sessions pour Google Antigravity.
/// Maintient la correspondance entre les identifiants de session client (user_id, session_id, x-session-id),
/// les identifiants de conversation natifs Antigravity (UUID) et le compte Google associé (AGY_ACCOUNT).
#[derive(Debug, Clone, Default)]
pub struct SessionManager {
    sessions: Arc<RwLock<HashMap<String, SessionEntry>>>,
}

impl SessionManager {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Récupère l'entrée de session (UUID Antigravity et compte optionnel) associée à une session client
    pub async fn get_session(&self, client_session: &str) -> Option<(String, Option<String>)> {
        let mut guard = self.sessions.write().await;
        if let Some(entry) = guard.get_mut(client_session) {
            entry.last_accessed = Instant::now();
            Some((entry.agy_conversation_id.clone(), entry.account.clone()))
        } else {
            None
        }
    }

    /// Récupère l'UUID conversation_id Antigravity associé à une session client
    pub async fn get_agy_conversation(&self, client_session: &str) -> Option<String> {
        self.get_session(client_session).await.map(|(c, _)| c)
    }

    /// Associe un client_session_id à un conversation_id Antigravity et un compte optionnel
    pub async fn bind_session(
        &self,
        client_session: &str,
        agy_conversation_id: &str,
        account: Option<&str>,
    ) {
        let mut guard = self.sessions.write().await;
        guard.insert(
            client_session.to_string(),
            SessionEntry {
                agy_conversation_id: agy_conversation_id.to_string(),
                account: account.map(ToString::to_string),
                last_accessed: Instant::now(),
            },
        );
    }

    /// Supprime une session
    pub async fn remove_session(&self, client_session: &str) -> Option<String> {
        let mut guard = self.sessions.write().await;
        guard.remove(client_session).map(|e| e.agy_conversation_id)
    }

    /// Retourne le nombre total de sessions actives en mémoire
    pub async fn count(&self) -> usize {
        self.sessions.read().await.len()
    }
}
