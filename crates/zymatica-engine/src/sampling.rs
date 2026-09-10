use rand::Rng;

#[derive(Debug, Clone, Copy)]
pub struct SamplingConfig {
    pub temperature: f32,
    pub top_k: usize,
    pub top_p: Option<f32>,
    pub min_p: Option<f32>,
}

impl Default for SamplingConfig {
    fn default() -> Self {
        Self {
            temperature: 0.0,
            top_k: 1,
            top_p: None,
            min_p: None,
        }
    }
}

pub fn sample_next<R: Rng + ?Sized>(logits: &[f32], cfg: SamplingConfig, rng: &mut R) -> usize {
    assert!(!logits.is_empty());
    if cfg.temperature <= 0.0 || cfg.top_k <= 1 {
        return crate::ops::argmax(logits);
    }

    let k = cfg.top_k.min(logits.len());
    let mut candidates: Vec<_> = logits.iter().copied().enumerate().collect();
    candidates.select_nth_unstable_by(k - 1, |(_, a), (_, b)| b.total_cmp(a));
    candidates.truncate(k);
    candidates.sort_by(|(_, a), (_, b)| b.total_cmp(a));

    let max = candidates
        .iter()
        .map(|(_, logit)| *logit)
        .fold(f32::NEG_INFINITY, f32::max);
    let temperature = cfg.temperature.max(1e-6);
    let mut total = 0.0;
    let mut probs = Vec::with_capacity(candidates.len());
    for (idx, logit) in candidates {
        let prob = ((logit - max) / temperature).exp();
        total += prob;
        probs.push((idx, prob));
    }

    if total <= 0.0 || !total.is_finite() {
        return probs[0].0;
    }

    // Apply min_p filtering if configured
    if let Some(min_p) = cfg.min_p
        && min_p > 0.0
    {
        let top_prob = probs[0].1;
        let threshold = top_prob * min_p;
        probs.retain(|(_, prob)| *prob >= threshold);
        total = probs.iter().map(|(_, prob)| *prob).sum();
    }

    // Apply top_p (nucleus) filtering if configured
    if let Some(top_p) = cfg.top_p
        && top_p > 0.0
        && top_p < 1.0
    {
        let cutoff = total * top_p;
        let mut cum_sum = 0.0;
        let mut kept = 0;
        for (_, prob) in &probs {
            cum_sum += prob;
            kept += 1;
            if cum_sum >= cutoff {
                break;
            }
        }
        probs.truncate(kept.max(1));
        total = probs.iter().map(|(_, prob)| *prob).sum();
    }

    if probs.is_empty() || total <= 0.0 {
        return 0;
    }

    let mut draw = rng.gen_range(0.0..total);
    for (idx, prob) in probs {
        if draw <= prob {
            return idx;
        }
        draw -= prob;
    }
    0
}

pub fn calculate_entropy(logits: &[f32]) -> f32 {
    let mut probs = logits.to_vec();
    let max = probs.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut sum = 0.0;
    for v in probs.iter_mut() {
        *v = (*v - max).exp();
        sum += *v;
    }
    if sum > 0.0 {
        for v in probs.iter_mut() {
            *v /= sum;
        }
    }

    let mut entropy = 0.0;
    for &p in &probs {
        if p > 1e-5 {
            entropy -= p * p.ln();
        }
    }
    entropy
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::{SeedableRng, rngs::StdRng};

    #[test]
    fn greedy_sampling_returns_argmax() {
        let mut rng = StdRng::seed_from_u64(1);
        assert_eq!(
            sample_next(&[1.0, 9.0, 2.0], SamplingConfig::default(), &mut rng),
            1
        );
    }

    #[test]
    fn top_k_sampling_stays_inside_top_k() {
        let mut rng = StdRng::seed_from_u64(1);
        for _ in 0..32 {
            let idx = sample_next(
                &[10.0, 9.0, -100.0],
                SamplingConfig {
                    temperature: 0.8,
                    top_k: 2,
                    top_p: None,
                    min_p: None,
                },
                &mut rng,
            );
            assert!(idx == 0 || idx == 1);
        }
    }

    #[test]
    fn min_p_sampling_excludes_low_probability_tails() {
        let mut rng = StdRng::seed_from_u64(42);
        // Logits with index 0 very high, 1 moderate, 2 low
        let logits = [10.0, 6.0, 1.0];
        for _ in 0..50 {
            let idx = sample_next(
                &logits,
                SamplingConfig {
                    temperature: 1.0,
                    top_k: 3,
                    top_p: None,
                    min_p: Some(0.1), // Excludes index 2 whose prob relative to top is < 0.1
                },
                &mut rng,
            );
            assert!(idx == 0 || idx == 1, "idx {} should not be 2", idx);
        }
    }

    #[test]
    fn top_p_sampling_restricts_probability_mass() {
        let mut rng = StdRng::seed_from_u64(100);
        let logits = [10.0, 9.5, 1.0, 0.5];
        for _ in 0..50 {
            let idx = sample_next(
                &logits,
                SamplingConfig {
                    temperature: 1.0,
                    top_k: 4,
                    top_p: Some(0.85), // Kept candidates should be indices 0 and 1
                    min_p: None,
                },
                &mut rng,
            );
            assert!(
                idx == 0 || idx == 1,
                "idx {} should be restricted to top_p mass",
                idx
            );
        }
    }
}
