use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SkillMutation {
    pub variant_id: String,
    pub original_skill_name: String,
    pub prompt_text: String,
    pub accuracy_score: f32,
    pub token_cost: usize,
}

impl SkillMutation {
    /// True Pareto Dominance check:
    /// Returns true if `self` dominates `other` (i.e. >= accuracy AND <= token_cost, with at least one strict improvement).
    pub fn dominates(&self, other: &Self) -> bool {
        (self.accuracy_score >= other.accuracy_score && self.token_cost <= other.token_cost)
            && (self.accuracy_score > other.accuracy_score || self.token_cost < other.token_cost)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ParetoFrontier {
    pub pareto_variants: Vec<SkillMutation>,
}

impl ParetoFrontier {
    /// Update frontier with a new candidate by removing any candidates dominated by it,
    /// and adding the candidate if it is not dominated by any existing frontier member.
    pub fn update(&mut self, candidate: SkillMutation) {
        // Check if candidate is dominated by any existing member
        let is_dominated = self.pareto_variants.iter().any(|v| v.dominates(&candidate));
        if is_dominated {
            return;
        }

        // Remove existing variants dominated by this new candidate
        self.pareto_variants.retain(|v| !candidate.dominates(v));
        self.pareto_variants.push(candidate);

        // Sort frontier by highest accuracy then lowest cost
        self.pareto_variants.sort_by(|a, b| {
            b.accuracy_score
                .partial_cmp(&a.accuracy_score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.token_cost.cmp(&b.token_cost))
        });
    }
}

pub struct GeneticSkillEvolver {
    pub population: Vec<SkillMutation>,
    pub frontier: ParetoFrontier,
}

impl Default for GeneticSkillEvolver {
    fn default() -> Self {
        Self::new()
    }
}

impl GeneticSkillEvolver {
    pub fn new() -> Self {
        Self {
            population: Vec::new(),
            frontier: ParetoFrontier::default(),
        }
    }

    pub fn mutate_prompt(
        &self,
        original_name: &str,
        base_prompt: &str,
        mutation_idx: usize,
    ) -> SkillMutation {
        let mutated = format!(
            "{}\n\n[GEPA Optimization Directive v{}]: Be extremely precise, verify code invariants, and minimize unnecessary turns.",
            base_prompt, mutation_idx
        );

        SkillMutation {
            variant_id: format!("{}-mut-{}", original_name, mutation_idx),
            original_skill_name: original_name.to_string(),
            prompt_text: mutated,
            accuracy_score: 0.95 + (mutation_idx as f32 * 0.01),
            token_cost: base_prompt.len() + 80,
        }
    }

    pub fn evaluate_and_update_pareto(
        &mut self,
        mut mut_variant: SkillMutation,
        benchmark_accuracy: f32,
    ) -> &ParetoFrontier {
        mut_variant.accuracy_score = benchmark_accuracy;
        self.population.push(mut_variant.clone());
        self.frontier.update(mut_variant);
        &self.frontier
    }

    pub fn get_best_mutation(&self) -> Option<&SkillMutation> {
        self.frontier.pareto_variants.first()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gepa_evolution_pareto_dominance() {
        let mut evolver = GeneticSkillEvolver::new();
        let mut1 = SkillMutation {
            variant_id: "mut-1".to_string(),
            original_skill_name: "code-review".to_string(),
            prompt_text: "p1".to_string(),
            accuracy_score: 0.90,
            token_cost: 100,
        };
        let mut2 = SkillMutation {
            variant_id: "mut-2".to_string(),
            original_skill_name: "code-review".to_string(),
            prompt_text: "p2".to_string(),
            accuracy_score: 0.95,
            token_cost: 90, // Dominates mut1 (higher accuracy AND lower cost)
        };
        let mut3 = SkillMutation {
            variant_id: "mut-3".to_string(),
            original_skill_name: "code-review".to_string(),
            prompt_text: "p3".to_string(),
            accuracy_score: 0.98,
            token_cost: 120, // Non-dominated (higher accuracy, higher cost)
        };

        evolver.evaluate_and_update_pareto(mut1, 0.90);
        assert_eq!(evolver.frontier.pareto_variants.len(), 1);

        evolver.evaluate_and_update_pareto(mut2, 0.95);
        // mut2 dominates mut1, so mut1 is pruned from frontier
        assert_eq!(evolver.frontier.pareto_variants.len(), 1);
        assert_eq!(evolver.frontier.pareto_variants[0].variant_id, "mut-2");

        evolver.evaluate_and_update_pareto(mut3, 0.98);
        // mut2 and mut3 are both non-dominated (trade-off)
        assert_eq!(evolver.frontier.pareto_variants.len(), 2);
        assert_eq!(evolver.get_best_mutation().unwrap().variant_id, "mut-3");
    }
}
