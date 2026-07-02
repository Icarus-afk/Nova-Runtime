use rand::RngCore;
use std::sync::atomic::{AtomicU64, Ordering};

pub struct SecureRng;

impl SecureRng {
    pub fn fill_bytes(buf: &mut [u8]) {
        rand::rngs::OsRng.fill_bytes(buf);
    }

    pub fn next_u64() -> u64 {
        rand::rngs::OsRng.next_u64()
    }

    pub fn next_u32() -> u32 {
        rand::rngs::OsRng.next_u32()
    }

    pub fn uuid_v4() -> [u8; 16] {
        let mut bytes = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        bytes[6] = (bytes[6] & 0x0f) | 0x40;
        bytes[8] = (bytes[8] & 0x3f) | 0x80;
        bytes
    }

    pub fn alphanumeric_string(length: usize) -> String {
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        let charset_len = CHARSET.len() as u8;
        let mut bytes = vec![0u8; length];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        for b in &mut bytes {
            *b = CHARSET[*b as usize % charset_len as usize];
        }
        String::from_utf8(bytes).expect("alphanumeric is valid utf-8")
    }

    pub fn session_id() -> String {
        let mut bytes = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        hex::encode(bytes)
    }
}

pub struct NonceGenerator {
    counter: AtomicU64,
    random_suffix: [u8; 4],
}

impl NonceGenerator {
    pub fn new() -> Self {
        let mut suffix = [0u8; 4];
        rand::rngs::OsRng.fill_bytes(&mut suffix);
        let counter = rand::rngs::OsRng.next_u64();
        NonceGenerator {
            counter: AtomicU64::new(counter),
            random_suffix: suffix,
        }
    }

    pub fn next_nonce(&self) -> [u8; 12] {
        let counter = self.counter.fetch_add(1, Ordering::Relaxed);
        let mut nonce = [0u8; 12];
        nonce[..8].copy_from_slice(&counter.to_be_bytes());
        nonce[8..].copy_from_slice(&self.random_suffix);
        nonce
    }
}

impl Default for NonceGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fill_bytes() {
        let mut buf = [0u8; 32];
        SecureRng::fill_bytes(&mut buf);
        assert!(!buf.iter().all(|&b| b == 0));
    }

    #[test]
    fn test_next_u64_non_zero() {
        let val = SecureRng::next_u64();
        assert!(val != 0 || SecureRng::next_u64() != 0 || SecureRng::next_u64() != 0);
    }

    #[test]
    fn test_next_u32_non_zero() {
        let val = SecureRng::next_u32();
        assert!(val != 0 || SecureRng::next_u32() != 0 || SecureRng::next_u32() != 0);
    }

    #[test]
    fn test_uuid_v4_format() {
        let uuid = SecureRng::uuid_v4();
        assert_eq!(uuid.len(), 16);
        assert_eq!(uuid[6] & 0xf0, 0x40);
        assert_eq!(uuid[8] & 0xc0, 0x80);
    }

    #[test]
    fn test_alphanumeric_string_length() {
        let s = SecureRng::alphanumeric_string(32);
        assert_eq!(s.len(), 32);
    }

    #[test]
    fn test_alphanumeric_string_charset() {
        let s = SecureRng::alphanumeric_string(256);
        assert!(s.chars().all(|c| c.is_ascii_alphanumeric()));
    }

    #[test]
    fn test_session_id_length() {
        let id = SecureRng::session_id();
        assert_eq!(id.len(), 64);
    }

    #[test]
    fn test_session_id_hex() {
        let id = SecureRng::session_id();
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_nonce_generator_unique() {
        let generator = NonceGenerator::new();
        let mut set = std::collections::HashSet::new();
        for _ in 0..1000 {
            let nonce = generator.next_nonce();
            assert!(set.insert(nonce), "nonce repeated");
        }
    }

    #[test]
    fn test_nonce_generator_counter_increments() {
        let generator = NonceGenerator::new();
        let n1 = generator.next_nonce();
        let n2 = generator.next_nonce();
        let c1 = u64::from_be_bytes(n1[..8].try_into().unwrap());
        let c2 = u64::from_be_bytes(n2[..8].try_into().unwrap());
        assert_eq!(c2, c1 + 1);
    }

    #[test]
    fn test_nonce_generator_suffix_constant() {
        let generator = NonceGenerator::new();
        let n1 = generator.next_nonce();
        let n2 = generator.next_nonce();
        assert_eq!(n1[8..], n2[8..]);
    }

    #[test]
    fn test_nonce_generator_default() {
        let generator = NonceGenerator::default();
        let nonce = generator.next_nonce();
        assert_eq!(nonce.len(), 12);
    }
}
