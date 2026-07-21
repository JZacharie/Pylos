use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::LazyLock;
use tracing::{debug, info, warn};

use pylos_core::domain::openai::MessageRole;
use pylos_core::domain::request::{PylosRequest, PylosResponse, RequestContext};
use pylos_core::domain::traits::LlmPlugin;
use pylos_core::error::PylosError;
use serde::Serialize;

use crate::config_store::ConfigStore;

// ─────────────────────────────────────────────────────────────────────────────
// Content Moderation Scoring
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct ContentModerationScores {
    pub racism: f64,
    pub toxicity: f64,
    pub harassment: f64,
    pub violence: f64,
    pub hate_speech: f64,
    pub self_harm: f64,
    pub sexual: f64,
    pub illegal: f64,
    pub personal_data_leak: f64,
    pub social_risk: f64,
}

#[allow(dead_code)]
struct ModCategory {
    name: &'static str,
    score_key: &'static str,
    patterns: &'static [(&'static str, f64)],
}

static MOD_CATEGORIES: &[ModCategory] = &[
    ModCategory {
        name: "racism",
        score_key: "racism",
        patterns: &[
            // Racial slurs and hate speech — weighted by severity
            (r"\b(m|n|w)igger\b", 1.0),
            (r"\b(k|w)ike\b", 1.0),
            (r"\bspic\b", 1.0),
            (r"\bchink\b", 1.0),
            (r"\bgook\b", 1.0),
            (r"\bcoon\b", 1.0),
            (r"\bwhite power\b", 0.9),
            (r"\bwhite supremacy\b", 0.9),
            (r"\brace war\b", 0.8),
            (r"\bracial purity\b", 0.8),
            (r"\bmaster race\b", 0.9),
            (r"\bblack? people (should|must|need to)\b", 0.6),
            (r"\bwhite? people (should|must|need to)\b", 0.6),
            (r"\bhate\s+\w+(race|color|religion)\b", 0.5),
            (r"\binferior race\b", 0.8),
            (r"\bsuperior race\b", 0.7),
            (r"\bethnic cleansing\b", 0.9),
            (r"\bracial slu[r]\b", 0.8),
            (r"\banti.semite\b", 0.8),
            (r"\bislamophobi\w+\b", 0.6),
            (r"\bwhite genocide\b", 0.8),
        ],
    },
    ModCategory {
        name: "toxicity",
        score_key: "toxicity",
        patterns: &[
            (r"\bfuck you\b", 0.6),
            (r"\bsuck my\b", 0.5),
            (r"\b(go )?(kill|die)\s+(yourself|ur self)\b", 0.8),
            (r"\bshut the fuck up\b", 0.5),
            (r"\bstupid (ass|bitch|fuck)\b", 0.4),
            (r"\bpiece of shit\b", 0.5),
            (r"\bdumbass\b", 0.3),
            (r"\bbastard\b", 0.3),
            (r"\bson of a bitch\b", 0.4),
            (r"\bdickhead\b", 0.4),
            (r"\basshole\b", 0.4),
            (r"\bscum(bag)?\b", 0.5),
        ],
    },
    ModCategory {
        name: "harassment",
        score_key: "harassment",
        patterns: &[
            (r"\bi (will )?(find|hunt|get) you\b", 0.6),
            (r"\byou (should|better) (watch out|be careful)\b", 0.4),
            (r"\bi know where you (live|work|sleep)\b", 0.7),
            (r"\byou('re| are) (worthless|useless|pathetic)\b", 0.5),
            (r"\bstop (bothering|annoying|harassing) me\b", 0.3),
            (r"\bleave me alone\b", 0.3),
            (r"\byou('re| are) a (loser|failure|disgrace)\b", 0.4),
            (r"\bdox(x?ing)?\b", 0.7),
            (r"\bstalking\b", 0.5),
            (r"\bswat(t?ing)?\b", 0.8),
        ],
    },
    ModCategory {
        name: "violence",
        score_key: "violence",
        patterns: &[
            (r"\b(kill|murder|assassinate)\b", 0.6),
            (r"\bbomb\b", 0.6),
            (r"\bshoot(ing|er| up)?\b", 0.5),
            (r"\bstab(b?ing)?\b", 0.6),
            (r"\btorture\b", 0.7),
            (r"\bbehead\b", 0.8),
            (r"\bmassacre\b", 0.8),
            (r"\bterrorist\b", 0.5),
            (r"\bexplosive\b", 0.5),
            (r"\bhow to (make|build) (a )?(bomb|weapon|explosive)\b", 0.8),
            (r"\bchemical weapon\b", 0.7),
            (r"\bbioweapon\b", 0.7),
            (r"\bgenocide\b", 0.8),
        ],
    },
    ModCategory {
        name: "hate_speech",
        score_key: "hate_speech",
        patterns: &[
            (r"\bhate (speech|crime)\b", 0.3),
            (
                r"\b(you|they) (should|must) (be )?(killed|eliminated|exterminated)\b",
                0.9,
            ),
            (r"\bdeath to\b", 0.8),
            (r"\bexterminat(e|ion)\b", 0.7),
            (r"\b(religious|ethnic) (cleansing|war)\b", 0.8),
            (r"\bnazi\b", 0.5),
            (r"\bfascist\b", 0.4),
            (r"\bkkk\b", 0.9),
            (r"\bneo.nazi\b", 0.8),
            (r"\bholocaust (denial|denier)\b", 0.9),
            (r"\bsupremacist\b", 0.6),
            (r"\bdeport (them|all)\b", 0.6),
        ],
    },
    ModCategory {
        name: "self_harm",
        score_key: "self_harm",
        patterns: &[
            (r"\b(kill|hurt|harm) (myself|me)\b", 0.8),
            (r"\bsuicide\b", 0.6),
            (r"\bend (my|the) (life|pain)\b", 0.5),
            (r"\bcut (my|the) (wrist|veins|throat)\b", 0.8),
            (r"\bhang (myself|me)\b", 0.8),
            (r"\boverdose\b", 0.6),
            (r"\bself.harm\b", 0.6),
            (r"\bi (want|am going) to (die|kill myself)\b", 0.9),
            (r"\bi (don't|do not) want to live\b", 0.7),
            (r"\bbetter off dead\b", 0.7),
            (r"\bno (reason|point) to live\b", 0.6),
            (r"\bjump (off|from) (a )? (bridge|building|cliff)\b", 0.7),
        ],
    },
    ModCategory {
        name: "sexual",
        score_key: "sexual",
        patterns: &[
            (r"\bexplicit (sexual|content|imagery)\b", 0.5),
            (r"\b(child|minor) (porn|abuse|exploitation)\b", 1.0),
            (r"\b(sexual|sex) (assault|abuse|harass)\b", 0.7),
            (r"\brape\b", 0.9),
            (r"\brevenge porn\b", 0.8),
            (r"\bsexualize\b", 0.4),
            (r"\bnsfw\b", 0.3),
            (r"\bdeepnude\b", 0.7),
            (r"\bonlyfans\b", 0.3),
        ],
    },
    ModCategory {
        name: "illegal",
        score_key: "illegal",
        patterns: &[
            (r"\bhow to (hack|steal|rob|scam)\b", 0.7),
            (r"\bcredit card (fraud|steal|number)\b", 0.8),
            (r"\bmoney laundering\b", 0.7),
            (r"\bdrug (trafficking|smuggl|deal)\b", 0.7),
            (r"\bhuman trafficking\b", 0.8),
            (r"\bidentity theft\b", 0.6),
            (r"\bphishing\b", 0.5),
            (r"\bmalware\b", 0.4),
            (r"\bran?somware\b", 0.6),
            (r"\b(illegal|black) (market|trade)\b", 0.6),
            (r"\bcounterfeit\b", 0.5),
            (r"\bterroris(m|t)\b", 0.5),
            (
                r"\bhow to (make|manufacture|synthesize)\s+(drug|meth|cocaine|heroin|lsd)\b",
                0.9,
            ),
        ],
    },
    ModCategory {
        name: "personal_data_leak",
        score_key: "personal_data_leak",
        patterns: &[
            (r"\b(ssn|social security|national id)\b", 0.5),
            (r"\b(passport|driver('s)? license) number\b", 0.5),
            (r"\bbank (account|routing) number\b", 0.6),
            (r"\bcredit (card|report)\b", 0.4),
            (r"\b(patient|medical) (record|history|info)\b", 0.5),
            (
                r"\b(cover|reveal|disclose) (personal|private|confidential)\b",
                0.4,
            ),
            (r"\bleak (personal|private|sensitive|confidential)\b", 0.5),
            (r"\b(leaked|breach) (data|information|password)\b", 0.5),
            (r"\bdox(x?ing)?\b", 0.6),
        ],
    },
    ModCategory {
        name: "social_risk",
        score_key: "social_risk",
        patterns: &[
            (r"\bhow to (scam|trick|deceive|manipulate)\b", 0.6),
            (r"\bsocial engineering\b", 0.6),
            (r"\bphishing (email|link|page|scam)\b", 0.5),
            (r"\bcatphish\b", 0.6),
            (r"\bromance scam\b", 0.7),
            (r"\bimpersonat(e|ion)\b", 0.4),
            (r"\bidentity fraud\b", 0.6),
            (r"\bdeepfake\b", 0.5),
            (r"\bfake (profile|account|identity)\b", 0.4),
            (r"\bhow to (solicit|extort|blackmail)\b", 0.8),
            (r"\b(ransom|sextortion)\b", 0.7),
        ],
    },
];

