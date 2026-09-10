use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EvaluationReport {
    pub faithfulness_score: f32,
    pub context_relevancy_score: f32,
    pub overall_quality_score: f32,
}

pub struct AgentEvaluator;

impl AgentEvaluator {
    pub fn faithfulness_score(response: &str, retrieved_docs: &[&str]) -> f32 {
        if response.trim().is_empty() || retrieved_docs.is_empty() {
            return 0.0;
        }

        let resp_words = extract_significant_words(response);
        if resp_words.is_empty() {
            return 1.0;
        }

        let mut doc_words = HashSet::new();
        for doc in retrieved_docs {
            for word in extract_significant_words(doc) {
                doc_words.insert(word);
            }
        }

        let grounded = resp_words.iter().filter(|w| doc_words.contains(*w)).count();
        grounded as f32 / resp_words.len() as f32
    }

    pub fn context_relevancy_score(query: &str, response: &str) -> f32 {
        let query_words = extract_significant_words(query);
        let resp_words = extract_significant_words(response);

        if query_words.is_empty() {
            return 1.0;
        }

        let matched = query_words
            .iter()
            .filter(|w| resp_words.contains(*w))
            .count();
        matched as f32 / query_words.len() as f32
    }

    pub fn evaluate(query: &str, response: &str, retrieved_docs: &[&str]) -> EvaluationReport {
        let faithfulness = Self::faithfulness_score(response, retrieved_docs);
        let relevancy = Self::context_relevancy_score(query, response);
        let overall = (faithfulness * 0.6) + (relevancy * 0.4);

        EvaluationReport {
            faithfulness_score: faithfulness,
            context_relevancy_score: relevancy,
            overall_quality_score: overall,
        }
    }
}

fn extract_significant_words(text: &str) -> HashSet<String> {
    let mut words = HashSet::new();
    for token in text.split_whitespace() {
        let clean: String = token
            .chars()
            .filter(|c| c.is_alphanumeric())
            .collect::<String>()
            .to_lowercase();
        if clean.len() > 3 {
            words.insert(clean);
        }
    }
    words
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_evaluator_scores() {
        let query = "What is the status of the reservoir water flow?";
        let docs = vec!["Reservoir water flow is 84 percent nominal."];
        let response = "The reservoir water flow status is nominal.";

        let report = AgentEvaluator::evaluate(query, response, &docs);
        assert!(report.faithfulness_score > 0.5);
        assert!(report.context_relevancy_score > 0.5);
        assert!(report.overall_quality_score > 0.5);
    }
}
