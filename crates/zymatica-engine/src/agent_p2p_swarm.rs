use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwarmNode {
    pub node_id: String,
    pub address: String,
    pub active_capacity_pct: f32,
    pub cached_sequences: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegatedSwarmTask {
    pub task_id: String,
    pub target_node_id: String,
    pub instruction: String,
    pub payload: serde_json::Value,
}

pub struct P2pSwarmEngine {
    local_node_id: String,
    nodes: HashMap<String, SwarmNode>,
}

impl P2pSwarmEngine {
    pub fn new(local_node_id: &str) -> Self {
        Self {
            local_node_id: local_node_id.to_string(),
            nodes: HashMap::new(),
        }
    }

    pub fn register_node(&mut self, node: SwarmNode) {
        self.nodes.insert(node.node_id.clone(), node);
    }

    pub fn select_best_subagent_node(&self) -> Option<String> {
        self.nodes
            .values()
            .max_by(|a, b| {
                a.active_capacity_pct
                    .partial_cmp(&b.active_capacity_pct)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|n| n.node_id.clone())
    }

    pub fn delegate_task(&self, instruction: &str) -> Result<DelegatedSwarmTask> {
        let target_node = self
            .select_best_subagent_node()
            .unwrap_or_else(|| self.local_node_id.clone());
        Ok(DelegatedSwarmTask {
            task_id: format!("swarm-task-{}", instruction.len()),
            target_node_id: target_node,
            instruction: instruction.to_string(),
            payload: serde_json::json!({}),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_p2p_swarm_delegation() -> Result<()> {
        let mut swarm = P2pSwarmEngine::new("node-local");
        swarm.register_node(SwarmNode {
            node_id: "node-remote-1".to_string(),
            address: "192.168.1.100:9090".to_string(),
            active_capacity_pct: 95.0,
            cached_sequences: vec!["seq-1".to_string()],
        });

        let task = swarm.delegate_task("Run parallel security scan")?;
        assert_eq!(task.target_node_id, "node-remote-1");
        Ok(())
    }
}
