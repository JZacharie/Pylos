use async_trait::async_trait;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::AsyncBufReadExt;
use tokio::process::Command;
use tracing::{debug, error};

use pylos_core::domain::openai::{ChatCompletionRequest, MessageRole};
use pylos_core::domain::provider::ProviderConfig;
use pylos_core::domain::request::{PylosRequest, PylosResponse};
use pylos_core::domain::traits::{ChunkStream, Provider};
use pylos_core::error::PylosError;

use super::converters::{
    normalize_antigravity_model, to_chat_completion_response, to_stream_chunk_delta,
    to_stream_chunk_final, AgyResponse, AgyStreamEvent,
};
use super::session::SessionManager;

/// Provider pour Google Antigravity.
/// Exécute les requêtes via l'interface CLI Antigravity (agy), permettant :
/// - L'imputation transparente sur les quotas et crédits Antigravity de l'utilisateur.
/// - La persistance des sessions et contextes multi-tours via `--conversation <conversation_id>`.
/// - Le support multi-comptes avec rotation / sticky sessions via `AGY_ACCOUNT`.
/// - Le streaming temps-réel via `--output-format stream-json`.
pub struct AntigravityProvider {
    session_manager: Arc<SessionManager>,
    custom_bin: Option<PathBuf>,
    rr_counter: Arc<AtomicUsize>,
}

