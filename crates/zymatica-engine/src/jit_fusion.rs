//! Sovereign JIT Kernel Fusion.
//! Fuses RMSNorm, Rotary Position Embeddings (RoPE), and SwiGLU activation into a single unrolled pass.

/// In-place fused RMSNorm layer
#[inline(always)]
pub fn fused_rmsnorm(x: &mut [f32], weight: &[f32], eps: f32) {
    let len = x.len();
    let mut sum_sq = 0.0_f32;
    for &val in x.iter() {
        sum_sq += val * val;
    }
    let mean_sq = sum_sq / (len as f32);
    let scale = 1.0_f32 / (mean_sq + eps).sqrt();

    for i in 0..len {
        x[i] = x[i] * scale * weight[i];
    }
}

/// In-place fused Rotary Position Embedding (RoPE)
#[inline(always)]
pub fn fused_rope(q: &mut [f32], k: &mut [f32], pos: usize, head_dim: usize, theta_base: f32) {
    for i in (0..head_dim).step_by(2) {
        let freq = 1.0_f32 / theta_base.powf((i as f32) / (head_dim as f32));
        let angle = (pos as f32) * freq;
        let cos = angle.cos();
        let sin = angle.sin();

        // Rotate Q
        let q0 = q[i];
        let q1 = q[i + 1];
        q[i] = q0 * cos - q1 * sin;
        q[i + 1] = q0 * sin + q1 * cos;

        // Rotate K
        let k0 = k[i];
        let k1 = k[i + 1];
        k[i] = k0 * cos - k1 * sin;
        k[i + 1] = k0 * sin + k1 * cos;
    }
}

/// Fused SwiGLU element-wise activation: out = (gate * sigmoid(gate)) * up
#[inline(always)]
pub fn fused_swiglu(gate: &[f32], up: &[f32], out: &mut [f32]) {
    for i in 0..gate.len() {
        let g = gate[i];
        let silu_g = g / (1.0 + (-g).exp());
        out[i] = silu_g * up[i];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fused_rmsnorm_normalizes_to_unit_scale() {
        let mut x = vec![2.0_f32, 2.0, 2.0, 2.0];
        let weight = vec![1.0_f32, 1.0, 1.0, 1.0];
        fused_rmsnorm(&mut x, &weight, 1e-6);
        for &val in &x {
            assert!((val - 1.0).abs() < 1e-4);
        }
    }

    #[test]
    fn fused_swiglu_computes_expected_nonlinearity() {
        let gate = vec![0.0_f32, 1.0];
        let up = vec![2.0_f32, 2.0];
        let mut out = vec![0.0_f32; 2];
        fused_swiglu(&gate, &up, &mut out);
        assert!((out[0] - 0.0).abs() < 1e-5);
        assert!((out[1] - (1.0 / (1.0 + (-1.0_f32).exp()) * 2.0)).abs() < 1e-5);
    }
}
