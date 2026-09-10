// Copyright © 2026 Zymatica
// SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
// See LICENSE for terms.

//! ==============================================================================
//! ZYMATICA CLASS 35: Z-MCTS — Continuous Latent Test-Time Search
//! ==============================================================================
//!
//! The search core is evaluator-agnostic. A real model/verifier can provide both value and
//! policy-prior signals through `LatentEvaluator`. The previous geometric goal search remains
//! available as a deterministic reference evaluator and is not presented as LLM reasoning.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LatentState8D {
    pub coords: [f32; 8],
}

impl LatentState8D {
    pub const fn new(coords: [f32; 8]) -> Self {
        Self { coords }
    }

    pub fn distance_to(&self, other: &Self) -> f32 {
        const WEIGHTS: [f32; 8] = [1.0, 1.0, 0.75, 0.75, 0.5, 0.5, 0.25, 0.25];
        self.coords
            .iter()
            .zip(&other.coords)
            .zip(WEIGHTS)
            .map(|((&lhs, &rhs), weight)| {
                let delta = lhs - rhs;
                weight * delta * delta
            })
            .sum::<f32>()
            .sqrt()
    }

    pub fn step(&self, velocity: &[f32; 8], dt: f32) -> Self {
        let mut next = [0.0f32; 8];
        for ((dst, &position), &speed) in next.iter_mut().zip(&self.coords).zip(velocity) {
            *dst = (position + speed * dt).clamp(0.0, 15.0);
        }
        Self::new(next)
    }
}

#[derive(Debug, Clone)]
pub struct LatentMctsNode {
    pub state: LatentState8D,
    pub parent_idx: Option<usize>,
    pub children_indices: Vec<usize>,
    pub action_taken: [f32; 8],
    pub visit_count: u32,
    pub total_value: f32,
    pub prior_prob: f32,
    pub depth: usize,
}

impl LatentMctsNode {
    pub fn new(
        state: LatentState8D,
        parent: Option<usize>,
        action: [f32; 8],
        prior: f32,
        depth: usize,
    ) -> Self {
        Self {
            state,
            parent_idx: parent,
            children_indices: Vec::new(),
            action_taken: action,
            visit_count: 0,
            total_value: 0.0,
            prior_prob: prior.max(0.0),
            depth,
        }
    }

    pub fn q_value(&self) -> f32 {
        if self.visit_count == 0 {
            0.0
        } else {
            self.total_value / self.visit_count as f32
        }
    }
}

#[derive(Debug, Clone)]
pub struct ZMctsConfig {
    pub num_simulations: usize,
    pub max_depth: usize,
    pub c_puct: f32,
    pub branch_factor: usize,
    pub step_dt: f32,
    pub curvature_penalty: f32,
}

impl Default for ZMctsConfig {
    fn default() -> Self {
        Self {
            num_simulations: 250,
            max_depth: 8,
            c_puct: std::f32::consts::SQRT_2,
            branch_factor: 8,
            step_dt: 0.25,
            curvature_penalty: 0.05,
        }
    }
}

/// Supplies the real objective for latent search.
///
/// Implementations may call an LM-head scorer, a reward model, a symbolic verifier, a unit-test
/// oracle, or another deterministic model-derived objective. Search itself does not assume a
/// geometric target.
pub trait LatentEvaluator {
    /// Higher values are better.
    fn evaluate(&mut self, state: &LatentState8D, depth: usize) -> f32;

    /// Non-negative prior used by PUCT. Returning 1.0 is a uniform prior.
    fn prior(
        &mut self,
        _parent: &LatentState8D,
        _action: &[f32; 8],
        _next: &LatentState8D,
        _depth: usize,
    ) -> f32 {
        1.0
    }

    /// Optional early termination condition.
    fn is_terminal(&mut self, _state: &LatentState8D, _depth: usize) -> bool {
        false
    }
}

pub struct GeodesicGoalEvaluator {
    goal: LatentState8D,
}

