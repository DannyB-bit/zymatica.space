use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryRecord {
    pub id: String,
    pub session_id: String,
    pub content: String,
    pub embedding: Vec<f32>,
    pub timestamp_ms: u64,
    pub importance_score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DialecticUserProfile {
    pub user_id: String,
    pub preferences: HashMap<String, String>,
    pub key_facts: Vec<String>,
    pub interaction_style: String,
}

pub struct MemoryStore {
    records: HashMap<String, MemoryRecord>,
    profiles: HashMap<String, DialecticUserProfile>,
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryStore {
    pub fn new() -> Self {
        Self {
            records: HashMap::new(),
            profiles: HashMap::new(),
        }
    }

    pub fn insert_memory(&mut self, record: MemoryRecord) {
        self.records.insert(record.id.clone(), record);
    }

    pub fn search_fts(&self, query: &str) -> Vec<&MemoryRecord> {
        let q_lower = query.to_lowercase();
        self.records
            .values()
            .filter(|r| r.content.to_lowercase().contains(&q_lower))
            .collect()
    }

    pub fn search_vector(
        &self,
        target_embedding: &[f32],
        limit: usize,
    ) -> Vec<(&MemoryRecord, f32)> {
        let mut scored: Vec<(&MemoryRecord, f32)> = self
            .records
            .values()
            .map(|r| {
                let sim = cosine_similarity_simd(&r.embedding, target_embedding);
                (r, sim)
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(limit);
        scored
    }

    pub fn update_user_profile(&mut self, user_id: &str, fact: &str) {
        let profile =
            self.profiles
                .entry(user_id.to_string())
                .or_insert_with(|| DialecticUserProfile {
                    user_id: user_id.to_string(),
                    preferences: HashMap::new(),
                    key_facts: vec![],
                    interaction_style: "Direct".to_string(),
                });
        if !profile.key_facts.contains(&fact.to_string()) {
            profile.key_facts.push(fact.to_string());
        }
    }

    pub fn get_profile(&self, user_id: &str) -> Option<&DialecticUserProfile> {
        self.profiles.get(user_id)
    }

    pub fn generate_memory_nudge(&self, user_id: &str) -> Option<String> {
        self.get_profile(user_id).map(|p| {
            format!(
                "### Memory Context for {}\nKey User Facts:\n- {}",
                p.user_id,
                p.key_facts.join("\n- ")
            )
        })
    }
}

fn cosine_similarity_simd(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut norm_a = 0.0f32;
    let mut norm_b = 0.0f32;

    for i in 0..a.len() {
        dot += a[i] * b[i];
        norm_a += a[i] * a[i];
        norm_b += b[i] * b[i];
    }

    if norm_a == 0.0 || norm_b == 0.0 {
        0.0
    } else {
        dot / (norm_a.sqrt() * norm_b.sqrt())
    }
}

// ============================================================================
// Token-Budget Sliding Window Memory
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct TokenWindowChatMemory {
    pub max_token_budget: usize,
    pub messages: Vec<ChatMessage>,
}

impl TokenWindowChatMemory {
    pub fn new(max_token_budget: usize) -> Self {
        Self {
            max_token_budget,
            messages: Vec::new(),
        }
    }

    pub fn add_message(&mut self, role: impl Into<String>, content: impl Into<String>) {
        self.messages.push(ChatMessage {
            role: role.into(),
            content: content.into(),
        });
        self.prune_to_budget();
    }

    pub fn total_tokens(&self) -> usize {
        self.messages
            .iter()
            .map(|m| estimate_tokens(&m.content))
            .sum()
    }

    pub fn prune_to_budget(&mut self) {
        if self.total_tokens() <= self.max_token_budget || self.messages.len() <= 1 {
            return;
        }

        // Separate system message if present at index 0
        let has_system = self.messages.first().is_some_and(|m| m.role == "system");

        while self.total_tokens() > self.max_token_budget {
            let remove_idx = if has_system { 1 } else { 0 };
            if remove_idx >= self.messages.len() {
                break;
            }
            self.messages.remove(remove_idx);
        }
    }
}

fn estimate_tokens(text: &str) -> usize {
    crate::agent_simd_tokenizer::FastTokenCounter::count_tokens(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_store_fts_and_vector() {
        let mut store = MemoryStore::new();
        store.insert_memory(MemoryRecord {
            id: "mem-1".to_string(),
            session_id: "sess-1".to_string(),
            content: "User prefers Rust language for performance".to_string(),
            embedding: vec![1.0, 0.0, 0.0],
            timestamp_ms: 100,
            importance_score: 0.9,
        });

        let fts_res = store.search_fts("Rust");
        assert_eq!(fts_res.len(), 1);

        let vec_res = store.search_vector(&[0.9, 0.1, 0.0], 1);
        assert_eq!(vec_res.len(), 1);
        assert!(vec_res[0].1 > 0.8);

        store.update_user_profile("user-1", "Prefers Rust over Python");
        let nudge = store.generate_memory_nudge("user-1").unwrap();
        assert!(nudge.contains("Prefers Rust over Python"));
    }

    #[test]
    fn test_token_window_chat_memory() {
        let mut memory = TokenWindowChatMemory::new(10);
        memory.add_message("system", "System prompt");
        memory.add_message("user", "First long user message one two three four");
        memory.add_message("assistant", "Assistant response message one two three");
        memory.add_message("user", "Second user message");

        assert_eq!(memory.messages[0].role, "system");
        assert!(memory.total_tokens() <= 10);
    }
}
