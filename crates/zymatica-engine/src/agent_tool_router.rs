use crate::agent_runtime::ToolSpec;
use crate::concept_rag::project_text_to_concept;
use crate::cuneiform::Concept6D;

pub struct ToolConceptEntry {
    pub spec: ToolSpec,
    pub concept: Concept6D,
}

pub struct ConceptToolRouter {
    tools: Vec<ToolConceptEntry>,
}

impl Default for ConceptToolRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl ConceptToolRouter {
    pub fn new() -> Self {
        Self { tools: Vec::new() }
    }

    pub fn register_tool(&mut self, spec: ToolSpec) {
        let text_representation = format!("{} {}", spec.name, spec.description);
        let concept = project_text_to_concept(&text_representation);
        self.tools.push(ToolConceptEntry { spec, concept });
    }

    pub fn route_tools(&self, user_prompt: &str, top_k: usize) -> Vec<ToolSpec> {
        if self.tools.is_empty() || top_k == 0 {
            return Vec::new();
        }

        let prompt_concept = project_text_to_concept(user_prompt);
        let mut scored: Vec<(&ToolSpec, u32)> = self
            .tools
            .iter()
            .map(|t| {
                let dist = prompt_concept.manhattan_distance(t.concept);
                (&t.spec, dist)
            })
            .collect();

        scored.sort_by_key(|(_, dist)| *dist);

        scored
            .into_iter()
            .take(top_k)
            .map(|(spec, _)| spec.clone())
            .collect()
    }

    pub fn len(&self) -> usize {
        self.tools.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tools.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_concept_tool_router_filters_relevant_tools() {
        let mut router = ConceptToolRouter::new();

        router.register_tool(ToolSpec {
            name: "solar_power_monitor".to_string(),
            description: "Monitors solar panel array power, battery grid load, and voltage"
                .to_string(),
            input_schema: json!({}),
        });

        router.register_tool(ToolSpec {
            name: "water_flow_sensor".to_string(),
            description: "Reads reservoir water levels, pump speeds, and fluid pressure"
                .to_string(),
            input_schema: json!({}),
        });

        router.register_tool(ToolSpec {
            name: "json_validator".to_string(),
            description: "Validates JSON object schema fields and types".to_string(),
            input_schema: json!({}),
        });

        assert_eq!(router.len(), 3);

        let routed = router.route_tools("check solar battery levels", 1);
        assert_eq!(routed.len(), 1);
        assert_eq!(routed[0].name, "solar_power_monitor");

        let routed_water = router.route_tools("reservoir water level status", 1);
        assert_eq!(routed_water.len(), 1);
        assert_eq!(routed_water[0].name, "water_flow_sensor");
    }
}
