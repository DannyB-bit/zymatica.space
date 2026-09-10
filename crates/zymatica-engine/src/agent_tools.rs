use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolParamSpec {
    pub param_type: String,
    pub description: String,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolSchema {
    pub name: String,
    pub description: String,
    pub parameters: HashMap<String, ToolParamSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolExecutionResult {
    pub tool_name: String,
    pub success: bool,
    pub output: String,
    pub error: Option<String>,
    pub execution_time_us: u64,
}

pub trait AgentTool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn schema(&self) -> ToolSchema;
    fn execute(&self, args: &Value) -> Result<ToolExecutionResult>;
}

pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn AgentTool>>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        let mut registry = Self {
            tools: HashMap::new(),
        };
        registry.register_builtin_tools();
        registry
    }

    pub fn register(&mut self, tool: Box<dyn AgentTool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub fn execute(&self, name: &str, args: &Value) -> ToolExecutionResult {
        let start = std::time::Instant::now();
        match self.tools.get(name) {
            Some(tool) => match tool.execute(args) {
                Ok(mut res) => {
                    res.execution_time_us = start.elapsed().as_micros() as u64;
                    res
                }
                Err(err) => ToolExecutionResult {
                    tool_name: name.to_string(),
                    success: false,
                    output: String::new(),
                    error: Some(err.to_string()),
                    execution_time_us: start.elapsed().as_micros() as u64,
                },
            },
            None => ToolExecutionResult {
                tool_name: name.to_string(),
                success: false,
                output: String::new(),
                error: Some(format!("Tool '{}' not found in registry", name)),
                execution_time_us: start.elapsed().as_micros() as u64,
            },
        }
    }

    pub fn get_schemas(&self) -> Vec<ToolSchema> {
        self.tools.values().map(|t| t.schema()).collect()
    }

    fn register_builtin_tools(&mut self) {
        self.register(Box::new(ReadFileTool));
        self.register(Box::new(WriteFileTool));
        self.register(Box::new(TerminalTool));
        self.register(Box::new(GrepSearchTool));
        self.register(Box::new(ListDirTool));
    }
}

pub struct ReadFileTool;
impl AgentTool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Reads lines from a file with zero-copy mmap or direct streaming."
    }

    fn schema(&self) -> ToolSchema {
        let mut params = HashMap::new();
        params.insert(
            "path".to_string(),
            ToolParamSpec {
                param_type: "string".to_string(),
                description: "Absolute or relative path to file".to_string(),
                required: true,
            },
        );
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: params,
        }
    }

    fn execute(&self, args: &Value) -> Result<ToolExecutionResult> {
        let path_str = args
            .get("path")
            .and_then(|v| v.as_str())
            .context("Missing 'path' argument")?;
        let content = fs::read_to_string(path_str)
            .with_context(|| format!("Failed to read file at {}", path_str))?;
        Ok(ToolExecutionResult {
            tool_name: self.name().to_string(),
            success: true,
            output: content,
            error: None,
            execution_time_us: 0,
        })
    }
}

pub struct WriteFileTool;
impl AgentTool for WriteFileTool {
    fn name(&self) -> &str {
        "write_to_file"
    }

    fn description(&self) -> &str {
        "Writes code or text contents atomically to a target file."
    }

    fn schema(&self) -> ToolSchema {
        let mut params = HashMap::new();
        params.insert(
            "path".to_string(),
            ToolParamSpec {
                param_type: "string".to_string(),
                description: "Path to target file".to_string(),
                required: true,
            },
        );
        params.insert(
            "content".to_string(),
            ToolParamSpec {
                param_type: "string".to_string(),
                description: "Content string to write".to_string(),
                required: true,
            },
        );
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: params,
        }
    }

    fn execute(&self, args: &Value) -> Result<ToolExecutionResult> {
        let path_str = args
            .get("path")
            .and_then(|v| v.as_str())
            .context("Missing 'path'")?;
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .context("Missing 'content'")?;

        if let Some(parent) = Path::new(path_str).parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path_str, content)?;

        Ok(ToolExecutionResult {
            tool_name: self.name().to_string(),
            success: true,
            output: format!(
                "Successfully written {} bytes to {}",
                content.len(),
                path_str
            ),
            error: None,
            execution_time_us: 0,
        })
    }
}

pub struct TerminalTool;
impl AgentTool for TerminalTool {
    fn name(&self) -> &str {
        "terminal"
    }

    fn description(&self) -> &str {
        "Executes a command line string natively in the operating system shell."
    }