impl GeodesicGoalEvaluator {
    pub const fn new(goal: LatentState8D) -> Self {
        Self { goal }
    }
}

impl LatentEvaluator for GeodesicGoalEvaluator {
    fn evaluate(&mut self, state: &LatentState8D, _depth: usize) -> f32 {
        10.0 / (1.0 + state.distance_to(&self.goal))
    }

    fn prior(
        &mut self,
        _parent: &LatentState8D,
        _action: &[f32; 8],
        next: &LatentState8D,
        _depth: usize,
    ) -> f32 {
        (1.0 / (1.0 + next.distance_to(&self.goal))).max(1.0e-6)
    }
}

pub struct ZMctsEngine {
    pub config: ZMctsConfig,
    pub nodes: Vec<LatentMctsNode>,
}

impl ZMctsEngine {
    pub fn new(config: ZMctsConfig) -> Self {
        Self {
            config,
            nodes: Vec::new(),
        }
    }

    /// Generic production search path. The evaluator supplies the actual model/verifier objective.
    pub fn search_with_evaluator<E: LatentEvaluator>(
        &mut self,
        start_state: LatentState8D,
        evaluator: &mut E,
    ) -> Vec<LatentState8D> {
        self.nodes.clear();
        self.nodes
            .push(LatentMctsNode::new(start_state, None, [0.0; 8], 1.0, 0));

        let actions = self.generate_candidate_velocities();
        let action_count = self.config.branch_factor.clamp(1, actions.len());

        for _ in 0..self.config.num_simulations {
            let mut current = 0usize;

            // Selection.
            while !self.nodes[current].children_indices.is_empty()
                && self.nodes[current].depth < self.config.max_depth
            {
                let parent_visits = self.nodes[current].visit_count.max(1) as f32;
                current = self.nodes[current]
                    .children_indices
                    .iter()
                    .copied()
                    .max_by(|&lhs, &rhs| {
                        let lhs_score = self.puct_score(lhs, parent_visits);
                        let rhs_score = self.puct_score(rhs, parent_visits);
                        lhs_score.total_cmp(&rhs_score)
                    })
                    .unwrap_or(current);
            }

            let depth = self.nodes[current].depth;
            if evaluator.is_terminal(&self.nodes[current].state, depth) {
                let value = evaluator.evaluate(&self.nodes[current].state, depth);
                self.backpropagate(current, value);
                continue;
            }

            // Expansion.
            if depth < self.config.max_depth && self.nodes[current].visit_count > 0 {
                let parent_state = self.nodes[current].state;
                let next_depth = depth + 1;
                for action in actions.iter().take(action_count) {
                    let next_state = parent_state.step(action, self.config.step_dt);
                    let prior = evaluator
                        .prior(&parent_state, action, &next_state, next_depth)
                        .max(1.0e-8);
                    let child_index = self.nodes.len();
                    self.nodes.push(LatentMctsNode::new(
                        next_state,
                        Some(current),
                        *action,
                        prior,
                        next_depth,
                    ));
                    self.nodes[current].children_indices.push(child_index);
                }

                current = self.nodes[current]
                    .children_indices
                    .iter()
                    .copied()
                    .max_by(|&lhs, &rhs| {
                        self.nodes[lhs]
                            .prior_prob
                            .total_cmp(&self.nodes[rhs].prior_prob)
                    })
                    .unwrap_or(current);
            }

            // Model/verifier evaluation plus a small path-energy regularizer.
            let node = &self.nodes[current];
            let action_energy = node
                .action_taken
                .iter()
                .map(|value| value * value)
                .sum::<f32>();
            let raw_value = evaluator.evaluate(&node.state, node.depth);
            let value = raw_value - self.config.curvature_penalty * action_energy;
            self.backpropagate(current, value);
        }

        self.best_trajectory(start_state)
    }

