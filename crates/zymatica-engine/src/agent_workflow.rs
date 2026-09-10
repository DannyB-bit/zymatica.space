use anyhow::{Result, bail};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct WorkflowContext {
    pub state: HashMap<String, Value>,
}

pub trait WorkflowStep: Send + Sync {
    fn name(&self) -> &str;
    fn execute(&self, ctx: &mut WorkflowContext) -> Result<String>;
}

pub enum WorkflowEdge {
    Next(String),
    Conditional(Box<dyn Fn(&WorkflowContext) -> String + Send + Sync>),
    Finish,
}

pub struct AgentGraph {
    steps: HashMap<String, Box<dyn WorkflowStep>>,
    edges: HashMap<String, WorkflowEdge>,
}

impl Default for AgentGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentGraph {
    pub fn new() -> Self {
        Self {
            steps: HashMap::new(),
            edges: HashMap::new(),
        }
    }

    pub fn add_step(&mut self, step: Box<dyn WorkflowStep>, edge: WorkflowEdge) {
        let name = step.name().to_string();
        self.steps.insert(name.clone(), step);
        self.edges.insert(name, edge);
    }

    pub fn execute(&self, start_node: &str, ctx: &mut WorkflowContext) -> Result<Vec<String>> {
        let mut current_node = start_node.to_string();
        let mut executed_path = Vec::new();
        let mut iterations = 0;

        while iterations < 100 {
            let step = self
                .steps
                .get(&current_node)
                .ok_or_else(|| anyhow::anyhow!("Workflow step '{}' not found", current_node))?;

            let output = step.execute(ctx)?;
            executed_path.push(format!("{}: {}", current_node, output));

            let edge = self
                .edges
                .get(&current_node)
                .ok_or_else(|| anyhow::anyhow!("Workflow edge for '{}' not found", current_node))?;

            match edge {
                WorkflowEdge::Next(next_name) => {
                    current_node = next_name.clone();
                }
                WorkflowEdge::Conditional(cond_fn) => {
                    current_node = cond_fn(ctx);
                }
                WorkflowEdge::Finish => {
                    break;
                }
            }

            iterations += 1;
        }

        if iterations >= 100 {
            bail!("Workflow exceeded maximum iteration limit of 100");
        }

        Ok(executed_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    struct IngestionStep;
    impl WorkflowStep for IngestionStep {
        fn name(&self) -> &str {
            "ingestion"
        }
        fn execute(&self, ctx: &mut WorkflowContext) -> Result<String> {
            ctx.state.insert("data_valid".to_string(), json!(true));
            Ok("Data ingested successfully".to_string())
        }
    }

    struct ProcessingStep;
    impl WorkflowStep for ProcessingStep {
        fn name(&self) -> &str {
            "processing"
        }
        fn execute(&self, _ctx: &mut WorkflowContext) -> Result<String> {
            Ok("Processed data".to_string())
        }
    }

    #[test]
    fn test_agent_graph_dag_execution() -> Result<()> {
        let mut graph = AgentGraph::new();
        graph.add_step(
            Box::new(IngestionStep),
            WorkflowEdge::Conditional(Box::new(|ctx| {
                if ctx
                    .state
                    .get("data_valid")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
                {
                    "processing".to_string()
                } else {
                    "finish".to_string()
                }
            })),
        );
        graph.add_step(Box::new(ProcessingStep), WorkflowEdge::Finish);

        let mut ctx = WorkflowContext::default();
        let path = graph.execute("ingestion", &mut ctx)?;

        assert_eq!(path.len(), 2);
        assert!(path[0].contains("ingestion"));
        assert!(path[1].contains("processing"));
        Ok(())
    }
}