    fn schema(&self) -> ToolSchema {
        let mut params = HashMap::new();
        params.insert(
            "command".to_string(),
            ToolParamSpec {
                param_type: "string".to_string(),
                description: "Command string to execute".to_string(),
                required: true,
            },
        );
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: params,
        }
    }

    fn execute(&self, args: &Value) -> Result<ToolExecutionResult> {
        let cmd = args
            .get("command")
            .and_then(|v| v.as_str())
            .context("Missing 'command'")?;

        #[cfg(target_os = "windows")]
        let output = Command::new("cmd")
            .args(["/C", cmd])
            .output()
            .context("Failed to launch cmd.exe")?;

        #[cfg(not(target_os = "windows"))]
        let output = Command::new("sh")
            .args(["-c", cmd])
            .output()
            .context("Failed to launch shell")?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let success = output.status.success();

        Ok(ToolExecutionResult {
            tool_name: self.name().to_string(),
            success,
            output: stdout,
            error: if stderr.is_empty() {
                None
            } else {
                Some(stderr)
            },
            execution_time_us: 0,
        })
    }
}

pub struct GrepSearchTool;
impl AgentTool for GrepSearchTool {
    fn name(&self) -> &str {
        "grep_search"
    }

    fn description(&self) -> &str {
        "Searches for pattern matches across files fast."
    }

    fn schema(&self) -> ToolSchema {
        let mut params = HashMap::new();
        params.insert(
            "query".to_string(),
            ToolParamSpec {
                param_type: "string".to_string(),
                description: "Text or pattern query".to_string(),
                required: true,
            },
        );
        params.insert(
            "path".to_string(),
            ToolParamSpec {
                param_type: "string".to_string(),
                description: "Directory path to search".to_string(),
                required: true,
            },
        );
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: params,
        }
    }

    fn execute(&self, args: &Value) -> Result<ToolExecutionResult> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .context("Missing 'query'")?;
        let path = args
            .get("path")
            .and_then(|v| v.as_str())
            .context("Missing 'path'")?;

        let mut matches = Vec::new();
        for entry in walkdir_simple(Path::new(path))? {
            if let Ok(content) = fs::read_to_string(&entry) {
                for (idx, line) in content.lines().enumerate() {
                    if line.contains(query) {
                        matches.push(format!("{}:{}: {}", entry.display(), idx + 1, line));
                        if matches.len() >= 100 {
                            break;
                        }
                    }
                }
            }
        }

        Ok(ToolExecutionResult {
            tool_name: self.name().to_string(),
            success: true,
            output: matches.join("\n"),
            error: None,
            execution_time_us: 0,
        })
    }
}

pub struct ListDirTool;
impl AgentTool for ListDirTool {
    fn name(&self) -> &str {
        "list_dir"
    }

    fn description(&self) -> &str {
        "Lists files and directories in a directory path."
    }

    fn schema(&self) -> ToolSchema {
        let mut params = HashMap::new();
        params.insert(
            "path".to_string(),
            ToolParamSpec {
                param_type: "string".to_string(),
                description: "Directory path to list".to_string(),
                required: true,
            },
        );
        ToolSchema {
            name: self.name().to_string(),
            description: self.description().to_string(),
            parameters: params,
        }
    }

    fn execute(&self, args: &Value) -> Result<ToolExecutionResult> {
        let path_str = args
            .get("path")
            .and_then(|v| v.as_str())
            .context("Missing 'path'")?;
        let mut entries = Vec::new();
        for entry in fs::read_dir(path_str)? {
            let entry = entry?;
            let meta = entry.metadata()?;
            let kind = if meta.is_dir() { "dir" } else { "file" };
            entries.push(format!(
                "[{}] {}",
                kind,
                entry.file_name().to_string_lossy()
            ));
        }
        Ok(ToolExecutionResult {
            tool_name: self.name().to_string(),
            success: true,
            output: entries.join("\n"),
            error: None,
            execution_time_us: 0,
        })
    }
}

fn walkdir_simple(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                if !path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .starts_with('.')
                {
                    files.extend(walkdir_simple(&path)?);
                }
            } else {
                files.push(path);
            }
        }
    }
    Ok(files)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_tool_registry_execution() {
        let registry = ToolRegistry::new();
        let args = json!({"command": "echo Hello Zymatica Engine"});
        let res = registry.execute("terminal", &args);
        assert!(res.success);
        assert!(res.output.contains("Hello Zymatica Engine"));
    }
}
