use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MoeLayerConfig {
    pub total_experts: usize,
    pub active_experts_per_token: usize,
    pub expert_size_bytes: usize,
    pub dense_layer_size_bytes: usize,
}

#[derive(Debug, Clone)]
pub struct ExpertSlice {
    pub expert_id: usize,
    pub data: Vec<u8>,
    pub loaded_from_disk: bool,
}

pub struct MoeStreamEngine {
    config: MoeLayerConfig,
    #[allow(dead_code)]
    weights_path: PathBuf,
    expert_ram_cache: HashMap<usize, Vec<u8>>,
}

impl MoeStreamEngine {
    pub fn new(config: MoeLayerConfig, weights_path: PathBuf) -> Self {
        Self {
            config,
            weights_path,
            expert_ram_cache: HashMap::new(),
        }
    }

    pub fn route_top_k(&self, router_logits: &[f32]) -> Vec<usize> {
        let mut indexed: Vec<(usize, f32)> = router_logits.iter().cloned().enumerate().collect();
        indexed.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        indexed
            .iter()
            .take(self.config.active_experts_per_token)
            .map(|(idx, _)| *idx)
            .collect()
    }

    pub fn load_expert(&mut self, expert_id: usize) -> Result<ExpertSlice> {
        if let Some(cached) = self.expert_ram_cache.get(&expert_id) {
            return Ok(ExpertSlice {
                expert_id,
                data: cached.clone(),
                loaded_from_disk: false,
            });
        }

        // Zero-copy simulation of streaming expert from NVMe SSD on demand
        let dummy_data = vec![0u8; self.config.expert_size_bytes.min(1024)];
        self.expert_ram_cache.insert(expert_id, dummy_data.clone());

        Ok(ExpertSlice {
            expert_id,
            data: dummy_data,
            loaded_from_disk: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_colibri_moe_routing_and_streaming() -> Result<()> {
        let config = MoeLayerConfig {
            total_experts: 64,
            active_experts_per_token: 2,
            expert_size_bytes: 4096,
            dense_layer_size_bytes: 16384,
        };

        let mut engine = MoeStreamEngine::new(config, PathBuf::from("moe_weights.bin"));
        let logits = vec![0.1, 0.8, 0.3, 0.95, 0.05];
        let top_k = engine.route_top_k(&logits);

        assert_eq!(top_k.len(), 2);
        assert_eq!(top_k[0], 3); // Highest logit (0.95)
        assert_eq!(top_k[1], 1); // Second highest logit (0.8)

        let slice = engine.load_expert(top_k[0])?;
        assert_eq!(slice.expert_id, 3);
        assert!(slice.loaded_from_disk);

        // Second load hits RAM cache
        let slice2 = engine.load_expert(top_k[0])?;
        assert!(!slice2.loaded_from_disk);
        Ok(())
    }
}
