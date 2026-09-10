use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A single turn in a conversation trajectory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TurnRecord {
    pub role: String,
    pub content: String,
    pub tool_calls: Option<Vec<serde_json::Value>>,
}

/// A preference pair: one chosen (successful) trajectory vs one rejected (failed) trajectory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreferencePair {
    pub prompt: String,
    pub chosen: Vec<TurnRecord>,
    pub rejected: Vec<TurnRecord>,
    pub metadata: PairMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairMetadata {
    pub task_id: String,
    pub model: String,
    pub chosen_score: f32,
    pub rejected_score: f32,
}

/// Collects DPO preference pairs from agent conversation trajectories.
#[derive(Default)]
pub struct DpoCollector {
    pairs: Vec<PreferencePair>,
}

impl DpoCollector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a preference pair from a successful vs failed attempt at the same task.
    #[allow(clippy::too_many_arguments)]
    pub fn record_pair(
        &mut self,
        prompt: &str,
        chosen_trajectory: Vec<TurnRecord>,
        rejected_trajectory: Vec<TurnRecord>,
        task_id: &str,
        model: &str,
        chosen_score: f32,
        rejected_score: f32,
    ) {
        self.pairs.push(PreferencePair {
            prompt: prompt.to_string(),
            chosen: chosen_trajectory,
            rejected: rejected_trajectory,
            metadata: PairMetadata {
                task_id: task_id.to_string(),
                model: model.to_string(),
                chosen_score,
                rejected_score,
            },
        });
    }

    pub fn record_guardrail_failure(
        &mut self,
        prompt: &str,
        rejected_response: &str,
        chosen_response: &str,
        guardrail_name: &str,
    ) {
        self.record_pair(
            prompt,
            vec![TurnRecord {
                role: "assistant".to_string(),
                content: chosen_response.to_string(),
                tool_calls: None,
            }],
            vec![TurnRecord {
                role: "assistant".to_string(),
                content: rejected_response.to_string(),
                tool_calls: None,
            }],
            &format!("guardrail-{}", guardrail_name),
            "native-zymatica-engine",
            1.0,
            0.0,
        );
    }

    /// Export all collected pairs as JSONL (one JSON object per line) for DPO fine-tuning.
    pub fn export_jsonl(&self, output_path: &Path) -> Result<usize> {
        use std::io::Write;
        let file = std::fs::File::create(output_path)?;
        let mut writer = std::io::BufWriter::new(file);
        let mut count = 0;
        for pair in &self.pairs {
            let line = serde_json::to_string(pair)?;
            writeln!(writer, "{}", line)?;
            count += 1;
        }
        writer.flush()?;
        Ok(count)
    }

    pub fn pair_count(&self) -> usize {
        self.pairs.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dpo_collector_records_and_exports() {
        let mut collector = DpoCollector::new();

        let chosen = vec![TurnRecord {
            role: "assistant".to_string(),
            content: "Fixed the bug by adding bounds check.".to_string(),
            tool_calls: None,
        }];
        let rejected = vec![TurnRecord {
            role: "assistant".to_string(),
            content: "I'm not sure what's wrong.".to_string(),
            tool_calls: None,
        }];

        collector.record_pair(
            "Fix the off-by-one error in parser.rs",
            chosen,
            rejected,
            "task-001",
            "gemma-4-e2b",
            0.95,
            0.30,
        );

        assert_eq!(collector.pair_count(), 1);

        let tmp = std::env::temp_dir().join("zymatica_dpo_test.jsonl");
        let exported = collector.export_jsonl(&tmp).unwrap();
        assert_eq!(exported, 1);

        let content = std::fs::read_to_string(&tmp).unwrap();
        assert!(content.contains("Fix the off-by-one error"));
        assert!(content.contains("\"chosen_score\":0.95"));
        let _ = std::fs::remove_file(&tmp);
    }
}
