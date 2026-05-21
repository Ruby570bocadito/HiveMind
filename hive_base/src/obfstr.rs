// Compile-time string obfuscation.
// Strings like "127.0.0.1:4242" or "colmena_arena" are XOR-encrypted
// at compile time via the obf!() macro and decrypted at runtime.
// Result: `strings binary` reveals nothing useful.

/// XOR key — deterministic per compilation.
/// Uses a simple compile-time hash of the package version.
pub const XOR_KEY: u8 = {
    const V: &[u8] = env!("CARGO_PKG_VERSION").as_bytes();
    // Manual const-compatible hash
    let mut h: u8 = 0x9A;
    let mut i = 0;
    while i < V.len() {
        h = h.wrapping_add(V[i]).wrapping_mul(31);
        i += 1;
    }
    h ^ 0xA5
};

/// Obfuscate a string literal at compile time.
/// Usage: `let s = obf!("colmena_arena");`
#[macro_export]
macro_rules! obf {
    ($s:expr) => {{
        const STR: &str = $s;
        const LEN: usize = STR.len();
        const KEY: u8 = $crate::obfstr::XOR_KEY;
        const ENC: [u8; LEN] = $crate::obfstr::xor_encrypt(STR.as_bytes(), KEY, LEN);
        $crate::obfstr::xor_decrypt(&ENC)
    }};
}

/// XOR encrypt a byte slice with a key. Const-compatible.
pub const fn xor_encrypt<const N: usize>(data: &[u8], key: u8, len: usize) -> [u8; N] {
    let mut result = [0u8; N];
    let mut i = 0;
    while i < len && i < N {
        result[i] = data[i] ^ key.wrapping_add(i as u8);
        i += 1;
    }
    result
}

/// XOR decrypt at runtime.
pub fn xor_decrypt<const N: usize>(encrypted: &[u8; N]) -> String {
    let key = XOR_KEY;
    let mut result = Vec::with_capacity(N);
    for (i, &b) in encrypted.iter().enumerate() {
        result.push(b ^ key.wrapping_add(i as u8));
    }
    String::from_utf8(result).unwrap_or_default()
}

/// Obfuscate a byte array directly (for binary blobs).
pub const fn xor_encrypt_bytes<const N: usize>(data: &[u8; N]) -> [u8; N] {
    let mut result = [0u8; N];
    let mut i = 0;
    while i < N {
        result[i] = data[i] ^ XOR_KEY.wrapping_add(i as u8);
        i += 1;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_xor_roundtrip() {
        let original = b"Hello World!";
        let len = original.len();
        let encrypted = xor_encrypt::<12>(original, 0x42, len);
        let decrypted: Vec<u8> = encrypted.iter().enumerate()
            .map(|(i, &b)| b ^ 0x42u8.wrapping_add(i as u8))
            .collect();
        assert_eq!(original.as_slice(), decrypted.as_slice());
    }

    #[test]
    fn test_obf_macro_compiles() {
        // This verifies the macro syntax is valid
        let _s: String = obf!("swarm_bus_address_4242");
        assert!(!_s.is_empty());
    }
}
