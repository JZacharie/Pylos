use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use jsonwebtoken::{encode, EncodingKey, Header};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct AuthConfigResponse {
    pub google_auth_enabled: bool,
    pub google_client_id: Option<String>,
    pub google_redirect_uri: Option<String>,
}

pub async fn get_auth_config(State(state): State<AppState>) -> impl IntoResponse {
    let enabled = state.google_client_id.is_some() && state.google_client_secret.is_some();
    Json(AuthConfigResponse {
        google_auth_enabled: enabled,
        google_client_id: state.google_client_id.clone(),
        google_redirect_uri: state.google_redirect_uri.clone(),
    })
}

#[derive(Debug, Deserialize)]
pub struct CallbackRequest {
    pub code: String,
    pub redirect_uri: Option<String>,
    /// État anti-CSRF, émis par GET /api/auth/google/start et vérifié ici.
    #[serde(default)]
    pub state: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GoogleTokenResponse {
    access_token: String,
}

#[derive(Debug, Deserialize)]
struct GoogleUserInfo {
    email: String,
    name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PylosSessionClaims {
    pub sub: String, // email
    pub exp: usize,  // expiration timestamp (seconds)
    pub role: String,
    pub name: String,
    #[serde(default = "default_group")]
    pub group: String,
}

fn default_group() -> String {
    "default".to_string()
}

/// Cookie portant l'état OAuth (protection CSRF).
const OAUTH_STATE_COOKIE: &str = "pylos_oauth_state";

fn random_oauth_state() -> String {
    format!("{:016x}{:016x}", fastrand::u64(..), fastrand::u64(..))
}

fn read_oauth_state_cookie(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(header::COOKIE)?.to_str().ok()?;
    raw.split(';')
        .filter_map(|part| part.trim().strip_prefix(OAUTH_STATE_COOKIE))
        .filter_map(|rest| rest.strip_prefix('='))
        .map(|value| value.to_string())
        .next()
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

/// Emails administrateurs déclarés via `ADMIN_EMAILS` (séparés par des virgules).
fn is_admin_email(email: &str) -> bool {
    let wanted = email.to_lowercase();
    std::env::var("ADMIN_EMAILS")
        .unwrap_or_default()
        .split(',')
        .map(|candidate| candidate.trim().to_lowercase())
        .any(|candidate| !candidate.is_empty() && candidate == wanted)
}

/// GET /api/auth/google/start — émet un état anti-CSRF et le pose en cookie.
///
/// L'UI utilise cet état dans l'URL d'autorisation Google ; le callback le vérifie.
pub async fn google_start(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if state.google_client_id.is_none() || state.google_client_secret.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Google OAuth is not configured on this server" })),
        )
            .into_response();
    }

    let oauth_state = random_oauth_state();
    let secure = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.eq_ignore_ascii_case("https"))
        .unwrap_or(false);

    let cookie = format!(
        "{OAUTH_STATE_COOKIE}={oauth_state}; HttpOnly; SameSite=Lax; Path=/; Max-Age=600{}",
        if secure { "; Secure" } else { "" }
    );

    let mut response = Json(json!({ "state": oauth_state })).into_response();
    if let Ok(value) = header::HeaderValue::from_str(&cookie) {
        response.headers_mut().insert(header::SET_COOKIE, value);
    }
    response
}

