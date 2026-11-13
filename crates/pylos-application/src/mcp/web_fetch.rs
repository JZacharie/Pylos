use pylos_core::domain::mcp_server::{McpContentItem, McpTool, McpToolCallResult};
use pylos_core::error::PylosError;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::IpAddr;
use std::time::Duration;
use tracing::warn;

pub const WEB_FETCH_TOOL_NAME: &str = "web_fetch";
pub const MAX_BODY_BYTES: usize = 512 * 1024; // 512 KB
pub const DEFAULT_TIMEOUT_SECS: u64 = 10;

#[derive(Debug, Deserialize, Serialize)]
pub struct WebFetchArgs {
    pub url: String,
    #[serde(default = "default_true")]
    pub extract_text: bool,
    #[serde(default)]
    pub max_chars: Option<usize>,
}

fn default_true() -> bool {
    true
}

pub fn web_fetch_tool_def() -> McpTool {
    McpTool {
        name: WEB_FETCH_TOOL_NAME.to_string(),
        description: Some(
            "Récupère le contenu d'une URL web (HTTP/HTTPS) et extrait son texte de manière sécurisée (avec protection anti-SSRF).".to_string(),
        ),
        input_schema: json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "L'URL complète à explorer (ex: https://example.com/page)"
                },
                "extract_text": {
                    "type": "boolean",
                    "description": "Si true (défaut), supprime les balises HTML et scripts pour ne garder que le texte principal.",
                    "default": true
                },
                "max_chars": {
                    "type": "integer",
                    "description": "Nombre maximal de caractères retournés (défaut: 50000)"
                }
            },
            "required": ["url"]
        }),
        server_id: Some("builtin".to_string()),
    }
}

