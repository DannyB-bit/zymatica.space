use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompressedTrajectory {
    pub retained_messages: Vec<ChatMessage>,
    pub summary_prefix: Option<String>,
    pub tokens_saved: usize,
}

pub struct TrajectoryCompressor {
    max_token_budget: usize,
}

impl TrajectoryCompressor {
    pub fn new(max_token_budget: usize) -> Self {
        Self { max_token_budget }
    }

    pub fn compress(&self, history: &[ChatMessage]) -> CompressedTrajectory {
        let total_tokens: usize = history.iter().map(|m| estimate_tokens(&m.content)).sum();

        if total_tokens <= self.max_token_budget {
            return CompressedTrajectory {
                retained_messages: history.to_vec(),
                summary_prefix: None,
                tokens_saved: 0,
            };
        }

        // Retain system prompt (first message if system) + latest N turns to preserve prompt caching
        let mut retained = Vec::new();
        let mut current_tokens = 0;

        let has_system = history.first().is_some_and(|m| m.role == "system");
        if has_system {
            retained.push(history[0].clone());
            current_tokens += estimate_tokens(&history[0].content);
        }

        let start_idx = if has_system { 1 } else { 0 };
        let mut tail = Vec::new();

        for msg in history[start_idx..].iter().rev() {
            let tokens = estimate_tokens(&msg.content);
            if current_tokens + tokens <= self.max_token_budget {
                tail.push(msg.clone());
                current_tokens += tokens;
            } else {
                break;
            }
        }
        tail.reverse();
        retained.extend(tail);

        let saved = total_tokens.saturating_sub(current_tokens);
        let summary = format!(
            "[Context Compressed: {} tokens saved from older turns]",
            saved
        );

        CompressedTrajectory {
            retained_messages: retained,
            summary_prefix: Some(summary),
            tokens_saved: saved,
        }
    }
}

fn estimate_tokens(text: &str) -> usize {
    // Fast estimation heuristic: ~4 chars per token
    text.len().div_ceil(4)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trajectory_compression() {
        let compressor = TrajectoryCompressor::new(50);
        let history = vec![
            ChatMessage {
                role: "system".to_string(),
                content: "System prompt".to_string(),
            },
            ChatMessage {
                role: "user".to_string(),
                content: "Long message 1 ".repeat(20),
            },
            ChatMessage {
                role: "assistant".to_string(),
                content: "Recent response".to_string(),
            },
        ];

        let compressed = compressor.compress(&history);
        assert!(compressed.tokens_saved > 0);
        assert!(compressed.retained_messages.len() < history.len());
        assert_eq!(compressed.retained_messages[0].role, "system");
    }
}
