use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    pub params: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    pub id: u64,
    pub result: Option<Value>,
    pub error: Option<Value>,
}

pub struct McpClientEngine {
    #[allow(dead_code)]
    pub server_name: String,
}

impl McpClientEngine {
    pub fn new(server_name: &str) -> Self {
        Self {
            server_name: server_name.to_string(),
        }
    }

    pub fn build_initialize_request(&self, req_id: u64) -> McpRequest {
        McpRequest {
            jsonrpc: "2.0".to_string(),
            id: req_id,
            method: "initialize".to_string(),
            params: serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {
                    "name": "Zymatica-Engine",
                    "version": "0.2.0"
                }
            }),
        }
    }

    pub fn build_tool_call_request(&self, req_id: u64, tool_name: &str, args: Value) -> McpRequest {
        McpRequest {
            jsonrpc: "2.0".to_string(),
            id: req_id,
            method: "tools/call".to_string(),
            params: serde_json::json!({
                "name": tool_name,
                "arguments": args
            }),
        }
    }

    pub fn handle_response(&self, raw_json: &str) -> Result<Value> {
        let resp: McpResponse =
            serde_json::from_str(raw_json).context("Failed to parse MCP JSON-RPC response")?;
        if let Some(err) = resp.error {
            anyhow::bail!("MCP Server Error: {:?}", err);
        }
        resp.result.context("Missing result in MCP response")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_client_req_resp() -> Result<()> {
        let client = McpClientEngine::new("test-server");
        let req = client.build_initialize_request(1);
        assert_eq!(req.method, "initialize");

        let json_resp = r#"{"jsonrpc":"2.0","id":1,"result":{"status":"initialized"}}"#;
        let res = client.handle_response(json_resp)?;
        assert_eq!(res["status"], "initialized");
        Ok(())
    }
}