/// Vérifie si une adresse IP est privée, loopback ou réservée (protection SSRF).
pub fn is_forbidden_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(ipv4) => {
            ipv4.is_loopback()
                || ipv4.is_private()
                || ipv4.is_link_local()
                || ipv4.is_broadcast()
                || ipv4.is_documentation()
                || ipv4.is_unspecified()
                || ipv4.octets()[0] == 0 // 0.0.0.0/8
                || (ipv4.octets()[0] == 100 && (ipv4.octets()[1] & 0xC0) == 64) // 100.64.0.0/10 Carrier-grade NAT
                || (ipv4.octets()[0] == 198 && (ipv4.octets()[1] == 18 || ipv4.octets()[1] == 19))
            // 198.18.0.0/15 benchmarking
        }
        IpAddr::V6(ipv6) => {
            ipv6.is_loopback()
                || ipv6.is_unspecified()
                // Unique Local Address (fc00::/7)
                || (ipv6.segments()[0] & 0xfe00) == 0xfc00
                // Link-Local (fe80::/10)
                || (ipv6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

/// Valide une URL pour se prémunir des attaques SSRF
pub async fn validate_url_for_ssrf(raw_url: &str) -> Result<reqwest::Url, PylosError> {
    let url = reqwest::Url::parse(raw_url)
        .map_err(|e| PylosError::InvalidRequest(format!("Invalid URL '{raw_url}': {e}")))?;

    let scheme = url.scheme().to_lowercase();
    if scheme != "http" && scheme != "https" {
        return Err(PylosError::InvalidRequest(format!(
            "Forbidden URL scheme '{scheme}'. Only http and https are allowed."
        )));
    }

    let host_str = url
        .host_str()
        .ok_or_else(|| PylosError::InvalidRequest("Missing host in URL".into()))?;

    // Résolution DNS pour bloquer les IP privées
    let port = url.port_or_known_default().unwrap_or(80);
    let socket_addrs = tokio::net::lookup_host(format!("{}:{}", host_str, port))
        .await
        .map_err(|e| {
            PylosError::InvalidRequest(format!("DNS resolution failed for '{host_str}': {e}"))
        })?;

    let mut found_valid = false;
    for addr in socket_addrs {
        if is_forbidden_ip(&addr.ip()) {
            return Err(PylosError::InvalidRequest(format!(
                "SSRF protection blocked access to forbidden IP {} for host '{}'",
                addr.ip(),
                host_str
            )));
        }
        found_valid = true;
    }

    if !found_valid {
        return Err(PylosError::InvalidRequest(format!(
            "No IP addresses resolved for '{host_str}'"
        )));
    }

    Ok(url)
}

/// Extrait le contenu texte lisible d'un document HTML.
pub fn html_to_plain_text(html: &str) -> String {
    let without_scripts = strip_tag_contents(html, "script");
    let without_styles = strip_tag_contents(&without_scripts, "style");
    let without_svg = strip_tag_contents(&without_styles, "svg");

    // Supprimer les balises HTML restantes
    let mut plain = String::with_capacity(without_svg.len());
    let mut in_tag = false;
    for ch in without_svg.chars() {
        if ch == '<' {
            in_tag = true;
            plain.push(' ');
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            plain.push(ch);
        }
    }

    // Normaliser les espaces consécutifs et retours à la ligne
    let mut lines = Vec::new();
    for raw_line in plain.lines() {
        let words: Vec<&str> = raw_line.split_whitespace().collect();
        if !words.is_empty() {
            lines.push(words.join(" "));
        }
    }
    lines.join("\n")
}

fn strip_tag_contents(input: &str, tag_name: &str) -> String {
    let open_tag = format!("<{}", tag_name);
    let close_tag = format!("</{}>", tag_name);

    let mut result = String::with_capacity(input.len());
    let mut cursor = 0;
    let lower = input.to_lowercase();

    while let Some(start) = lower[cursor..].find(&open_tag) {
        let abs_start = cursor + start;
        result.push_str(&input[cursor..abs_start]);
        if let Some(end) = lower[abs_start..].find(&close_tag) {
            cursor = abs_start + end + close_tag.len();
        } else {
            // Pas de balise fermante, on ignore le reste
            cursor = input.len();
            break;
        }
    }

    if cursor < input.len() {
        result.push_str(&input[cursor..]);
    }

    result
}

/// Exécute l'outil web_fetch
pub async fn execute_web_fetch(args: WebFetchArgs) -> Result<McpToolCallResult, PylosError> {
    let url = validate_url_for_ssrf(&args.url).await?;

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(DEFAULT_TIMEOUT_SECS))
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
        .map_err(|e| PylosError::Internal(format!("Failed to create HTTP client: {e}")))?;

    let response = client
        .get(url.clone())
        .header(
            reqwest::header::USER_AGENT,
            "Pylos-AIGateway/0.1 (+https://github.com/JZacharie/Pylos)",
        )
        .header(
            reqwest::header::ACCEPT,
            "text/html,application/xhtml+xml,application/xml;q=0.9,text/plain;q=0.8,*/*;q=0.7",
        )
        .send()
        .await
        .map_err(|e| {
            warn!(url = %url, error = %e, "web_fetch request failed");
            PylosError::Internal(format!("Failed to fetch URL: {e}"))
        })?;

    let status = response.status();
    if !status.is_success() {
        return Ok(McpToolCallResult {
            content: vec![McpContentItem::Text {
                text: format!("HTTP error: status {}", status.as_u16()),
            }],
            is_error: true,
        });
    }

    // Limiter la lecture pour éviter l'épuisement mémoire
    let bytes = response
        .bytes()
        .await
        .map_err(|e| PylosError::Internal(format!("Failed to read response body: {e}")))?;

    let slice = if bytes.len() > MAX_BODY_BYTES {
        &bytes[..MAX_BODY_BYTES]
    } else {
        &bytes[..]
    };

    let raw_text = String::from_utf8_lossy(slice).to_string();

    let text = if args.extract_text {
        html_to_plain_text(&raw_text)
    } else {
        raw_text
    };

    let max_chars = args.max_chars.unwrap_or(50_000);
    let final_text = if text.len() > max_chars {
        format!(
            "{}\n\n[... Truncated at {} characters]",
            &text[..max_chars],
            max_chars
        )
    } else {
        text
    };

    Ok(McpToolCallResult {
        content: vec![McpContentItem::Text { text: final_text }],
        is_error: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{Ipv4Addr, Ipv6Addr};

    #[test]
    fn test_is_forbidden_ip() {
        assert!(is_forbidden_ip(&IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
        assert!(is_forbidden_ip(&IpAddr::V4(Ipv4Addr::new(10, 0, 0, 5))));
        assert!(is_forbidden_ip(&IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))));
        assert!(is_forbidden_ip(&IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))));
        assert!(is_forbidden_ip(&IpAddr::V4(Ipv4Addr::new(169, 254, 1, 1))));
        assert!(is_forbidden_ip(&IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0))));
        assert!(is_forbidden_ip(&IpAddr::V6(Ipv6Addr::LOCALHOST)));

        // Public IP
        assert!(!is_forbidden_ip(&IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(!is_forbidden_ip(&IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
    }

    #[test]
    fn test_html_to_plain_text() {
        let html = r#"
            <html>
                <head>
                    <title>Test Page</title>
                    <style>body { color: red; }</style>
                    <script>alert("hello");</script>
                </head>
                <body>
                    <h1>Welcome</h1>
                    <p>This is a <b>test</b> paragraph.</p>
                </body>
            </html>
        "#;
        let text = html_to_plain_text(html);
        assert!(text.contains("Welcome"));
        assert!(text.contains("This is a test paragraph."));
        assert!(!text.contains("alert"));
        assert!(!text.contains("color: red"));
    }
}
