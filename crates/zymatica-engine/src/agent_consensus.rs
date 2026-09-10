use crate::cascade::ModelEndpoint;
use anyhow::{Result, bail};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Debug, Clone, PartialEq)]
pub struct ConsensusResult {
    pub selected_response: String,
    pub agreement_ratio: f32,
    pub votes: usize,
    pub total_participants: usize,
}

pub struct ConsensusEngine;

impl ConsensusEngine {
    pub fn evaluate_consensus<F>(
        endpoints: &[ModelEndpoint],
        generate_fn: F,
    ) -> Result<ConsensusResult>
    where
        F: Fn(&ModelEndpoint) -> Result<String> + Send + Sync + 'static,
    {
        let enabled_endpoints: Vec<ModelEndpoint> =
            endpoints.iter().filter(|e| e.enabled).cloned().collect();
        if enabled_endpoints.is_empty() {
            bail!("No enabled endpoints available for consensus voting");
        }

        let total_participants = enabled_endpoints.len();
        let results = Arc::new(Mutex::new(Vec::<String>::new()));
        let generate_fn = Arc::new(generate_fn);

        let mut handles = Vec::new();
        for ep in enabled_endpoints {
            let gen_fn = Arc::clone(&generate_fn);
            let res_store = Arc::clone(&results);
            let handle = thread::spawn(move || {
                if let Ok(out) = gen_fn(&ep) {
                    res_store.lock().unwrap().push(out);
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            let _ = handle.join();
        }

        let responses = results.lock().unwrap().clone();
        if responses.is_empty() {
            bail!("All model consensus participants failed to generate a response");
        }

        // Group responses by normalized text
        let mut counts: HashMap<String, usize> = HashMap::new();
        for resp in &responses {
            let normalized = resp.trim().to_lowercase();
            *counts.entry(normalized).or_insert(0) += 1;
        }

        let (best_normalized, max_votes) =
            counts.into_iter().max_by_key(|(_, count)| *count).unwrap();

        let winning_response = responses
            .into_iter()
            .find(|r| r.trim().to_lowercase() == best_normalized)
            .unwrap();

        let agreement_ratio = max_votes as f32 / total_participants as f32;

        Ok(ConsensusResult {
            selected_response: winning_response,
            agreement_ratio,
            votes: max_votes,
            total_participants,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cascade::ModelTier;

    #[test]
    fn test_consensus_voting_aggregator() -> Result<()> {
        let endpoints = vec![
            ModelEndpoint {
                name: "gpu-node".to_string(),
                tier: ModelTier::LocalGpu,
                enabled: true,
                max_consecutive_failures: 3,
            },
            ModelEndpoint {
                name: "cpu-node".to_string(),
                tier: ModelTier::LocalCpu,
                enabled: true,
                max_consecutive_failures: 3,
            },
            ModelEndpoint {
                name: "swarm-node".to_string(),
                tier: ModelTier::P2pSwarm,
                enabled: true,
                max_consecutive_failures: 3,
            },
        ];

        let result = ConsensusEngine::evaluate_consensus(&endpoints, |ep| {
            if ep.name == "gpu-node" || ep.name == "cpu-node" {
                Ok("System Status: OPERATIONAL".to_string())
            } else {
                Ok("System Status: UNKNOWN".to_string())
            }
        })?;

        assert_eq!(result.selected_response, "System Status: OPERATIONAL");
        assert_eq!(result.votes, 2);
        assert_eq!(result.total_participants, 3);
        assert!((result.agreement_ratio - 0.666).abs() < 0.01);
        Ok(())
    }
}
