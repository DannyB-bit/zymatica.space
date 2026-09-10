use anyhow::Result;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct PromptTemplate {
    template: String,
}

impl PromptTemplate {
    pub fn compile(template: impl Into<String>) -> Result<Self> {
        let t = template.into();
        Ok(Self { template: t })
    }

    pub fn render(&self, params: &Value) -> Result<String> {
        let mut rendered = self.template.clone();

        // Step 1: Render {{#each key}} ... {{/each}} blocks
        while let Some(start_idx) = rendered.find("{{#each ") {
            let rest = &rendered[start_idx + 8..];
            let key_end = rest
                .find("}}")
                .ok_or_else(|| anyhow::anyhow!("Unclosed {{#each}} tag"))?;
            let array_key = rest[..key_end].trim();

            let block_start = start_idx + 8 + key_end + 2;
            let end_tag = "{{/each}}";
            let block_end = rendered[block_start..]
                .find(end_tag)
                .map(|i| block_start + i)
                .ok_or_else(|| anyhow::anyhow!("Missing {{/each}} tag for {}", array_key))?;

            let inner_template = &rendered[block_start..block_end];

            let mut iterated_output = String::new();
            if let Some(arr) = params.get(array_key).and_then(|v| v.as_array()) {
                for item in arr {
                    let item_str = match item {
                        Value::String(s) => s.clone(),
                        v => v.to_string(),
                    };
                    let substituted = inner_template.replace("{{this}}", &item_str);
                    iterated_output.push_str(&substituted);
                }
            }

            rendered.replace_range(start_idx..(block_end + end_tag.len()), &iterated_output);
        }

        // Step 2: Render simple {{var}} placeholders
        while let Some(start_idx) = rendered.find("{{") {
            let rest = &rendered[start_idx + 2..];
            let end_idx = rest
                .find("}}")
                .ok_or_else(|| anyhow::anyhow!("Unclosed placeholder"))?;
            let var_name = rest[..end_idx].trim();

            let replacement = match params.get(var_name) {
                Some(Value::String(s)) => s.clone(),
                Some(v) => v.to_string(),
                None => format!("{{{{{}}}}}", var_name), // Leave unreplaced if missing
            };

            rendered.replace_range(start_idx..(start_idx + 2 + end_idx + 2), &replacement);
        }

        Ok(rendered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_prompt_template_basic_substitution() -> Result<()> {
        let template = PromptTemplate::compile("Hello {{name}}, system is {{status}}!")?;
        let output = template.render(&json!({
            "name": "Alice",
            "status": "OPERATIONAL"
        }))?;
        assert_eq!(output, "Hello Alice, system is OPERATIONAL!");
        Ok(())
    }

    #[test]
    fn test_prompt_template_array_iteration() -> Result<()> {
        let template =
            PromptTemplate::compile("Context:\n{{#each docs}}- {{this}}\n{{/each}}Done.")?;
        let output = template.render(&json!({
            "docs": ["First doc", "Second doc"]
        }))?;
        assert_eq!(output, "Context:\n- First doc\n- Second doc\nDone.");
        Ok(())
    }
}
