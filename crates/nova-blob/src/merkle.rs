use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct ProofStep {
    pub hash: String,
    pub is_right: bool,
}

pub struct MerkleTree;

impl MerkleTree {
    pub fn build(chunk_hashes: &[String]) -> String {
        if chunk_hashes.is_empty() {
            return hex::encode(Sha256::digest(b""));
        }
        let mut level: Vec<String> = chunk_hashes.to_vec();
        while level.len() > 1 {
            let mut next = Vec::new();
            for pair in level.chunks(2) {
                if pair.len() == 2 {
                    let combined = format!("{}{}", pair[0], pair[1]);
                    let hash = hex::encode(Sha256::digest(combined.as_bytes()));
                    next.push(hash);
                } else {
                    next.push(pair[0].clone());
                }
            }
            level = next;
        }
        level.into_iter().next().unwrap()
    }

    pub fn verify(chunk_hash: &str, proof: &[ProofStep], root: &str) -> bool {
        let mut current = chunk_hash.to_string();
        for step in proof {
            let combined = if step.is_right {
                format!("{}{}", current, step.hash)
            } else {
                format!("{}{}", step.hash, current)
            };
            current = hex::encode(Sha256::digest(combined.as_bytes()));
        }
        current == root
    }

    pub fn generate_proof(chunk_hashes: &[String], chunk_index: usize) -> Vec<ProofStep> {
        if chunk_index >= chunk_hashes.len() {
            return Vec::new();
        }
        let mut proof = Vec::new();
        let mut level: Vec<String> = chunk_hashes.to_vec();
        let mut idx = chunk_index;
        while level.len() > 1 {
            let sibling_idx = if idx % 2 == 0 { idx + 1 } else { idx - 1 };
            if sibling_idx < level.len() {
                proof.push(ProofStep {
                    hash: level[sibling_idx].clone(),
                    is_right: idx % 2 == 0,
                });
            }
            let mut next = Vec::new();
            for pair in level.chunks(2) {
                if pair.len() == 2 {
                    let combined = format!("{}{}", pair[0], pair[1]);
                    let hash = hex::encode(Sha256::digest(combined.as_bytes()));
                    next.push(hash);
                } else {
                    next.push(pair[0].clone());
                }
            }
            idx /= 2;
            level = next;
        }
        proof
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chunk::ChunkManager;

    #[test]
    fn test_build_single_chunk() {
        let hash = ChunkManager::hash(b"abc123");
        let root = MerkleTree::build(&[hash.clone()]);
        assert_eq!(root, hash);
    }

    #[test]
    fn test_verify_valid_proof() {
        let hashes: Vec<String> = vec![
            ChunkManager::hash(b"chunk1"),
            ChunkManager::hash(b"chunk2"),
            ChunkManager::hash(b"chunk3"),
            ChunkManager::hash(b"chunk4"),
        ];
        let root = MerkleTree::build(&hashes);
        for i in 0..hashes.len() {
            let proof = MerkleTree::generate_proof(&hashes, i);
            assert!(MerkleTree::verify(&hashes[i], &proof, &root), "failed for index {}", i);
        }
    }

    #[test]
    fn test_verify_invalid_proof() {
        let hashes: Vec<String> = vec![
            ChunkManager::hash(b"chunk1"),
            ChunkManager::hash(b"chunk2"),
        ];
        let root = MerkleTree::build(&hashes);
        let wrong_hash = ChunkManager::hash(b"wrong");
        let proof = MerkleTree::generate_proof(&hashes, 0);
        assert!(!MerkleTree::verify(&wrong_hash, &proof, &root));
    }

    #[test]
    fn test_build_empty() {
        let root = MerkleTree::build(&[]);
        assert_eq!(root, hex::encode(Sha256::digest(b"")));
    }
}
