//! Sovereign Tiled FlashAttention Kernel with Online Softmax.
//! Evaluates fused multi-head self-attention without materializing N x N attention matrices in RAM.
//! Memory complexity: O(1) working SRAM per head.

#[derive(Clone, Debug)]
pub struct FlashAttentionConfig {
    pub num_heads: usize,
    pub num_kv_heads: usize,
    pub head_dim: usize,
    pub tile_size: usize,
    pub scale: f32,
}

impl FlashAttentionConfig {
    pub fn new(num_heads: usize, num_kv_heads: usize, head_dim: usize) -> Self {
        Self {
            num_heads,
            num_kv_heads,
            head_dim,
            tile_size: 64,
            scale: 1.0 / (head_dim as f32).sqrt(),
        }
    }
}

/// Compute FlashAttention forward pass for a single query token against key/value history.
/// Uses online softmax scaling: m_new = max(m_old, m_block), l_new = l_old * exp(m_old - m_new) + sum(exp(m_block - m_new)).
pub fn flash_attention_forward(
    q: &[f32],       // [num_heads, head_dim]
    k_cache: &[f32], // [seq_len, num_kv_heads, head_dim]
    v_cache: &[f32], // [seq_len, num_kv_heads, head_dim]
    seq_len: usize,
    config: &FlashAttentionConfig,
    out: &mut [f32], // [num_heads, head_dim]
) {
    let num_heads = config.num_heads;
    let num_kv_heads = config.num_kv_heads;
    let head_dim = config.head_dim;
    let scale = config.scale;
    let gqa_ratio = num_heads / num_kv_heads;

    for h in 0..num_heads {
        let kv_h = h / gqa_ratio;
        let q_head = &q[h * head_dim..(h + 1) * head_dim];
        let out_head = &mut out[h * head_dim..(h + 1) * head_dim];

        let mut max_score = -f32::INFINITY;
        let mut sum_exp = 0.0_f32;
        let mut acc = vec![0.0_f32; head_dim];

        for pos in 0..seq_len {
            let k_idx = (pos * num_kv_heads + kv_h) * head_dim;
            let v_idx = (pos * num_kv_heads + kv_h) * head_dim;
            let k_vec = &k_cache[k_idx..k_idx + head_dim];
            let v_vec = &v_cache[v_idx..v_idx + head_dim];

            // Compute dot product Q . K
            let mut dot = 0.0_f32;
            for d in 0..head_dim {
                dot += q_head[d] * k_vec[d];
            }
            let score = dot * scale;

            // Online Softmax update
            if score > max_score {
                let factor = (max_score - score).exp();
                sum_exp = sum_exp * factor + 1.0;
                for d in 0..head_dim {
                    acc[d] = acc[d] * factor + v_vec[d];
                }
                max_score = score;
            } else {
                let exp_val = (score - max_score).exp();
                sum_exp += exp_val;
                for d in 0..head_dim {
                    acc[d] += exp_val * v_vec[d];
                }
            }
        }

        let inv_sum = if sum_exp > 0.0 { 1.0 / sum_exp } else { 0.0 };
        for d in 0..head_dim {
            out_head[d] = acc[d] * inv_sum;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flash_attention_computes_normalized_output() {
        let config = FlashAttentionConfig::new(4, 4, 16);
        let seq_len = 32;
        let q = vec![0.5_f32; 4 * 16];
        let k_cache = vec![0.1_f32; seq_len * 4 * 16];
        let v_cache = vec![1.0_f32; seq_len * 4 * 16];
        let mut out = vec![0.0_f32; 4 * 16];

        flash_attention_forward(&q, &k_cache, &v_cache, seq_len, &config, &mut out);
        for &val in &out {
            assert!((val - 1.0).abs() < 1e-4, "Value expectation mismatch");
        }
    }
}