impl AntigravityProvider {
    pub fn new() -> Self {
        Self {
            session_manager: Arc::new(SessionManager::new()),
            custom_bin: None,
            rr_counter: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn with_bin(bin: PathBuf) -> Self {
        Self {
            session_manager: Arc::new(SessionManager::new()),
            custom_bin: Some(bin),
            rr_counter: Arc::new(AtomicUsize::new(0)),
        }
    }

    pub fn session_manager(&self) -> Arc<SessionManager> {
        self.session_manager.clone()
    }

    /// Résout le compte Antigravity à utiliser pour la requête
    pub fn resolve_account(
        &self,
        config: &ProviderConfig,
        explicit_account: Option<&str>,
        session_account: Option<&str>,
        req_user: Option<&str>,
    ) -> Option<String> {
        // 1. Session déjà attachée à un compte (sticky session Antigravity)
        if let Some(acc) = session_account {
            let trimmed = acc.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }

        // 2. Compte explicitement demandé (via header x-antigravity-account ou payload.account)
        if let Some(acc) = explicit_account {
            let trimmed = acc.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }

        // 3. Si `req.user` ressemble à une adresse e-mail de compte
        if let Some(user) = req_user {
            let trimmed = user.trim();
            if trimmed.contains('@') {
                return Some(trimmed.to_string());
            }
        }

        // 4. Clés configurées dans ProviderConfig.keys (par exemple joseph.zacharie@gmail.com, joseph.zacharie.ai3@gmail.com)
        let accounts: Vec<&str> = config
            .keys
            .iter()
            .map(|k| k.value.trim())
            .filter(|v| !v.is_empty() && *v != "antigravity-local")
            .collect();

        if !accounts.is_empty() {
            let idx = self.rr_counter.fetch_add(1, Ordering::Relaxed) % accounts.len();
            return Some(accounts[idx].to_string());
        }

        // 5. Fallback variable d'environnement AGY_ACCOUNT
        std::env::var("AGY_ACCOUNT").ok().filter(|s| !s.trim().is_empty())
    }

    /// Résout le chemin vers l'exécutable `agy`
    pub fn resolve_bin(&self, config: &ProviderConfig) -> Result<PathBuf, PylosError> {
        if let Some(ref bin) = self.custom_bin {
            if bin.exists() {
                return Ok(bin.clone());
            }
        }

        // 1. URL de base configurée comme chemin de fichier
        if let Some(ref base_url) = config.base_url {
            let path = Path::new(base_url);
            if path.exists() {
                return Ok(path.to_path_buf());
            }
        }

        // 2. Variables d'environnement spécifiques
        for env_var in &["ANTIGRAVITY_BIN", "AGY_BIN"] {
            if let Ok(val) = std::env::var(env_var) {
                let path = PathBuf::from(val);
                if path.exists() {
                    return Ok(path);
                }
            }
        }

        // 3. Emplacement standard ~/.gemini/bin/agy[.exe]
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_default();

        if !home.is_empty() {
            let home_bin_win = Path::new(&home).join(".gemini").join("bin").join("agy.exe");
            if home_bin_win.exists() {
                return Ok(home_bin_win);
            }
            let home_bin_unix = Path::new(&home).join(".gemini").join("bin").join("agy");
            if home_bin_unix.exists() {
                return Ok(home_bin_unix);
            }
        }

        // 4. Recherche dans le PATH
        if let Ok(path) = which_bin("agy") {
            return Ok(path);
        }

        Err(PylosError::ProviderError {
            provider: "antigravity".into(),
            message: "Antigravity CLI (agy) introuvable. Assurez-vous que Google Antigravity est installé dans ~/.gemini/bin/agy ou défini dans ANTIGRAVITY_BIN.".into(),
        })
    }

    /// Construit le prompt, identifie la session conversationnelle et résout le compte
    async fn prepare_request(
        &self,
        req: &ChatCompletionRequest,
        config: &ProviderConfig,
    ) -> (String, Option<String>, Option<String>, Option<String>) {
        // Détection de l'identifiant client : req.session_id ou req.user
        let client_session = req
            .session_id
            .clone()
            .or_else(|| req.user.clone())
            .filter(|s| !s.trim().is_empty());

        let mut agy_conversation_id = None;
        let mut session_account = None;

        if let Some(ref s_id) = client_session {
            // Vérifie si la session est déjà associée à un conversation_id et un compte
            if let Some((conv_id, acc)) = self.session_manager.get_session(s_id).await {
                agy_conversation_id = Some(conv_id);
                session_account = acc;
            } else if is_uuid(s_id) {
                // Si le client fournit directement un UUID Antigravity valide
                agy_conversation_id = Some(s_id.clone());
            }
        }

        let selected_account = self.resolve_account(
            config,
            req.account.as_deref(),
            session_account.as_deref(),
            req.user.as_deref(),
        );

        // Construction du prompt :
        // Si une conversation Antigravity existe déjà, Antigravity détient l'historique complet,
        // nous n'avons besoin d'envoyer que le dernier message utilisateur.
        let prompt = if agy_conversation_id.is_some() {
            req.messages
                .iter()
                .rev()
                .find(|m| matches!(m.role, MessageRole::User))
                .map(|m| m.content_str().to_string())
                .unwrap_or_else(|| {
                    req.messages
                        .last()
                        .map(|m| m.content_str().to_string())
                        .unwrap_or_default()
                })
        } else if req.messages.len() == 1 {
            req.messages[0].content_str().to_string()
        } else {
            // Première initialisation avec contexte multi-tours
            let mut full = String::new();
            for msg in &req.messages {
                match msg.role {
                    MessageRole::System => {
                        full.push_str(&format!("Instructions: {}\n\n", msg.content_str()));
                    }
                    MessageRole::User => {
                        full.push_str(&format!("User: {}\n\n", msg.content_str()));
                    }
                    MessageRole::Assistant => {
                        full.push_str(&format!("Assistant: {}\n\n", msg.content_str()));
                    }
                    MessageRole::Tool | MessageRole::Function => {
                        full.push_str(&format!("Tool Output: {}\n\n", msg.content_str()));
                    }
                }
            }
            full.trim_end().to_string()
        };

        (prompt, agy_conversation_id, client_session, selected_account)
    }
}

fn is_uuid(s: &str) -> bool {
    s.len() == 36 && s.chars().filter(|&c| c == '-').count() == 4
}

fn which_bin(name: &str) -> Result<PathBuf, ()> {
    if let Ok(paths) = std::env::var("PATH") {
        let separator = if cfg!(windows) { ';' } else { ':' };
        for dir in paths.split(separator) {
            let p = Path::new(dir).join(name);
            if p.is_file() {
                return Ok(p);
            }
            #[cfg(windows)]
            {
                let p_exe = Path::new(dir).join(format!("{}.exe", name));
                if p_exe.is_file() {
                    return Ok(p_exe);
                }
            }
        }
    }
    Err(())
}

#[async_trait]
impl Provider for AntigravityProvider {
    fn name(&self) -> &str {
        "antigravity"
    }

    async fn complete(
        &self,
        request: &PylosRequest,
        config: &ProviderConfig,
    ) -> Result<PylosResponse, PylosError> {
        let req = match request {
            PylosRequest::ChatCompletion(r) => r,
            PylosRequest::TextCompletion(_) => {
                return Err(PylosError::Unsupported(
                    "AntigravityProvider requires ChatCompletion".into(),
                ));
            }
            PylosRequest::Embedding(_) => {
                return Err(PylosError::Unsupported(
                    "Antigravity does not support embeddings".into(),
                ));
            }
            PylosRequest::Image(_) => {
                return Err(PylosError::Unsupported(
                    "Antigravity does not support direct image generation".into(),
                ));
            }
        };

        let bin = self.resolve_bin(config)?;
        let (prompt, agy_conversation_id, client_session, selected_account) =
            self.prepare_request(req, config).await;
        let agy_model = normalize_antigravity_model(&req.model);

        debug!(
            bin = %bin.display(),
            model = %agy_model,
            account = ?selected_account,
            conversation_id = ?agy_conversation_id,
            "Invoking Antigravity CLI"
        );

        let mut cmd = Command::new(&bin);
        if let Some(ref acc) = selected_account {
            cmd.env("AGY_ACCOUNT", acc);
        }
        cmd.arg("-p")
            .arg(&prompt)
            .arg("--model")
            .arg(&agy_model)
            .arg("--disable-slash-commands")
            .arg("--output-format")
            .arg("json");

        if let Some(ref conv) = agy_conversation_id {
            cmd.arg("--conversation").arg(conv);
        }

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let timeout_duration = Duration::from_millis(config.timeout_ms.max(120_000));
        let output = tokio::time::timeout(timeout_duration, cmd.output())
            .await
            .map_err(|_| {
                PylosError::Timeout(format!(
                    "Antigravity CLI timed out after {}s",
                    timeout_duration.as_secs()
                ))
            })?
            .map_err(|e| PylosError::ProviderError {
                provider: "antigravity".into(),
                message: format!("Failed to execute agy: {}", e),
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            error!(status = ?output.status.code(), stderr = %stderr, "Antigravity CLI execution failed");
            return Err(PylosError::ProviderError {
                provider: "antigravity".into(),
                message: format!("Antigravity CLI error: {}", stderr.trim()),
            });
        }

        let stdout_str = String::from_utf8_lossy(&output.stdout);
        let agy_resp: AgyResponse = serde_json::from_str(&stdout_str).map_err(|e| {
            error!(stdout = %stdout_str, error = %e, "Failed to parse Antigravity JSON output");
            PylosError::ProviderError {
                provider: "antigravity".into(),
                message: format!("Invalid Antigravity JSON output: {}", e),
            }
        })?;

        if agy_resp.status == "ERROR" {
            let msg = agy_resp.error.unwrap_or_else(|| "Unknown error".into());
            return Err(PylosError::ProviderError {
                provider: "antigravity".into(),
                message: format!("Antigravity: {}", msg),
            });
        }

        // Liaison de session pour les futurs tours (avec compte associé)
        if let Some(ref conv_id) = agy_resp.conversation_id {
            if let Some(ref c_id) = client_session {
                self.session_manager
                    .bind_session(c_id, conv_id, selected_account.as_deref())
                    .await;
            }
        }

        let response = to_chat_completion_response(agy_resp, &req.model);
        Ok(PylosResponse::ChatCompletion(response))
    }

    async fn stream(
        &self,
        request: &PylosRequest,
        config: &ProviderConfig,
    ) -> Result<ChunkStream, PylosError> {
        let req = match request {
            PylosRequest::ChatCompletion(r) => r.clone(),
            _ => {
                return Err(PylosError::Unsupported(
                    "AntigravityProvider stream requires ChatCompletion".into(),
                ));
            }
        };

        let bin = self.resolve_bin(config)?;
        let (prompt, agy_conversation_id, client_session, selected_account) =
            self.prepare_request(&req, config).await;
        let agy_model = normalize_antigravity_model(&req.model);
        let requested_model = req.model.clone();

        let mut cmd = Command::new(&bin);
        if let Some(ref acc) = selected_account {
            cmd.env("AGY_ACCOUNT", acc);
        }
        cmd.arg("-p")
            .arg(&prompt)
            .arg("--model")
            .arg(&agy_model)
            .arg("--disable-slash-commands")
            .arg("--output-format")
            .arg("stream-json");

        if let Some(ref conv) = agy_conversation_id {
            cmd.arg("--conversation").arg(conv);
        }

        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

        let mut child = cmd.spawn().map_err(|e| PylosError::ProviderError {
            provider: "antigravity".into(),
            message: format!("Failed to spawn agy process: {}", e),
        })?;

        let stdout = child.stdout.take().ok_or_else(|| PylosError::ProviderError {
            provider: "antigravity".into(),
            message: "Failed to capture agy stdout".into(),
        })?;

        let session_manager = self.session_manager.clone();
        let bound_account = selected_account.clone();

        let stream = async_stream::try_stream! {
            let mut reader = tokio::io::BufReader::new(stdout).lines();
            let mut current_conv_id = agy_conversation_id.unwrap_or_default();

            while let Some(line) = reader.next_line().await.map_err(|e| {
                PylosError::ProviderError {
                    provider: "antigravity".into(),
                    message: format!("IO error reading agy stream: {}", e),
                }
            })? {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                if let Ok(event) = serde_json::from_str::<AgyStreamEvent>(trimmed) {
                    if let Some(ref c_id) = event.conversation_id {
                        current_conv_id = c_id.clone();
                    }

                    match event.event.as_str() {
                        "step_update" => {
                            if let Some(step) = event.step_update {
                                if let Some(delta) = step.text_delta {
                                    if !delta.is_empty() {
                                        yield to_stream_chunk_delta(&delta, &current_conv_id, &requested_model);
                                    }
                                }
                            }
                        }
                        "result" => {
                            if let Some(res) = event.result {
                                if let Some(ref c_id) = res.conversation_id {
                                    if let Some(ref s_id) = client_session {
                                        session_manager
                                            .bind_session(s_id, c_id, bound_account.as_deref())
                                            .await;
                                    }
                                }
                                yield to_stream_chunk_final(&current_conv_id, &requested_model, res.usage);
                            }
                        }
                        _ => {}
                    }
                }
            }

            let _ = child.wait().await;
        };

        Ok(Box::pin(stream))
    }

    async fn health_check(&self, config: &ProviderConfig) -> Result<(), PylosError> {
        let bin = self.resolve_bin(config)?;
        let mut cmd = Command::new(&bin);
        cmd.arg("--version");

        let output = tokio::time::timeout(Duration::from_secs(3), cmd.output())
            .await
            .map_err(|_| PylosError::Timeout("Antigravity health check timed out".into()))?
            .map_err(|e| PylosError::ProviderError {
                provider: "antigravity".into(),
                message: format!("Health check error: {}", e),
            })?;

        if output.status.success() {
            Ok(())
        } else {
            Err(PylosError::ProviderError {
                provider: "antigravity".into(),
                message: "Antigravity CLI did not return status 0".into(),
            })
        }
    }
}
