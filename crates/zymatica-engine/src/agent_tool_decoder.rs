use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A parsed Zymatica tool call extracted from a streaming token buffer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZymaticaToolCall {
    pub name: String,
    pub arguments: Value,
}

/// Zymatica native `<tool_call>` XML/JSON token stream decoder.
pub struct ZymaticaToolDecoder;

impl ZymaticaToolDecoder {
    /// Parse a `<tool_call>{...}</tool_call>` block from a streaming LLM output buffer.
    pub fn parse_tool_call(stream_buffer: &str) -> Option<ZymaticaToolCall> {
        let (start_idx, end_idx) =
            crate::agent_simd_tokenizer::SpecialTokenMatcher::find_tool_call_bounds(stream_buffer)?;
        let json_slice = stream_buffer[start_idx..end_idx].trim();
        let parsed = serde_json::from_str::<Value>(json_slice).ok()?;
        let name = parsed.get("name").and_then(|v| v.as_str())?;
        let arguments = parsed.get("arguments").cloned().unwrap_or(Value::Null);
        Some(ZymaticaToolCall {
            name: name.to_string(),
            arguments,
        })
    }

    /// Extract partial tool name from an incomplete streaming buffer before generation completes.
    pub fn parse_partial_tool_name(stream_buffer: &str) -> Option<String> {
        let tag_start = "<tool_call>";
        let start_idx = stream_buffer.find(tag_start)?;
        let slice = &stream_buffer[start_idx + tag_start.len()..];
        let name_idx = slice.find("\"name\"")?;
        let after_name = &slice[name_idx + 6..];
        let colon_idx = after_name.find(':')?;
        let val_slice = after_name[colon_idx + 1..].trim_start();
        if let Some(name_content) = val_slice.strip_prefix('"') {
            let end_quote = name_content.find('"')?;
            Some(name_content[..end_quote].to_string())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zymatica_xml_tool_call_parsing() {
        let stream = r#"I will call the tool now. <tool_call>{"name": "terminal", "arguments": {"command": "cargo check"}}</tool_call> Done."#;
        let call = ZymaticaToolDecoder::parse_tool_call(stream);
        assert!(call.is_some());
        let c = call.unwrap();
        assert_eq!(c.name, "terminal");
        assert_eq!(c.arguments["command"], "cargo check");
    }

    #[test]
    fn test_partial_tool_name_decoding() {
        let partial_stream =
            r#"Executing task... <tool_call>{"name": "solar_power_monitor", "arguments": {"range"#;
        let partial_name = ZymaticaToolDecoder::parse_partial_tool_name(partial_stream);
        assert_eq!(partial_name, Some("solar_power_monitor".to_string()));
    }
}
