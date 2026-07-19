use pylos_core::domain::openai::{ChatCompletionMessage, ChatCompletionRequest, MessageRole};
use std::str::FromStr;

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CavemanMode {
    Off,
    Lite,
    Full,
    Ultra,
    Wenyan,
    Spaghetti,
}

impl FromStr for CavemanMode {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "lite" => Ok(CavemanMode::Lite),
            "full" => Ok(CavemanMode::Full),
            "ultra" => Ok(CavemanMode::Ultra),
            "wenyan" => Ok(CavemanMode::Wenyan),
            "spaghetti" | "spaghettimix" | "mix" => Ok(CavemanMode::Spaghetti),
            _ => Ok(CavemanMode::Off),
        }
    }
}

/// System prompts corresponding to each Caveman level
const PROMPT_LITE: &str = "Talk like caveman. Terse. Drop hedging, filler words (just, really, basically, simply), and polite pleasantries. Maintain proper grammar and complete sentences, but be extremely direct and concise. Code remains unchanged.";
const PROMPT_FULL: &str = "Talk like caveman. Why use many token when few token do trick. Terse. Drop: articles (a, an, the), filler (just, really, basically, simply), pleasantries, hedging. Short sentence fragments OK. Short synonyms. Code/commits/PRs: normal format. Code blocks unchanged.";
const PROMPT_ULTRA: &str = "Talk like caveman. Max compression. Drop all articles, filler, pleasantries, verbs where possible. Shorthand, arrows, symbols allowed (e.g. '->' for leads to/resulting in, '+' for addition/and). Bullet lists or very short words. Code blocks unchanged.";
const PROMPT_WENYAN: &str = "Talk like caveman. Respond using Classical Chinese style structure (Wenyan patterns) or extremely terse telegraphic words. Maximum brevity. Code blocks unchanged.";
const PROMPT_SPAGHETTI: &str = "The user message below has been encoded using a multi-script Unicode homoglyph substitution for privacy. Decode each character using the inverse of this mapping before responding: each visually similar character from Cyrillic, Greek, and other scripts should be mapped back to its Latin equivalent. Respond normally after decoding.";