fn compute_moderation_scores(text: &str) -> ContentModerationScores {
    let lower = text.to_lowercase();
    let mut scores = ContentModerationScores {
        racism: 0.0,
        toxicity: 0.0,
        harassment: 0.0,
        violence: 0.0,
        hate_speech: 0.0,
        self_harm: 0.0,
        sexual: 0.0,
        illegal: 0.0,
        personal_data_leak: 0.0,
        social_risk: 0.0,
    };

    for cat in MOD_CATEGORIES {
        let mut max_weight = 0.0_f64;
        for (pattern, weight) in cat.patterns {
            if let Ok(re) = regex::Regex::new(pattern) {
                if re.is_match(&lower) {
                    max_weight = max_weight.max(*weight);
                }
            }
        }
        let field = match cat.score_key {
            "racism" => &mut scores.racism,
            "toxicity" => &mut scores.toxicity,
            "harassment" => &mut scores.harassment,
            "violence" => &mut scores.violence,
            "hate_speech" => &mut scores.hate_speech,
            "self_harm" => &mut scores.self_harm,
            "sexual" => &mut scores.sexual,
            "illegal" => &mut scores.illegal,
            "personal_data_leak" => &mut scores.personal_data_leak,
            "social_risk" => &mut scores.social_risk,
            _ => unreachable!(),
        };
        *field = max_weight;
    }

    scores
}