    /// Deterministic geometry-only reference path retained for regression tests and visual demos.
    pub fn search_optimal_trajectory(
        &mut self,
        start_state: LatentState8D,
        goal_state: LatentState8D,
    ) -> Vec<LatentState8D> {
        let mut evaluator = GeodesicGoalEvaluator::new(goal_state);
        self.search_with_evaluator(start_state, &mut evaluator)
    }

    fn puct_score(&self, node_index: usize, parent_visits: f32) -> f32 {
        let child = &self.nodes[node_index];
        child.q_value()
            + self.config.c_puct * child.prior_prob * parent_visits.sqrt()
                / (1.0 + child.visit_count as f32)
    }

    fn backpropagate(&mut self, leaf_index: usize, value: f32) {
        let mut cursor = Some(leaf_index);
        while let Some(index) = cursor {
            self.nodes[index].visit_count = self.nodes[index].visit_count.saturating_add(1);
            self.nodes[index].total_value += value;
            cursor = self.nodes[index].parent_idx;
        }
    }

    fn best_trajectory(&self, start_state: LatentState8D) -> Vec<LatentState8D> {
        let mut trajectory = vec![start_state];
        let mut current = 0usize;

        while let Some(next) = self.nodes[current]
            .children_indices
            .iter()
            .copied()
            .filter(|&index| self.nodes[index].visit_count > 0)
            .max_by_key(|&index| self.nodes[index].visit_count)
        {
            trajectory.push(self.nodes[next].state);
            current = next;
            if self.nodes[current].depth >= self.config.max_depth {
                break;
            }
        }
        trajectory
    }

    fn generate_candidate_velocities(&self) -> Vec<[f32; 8]> {
        let mut velocities = Vec::with_capacity(24);
        for axis in 0..8 {
            let mut positive = [0.0f32; 8];
            positive[axis] = 1.0;
            velocities.push(positive);

            let mut negative = [0.0f32; 8];
            negative[axis] = -1.0;
            velocities.push(negative);
        }

        // Add a small set of diagonal tangents so the branch factor can exceed 16 without
        // inventing random actions. These are normalized to unit L2 length.
        for sign in [-1.0f32, 1.0] {
            let mut diagonal = [0.0f32; 8];
            diagonal.fill(sign / 8.0f32.sqrt());
            velocities.push(diagonal);
        }
        velocities
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometric_reference_search_moves_toward_goal() {
        let config = ZMctsConfig {
            num_simulations: 120,
            max_depth: 6,
            branch_factor: 16,
            step_dt: 0.5,
            curvature_penalty: 0.01,
            ..ZMctsConfig::default()
        };
        let mut engine = ZMctsEngine::new(config);
        let start = LatentState8D::new([1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]);
        let goal = LatentState8D::new([5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0]);
        let trajectory = engine.search_optimal_trajectory(start, goal);

        assert!(trajectory.len() >= 2);
        assert_eq!(trajectory[0], start);
        assert!(trajectory.last().unwrap().distance_to(&goal) < start.distance_to(&goal));
    }

    struct AxisReward;

    impl LatentEvaluator for AxisReward {
        fn evaluate(&mut self, state: &LatentState8D, _depth: usize) -> f32 {
            state.coords[0]
        }

        fn prior(
            &mut self,
            _parent: &LatentState8D,
            action: &[f32; 8],
            _next: &LatentState8D,
            _depth: usize,
        ) -> f32 {
            if action[0] > 0.0 { 2.0 } else { 1.0 }
        }
    }

    #[test]
    fn generic_evaluator_controls_search_objective() {
        let config = ZMctsConfig {
            num_simulations: 80,
            max_depth: 5,
            branch_factor: 16,
            step_dt: 0.5,
            curvature_penalty: 0.0,
            ..ZMctsConfig::default()
        };
        let mut engine = ZMctsEngine::new(config);
        let start = LatentState8D::new([1.0; 8]);
        let mut evaluator = AxisReward;
        let trajectory = engine.search_with_evaluator(start, &mut evaluator);
        assert!(trajectory.last().unwrap().coords[0] > start.coords[0]);
    }
}
