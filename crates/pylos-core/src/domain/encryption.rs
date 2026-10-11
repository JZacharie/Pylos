use std::sync::OnceLock;

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::{rngs::OsRng, Rng, RngCore};

const NONCE_SIZE: usize = 12;
const KEY_ENV_VAR: &str = "PYLOS_ENCRYPTION_KEY";

/// Cipher unique du processus : initialisé une seule fois, partagé par tous les threads.
static CIPHER: OnceLock<Aes256Gcm> = OnceLock::new();

/// Une clé éphémère est-elle interdite ? (production ou mode strict explicite)
fn strict_mode() -> bool {
    let production = std::env::var("PYLOS_ENV")
        .map(|v| v.eq_ignore_ascii_case("production"))
        .unwrap_or(false);
    let strict = std::env::var("PYLOS_STRICT_ENCRYPTION")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    production || strict
}

/// Construit le cipher depuis l'environnement.
/// La clé n'est JAMAIS journalisée.
fn build_cipher() -> Result<Aes256Gcm, String> {
    match std::env::var(KEY_ENV_VAR) {
        Ok(value) if !value.trim().is_empty() => {
            let bytes = BASE64
                .decode(value.trim().as_bytes())
                .map_err(|e| format!("{KEY_ENV_VAR} must be base64-encoded 32 bytes: {e}"))?;
            if bytes.len() != 32 {
                return Err(format!(
                    "{KEY_ENV_VAR} must decode to exactly 32 bytes (got {})",
                    bytes.len()
                ));
            }
            Aes256Gcm::new_from_slice(&bytes).map_err(|e| format!("invalid AES-256 key: {e}"))
        }
        _ => {
            if strict_mode() {
                return Err(format!(
                    "{KEY_ENV_VAR} is required: ephemeral keys are refused in production/strict mode. \
                     Set it to a stable base64-encoded 32-byte key."
                ));
            }
            // Génération directe : un tableau initialisé à zéro puis rempli par OsRng
            // était détecté par CodeQL comme « clé cryptographique en dur ».
            let key: [u8; 32] = OsRng.gen();
            tracing::warn!(
                "{KEY_ENV_VAR} is not set: an EPHEMERAL encryption key was generated. \
                 Encrypted values will be unreadable after a restart. Set a stable key."
            );
            Aes256Gcm::new_from_slice(&key).map_err(|e| format!("invalid AES-256 key: {e}"))
        }
    }
}

/// Initialise le chiffrement (idempotent). À appeler au démarrage du serveur.
pub fn init() -> Result<(), String> {
    if CIPHER.get().is_some() {
        return Ok(());
    }
    let cipher = build_cipher()?;
    let _ = CIPHER.set(cipher);
    Ok(())
}

fn cipher() -> Result<&'static Aes256Gcm, String> {
    if CIPHER.get().is_none() {
        init()?;
    }
    CIPHER.get().ok_or_else(|| "encryption is not initialized".to_string())
}

/// Chiffre une valeur (nonce aléatoire préfixé au ciphertext).
pub fn encrypt(plaintext: &str) -> Result<String, String> {
    let cipher = cipher()?;
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from(nonce_bytes);
    let ciphertext = cipher
        .encrypt(&nonce, plaintext.as_bytes())
        .map_err(|_| "encryption failed".to_string())?;

    let mut combined = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
    combined.extend_from_slice(&nonce_bytes);
    combined.extend_from_slice(&ciphertext);
    Ok(BASE64.encode(&combined))
}

/// Déchiffre une valeur encodée (nonce || ciphertext).
pub fn decrypt(encoded: &str) -> Result<String, String> {
    let cipher = cipher()?;
    let combined = BASE64
        .decode(encoded.as_bytes())
        .map_err(|_| "invalid base64".to_string())?;
    if combined.len() < NONCE_SIZE {
        return Err("ciphertext too short".to_string());
    }
    let (nonce_bytes, ciphertext) = combined.split_at(NONCE_SIZE);
    let nonce = Nonce::try_from(nonce_bytes).map_err(|_| "invalid nonce".to_string())?;
    let plaintext = cipher
        .decrypt(&nonce, ciphertext)
        .map_err(|_| "decryption failed".to_string())?;
    String::from_utf8(plaintext).map_err(|_| "plaintext is not valid UTF-8".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let plaintext = "sk-pylos-super-secret-key-12345";
        let encrypted = encrypt(plaintext).expect("encrypt");
        assert_ne!(encrypted, plaintext);
        assert_eq!(decrypt(&encrypted).expect("decrypt"), plaintext);
    }

    #[test]
    fn test_invalid_ciphertext() {
        assert!(decrypt("invalid-base64!").is_err());
        assert!(decrypt("AAAA").is_err());
    }

    #[test]
    fn test_empty_string() {
        let encrypted = encrypt("").expect("encrypt");
        assert_eq!(decrypt(&encrypted).expect("decrypt"), "");
    }

    #[test]
    fn test_deterministic_not() {
        let a = encrypt("same-value").expect("encrypt");
        let b = encrypt("same-value").expect("encrypt");
        assert_ne!(a, b, "each encryption must produce unique ciphertext");
    }

    #[test]
    fn test_cipher_shared_across_threads() {
        let encrypted = encrypt("shared-across-threads").expect("encrypt");
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let value = encrypted.clone();
                std::thread::spawn(move || decrypt(&value).expect("decrypt in thread"))
            })
            .collect();

        for handle in handles {
            assert_eq!(handle.join().expect("thread"), "shared-across-threads");
        }
    }
}
