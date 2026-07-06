// Lightweight model obfuscation.
// Models are obfuscated at build time with XOR keystream,
// deobfuscated in memory at runtime. Keys exist only in the binary.
// Supports ChaCha20 for stronger encryption when needed.

use chacha20::ChaCha20;
use chacha20::cipher::{KeyIvInit, StreamCipher};
use rand::Rng;

type ChaCha = ChaCha20;

// ── XOR-based obfuscation (matches build.rs) ─────────────────────────────────

/// Decrypt a model obfuscated with XOR keystream.
/// Format: [16-byte nonce][XOR-encrypted data]
pub fn decrypt_model(encrypted: &[u8], seed: &[u8]) -> Option<Vec<u8>> {
    if encrypted.len() < 16 {
        return None;
    }
    let nonce = &encrypted[..16];
    let ciphertext = &encrypted[16..];

    let mut plaintext = Vec::with_capacity(ciphertext.len());
    for (i, &byte) in ciphertext.iter().enumerate() {
        let ks = keystream_byte(seed, nonce, i);
        plaintext.push(byte ^ ks);
    }
    Some(plaintext)
}

fn keystream_byte(seed: &[u8], nonce: &[u8], pos: usize) -> u8 {
    let mut h: u32 = 0x9e3779b9;
    for &b in seed { h = h.wrapping_mul(31).wrapping_add(b as u32); }
    for &b in nonce { h = h.wrapping_mul(31).wrapping_add(b as u32); }
    h = h.wrapping_mul(31).wrapping_add(pos as u32);
    h = h.wrapping_mul(31).wrapping_add(pos.wrapping_mul(0x517cc1b7) as u32);
    ((h >> 16) ^ h) as u8
}

// ── ChaCha20 strong encryption (for cross-process key material) ──────────────

/// Encrypt data with ChaCha20. Returns (nonce || ciphertext).
pub fn encrypt_chacha20(plaintext: &[u8], key: &[u8; 32]) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let mut nonce = [0u8; 12];
    rng.fill(&mut nonce);

    let mut cipher = ChaCha::new(key.into(), (&nonce).into());
    let mut ciphertext = plaintext.to_vec();
    cipher.apply_keystream(&mut ciphertext);

    let mut result = Vec::with_capacity(12 + ciphertext.len());
    result.extend_from_slice(&nonce);
    result.extend_from_slice(&ciphertext);
    result
}

/// Decrypt ChaCha20-encrypted data. Input: (nonce || ciphertext).
pub fn decrypt_chacha20(encrypted: &[u8], key: &[u8; 32]) -> Option<Vec<u8>> {
    if encrypted.len() < 12 {
        return None;
    }
    let nonce: [u8; 12] = encrypted[..12].try_into().unwrap();
    let mut ciphertext = encrypted[12..].to_vec();

    let mut cipher = ChaCha::new(key.into(), (&nonce).into());
    cipher.apply_keystream(&mut ciphertext);

    Some(ciphertext)
}

// ── key derivation ───────────────────────────────────────────────────────────

/// Derive a 32-byte key from a seed string (SHA-256).
pub fn derive_key(seed: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(seed.as_bytes());
    let result = hasher.finalize();
    let mut key = [0u8; 32];
    key.copy_from_slice(&result);
    key
}

/// Derive seed bytes from string for XOR obfuscation (SHA-256).
pub fn derive_seed(seed_str: &str) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(seed_str.as_bytes());
    hasher.finalize().to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xor_roundtrip() {
        // Simulate build-time encryption with matching runtime decryption
        let seed = derive_seed("test_seed");
        let plaintext = b"ONNX model data would go here";
        // Build-time: same as build.rs would do
        let nonce: Vec<u8> = (0..16).map(|_| rand::thread_rng().gen()).collect();
        let mut ct = Vec::new();
        ct.extend_from_slice(&nonce);
        for (i, &b) in plaintext.iter().enumerate() {
            let ks = keystream_byte(&seed, &nonce, i);
            ct.push(b ^ ks);
        }
        // Runtime: decrypt
        let pt = decrypt_model(&ct, &seed).unwrap();
        assert_eq!(plaintext.as_slice(), pt.as_slice());
    }

    #[test]
    fn test_chacha20_roundtrip() {
        let key = derive_key("chacha_test_key");
        let pt = b"secret payload bytes";
        let ct = encrypt_chacha20(pt, &key);
        let dec = decrypt_chacha20(&ct, &key).unwrap();
        assert_eq!(pt.as_slice(), dec.as_slice());
    }
}
