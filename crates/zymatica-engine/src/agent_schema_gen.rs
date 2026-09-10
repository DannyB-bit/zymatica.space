use serde_json::{Value, json};

pub struct JsonSchemaGenerator;

impl JsonSchemaGenerator {
    /// Generate an OpenAI-compliant JSON Schema object for tool argument parameters.
    ///
    /// Parameters are provided as slice of tuples: `(name, type_str, description, is_required)`.
    pub fn build_tool_schema(fields: &[(&str, &str, &str, bool)]) -> Value {
        let mut properties = json!({});
        let mut required = Vec::new();

        for &(name, type_str, description, is_req) in fields {
            properties[name] = json!({
                "type": type_str,
                "description": description,
            });
            if is_req {
                required.push(name);
            }
        }

        json!({
            "type": "object",
            "properties": properties,
            "required": required,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_schema_generator_output() {
        let schema = JsonSchemaGenerator::build_tool_schema(&[
            ("command", "string", "Shell command line to execute", true),
            (
                "timeout_ms",
                "integer",
                "Optional execution timeout in milliseconds",
                false,
            ),
        ]);

        assert_eq!(schema["type"], "object");
        assert_eq!(schema["properties"]["command"]["type"], "string");
        assert_eq!(schema["properties"]["timeout_ms"]["type"], "integer");
        assert_eq!(schema["required"], json!(["command"]));
    }
}