/// Homoglyph substitution map: Latin char -> visually similar Unicode chars from other scripts
fn homoglyph_map() -> Vec<(char, &'static [char])> {
    vec![
        ('a', &['а', 'α', 'à', 'á', 'â', 'ã', 'ä', 'å']),  // Cyrillic, Greek
        ('b', &['Ь', 'ъ', 'β']),  // Cyrillic soft sign looks like b, Greek beta
        ('c', &['с', 'ϲ', '¢']),  // Cyrillic es, Greek lunate sigma
        ('d', &['ԁ', 'ɗ', 'ԃ']),
        ('e', &['е', 'ё', 'є', 'ε', 'é', 'è', 'ê', 'ë']),  // Cyrillic, Greek
        ('f', &['ғ', 'ƒ']),
        ('g', &['ɡ', 'ĝ', 'ğ']),
        ('h', &['һ', 'հ', 'н']),  // Cyrillic shha, Armenian ho
        ('i', &['і', 'ɪ', 'ι', 'í', 'ì', 'î', 'ï']),  // Cyrillic, Greek
        ('j', &['ј', 'ϳ']),  // Cyrillic je, Greek yot
        ('k', &['к', 'κ', 'ķ']),  // Cyrillic, Greek
        ('l', &['ӏ', 'ӏ', 'ӏ']),
        ('m', &['м', 'ṃ', 'ϻ']),
        ('n', &['п', 'η', 'ή']),  // Cyrillic pe looks like n, Greek eta
        ('o', &['о', 'ο', 'σ', 'ó', 'ò', 'ô', 'õ', 'ö', 'ø']),
        ('p', &['р', 'ρ', 'þ']),  // Cyrillic er, Greek rho
        ('q', &['ԛ', 'զ']),
        ('r', &['г', 'я']),  // Cyrillic ge, ya (lowercase)
        ('s', &['ѕ', 'ʂ', 'ş']),
        ('t', &['т', 'ţ', '†']),
        ('u', &['у', 'υ', 'ú', 'ù', 'û', 'ü']),  // Cyrillic, Greek
        ('v', &['ѵ', 'ν', '∨']),  // Cyrillic, Greek nu
        ('w', &['ш', 'ω', 'ώ']),  // Cyrillic sha, Greek omega
        ('x', &['х', 'χ', '×']),  // Cyrillic, Greek
        ('y', &['у', 'γ', 'ý', 'ÿ']),  // Cyrillic u, Greek gamma
        ('z', &['z', 'ζ', 'ż']),
        ('A', &['Α', 'Α', 'À', 'Á', 'Â', 'Ã', 'Ä', 'Å']),
        ('B', &['Β', 'В']),
        ('C', &['Ϲ', 'С']),
        ('D', &['Ɗ', 'Đ']),
        ('E', &['Ε', 'Е', 'Ё', 'Є', 'É', 'È', 'Ê', 'Ë']),
        ('F', &['Ϝ', 'Ғ']),
        ('G', &['Ԍ', 'Ǵ']),
        ('H', &['Η', 'Н', 'Ң']),
        ('I', &['І', 'Ӏ', 'Í', 'Ì', 'Î', 'Ï']),
        ('J', &['Ј', 'Ղ']),
        ('K', &['Κ', 'К']),
        ('L', &['Ⅼ', 'Ĺ']),
        ('M', &['Μ', 'М']),
        ('N', &['Ν', 'П']),
        ('O', &['Ο', 'О', 'Ó', 'Ò', 'Ô', 'Õ', 'Ö', 'Ø']),
        ('P', &['Ρ', 'Р']),
        ('Q', &['Ԛ', 'Ⴍ']),
        ('R', &['Я']),
        ('S', &['Ѕ', 'Տ']),
        ('T', &['Τ', 'Т']),
        ('U', &['Υ', 'У', 'Ú', 'Ù', 'Û', 'Ü']),
        ('V', &['Ѵ', 'Ⅴ']),
        ('W', &['Ւ', 'Ԝ']),
        ('X', &['Χ', 'Х']),
        ('Y', &['Υ', 'Ύ', 'Ý', 'Ÿ']),
        ('Z', &['Ζ', 'Ζ']),
    ]
}

/// Applies spaghetti mix encoding: replaces Latin characters with multi-script homoglyphs.
/// Characters not in the map are left unchanged.
pub fn spaghetti_encode(text: &str) -> String {
    let map: HashMap<char, &[char]> = homoglyph_map().into_iter().collect();
    let mut rng = fastrand::Rng::new();
    text.chars()
        .map(|c| {
            if let Some(alternates) = map.get(&c) {
                if !alternates.is_empty() {
                    alternates[rng.usize(0..alternates.len())]
                } else {
                    c
                }
            } else {
                c
            }
        })
        .collect()
}

/// Checks if the request contains critical keywords requiring maximum clarity.
/// If true, Caveman prompt modifications and shrinking will be bypassed.
pub fn is_critical_request(request: &ChatCompletionRequest) -> bool {
    let critical_keywords = [
        "delete",
        "drop",
        "destroy",
        "truncate",
        "erase",
        "format",
        "wipe",
        "remove",
        "uninstall",
        "purge",
        "bypass",
        "override",
        "auth",
        "critical",
        "exploit",
        "vulnerability",
        "cve",
        "security",
        "production",
        "prod",
    ];

    for msg in &request.messages {
        if let Some(content) = &msg.content {
            let lower_content = content.to_lowercase();
            for kw in &critical_keywords {
                if lower_content.contains(kw) {
                    return true;
                }
            }
        }
    }
    false
}