static RE_EMAIL: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap());
static RE_PHONE: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"\+?\d{1,4}[-.\s]?\(?\d{1,4}?\)?(?:[-.\s]?\d{1,4}){2,5}").unwrap()
});
static RE_IBAN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"\b[a-zA-Z]{2}\d{2}(?:[ -]*[a-zA-Z0-9]){12,30}\b").unwrap()
});
static RE_SSN: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"\b[12][ -]*\d{2}[ -]*\d{2}[ -]*\d{2}[ -]*\d{3}[ -]*\d{3}[ -]*\d{2}\b")
        .unwrap()
});
static RE_IP: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"\b\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}\b").unwrap());
static RE_CC: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"\b(?:\d[ -]*?){13,16}\b").unwrap());
static RE_OPENAI_KEY: LazyLock<regex::Regex> =
    LazyLock::new(|| regex::Regex::new(r"\bsk-[a-zA-Z0-9]{20,}\b").unwrap());
static RE_JWT: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"\b[A-Za-z0-9-_=]+\.[A-Za-z0-9-_=]+\.[A-Za-z0-9-_=]*\b").unwrap()
});
static RE_PRIVATE_KEY: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(
        r"(?s)-----BEGIN [A-Z ]+ PRIVATE KEY-----.*?-----END [A-Z ]+ PRIVATE KEY-----",
    )
    .unwrap()
});

pub struct GuardrailsPlugin {
    config_store: Arc<ConfigStore>,
}

impl GuardrailsPlugin {
    pub fn new(config_store: Arc<ConfigStore>) -> Self {
        Self { config_store }
    }

