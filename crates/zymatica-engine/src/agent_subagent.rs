use crate::agent_tools::{ToolExecutionResult, ToolRegistry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentTask {
    pub subagent_id: String,
    pub parent_task_id: String,
    pub instruction: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentResult {
    pub subagent_id: String,
    pub success: bool,
    pub output: String,
}

pub struct SubagentOrchestrator {
    tool_registry: Arc<ToolRegistry>,
    active_tasks: Arc<Mutex<HashMap<String, SubagentTask>>>,
    results: Arc<Mutex<HashMap<String, SubagentResult>>>,
}

impl SubagentOrchestrator {
    pub fn new(tool_registry: Arc<ToolRegistry>) -> Self {
        Self {
            tool_registry,
            active_tasks: Arc::new(Mutex::new(HashMap::new())),
            results: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn spawn_subagent(&self, subagent_id: &str, parent_id: &str, instruction: &str) -> String {
        let task = SubagentTask {
            subagent_id: subagent_id.to_string(),
            parent_task_id: parent_id.to_string(),
            instruction: instruction.to_string(),
            status: "RUNNING".to_string(),
        };

        self.active_tasks
            .lock()
            .unwrap()
            .insert(subagent_id.to_string(), task);

        let sub_id = subagent_id.to_string();
        let instr = instruction.to_string();
        let registry = Arc::clone(&self.tool_registry);
        let results = Arc::clone(&self.results);
        let tasks = Arc::clone(&self.active_tasks);

        std::thread::spawn(move || {
            // Execute subagent work pipeline
            let tool_res: ToolExecutionResult = registry.execute(
                "terminal",
                &serde_json::json!({"command": format!("echo Subagent Task: {}", instr)}),
            );

            let res = SubagentResult {
                subagent_id: sub_id.clone(),
                success: tool_res.success,
                output: tool_res.output,
            };

            results.lock().unwrap().insert(sub_id.clone(), res);
            if let Some(t) = tasks.lock().unwrap().get_mut(&sub_id) {
                t.status = "COMPLETED".to_string();
            }
        });

        subagent_id.to_string()
    }

    pub fn get_subagent_result(&self, subagent_id: &str) -> Option<SubagentResult> {
        self.results.lock().unwrap().get(subagent_id).cloned()
    }

    pub fn list_active_tasks(&self) -> Vec<SubagentTask> {
        self.active_tasks
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect()
    }
}

pub struct SubAgentToolAdapter {
    pub orchestrator: Arc<SubagentOrchestrator>,
    pub subagent_name: String,
    pub description: String,
}

impl SubAgentToolAdapter {
    pub fn new(
        orchestrator: Arc<SubagentOrchestrator>,
        name: impl Into<String>,
        description: impl Into<String>,
    ) -> Self {
        Self {
            orchestrator,
            subagent_name: name.into(),
            description: description.into(),
        }
    }

    pub fn as_tool_spec(&self) -> crate::agent_runtime::ToolSpec {
        crate::agent_runtime::ToolSpec {
            name: format!("subagent_{}", self.subagent_name),
            description: self.description.clone(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "instruction": {
                        "type": "string",
                        "description": "Specific task instruction for the subagent"
                    }
                },
                "required": ["instruction"]
            }),
        }
    }

    pub fn invoke_as_tool(
        &self,
        parent_task_id: &str,
        args: &serde_json::Value,
    ) -> anyhow::Result<SubagentResult> {
        let instruction = args
            .get("instruction")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing 'instruction' argument"))?;

        let task_id = format!("{}-{}", self.subagent_name, uuid_short());
        self.orchestrator
            .spawn_subagent(&task_id, parent_task_id, instruction);

        for _ in 0..100 {
            if let Some(res) = self.orchestrator.get_subagent_result(&task_id) {
                return Ok(res);
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        anyhow::bail!("Subagent invocation timed out")
    }
}

fn uuid_short() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!("{:x}", nanos)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subagent_orchestration() {
        let registry = Arc::new(ToolRegistry::new());
        let orchestrator = SubagentOrchestrator::new(registry);

        let sub_id = orchestrator.spawn_subagent("sub-1", "parent-1", "Compile check");
        assert_eq!(sub_id, "sub-1");

        std::thread::sleep(std::time::Duration::from_millis(200));

        let res = orchestrator.get_subagent_result("sub-1");
        assert!(res.is_some());
        assert!(res.unwrap().output.contains("Compile check"));
    }

    #[test]
    fn test_subagent_as_tool_adapter() {
        let registry = Arc::new(ToolRegistry::new());
        let orchestrator = Arc::new(SubagentOrchestrator::new(registry));
        let adapter = SubAgentToolAdapter::new(orchestrator, "auditor", "Audits security bounds");

        let spec = adapter.as_tool_spec();
        assert_eq!(spec.name, "subagent_auditor");

        let res = adapter.invoke_as_tool(
            "parent-100",
            &serde_json::json!({"instruction": "Audit memory"}),
        );
        assert!(res.is_ok());
        assert!(res.unwrap().output.contains("Audit memory"));
    }
}