/// Applies Caveman prompt engineering and input compression.
/// Returns (saved_bytes, caveman_input_json) — the caveman input is the serialized
/// messages after compression, for logging purposes.
pub fn apply_caveman(
    request: &mut ChatCompletionRequest,
    mode: CavemanMode,
    shrink_input: bool,
) -> usize {
    if mode == CavemanMode::Off && !shrink_input {
        return 0;
    }

    // Auto-Clarity Guard
    if is_critical_request(request) {
        return 0;
    }

    let mut saved_bytes = 0;

    // 1. Input Prompt Shrinking (only for non-Spaghetti modes)
    if shrink_input && mode != CavemanMode::Spaghetti {
        for msg in &mut request.messages {
            if let Some(content) = &mut msg.content {
                let original_len = content.len();
                let compressed = shrink_text(content);
                let compressed_len = compressed.len();
                if compressed_len < original_len {
                    saved_bytes += original_len - compressed_len;
                    *content = compressed;
                }
            }
        }
    }

    // 2. Input Encodings (Spaghetti mix)
    if mode == CavemanMode::Spaghetti {
        for msg in &mut request.messages {
            if let Some(ref mut content) = msg.content {
                let encoded = spaghetti_encode(content);
                saved_bytes += content.len().saturating_sub(encoded.len());
                *content = encoded;
            }
        }
    }

    // 3. Output Prompt Engineering
    let prompt_rules = match mode {
        CavemanMode::Lite => Some(PROMPT_LITE),
        CavemanMode::Full => Some(PROMPT_FULL),
        CavemanMode::Ultra => Some(PROMPT_ULTRA),
        CavemanMode::Wenyan => Some(PROMPT_WENYAN),
        CavemanMode::Spaghetti => Some(PROMPT_SPAGHETTI),
        CavemanMode::Off => None,
    };

    if let Some(rules) = prompt_rules {
        inject_system_prompt(request, rules);
    }

    saved_bytes
}

/// Injects rules into the system prompt. If none exists, creates one at the beginning.
fn inject_system_prompt(request: &mut ChatCompletionRequest, rules: &str) {
    let system_msg = request
        .messages
        .iter_mut()
        .find(|msg| msg.role == MessageRole::System);

    if let Some(msg) = system_msg {
        if let Some(content) = &mut msg.content {
            *content = format!("{}\n\n[CAVEMAN MODE RULES]\n{}", content, rules);
        } else {
            msg.content = Some(rules.to_string());
        }
    } else {
        let new_sys_msg = ChatCompletionMessage {
            role: MessageRole::System,
            content: Some(rules.to_string()),
            ..Default::default()
        };
        request.messages.insert(0, new_sys_msg);
    }
}

/// Shrinks text by removing common filler words outside of markdown code blocks
fn shrink_text(text: &str) -> String {
    let mut result = Vec::new();
    let mut in_code_block = false;
    let mut in_inline_code = false;

    // Split into lines to protect code blocks
    for line in text.lines() {
        if line.trim_start().starts_with("```") {
            in_code_block = !in_code_block;
            result.push(line.to_string());
            continue;
        }

        if in_code_block {
            result.push(line.to_string());
            continue;
        }

        // Process line to remove filler words, keeping inline code intact
        let mut processed_line = String::new();
        let words: Vec<&str> = line.split_whitespace().collect();
        let mut i = 0;
        while i < words.len() {
            let word = words[i];

            // Track inline code backticks
            if word.contains('`') {
                let backtick_count = word.chars().filter(|&c| c == '`').count();
                if backtick_count % 2 != 0 {
                    in_inline_code = !in_inline_code;
                }
                processed_line.push_str(word);
                processed_line.push(' ');
                i += 1;
                continue;
            }

            if in_inline_code {
                processed_line.push_str(word);
                processed_line.push(' ');
                i += 1;
                continue;
            }

            // Remove common polite filler/hedging phrases
            if i + 3 < words.len()
                && matches_phrase(&words[i..i + 4], &["i", "would", "like", "to"])
            {
                i += 4;
                continue;
            }
            if i + 2 < words.len() && matches_phrase(&words[i..i + 3], &["could", "you", "please"])
            {
                i += 3;
                continue;
            }
            if i + 1 < words.len() && matches_phrase(&words[i..i + 2], &["please", "can"]) {
                i += 2;
                continue;
            }
            if i + 1 < words.len() && matches_phrase(&words[i..i + 2], &["would", "you"]) {
                i += 2;
                continue;
            }

            // Single word fillers (case-insensitive comparison)
            let lower_word = word.to_lowercase();
            let clean_word = lower_word.trim_matches(|c: char| !c.is_alphanumeric());
            if [
                "please",
                "actually",
                "basically",
                "really",
                "simply",
                "just",
                "kindly",
            ]
            .contains(&clean_word)
            {
                // Keep the punctuation if any
                let punct: String = word.chars().filter(|c| !c.is_alphanumeric()).collect();
                if !punct.is_empty() && punct != "`" {
                    processed_line.push_str(&punct);
                    processed_line.push(' ');
                }
                i += 1;
                continue;
            }

            processed_line.push_str(word);
            processed_line.push(' ');
            i += 1;
        }

        result.push(processed_line.trim_end().to_string());
    }

    result.join("\n")
}