    fn mask_text(
        &self,
        text: &str,
        pii_map: &mut HashMap<String, String>,
        mask_pii: bool,
        mask_secrets: bool,
    ) -> String {
        let mut masked = text.to_string();

        // 1. PII Masking
        if mask_pii {
            // Emails
            let start_email_idx = pii_map.len() + 1;
            let mut next_masked = masked.clone();
            for (idx, mat) in (start_email_idx..).zip(RE_EMAIL.find_iter(&masked)) {
                let original = mat.as_str().to_string();
                let placeholder = format!("[EMAIL_{}]", idx);
                pii_map.insert(placeholder.clone(), original);
                next_masked = next_masked.replace(mat.as_str(), &placeholder);
            }
            masked = next_masked;

            // IBANs
            let start_iban_idx = pii_map.len() + 1;
            let mut next_masked = masked.clone();
            for (idx, mat) in (start_iban_idx..).zip(RE_IBAN.find_iter(&masked)) {
                let original = mat.as_str().to_string();
                let placeholder = format!("[IBAN_{}]", idx);
                pii_map.insert(placeholder.clone(), original);
                next_masked = next_masked.replace(mat.as_str(), &placeholder);
            }
            masked = next_masked;

            // Social Security Numbers (SSN)
            let start_ssn_idx = pii_map.len() + 1;
            let mut next_masked = masked.clone();
            for (idx, mat) in (start_ssn_idx..).zip(RE_SSN.find_iter(&masked)) {
                let original = mat.as_str().to_string();
                let placeholder = format!("[SSN_{}]", idx);
                pii_map.insert(placeholder.clone(), original);
                next_masked = next_masked.replace(mat.as_str(), &placeholder);
            }
            masked = next_masked;

            // IP Addresses
            let start_ip_idx = pii_map.len() + 1;
            let mut next_masked = masked.clone();
            for (idx, mat) in (start_ip_idx..).zip(RE_IP.find_iter(&masked)) {
                let original = mat.as_str().to_string();
                let placeholder = format!("[IP_{}]", idx);
                pii_map.insert(placeholder.clone(), original);
                next_masked = next_masked.replace(mat.as_str(), &placeholder);
            }
            masked = next_masked;

            // Phone Numbers
            let start_phone_idx = pii_map.len() + 1;
            let mut next_masked = masked.clone();
            for (phone_idx, mat) in (start_phone_idx..).zip(RE_PHONE.find_iter(&masked)) {
                let original = mat.as_str().to_string();
                if original.chars().filter(|c| c.is_ascii_digit()).count() >= 7 {
                    let placeholder = format!("[PHONE_{}]", phone_idx);
                    pii_map.insert(placeholder.clone(), original);
                    next_masked = next_masked.replace(mat.as_str(), &placeholder);
                }
            }
            masked = next_masked;

            // Credit cards
            let start_cc_idx = pii_map.len() + 1;
            let mut next_masked = masked.clone();
            for (idx, mat) in (start_cc_idx..).zip(RE_CC.find_iter(&masked)) {
                let original = mat.as_str().to_string();
                let placeholder = format!("[CREDIT_CARD_{}]", idx);
                pii_map.insert(placeholder.clone(), original);
                next_masked = next_masked.replace(mat.as_str(), &placeholder);
            }
            masked = next_masked;
        }

        // 2. Secrets Masking
        if mask_secrets {
            // OpenAI keys
            let mut next_masked = masked.clone();
            for (secret_idx, mat) in (pii_map.len() + 1..).zip(RE_OPENAI_KEY.find_iter(&masked)) {
                let original = mat.as_str().to_string();
                let placeholder = format!("[API_KEY_{}]", secret_idx);
                pii_map.insert(placeholder.clone(), original);
                next_masked = next_masked.replace(mat.as_str(), &placeholder);
            }
            masked = next_masked;

            // JWT Tokens
            let mut jwt_idx = pii_map.len() + 1;
            let mut next_masked = masked.clone();
            for mat in RE_JWT.find_iter(&masked) {
                let original = mat.as_str().to_string();
                if original.len() > 20 {
                    let placeholder = format!("[JWT_{}]", jwt_idx);
                    pii_map.insert(placeholder.clone(), original);
                    next_masked = next_masked.replace(mat.as_str(), &placeholder);
                    jwt_idx += 1;
                }
            }
            masked = next_masked;

            // Private Keys
            let mut next_masked = masked.clone();
            for (pkey_idx, mat) in (pii_map.len() + 1..).zip(RE_PRIVATE_KEY.find_iter(&masked)) {
                let original = mat.as_str().to_string();
                let placeholder = format!("[PRIVATE_KEY_{}]", pkey_idx);
                pii_map.insert(placeholder.clone(), original);
                next_masked = next_masked.replace(mat.as_str(), &placeholder);
            }
            masked = next_masked;
        }

        masked
    }

