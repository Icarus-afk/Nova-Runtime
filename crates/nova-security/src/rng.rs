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
