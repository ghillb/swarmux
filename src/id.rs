use crate::model::SubmitPayload;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone)]
pub struct IdConfig {
    pub min_hash_length: usize,
    pub max_hash_length: usize,
    pub max_collision_prob: f64,
}

impl Default for IdConfig {
    fn default() -> Self {
        Self {
            min_hash_length: 3,
            max_hash_length: 8,
            max_collision_prob: 0.25,
        }
    }
}

#[derive(Debug, Clone)]
pub struct IdGenerator {
    config: IdConfig,
}

impl IdGenerator {
    pub fn with_defaults() -> Self {
        Self {
            config: IdConfig::default(),
        }
    }

    #[allow(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap
    )]
    pub fn optimal_length(&self, issue_count: usize) -> usize {
        let n = issue_count as f64;
        let max_prob = self.config.max_collision_prob;

        for len in self.config.min_hash_length..=self.config.max_hash_length {
            let space = 36_f64.powi(len as i32);
            let prob = 1.0 - (-n * n / (2.0 * space)).exp();
            if prob < max_prob {
                return len;
            }
        }

        self.config.max_hash_length
    }

    pub fn generate_candidate(
        &self,
        payload: &SubmitPayload,
        created_at: DateTime<Utc>,
        nonce: u32,
        hash_length: usize,
    ) -> String {
        let seed = format!(
            "{}|{}|{}|{}|{}",
            payload.title,
            payload.repo_ref,
            payload.repo_root,
            created_at.timestamp_nanos_opt().unwrap_or(0),
            nonce
        );
        compute_id_hash(&seed, hash_length)
    }

    pub fn generate<F>(
        &self,
        payload: &SubmitPayload,
        created_at: DateTime<Utc>,
        issue_count: usize,
        exists: F,
    ) -> String
    where
        F: Fn(&str) -> bool,
    {
        let mut length = self.optimal_length(issue_count);

        loop {
            for nonce in 0..10 {
                let id = self.generate_candidate(payload, created_at, nonce, length);
                if !exists(&id) {
                    return id;
                }
            }

            if length < self.config.max_hash_length {
                length += 1;
                continue;
            }

            let mut nonce = 0u32;
            loop {
                let id = self.generate_candidate(payload, created_at, nonce, 12);
                if !exists(&id) {
                    return id;
                }

                nonce += 1;
                if nonce > 1000 {
                    return format!("{id}{nonce}");
                }
            }
        }
    }
}

fn compute_id_hash(input: &str, length: usize) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let result = hasher.finalize();

    let mut num = 0u64;
    for &byte in result.iter().take(8) {
        num = (num << 8) | u64::from(byte);
    }

    let mut encoded = base36_encode(num);
    if encoded.len() < length {
        encoded = format!("{encoded:0>length$}");
    }

    encoded.chars().take(length).collect()
}

fn base36_encode(mut num: u64) -> String {
    const ALPHABET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if num == 0 {
        return "0".to_string();
    }

    let mut chars = Vec::new();
    while num > 0 {
        chars.push(ALPHABET[(num % 36) as usize] as char);
        num /= 36;
    }
    chars.into_iter().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::TaskMode;

    fn sample_payload() -> SubmitPayload {
        SubmitPayload {
            title: "hello".to_string(),
            repo_ref: "core".to_string(),
            repo_root: "/tmp/core".to_string(),
            mode: TaskMode::Manual,
            worktree: Some("/tmp/core".to_string()),
            session: Some("manual".to_string()),
            command: vec!["echo".to_string(), "ok".to_string()],
            priority: None,
            external_ref: None,
            origin: None,
        }
    }

    #[test]
    fn optimal_length_grows_with_dataset() {
        let generator = IdGenerator::with_defaults();
        assert_eq!(generator.optimal_length(0), 3);
        assert!(generator.optimal_length(10_000) >= 4);
    }

    #[test]
    fn generated_id_is_base36_without_prefix() {
        let generator = IdGenerator::with_defaults();
        let payload = sample_payload();
        let now = Utc::now();
        let id = generator.generate(&payload, now, 0, |_| false);
        assert!(
            id.chars()
                .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())
        );
        assert!(!id.starts_with("swx-"));
    }
}