    fn restore_text(&self, text: &str, pii_map: &HashMap<String, String>) -> String {
        let mut restored = text.to_string();
        for (placeholder, original) in pii_map {
            restored = restored.replace(placeholder, original);
        }
        restored
    }
}

#[async_trait]
impl LlmPlugin for GuardrailsPlugin {
    fn name(&self) -> &str {
        "guardrails"
    }

    async fn pre_hook(
        &self,
        request: &mut PylosRequest,
        ctx: &mut RequestContext,
    ) -> Result<Option<PylosResponse>, PylosError> {
        if ctx
            .headers
            .get("x-bypass-guardrails")
            .map(|s| s == "true")
            .unwrap_or(false)
        {
            return Ok(None);
        }

        let chat_req = match request {
            PylosRequest::ChatCompletion(ref mut req) => req,
            _ => return Ok(None),
        };

        // Lire dynamiquement la configuration du plugin guardrails
        let cfg = self.config_store.get().await;
        let plugin_cfg = cfg.plugins.iter().find(|p| p.name == "guardrails");
        let (
            enabled,
            mask_pii,
            mask_secrets,
            prevent_prompt_injection,
            blocked_keywords,
            enable_moderation,
            moderation_threshold,
        ) = match plugin_cfg {
            Some(p) => {
                let mask_pii = p
                    .config
                    .get("mask_pii")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                let mask_secrets = p
                    .config
                    .get("mask_secrets")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let prevent_prompt_injection = p
                    .config
                    .get("prevent_prompt_injection")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                let blocked_keywords = p
                    .config
                    .get("blocked_keywords")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|val| val.as_str().map(|s| s.to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                let enable_moderation = p
                    .config
                    .get("enable_moderation")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                let moderation_threshold = p
                    .config
                    .get("moderation_threshold")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(0.7);
                (
                    p.enabled,
                    mask_pii,
                    mask_secrets,
                    prevent_prompt_injection,
                    blocked_keywords,
                    enable_moderation,
                    moderation_threshold,
                )
            }
            None => (false, false, false, false, vec![], true, 0.7),
        };

        if !enabled {
            return Ok(None);
        }

        // 0. Content Moderation Scoring
        if enable_moderation {
            let mut combined_scores = ContentModerationScores {
                racism: 0.0,
                toxicity: 0.0,
                harassment: 0.0,
                violence: 0.0,
                hate_speech: 0.0,
                self_harm: 0.0,
                sexual: 0.0,
                illegal: 0.0,
                personal_data_leak: 0.0,
                social_risk: 0.0,
            };
            for message in &chat_req.messages {
                if let Some(ref content) = message.content {
                    let msg_scores = compute_moderation_scores(content);
                    combined_scores.racism = combined_scores.racism.max(msg_scores.racism);
                    combined_scores.toxicity = combined_scores.toxicity.max(msg_scores.toxicity);
                    combined_scores.harassment =
                        combined_scores.harassment.max(msg_scores.harassment);
                    combined_scores.violence = combined_scores.violence.max(msg_scores.violence);
                    combined_scores.hate_speech =
                        combined_scores.hate_speech.max(msg_scores.hate_speech);
                    combined_scores.self_harm = combined_scores.self_harm.max(msg_scores.self_harm);
                    combined_scores.sexual = combined_scores.sexual.max(msg_scores.sexual);
                    combined_scores.illegal = combined_scores.illegal.max(msg_scores.illegal);
                    combined_scores.personal_data_leak = combined_scores
                        .personal_data_leak
                        .max(msg_scores.personal_data_leak);
                    combined_scores.social_risk =
                        combined_scores.social_risk.max(msg_scores.social_risk);
                }
            }
            if let Ok(json) = serde_json::to_string(&combined_scores) {
                ctx.headers.insert("x-moderation-scores".to_string(), json);
                let max_score = combined_scores
                    .racism
                    .max(combined_scores.toxicity)
                    .max(combined_scores.harassment)
                    .max(combined_scores.violence)
                    .max(combined_scores.hate_speech)
                    .max(combined_scores.self_harm)
                    .max(combined_scores.sexual)
                    .max(combined_scores.illegal)
                    .max(combined_scores.personal_data_leak)
                    .max(combined_scores.social_risk);
                if max_score >= moderation_threshold {
                    info!(
                        max_score = max_score,
                        threshold = moderation_threshold,
                        "[Moderation] Content moderation triggered"
                    );
                    ctx.headers
                        .insert("guardrail_triggered".to_string(), "true".to_string());
                    ctx.headers.insert(
                        "guardrail_type".to_string(),
                        "content_moderation".to_string(),
                    );
                    ctx.headers.insert(
                        "guardrail_detail".to_string(),
                        format!(
                            "Moderation score {:.2} exceeded threshold {:.2}",
                            max_score, moderation_threshold
                        ),
                    );
                }
            }
        }

        // 1. Keyword Blocklist Check
        for message in &chat_req.messages {
            if let Some(ref content) = message.content {
                let lower_content = content.to_lowercase();
                for keyword in &blocked_keywords {
                    if lower_content.contains(&keyword.to_lowercase()) {
                        warn!(keyword = %keyword, "Guardrails: Keyword match detected (logging but not blocking)");
                        ctx.headers
                            .insert("guardrail_triggered".to_string(), "true".to_string());
                        ctx.headers
                            .insert("guardrail_type".to_string(), "keyword_block".to_string());
                        ctx.headers.insert(
                            "guardrail_detail".to_string(),
                            format!("Blocked keyword: {}", keyword),
                        );
                    }
                }
            }
        }

        // 1b. Prompt Injection Prevention
        if prevent_prompt_injection {
            let injection_indicators = [
                "ignore previous instructions",
                "ignore all instructions",
                "ignore instructions above",
                "reveal your system prompt",
                "reveal your instructions",
                "output the system prompt",
            ];
            for message in &chat_req.messages {
                if let Some(ref content) = message.content {
                    let lower_content = content.to_lowercase();
                    for indicator in &injection_indicators {
                        if lower_content.contains(indicator) {
                            warn!(indicator = %indicator, "Guardrails: Prompt injection detected (logging but not blocking)");
                            ctx.headers
                                .insert("guardrail_triggered".to_string(), "true".to_string());
                            ctx.headers.insert(
                                "guardrail_type".to_string(),
                                "prompt_injection".to_string(),
                            );
                            ctx.headers.insert(
                                "guardrail_detail".to_string(),
                                format!("Prompt injection indicator: {}", indicator),
                            );
                        }
                    }
                }
            }
        }

        // 2. Detection & Masking (PII & Secrets)
        let mut pii_map = HashMap::new();
        for message in &mut chat_req.messages {
            if message.role == MessageRole::User {
                if let Some(ref mut content) = message.content {
                    let original = content.clone();
                    let masked = self.mask_text(&original, &mut pii_map, mask_pii, mask_secrets);

                    info!(
                        "[Guardrails] Original message:\n{}\n[Guardrails] Obfuscated message:\n{}",
                        original, masked
                    );

                    if (mask_pii || mask_secrets) && masked != original {
                        *content = masked.clone();
                        ctx.headers.insert("x-obfuscated-input".to_string(), masked);
                    }
                }
            }
        }

        if !pii_map.is_empty() && (mask_pii || mask_secrets) {
            warn!("Guardrails: PII or Secrets detected and masked");
            ctx.headers
                .insert("guardrail_triggered".to_string(), "true".to_string());
            ctx.headers
                .insert("guardrail_type".to_string(), "pii".to_string());
            let detected_keys: Vec<String> = pii_map.keys().cloned().collect();
            ctx.headers.insert(
                "guardrail_detail".to_string(),
                format!("Detected PII/Secrets keys: {:?}", detected_keys),
            );

            if let Ok(serialized) = serde_json::to_string(&pii_map) {
                ctx.headers.insert("x-pii-mapping".to_string(), serialized);
            }
        }

        Ok(None)
    }

    async fn post_hook(
        &self,
        _request: &PylosRequest,
        response: &mut PylosResponse,
        ctx: &mut RequestContext,
    ) -> Result<(), PylosError> {
        let chat_resp = match response {
            PylosResponse::ChatCompletion(ref mut resp) => resp,
            _ => return Ok(()),
        };

        // Retrieve unmasking map
        if let Some(serialized) = ctx.headers.get("x-pii-mapping") {
            if let Ok(pii_map) = serde_json::from_str::<HashMap<String, String>>(serialized) {
                if !pii_map.is_empty() {
                    debug!("Guardrails: Unmasking response choices");
                    for choice in &mut chat_resp.choices {
                        if let Some(ref mut content) = choice.message.content {
                            *content = self.restore_text(content, &pii_map);
                        }
                    }
                }
            }
        }

        Ok(())
    }
}
