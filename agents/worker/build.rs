// Build script: obfuscates ONNX model at compile time.
// Uses XOR with a derived keystream (std-only, no external deps).
// The encrypted model is embedded via include_bytes! and decrypted at runtime.

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=models/scout_classifier.onnx");

    let model_path = Path::new("models/scout_classifier.onnx");
    if !model_path.exists() {
        println!("cargo:warning=Model file not found: {:?}", model_path);
        return;
    }

    let model_bytes = fs::read(model_path).expect("Failed to read ONNX model");

    // Simple XOR obfuscation with key derived from seed
    let seed = b"SWARM_SCOUT_ONNX_V1_X7k2Mp9Q_n3R4sT8v";
    let encrypted = xor_encrypt(&model_bytes, seed);

    let out_dir = env::var("OUT_DIR").unwrap();
    let enc_path = Path::new(&out_dir).join("scout_classifier.onnx.enc");
    fs::write(&enc_path, &encrypted).expect("Failed to write encrypted model");

    println!("cargo:warning=Obfuscated scout model: {} bytes -> {} bytes ({} overhead)",
        model_bytes.len(), encrypted.len(), encrypted.len() - model_bytes.len());
}

fn xor_encrypt(data: &[u8], seed: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(16 + data.len());
    // Prepend 16-byte random nonce
    let nonce: Vec<u8> = (0..16).map(|_| rand_u8()).collect();
    result.extend_from_slice(&nonce);

    // XOR each byte with a keystream byte derived from seed + nonce + position
    for (i, &byte) in data.iter().enumerate() {
        let ks = keystream_byte(seed, &nonce, i);
        result.push(byte ^ ks);
    }
    result
}

fn keystream_byte(seed: &[u8], nonce: &[u8], pos: usize) -> u8 {
    // Simple hash mixing
    let mut h: u32 = 0x9e3779b9; // golden ratio
    for &b in seed { h = h.wrapping_mul(31).wrapping_add(b as u32); }
    for &b in nonce { h = h.wrapping_mul(31).wrapping_add(b as u32); }
    h = h.wrapping_mul(31).wrapping_add(pos as u32);
    h = h.wrapping_mul(31).wrapping_add(pos.wrapping_mul(0x517cc1b7) as u32);
    ((h >> 16) ^ h) as u8
}

fn rand_u8() -> u8 {
    // Simple LCG PRNG seeded with time
    use std::time::{SystemTime, UNIX_EPOCH};
    let seed = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos() as u64;
    let mut state = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (state >> 32) as u8
}
