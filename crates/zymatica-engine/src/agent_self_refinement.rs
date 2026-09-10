use anyhow::{Result, bail};

#[derive(Debug, Clone, PartialEq)]
pub struct RefinementResult {
    pub final_response: String,
    pub iterations: usize,
    pub final_quality_score: f32,
}

pub struct SelfRefinementEngine {
    pub max_iterations: usize,
    pub min_quality_threshold: f32,
}

impl Default for SelfRefinementEngine {
    fn default() -> Self {
        Self {
            max_iterations: 3,
            min_quality_threshold: 0.8,
        }
    }
}

impl SelfRefinementEngine {
    pub fn new(max_iterations: usize, min_quality_threshold: f32) -> Self {
        Self {
            max_iterations: max_iterations.max(1),
            min_quality_threshold: min_quality_threshold.clamp(0.0, 1.0),
        }
    }

    pub fn refine<GenFn, EvalFn>(
        &self,
        query: &str,
        mut generate_fn: GenFn,
        mut eval_fn: EvalFn,
    ) -> Result<RefinementResult>
    where
        GenFn: FnMut(&str, Option<&str>) -> Result<String>,
        EvalFn: FnMut(&str, &str) -> f32,
    {
        let mut feedback: Option<String> = None;
        let mut best_response = String::new();
        let mut best_score = 0.0f32;

        for iteration in 1..=self.max_iterations {
            let draft = generate_fn(query, feedback.as_deref())?;
            let score = eval_fn(query, &draft);

            if score > best_score {
                best_score = score;
                best_response = draft.clone();
            }

            if score >= self.min_quality_threshold {
                return Ok(RefinementResult {
                    final_response: draft,
                    iterations: iteration,
                    final_quality_score: score,
                });
            }

            // Generate critique feedback for next refinement iteration
            feedback = Some(format!(
                "Previous draft scored {:.2} below required threshold {:.2}. Please improve accuracy and completeness.",
                score, self.min_quality_threshold
            ));
        }

        if best_response.is_empty() {
            bail!("Self-refinement loop failed to produce a valid response");
        }

        Ok(RefinementResult {
            final_response: best_response,
            iterations: self.max_iterations,
            final_quality_score: best_score,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_self_refinement_improves_quality() -> Result<()> {
        let engine = SelfRefinementEngine::new(3, 0.85);

        let result = engine.refine(
            "Summarize memory status",
            |_query, feedback| {
                if feedback.is_none() {
                    Ok("Short summary".to_string())
                } else {
                    Ok(
                        "Detailed memory status summary: 84MB allocated across 3 pages."
                            .to_string(),
                    )
                }
            },
            |_query, response| {
                if response.contains("Detailed") {
                    0.90
                } else {
                    0.50
                }
            },
        )?;

        assert_eq!(result.iterations, 2);
        assert!(result.final_quality_score >= 0.85);
        assert!(result.final_response.contains("Detailed memory status"));
        Ok(())
    }
}
