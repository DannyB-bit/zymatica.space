use crate::agent_tools::{ToolExecutionResult, ToolRegistry};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Debug, Clone)]
pub struct SpeculativeToolMatch {
    pub tool_name: String,
    pub partial_args: Value,
    pub spec_id: u64,
}

pub struct SpeculativeToolEngine {
    registry: Arc<ToolRegistry>,
    cache: Arc<Mutex<HashMap<u64, ToolExecutionResult>>>,
    next_id: Arc<Mutex<u64>>,
}

impl SpeculativeToolEngine {
    pub fn new(registry: Arc<ToolRegistry>) -> Self {
        Self {
            registry,
            cache: Arc::new(Mutex::new(HashMap::new())),
            next_id: Arc::new(Mutex::new(1)),
        }
    }

    pub fn inspect_streaming_chunk(&self, streaming_buffer: &str) -> Option<SpeculativeToolMatch> {
        // Detect tool invocation signature e.g., {"tool": "read_file", "args": {"path": "..."}}
        let tool_name = self.extract_tool_name(streaming_buffer)?;
        let partial_args = self.extract_args(streaming_buffer)?;
        let spec_id = {
            let mut id = self.next_id.lock().unwrap();
            let current = *id;
            *id += 1;
            current
        };

        let spec_match = SpeculativeToolMatch {
            tool_name: tool_name.clone(),
            partial_args: partial_args.clone(),
            spec_id,
        };

        // Trigger speculative execution in background worker thread
        let registry = Arc::clone(&self.registry);
        let cache = Arc::clone(&self.cache);
        thread::spawn(move || {
            let res = registry.execute(&tool_name, &partial_args);
            if let Ok(mut c) = cache.lock() {
                c.insert(spec_id, res);
            }
        });

        Some(spec_match)
    }

    pub fn claim_speculative_result(&self, spec_id: u64) -> Option<ToolExecutionResult> {
        let mut cache = self.cache.lock().unwrap();
        cache.remove(&spec_id)
    }

    fn extract_tool_name(&self, buf: &str) -> Option<String> {
        for name in &[
            "read_file",
            "write_to_file",
            "terminal",
            "grep_search",
            "list_dir",
        ] {
            if buf.contains(name) {
                return Some(name.to_string());
            }
        }
        None
    }

    fn extract_args(&self, buf: &str) -> Option<Value> {
        let start = buf.find('{')?;
        let end = buf.rfind('}')?;
        if end <= start {
            return None;
        }
        let slice = &buf[start..=end];
        let val = serde_json::from_str::<Value>(slice).ok()?;
        if val.is_object() { Some(val) } else { None }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_speculative_tool_pre_execution() {
        let registry = Arc::new(ToolRegistry::new());
        let engine = SpeculativeToolEngine::new(registry);

        let stream_buf = r#"I will now inspect the file read_file {"path": "Cargo.toml"}"#;
        let matched = engine.inspect_streaming_chunk(stream_buf);
        assert!(matched.is_some());

        let m = matched.unwrap();
        assert_eq!(m.tool_name, "read_file");

        // Wait brief moment for background speculative execution to complete
        std::thread::sleep(std::time::Duration::from_millis(50));

        let res = engine.claim_speculative_result(m.spec_id);
        assert!(res.is_some());
        let res = res.unwrap();
        assert!(res.success);
        assert!(res.output.contains("zymatica-engine"));
    }
}
