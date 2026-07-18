use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use rand::{rngs::OsRng, RngCore};

const NONCE_SIZE: usize = 12;
const KEY_ENV_VAR: &str = "PYLOS_ENCRYPTION_KEY";

fn load_or_generate_key() -> [u8; 32] {
    if let Ok(key_b64) = std::env::var(KEY_ENV_VAR) {
        let key_bytes = BASE64.decode(key_b64.as_bytes()).unwrap_or_else(|e| {
            panic!("PYLOS_ENCRYPTION_KEY must be base64-encoded 32 bytes: {e}")
        });
        if key_bytes.len() != 32 {
            panic!("PYLOS_ENCRYPTION_KEY must decode to exactly 32 bytes");
        }
        let mut key = [0u8; 32];
        key.copy_from_slice(&key_bytes);
        key
    } else {
        let mut key = [0u8; 32];
        OsRng.fill_bytes(&mut key);
        let b64 = BASE64.encode(key);
        tracing::warn!(
            "PYLOS_ENCRYPTION_KEY not set — generated ephemeral key (keys will be lost on restart): {b64}"
        );
        key
    }
}

thread_local! {
    static CIPHER: Aes256Gcm = {
        let key = load_or_generate_key();
        Aes256Gcm::new_from_slice(&key).expect("valid AES-256 key")
    };
}

pub fn encrypt(plaintext: &str) -> String {
    let mut nonce_bytes = [0u8; NONCE_SIZE];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from(nonce_bytes);
    CIPHER.with(|cipher| {
        let ciphertext = cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .expect("encryption failed");
        let mut combined = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
        combined.extend_from_slice(&nonce_bytes);
        combined.extend_from_slice(&ciphertext);
        BASE64.encode(&combined)
    })
}

pub fn decrypt(encoded: &str) -> Option<String> {
    let combined = BASE64.decode(encoded.as_bytes()).ok()?;
    if combined.len() < NONCE_SIZE {
        return None;
    }
    let (nonce_bytes, ciphertext) = combined.split_at(NONCE_SIZE);
    let nonce = Nonce::try_from(nonce_bytes).ok()?;
    CIPHER.with(|cipher| {
        cipher
            .decrypt(&nonce, ciphertext)
            .ok()
            .and_then(|v| String::from_utf8(v).ok())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_roundtrip() {
        let plaintext = "sk-pylos-super-secret-key-12345";
        let encrypted = encrypt(plaintext);
        assert_ne!(encrypted, plaintext);
        let decrypted = decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, plaintext);
    }

    #[test]
    fn test_invalid_ciphertext() {
        assert!(decrypt("invalid-base64!").is_none());
        assert!(decrypt("AAAA").is_none());
    }

    #[test]
    fn test_empty_string() {
        let encrypted = encrypt("");
        let decrypted = decrypt(&encrypted).unwrap();
        assert_eq!(decrypted, "");
    }

    #[test]
    fn test_deterministic_not() {
        let a = encrypt("same-value");
        let b = encrypt("same-value");
        assert_ne!(a, b, "each encryption must produce unique ciphertext");
    }
}
