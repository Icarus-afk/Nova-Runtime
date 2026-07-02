use crate::{Result, SecurityError};
use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Key, Nonce};
use parking_lot::RwLock;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EncryptionAlgorithm {
    Aes256Gcm,
    Aes256GcmSiv,
    ChaCha20Poly1305,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyId(pub [u8; 8]);

impl KeyId {
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyWrapper {
    pub id: KeyId,
    pub key: [u8; 32],
    pub created_at: u64,
    pub expires_at: u64,
    pub algorithm: EncryptionAlgorithm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    pub key_id: KeyId,
    pub algorithm: EncryptionAlgorithm,
    pub nonce: [u8; 12],
    pub ciphertext: Vec<u8>,
    pub tag: [u8; 16],
}

pub trait KeyProvider: Send + Sync {
    fn get_key(&self, key_id: &KeyId) -> Result<Vec<u8>>;
    fn generate_key(&self, algorithm: &str) -> Result<(KeyId, Vec<u8>)>;
    fn rotate_key(&self, key_id: &KeyId) -> Result<KeyId>;
    fn delete_key(&self, key_id: &KeyId) -> Result<()>;
}

pub struct EncryptionEngine {
    active_key: RwLock<KeyWrapper>,
    previous_keys: RwLock<Vec<KeyWrapper>>,
}

impl EncryptionEngine {
    pub fn new(active_key: KeyWrapper) -> Self {
        EncryptionEngine {
            active_key: RwLock::new(active_key),
            previous_keys: RwLock::new(Vec::new()),
        }
    }

    pub fn encrypt(&self, plaintext: &[u8]) -> Result<EncryptedData> {
        let key_wrapper = self.active_key.read().clone();
        match key_wrapper.algorithm {
            EncryptionAlgorithm::Aes256Gcm => self.encrypt_aes256gcm(&key_wrapper, plaintext),
            EncryptionAlgorithm::Aes256GcmSiv => self.encrypt_aes256gcm_siv(&key_wrapper, plaintext),
            EncryptionAlgorithm::ChaCha20Poly1305 => self.encrypt_chacha20(&key_wrapper, plaintext),
        }
    }

    fn encrypt_aes256gcm(&self, key_wrapper: &KeyWrapper, plaintext: &[u8]) -> Result<EncryptedData> {
        let key = Key::<Aes256Gcm>::from_slice(&key_wrapper.key);
        let cipher = Aes256Gcm::new(key);

        let mut nonce_bytes = [0u8; 12];
        rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext_with_tag = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| SecurityError::Encryption(e.to_string()))?;

        let tag_start = ciphertext_with_tag.len() - 16;
        let (ct, tag_bytes) = ciphertext_with_tag.split_at(tag_start);

        let mut tag = [0u8; 16];
        tag.copy_from_slice(tag_bytes);

        Ok(EncryptedData {
            key_id: key_wrapper.id,
            algorithm: key_wrapper.algorithm,
            nonce: nonce_bytes,
            ciphertext: ct.to_vec(),
            tag,
        })
    }

    fn encrypt_aes256gcm_siv(&self, key_wrapper: &KeyWrapper, plaintext: &[u8]) -> Result<EncryptedData> {
        use aes_gcm_siv::aead::{Aead, NewAead};
        use aes_gcm_siv::{Aes256GcmSiv as Aes256GcmSivCipher, Key as AesGcmSivKey, Nonce as AesGcmSivNonce};

        let key = AesGcmSivKey::from_slice(&key_wrapper.key);
        let cipher = Aes256GcmSivCipher::new(key);

        let mut nonce_bytes = [0u8; 12];
        rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = AesGcmSivNonce::from_slice(&nonce_bytes);

        let ciphertext_with_tag = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| SecurityError::Encryption(e.to_string()))?;

        let tag_start = ciphertext_with_tag.len() - 16;
        let (ct, tag_bytes) = ciphertext_with_tag.split_at(tag_start);

        let mut tag = [0u8; 16];
        tag.copy_from_slice(tag_bytes);

        Ok(EncryptedData {
            key_id: key_wrapper.id,
            algorithm: key_wrapper.algorithm,
            nonce: nonce_bytes,
            ciphertext: ct.to_vec(),
            tag,
        })
    }

    fn encrypt_chacha20(&self, key_wrapper: &KeyWrapper, plaintext: &[u8]) -> Result<EncryptedData> {
        use chacha20poly1305::aead::{Aead, KeyInit};
        use chacha20poly1305::{ChaCha20Poly1305, Key as ChaChaKey, Nonce as ChaChaNonce};

        let key = ChaChaKey::from_slice(&key_wrapper.key);
        let cipher = ChaCha20Poly1305::new(key);

        let mut nonce_bytes = [0u8; 12];
        rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);
        let nonce = ChaChaNonce::from_slice(&nonce_bytes);

        let ciphertext_with_tag = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| SecurityError::Encryption(e.to_string()))?;

        let tag_start = ciphertext_with_tag.len() - 16;
        let (ct, tag_bytes) = ciphertext_with_tag.split_at(tag_start);

        let mut tag = [0u8; 16];
        tag.copy_from_slice(tag_bytes);

        Ok(EncryptedData {
            key_id: key_wrapper.id,
            algorithm: key_wrapper.algorithm,
            nonce: nonce_bytes,
            ciphertext: ct.to_vec(),
            tag,
        })
    }

    pub fn decrypt(&self, data: &EncryptedData) -> Result<Vec<u8>> {
        let key_wrapper = {
            let active = self.active_key.read();
            if active.id == data.key_id {
                Some(active.clone())
            } else {
                None
            }
        };

        let key_wrapper = match key_wrapper {
            Some(k) => k,
            None => {
                let prev = self.previous_keys.read();
                prev.iter()
                    .find(|k| k.id == data.key_id)
                    .cloned()
                    .ok_or_else(|| SecurityError::KeyNotFound(format!("{:?}", data.key_id)))?
            }
        };

        match data.algorithm {
            EncryptionAlgorithm::Aes256Gcm => self.decrypt_aes256gcm(&key_wrapper, data),
            EncryptionAlgorithm::Aes256GcmSiv => self.decrypt_aes256gcm_siv(&key_wrapper, data),
            EncryptionAlgorithm::ChaCha20Poly1305 => self.decrypt_chacha20(&key_wrapper, data),
        }
    }

    fn decrypt_aes256gcm(&self, key_wrapper: &KeyWrapper, data: &EncryptedData) -> Result<Vec<u8>> {
        let key = Key::<Aes256Gcm>::from_slice(&key_wrapper.key);
        let cipher = Aes256Gcm::new(key);
        let nonce = Nonce::from_slice(&data.nonce);

        let mut ciphertext_with_tag = data.ciphertext.clone();
        ciphertext_with_tag.extend_from_slice(&data.tag);

        cipher
            .decrypt(nonce, ciphertext_with_tag.as_ref())
            .map_err(|e| SecurityError::Decryption(e.to_string()))
    }

    fn decrypt_aes256gcm_siv(&self, key_wrapper: &KeyWrapper, data: &EncryptedData) -> Result<Vec<u8>> {
        use aes_gcm_siv::aead::{Aead, NewAead};
        use aes_gcm_siv::{Aes256GcmSiv as Aes256GcmSivCipher, Key as AesGcmSivKey, Nonce as AesGcmSivNonce};

        let key = AesGcmSivKey::from_slice(&key_wrapper.key);
        let cipher = Aes256GcmSivCipher::new(key);
        let nonce = AesGcmSivNonce::from_slice(&data.nonce);

        let mut ciphertext_with_tag = data.ciphertext.clone();
        ciphertext_with_tag.extend_from_slice(&data.tag);

        cipher
            .decrypt(nonce, ciphertext_with_tag.as_ref())
            .map_err(|e| SecurityError::Decryption(e.to_string()))
    }

    fn decrypt_chacha20(&self, key_wrapper: &KeyWrapper, data: &EncryptedData) -> Result<Vec<u8>> {
        use chacha20poly1305::aead::{Aead, KeyInit};
        use chacha20poly1305::{ChaCha20Poly1305, Key as ChaChaKey, Nonce as ChaChaNonce};

        let key = ChaChaKey::from_slice(&key_wrapper.key);
        let cipher = ChaCha20Poly1305::new(key);
        let nonce = ChaChaNonce::from_slice(&data.nonce);

        let mut ciphertext_with_tag = data.ciphertext.clone();
        ciphertext_with_tag.extend_from_slice(&data.tag);

        cipher
            .decrypt(nonce, ciphertext_with_tag.as_ref())
            .map_err(|e| SecurityError::Decryption(e.to_string()))
    }

    pub fn rotate_key(&self, new_key: KeyWrapper) {
        let mut active = self.active_key.write();
        let mut prev = self.previous_keys.write();
        prev.push(active.clone());
        *active = new_key;
    }

    pub fn active_key_id(&self) -> KeyId {
        self.active_key.read().id
    }
}

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn generate_key() -> KeyWrapper {
    let mut key_material = [0u8; 32];
    rand::rngs::OsRng.fill_bytes(&mut key_material);

    let mut key_id_bytes = [0u8; 8];
    rand::rngs::OsRng.fill_bytes(&mut key_id_bytes);

    let now = now_ms();
    KeyWrapper {
        id: KeyId(key_id_bytes),
        key: key_material,
        created_at: now,
        expires_at: now + 90 * 24 * 60 * 60 * 1000,
        algorithm: EncryptionAlgorithm::Aes256Gcm,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt_round_trip() {
        let key = generate_key();
        let engine = EncryptionEngine::new(key);
        let plaintext = b"Hello, Nova Runtime!";
        let encrypted = engine.encrypt(plaintext).unwrap();
        let decrypted = engine.decrypt(&encrypted).unwrap();
        assert_eq!(plaintext.to_vec(), decrypted);
    }

    #[test]
    fn test_encrypt_decrypt_empty() {
        let key = generate_key();
        let engine = EncryptionEngine::new(key);
        let encrypted = engine.encrypt(b"").unwrap();
        let decrypted = engine.decrypt(&encrypted).unwrap();
        assert_eq!(Vec::<u8>::new(), decrypted);
    }

    #[test]
    fn test_encrypt_large_data() {
        let key = generate_key();
        let engine = EncryptionEngine::new(key);
        let plaintext = vec![0xABu8; 10_000];
        let encrypted = engine.encrypt(&plaintext).unwrap();
        let decrypted = engine.decrypt(&encrypted).unwrap();
        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_unique_nonces() {
        let key = generate_key();
        let engine = EncryptionEngine::new(key);
        let msg = b"same plaintext";
        let e1 = engine.encrypt(msg).unwrap();
        let e2 = engine.encrypt(msg).unwrap();
        assert_ne!(e1.nonce, e2.nonce);
        assert_ne!(e1.ciphertext, e2.ciphertext);
    }

    #[test]
    fn test_authentication_tag_protects_integrity() {
        let key = generate_key();
        let engine = EncryptionEngine::new(key);
        let encrypted = engine.encrypt(b"integrity check").unwrap();
        let mut tampered = EncryptedData { ..encrypted.clone() };
        tampered.tag[0] ^= 0x01;
        let result = engine.decrypt(&tampered);
        assert!(result.is_err());
    }

    #[test]
    fn test_tampered_ciphertext_fails() {
        let key = generate_key();
        let engine = EncryptionEngine::new(key);
        let encrypted = engine.encrypt(b"sensitive data").unwrap();
        let mut tampered = EncryptedData { ..encrypted.clone() };
        tampered.ciphertext[0] ^= 0xFF;
        let result = engine.decrypt(&tampered);
        assert!(result.is_err());
    }

    #[test]
    fn test_tampered_nonce_fails() {
        let key = generate_key();
        let engine = EncryptionEngine::new(key);
        let encrypted = engine.encrypt(b"nonce protected").unwrap();
        let mut tampered = EncryptedData { ..encrypted.clone() };
        tampered.nonce[0] ^= 0x01;
        let result = engine.decrypt(&tampered);
        assert!(result.is_err());
    }

    #[test]
    fn test_decrypt_with_wrong_engine_fails() {
        let key1 = generate_key();
        let engine1 = EncryptionEngine::new(key1);
        let encrypted = engine1.encrypt(b"secret").unwrap();
        let key2 = generate_key();
        let engine2 = EncryptionEngine::new(key2);
        let result = engine2.decrypt(&encrypted);
        assert!(matches!(result, Err(SecurityError::KeyNotFound(_))));
    }

    #[test]
    fn test_key_not_found_error() {
        let key = generate_key();
        let engine = EncryptionEngine::new(key);
        let bogus = EncryptedData {
            key_id: KeyId([0; 8]),
            algorithm: EncryptionAlgorithm::Aes256Gcm,
            nonce: [0; 12],
            ciphertext: vec![],
            tag: [0; 16],
        };
        let result = engine.decrypt(&bogus);
        assert!(matches!(result, Err(SecurityError::KeyNotFound(_))));
    }

    #[test]
    fn test_rotate_key_then_decrypt_old_data() {
        let key1 = generate_key();
        let engine = EncryptionEngine::new(key1);
        let encrypted = engine.encrypt(b"data before rotation").unwrap();
        let key2 = generate_key();
        engine.rotate_key(key2);
        let decrypted = engine.decrypt(&encrypted).unwrap();
        assert_eq!(b"data before rotation".to_vec(), decrypted);
    }

    #[test]
    fn test_active_key_id_after_rotation() {
        let key1 = generate_key();
        let engine = EncryptionEngine::new(key1);
        let id1 = engine.active_key_id();
        let key2 = generate_key();
        engine.rotate_key(key2);
        let id2 = engine.active_key_id();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_generate_key_defaults() {
        let key = generate_key();
        assert_eq!(key.key.len(), 32);
        assert_eq!(key.algorithm, EncryptionAlgorithm::Aes256Gcm);
        assert!(key.expires_at > key.created_at);
    }

    #[test]
    fn test_generate_key_unique_ids() {
        let k1 = generate_key();
        let k2 = generate_key();
        assert_ne!(k1.id, k2.id);
    }

    #[test]
    fn test_now_ms_positive() {
        let t = now_ms();
        assert!(t > 1_700_000_000_000u64);
    }

    #[test]
    fn test_encrypt_with_chacha20() {
        let key = generate_key();
        let chacha_key = KeyWrapper {
            algorithm: EncryptionAlgorithm::ChaCha20Poly1305,
            ..key
        };
        let engine = EncryptionEngine::new(chacha_key);
        let plaintext = b"ChaCha20 test";
        let encrypted = engine.encrypt(plaintext).unwrap();
        let decrypted = engine.decrypt(&encrypted).unwrap();
        assert_eq!(plaintext.to_vec(), decrypted);
    }

    #[test]
    fn test_encrypt_with_aes256gcm_siv() {
        let key = generate_key();
        let siv_key = KeyWrapper {
            algorithm: EncryptionAlgorithm::Aes256GcmSiv,
            ..key
        };
        let engine = EncryptionEngine::new(siv_key);
        let plaintext = b"AES256-GCM-SIV test";
        let encrypted = engine.encrypt(plaintext).unwrap();
        let decrypted = engine.decrypt(&encrypted).unwrap();
        assert_eq!(plaintext.to_vec(), decrypted);
    }
}