fn matches_phrase(words: &[&str], phrase: &[&str]) -> bool {
    if words.len() < phrase.len() {
        return false;
    }
    for (i, p) in phrase.iter().enumerate() {
        if words[i]
            .to_lowercase()
            .trim_matches(|c: char| !c.is_alphanumeric())
            != *p
        {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use pylos_core::domain::openai::{ChatCompletionMessage, ChatCompletionRequest, MessageRole};

    #[test]
    fn test_shrink_text() {
        let input = "Could you please help me with this? I would like to build a web page. Please keep in mind that it actually needs to look nice.";
        let expected =
            "help me with this? build a web page. keep in mind that it needs to look nice.";
        assert_eq!(shrink_text(input), expected);
    }

    #[test]
    fn test_shrink_text_code_blocks() {
        let input = "Please look at this code:\n```rust\nlet x = basically_something();\n```\nIt is really simple.";
        let expected =
            "look at this code:\n```rust\nlet x = basically_something();\n```\nIt is simple.";
        assert_eq!(shrink_text(input), expected);
    }

    #[test]
    fn test_auto_clarity_guard() {
        let req = ChatCompletionRequest {
            model: "test-model".to_string(),
            messages: vec![ChatCompletionMessage {
                role: MessageRole::User,
                content: Some("Please delete the database production records".to_string()),
                ..Default::default()
            }],
            temperature: None,
            top_p: None,
            n: None,
            stream: None,
            stop: None,
            max_tokens: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            user: None,
            tools: None,
            tool_choice: None,
            response_format: None,
            seed: None,
            top_k: None,
            min_p: None,
            repetition_penalty: None,
            max_completion_tokens: None,
        };
        assert!(is_critical_request(&req));
    }

    #[test]
    fn test_apply_caveman() {
        let mut req = ChatCompletionRequest {
            model: "test-model".to_string(),
            messages: vec![ChatCompletionMessage {
                role: MessageRole::User,
                content: Some("Could you please show me the error log?".to_string()),
                ..Default::default()
            }],
            temperature: None,
            top_p: None,
            n: None,
            stream: None,
            stop: None,
            max_tokens: None,
            presence_penalty: None,
            frequency_penalty: None,
            logit_bias: None,
            user: None,
            tools: None,
            tool_choice: None,
            response_format: None,
            seed: None,
            top_k: None,
            min_p: None,
            repetition_penalty: None,
            max_completion_tokens: None,
        };

        let saved = apply_caveman(&mut req, CavemanMode::Full, true);
        assert!(saved > 0);
        assert_eq!(
            req.messages[1].content.as_deref().unwrap(),
            "show me the error log?"
        );
        assert_eq!(req.messages[0].role, MessageRole::System);
        assert!(req.messages[0]
            .content
            .as_deref()
            .unwrap()
            .contains("Why use many token"));
    }
}