pub async fn google_callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CallbackRequest>,
) -> impl IntoResponse {
    // Protection CSRF : l'état reçu doit correspondre au cookie émis par /start.
    let provided_state = req.state.clone().unwrap_or_default();
    let state_ok = !provided_state.is_empty()
        && read_oauth_state_cookie(&headers)
            .map(|cookie| constant_time_eq(cookie.as_bytes(), provided_state.as_bytes()))
            .unwrap_or(false);
    if !state_ok {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({ "error": "Invalid or missing OAuth state (CSRF protection)" })),
        )
            .into_response();
    }

    let client_id = match &state.google_client_id {
        Some(cid) => cid,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "Google OAuth is not configured on this server" })),
            )
                .into_response()
        }
    };
    let client_secret = match &state.google_client_secret {
        Some(sec) => sec,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "Google OAuth client secret is not configured" })),
            )
                .into_response()
        }
    };

    // Le redirect_uri est imposé par le serveur dès qu'il est configuré ; la valeur
    // envoyée par le client n'est utilisée qu'en dernier recours (développement local).
    let redirect_uri = state
        .google_redirect_uri
        .clone()
        .or_else(|| req.redirect_uri.clone())
        .unwrap_or_else(|| "https://pylos-dev.p.zacharie.org/callback".to_string());

    let client = reqwest::Client::new();

    // 1. Exchange authorization code for tokens
    let params = [
        ("code", req.code.as_str()),
        ("client_id", client_id.as_str()),
        ("client_secret", client_secret.as_str()),
        ("redirect_uri", redirect_uri.as_str()),
        ("grant_type", "authorization_code"),
    ];

    let token_resp =
        match client
            .post("https://oauth2.googleapis.com/token")
            .form(&params)
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to contact Google token endpoint: {}", e) })),
            )
                .into_response(),
        };

    if !token_resp.status().is_success() {
        let status = token_resp.status();
        let body = token_resp.text().await.unwrap_or_default();
        return (
            StatusCode::BAD_REQUEST,
            Json(
                json!({ "error": format!("Google token exchange failed ({}): {}", status, body) }),
            ),
        )
            .into_response();
    }

    let token_data: GoogleTokenResponse = match token_resp.json().await {
        Ok(data) => data,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to parse Google token response: {}", e) })),
            )
                .into_response()
        }
    };

    // 2. Retrieve user info using access_token
    let user_info_resp = match client
        .get("https://www.googleapis.com/oauth2/v3/userinfo")
        .bearer_auth(&token_data.access_token)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to fetch user info from Google: {}", e) })),
            )
                .into_response()
        }
    };

    if !user_info_resp.status().is_success() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": "Google userinfo request failed" })),
        )
            .into_response();
    }

    let google_user: GoogleUserInfo = match user_info_resp.json().await {
        Ok(info) => info,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to parse Google user info: {}", e) })),
            )
                .into_response()
        }
    };

    // 3. Match Google email with InternalUser from OrganizationStore
    let users = match state.org_store.list_users().await {
        Ok(u) => u,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Database error listing users: {}", e) })),
            )
                .into_response()
        }
    };

    let matched_user = users
        .iter()
        .find(|u| u.email.to_lowercase() == google_user.email.to_lowercase());

    let (user_role, user_group) = match matched_user {
        Some(user) => {
            if !user.is_active {
                return (
                    StatusCode::FORBIDDEN,
                    Json(json!({ "error": format!("User email {} is inactive in Pylos", google_user.email) })),
                )
                    .into_response();
            }
            // ADMIN_EMAILS peut promouvoir un compte existant (jamais le rétrograder).
            let role = if is_admin_email(&user.email) {
                "admin".to_string()
            } else {
                user.role.clone()
            };
            (role, user.group.clone())
        }
        None => {
            // Plus d'élévation automatique du premier inscrit : le rôle admin est
            // explicitement accordé via ADMIN_EMAILS.
            let is_admin = is_admin_email(&google_user.email);
            let role = if is_admin { "admin" } else { "member" };
            let group = if is_admin { "default" } else { "playgroup" };
            // Generate a random ID using fastrand
            let random_id = (0..16)
                .map(|_| fastrand::alphanumeric())
                .collect::<String>();
            let new_user = pylos_core::domain::organization::InternalUser {
                id: format!("usr_{}", random_id),
                email: google_user.email.clone(),
                name: google_user
                    .name
                    .clone()
                    .unwrap_or_else(|| google_user.email.clone()),
                role: role.to_string(),
                group: group.to_string(),
                organization_id: None,
                team_ids: vec![],
                is_active: true,
                created_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64,
                updated_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64,
            };
            if let Err(e) = state.org_store.upsert_user(&new_user).await {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": format!("Failed to auto-register user: {}", e) })),
                )
                    .into_response();
            }

            // Create a default virtual key with $1 budget
            let vk_id = format!("vk-{}", random_id);
            let vk_value = format!("sk-pylos-{}", fastrand::u64(..));
            let now_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            let vk_cfg = pylos_core::domain::config::VirtualKeyConfig {
                id: vk_id.clone(),
                name: format!("Default Key for {}", google_user.email),
                description: Some("Automatically generated key on registration".to_string()),
                value: Some(pylos_core::domain::config::EnvVar::Literal(
                    vk_value.clone(),
                )),
                is_active: true,
                rate_limit_id: None,
                provider_configs: vec![],
                team_alias: None,
                team_id: None,
                organization_id: None,
                access_group_id: None,
                user_email: Some(google_user.email.clone()),
                user_id: Some(new_user.id.clone()),
                created_at: Some(now_secs * 1000),
                created_by: Some("auto-registration".to_string()),
                updated_at: Some(now_secs * 1000),
                last_active: None,
                expires_at: None,
            };

            if let Err(e) = state.vk_store.upsert_key(&vk_cfg).await {
                tracing::error!(error = %e, "Failed to create default virtual key for new user");
            } else {
                let vk = pylos_core::domain::virtual_key::VirtualKey::new(
                    vk_value.clone(),
                    &vk_cfg.name,
                );
                state.vk_registry.register(vk).await;
            }

            let budget_cfg = pylos_core::domain::config::BudgetConfig {
                id: format!("b-{}", random_id),
                max_limit: 1.0,
                reset_duration: pylos_core::domain::config::Duration("1M".to_string()),
                current_usage: 0.0,
                virtual_key_id: Some(vk_id.clone()),
            };
            if let Err(e) = state.budget_store.upsert_budget(&vk_id, &budget_cfg).await {
                tracing::error!(error = %e, "Failed to create default budget for new user VK");
            }

            (role.to_string(), group.to_string())
        }
    };

    // 4. Issue local session JWT
    let exp_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as usize
        + 24 * 3600; // 24 hours validity

    let claims = PylosSessionClaims {
        sub: google_user.email.clone(),
        exp: exp_time,
        role: user_role.clone(),
        name: google_user
            .name
            .unwrap_or_else(|| google_user.email.clone()),
        group: user_group.clone(),
    };

    let token = match encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(state.jwt_secret.as_bytes()),
    ) {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": format!("Failed to sign JWT: {}", e) })),
            )
                .into_response()
        }
    };

    let mut response = Json(json!({
        "token": token,
        "user": {
            "email": claims.sub,
            "name": claims.name,
            "role": claims.role,
            "group": claims.group,
        }
    }))
    .into_response();

    // L'état OAuth est à usage unique : on invalide le cookie.
    response.headers_mut().insert(
        header::SET_COOKIE,
        header::HeaderValue::from_static(
            "pylos_oauth_state=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0",
        ),
    );
    response
}

pub async fn logout() -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(json!({
            "status": "success",
            "message": "Logged out successfully"
        })),
    )
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_oauth_state_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            header::HeaderValue::from_static("a=1; pylos_oauth_state=abc123; b=2"),
        );
        assert_eq!(read_oauth_state_cookie(&headers).as_deref(), Some("abc123"));
        assert_eq!(read_oauth_state_cookie(&HeaderMap::new()), None);
    }

    #[test]
    fn test_constant_time_eq() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"abcd"));
    }

    #[test]
    fn test_admin_email_matching() {
        std::env::set_var("ADMIN_EMAILS", " Boss@Example.com , other@x.io ");
        assert!(is_admin_email("boss@example.com"));
        assert!(is_admin_email("OTHER@X.IO"));
        assert!(!is_admin_email("random@example.com"));
        std::env::remove_var("ADMIN_EMAILS");
    }

    #[test]
    fn test_random_oauth_state_is_unique_and_hex() {
        let a = random_oauth_state();
        let b = random_oauth_state();
        assert_ne!(a, b);
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
