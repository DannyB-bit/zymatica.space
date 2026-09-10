use crate::tensor::Matrix;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

#[cfg(feature = "parallel")]
const PARALLEL_F32_MATVEC_WORK_ITEMS: usize = 65_536;

#[inline]
pub fn dot(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    crate::kernels::f32_dot(a, b)
}

#[inline]
pub fn silu(x: f32) -> f32 {
    x / (1.0 + (-x).exp())
}

#[inline]
pub fn silu_product_in_place(values: &mut [f32], inputs: &[f32]) {
    assert_eq!(values.len(), inputs.len());
    for (value, input) in values.iter_mut().zip(inputs) {
        *value = silu(*value) * *input;
    }
}

pub fn gelu_pytorch_tanh(x: f32) -> f32 {
    let sqrt_2_over_pi = (2.0f32 / std::f32::consts::PI).sqrt();
    0.5 * x * (1.0 + (sqrt_2_over_pi * (x + 0.044_715 * x * x * x)).tanh())
}

pub fn rms_norm(x: &[f32], weight: &[f32], eps: f32) -> Vec<f32> {
    assert_eq!(x.len(), weight.len());
    #[cfg(target_arch = "aarch64")]
    {
        crate::kernels::rms_norm_neon(x, weight, eps)
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            if let Some(out) = crate::kernels::rms_norm_x86_avx2_fma(x, weight, eps) {
                return out;
            }
        }
        let mean_square = x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32;
        let scale = 1.0 / (mean_square + eps).sqrt();
        x.iter().zip(weight).map(|(v, w)| v * scale * w).collect()
    }
}

pub fn rms_norm_unit(x: &[f32], eps: f32) -> Vec<f32> {
    #[cfg(target_arch = "aarch64")]
    {
        crate::kernels::rms_norm_unit_neon(x, eps)
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        let mean_square = x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32;
        let scale = 1.0 / (mean_square + eps).sqrt();
        x.iter().map(|v| v * scale).collect()
    }
}

pub fn rms_norm_in_place(values: &mut [f32], weight: &[f32], eps: f32) {
    assert_eq!(values.len(), weight.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if crate::kernels::rms_norm_in_place_x86_avx2_fma(values, weight, eps) {
            return;
        }
    }
    let mean_square = values.iter().map(|v| v * v).sum::<f32>() / values.len() as f32;
    let scale = 1.0 / (mean_square + eps).sqrt();
    for (value, weight) in values.iter_mut().zip(weight) {
        *value *= scale * *weight;
    }
}

pub fn rms_norm_chunks_in_place(values: &mut [f32], chunk_len: usize, weight: &[f32], eps: f32) {
    assert_eq!(chunk_len, weight.len());
    assert_eq!(values.len() % chunk_len, 0);
    for chunk in values.chunks_exact_mut(chunk_len) {
        let mean_square = chunk.iter().map(|v| v * v).sum::<f32>() / chunk_len as f32;
        let scale = 1.0 / (mean_square + eps).sqrt();
        for (value, weight) in chunk.iter_mut().zip(weight) {
            *value *= scale * *weight;
        }
    }
}

pub fn rms_norm_unit_chunks_in_place(values: &mut [f32], chunk_len: usize, eps: f32) {
    assert_eq!(values.len() % chunk_len, 0);
    for chunk in values.chunks_exact_mut(chunk_len) {
        let mean_square = chunk.iter().map(|v| v * v).sum::<f32>() / chunk_len as f32;
        let scale = 1.0 / (mean_square + eps).sqrt();
        for value in chunk {
            *value *= scale;
        }
    }
}

pub fn matvec(matrix: &Matrix, x: &[f32]) -> Vec<f32> {
    let mut out = vec![0.0; matrix.rows];
    matvec_into(matrix, x, &mut out);
    out
}

pub fn matvec_into(matrix: &Matrix, x: &[f32], out: &mut [f32]) {
    assert_eq!(matrix.cols, x.len());
    assert_eq!(matrix.rows, out.len());
    #[cfg(feature = "parallel")]
    {
        if matrix.rows * matrix.cols >= PARALLEL_F32_MATVEC_WORK_ITEMS {
            out.par_chunks_mut(64)
                .enumerate()
                .for_each(|(chunk_idx, chunk)| {
                    let chunk_row_base = chunk_idx * 64;
                    for (sub_idx, sub_chunk) in chunk.chunks_mut(8).enumerate() {
                        let row_base = chunk_row_base + sub_idx * 8;
                        let len = sub_chunk.len();
                        if len == 8 {
                            let (r0, r1, r2, r3, r4, r5, r6, r7) = crate::kernels::f32_dot8(
                                matrix.row(row_base),
                                matrix.row(row_base + 1),
                                matrix.row(row_base + 2),
                                matrix.row(row_base + 3),
                                matrix.row(row_base + 4),
                                matrix.row(row_base + 5),
                                matrix.row(row_base + 6),
                                matrix.row(row_base + 7),
                                x,
                            );
                            sub_chunk[0] = r0;
                            sub_chunk[1] = r1;
                            sub_chunk[2] = r2;
                            sub_chunk[3] = r3;
                            sub_chunk[4] = r4;
                            sub_chunk[5] = r5;
                            sub_chunk[6] = r6;
                            sub_chunk[7] = r7;
                        } else {
                            for (i, cell) in sub_chunk.iter_mut().enumerate() {
                                *cell = dot(matrix.row(row_base + i), x);
                            }
                        }
                    }
                });
            return;
        }
    }
    let mut row_idx = 0;
    while row_idx + 8 <= matrix.rows {
        let (r0, r1, r2, r3, r4, r5, r6, r7) = crate::kernels::f32_dot8(
            matrix.row(row_idx),
            matrix.row(row_idx + 1),
            matrix.row(row_idx + 2),
            matrix.row(row_idx + 3),
            matrix.row(row_idx + 4),
            matrix.row(row_idx + 5),
            matrix.row(row_idx + 6),
            matrix.row(row_idx + 7),
            x,
        );
        out[row_idx] = r0;
        out[row_idx + 1] = r1;
        out[row_idx + 2] = r2;
        out[row_idx + 3] = r3;
        out[row_idx + 4] = r4;
        out[row_idx + 5] = r5;
        out[row_idx + 6] = r6;
        out[row_idx + 7] = r7;
        row_idx += 8;
    }
    while row_idx < matrix.rows {
        out[row_idx] = dot(matrix.row(row_idx), x);
        row_idx += 1;
    }
}

pub fn matvec2(a: &Matrix, b: &Matrix, x: &[f32]) -> (Vec<f32>, Vec<f32>) {
    assert_eq!(a.cols, x.len());
    assert_eq!(b.cols, x.len());

    #[cfg(feature = "parallel")]
    {
        rayon::join(|| matvec(a, x), || matvec(b, x))
    }

    #[cfg(not(feature = "parallel"))]
    {
        let mut out_a = vec![0.0; a.rows];
        let mut out_b = vec![0.0; b.rows];

        for (row_idx, out_cell) in out_a.iter_mut().enumerate() {
            *out_cell = dot(a.row(row_idx), x);
        }
        for (row_idx, out_cell) in out_b.iter_mut().enumerate() {
            *out_cell = dot(b.row(row_idx), x);
        }

        (out_a, out_b)
    }
}

pub fn matvec3(a: &Matrix, b: &Matrix, c: &Matrix, x: &[f32]) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    assert_eq!(a.cols, x.len());
    assert_eq!(b.cols, x.len());
    assert_eq!(c.cols, x.len());

    #[cfg(feature = "parallel")]
    {
        let (out_a, (out_b, out_c)) = rayon::join(
            || matvec(a, x),
            || rayon::join(|| matvec(b, x), || matvec(c, x)),
        );
        (out_a, out_b, out_c)
    }

    #[cfg(not(feature = "parallel"))]
    {
        (matvec(a, x), matvec(b, x), matvec(c, x))
    }
}

pub fn matmat_4col(
    matrix: &Matrix,
    x0: &[f32],
    x1: &[f32],
    x2: &[f32],
    x3: &[f32],
) -> (Vec<f32>, Vec<f32>, Vec<f32>, Vec<f32>) {
    assert_eq!(matrix.cols, x0.len());
    assert_eq!(matrix.cols, x1.len());
    assert_eq!(matrix.cols, x2.len());
    assert_eq!(matrix.cols, x3.len());

    let mut out0 = vec![0.0; matrix.rows];
    let mut out1 = vec![0.0; matrix.rows];
    let mut out2 = vec![0.0; matrix.rows];
    let mut out3 = vec![0.0; matrix.rows];

    #[cfg(feature = "parallel")]
    {
        if matrix.rows * matrix.cols >= PARALLEL_F32_MATVEC_WORK_ITEMS {
            let (out0_b, (out1_b, (out2_b, out3_b))) = rayon::join(
                || matvec(matrix, x0),
                || {
                    rayon::join(
                        || matvec(matrix, x1),
                        || rayon::join(|| matvec(matrix, x2), || matvec(matrix, x3)),
                    )
                },
            );
            return (out0_b, out1_b, out2_b, out3_b);
        }
    }

    matvec_into(matrix, x0, &mut out0);
    matvec_into(matrix, x1, &mut out1);
    matvec_into(matrix, x2, &mut out2);
    matvec_into(matrix, x3, &mut out3);

    (out0, out1, out2, out3)
}

pub fn softmax_in_place(values: &mut [f32]) {
    let max = values.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let mut sum = 0.0;
    for v in values.iter_mut() {
        *v = (*v - max).exp();
        sum += *v;
    }
    if sum > 0.0 {
        for v in values.iter_mut() {
            *v /= sum;
        }
    }
}

pub fn softcap_in_place(values: &mut [f32], softcap: f32) {
    crate::kernels::softcap_pade_in_place(values, softcap);
}

pub fn apply_rope_pairwise(v: &mut [f32], position: usize, rope_theta: f32) {
    assert_eq!(v.len() % 2, 0);
    let half_pairs = v.len() / 2;
    for pair in 0..half_pairs {
        let i0 = pair * 2;
        let i1 = i0 + 1;
        let inv_freq = rope_theta.powf(-(i0 as f32) / v.len() as f32);
        let angle = position as f32 * inv_freq;
        let (sin, cos) = angle.sin_cos();
        let x0 = v[i0];
        let x1 = v[i1];
        v[i0] = x0 * cos - x1 * sin;
        v[i1] = x0 * sin + x1 * cos;
    }
}

pub fn apply_rope_split_half(
    v: &mut [f32],
    position: usize,
    rope_theta: f32,
    rotary_fraction: f32,
) {
    assert_eq!(v.len() % 2, 0);
    let half = v.len() / 2;
    let rotary_angles = ((half as f32) * rotary_fraction)
        .round()
        .clamp(0.0, half as f32) as usize;
    for idx in 0..half {
        let angle = if idx < rotary_angles {
            let inv_freq = rope_theta.powf(-((2 * idx) as f32) / v.len() as f32);
            position as f32 * inv_freq
        } else {
            0.0
        };
        let (sin, cos) = angle.sin_cos();
        let x0 = v[idx];
        let x1 = v[idx + half];
        v[idx] = x0 * cos - x1 * sin;
        v[idx + half] = x1 * cos + x0 * sin;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RopeTrigTable {
    pub cos: Vec<f32>,
    pub sin: Vec<f32>,
    pub rotary_angles: usize,
    pub cached_positions: usize,
    pub head_dim: usize,
    pub rotary_fraction: f32,
    pub rope_theta: f32,
}

impl RopeTrigTable {
    pub fn new(
        max_position: usize,
        head_dim: usize,
        rotary_fraction: f32,
        rope_theta: f32,
    ) -> Self {
        let half = head_dim / 2;
        let rotary_angles = ((half as f32) * rotary_fraction)
            .round()
            .clamp(0.0, half as f32) as usize;
        let cached_positions = rope_cache_positions(max_position);
        let inv_freq: Vec<f32> = (0..rotary_angles)
            .map(|idx| rope_theta.powf(-((2 * idx) as f32) / head_dim as f32))
            .collect();
        let mut cos = vec![0.0; cached_positions * rotary_angles];
        let mut sin = vec![0.0; cached_positions * rotary_angles];
        for pos in 0..cached_positions {
            for idx in 0..rotary_angles {
                let angle = pos as f32 * inv_freq[idx];
                let (s, c) = angle.sin_cos();
                cos[pos * rotary_angles + idx] = c;
                sin[pos * rotary_angles + idx] = s;
            }
        }
        Self {
            cos,
            sin,
            rotary_angles,
            cached_positions,
            head_dim,
            rotary_fraction,
            rope_theta,
        }
    }
}

fn rope_cache_positions(max_position: usize) -> usize {
    let requested = std::env::var("ZYMATICA_ROPE_CACHE_POSITIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(4096);
    max_position.min(requested)
}

pub fn apply_rope_split_half_cached(v: &mut [f32], position: usize, table: &RopeTrigTable) {
    assert_eq!(v.len() % 2, 0);
    if position >= table.cached_positions {
        apply_rope_split_half(v, position, table.rope_theta, table.rotary_fraction);
        return;
    }
    let half = v.len() / 2;
    let offset = position * table.rotary_angles;
    let cos_row = &table.cos[offset..offset + table.rotary_angles];
    let sin_row = &table.sin[offset..offset + table.rotary_angles];
    for idx in 0..table.rotary_angles {
        let cos = cos_row[idx];
        let sin = sin_row[idx];
        let x0 = v[idx];
        let x1 = v[idx + half];
        v[idx] = x0 * cos - x1 * sin;
        v[idx + half] = x1 * cos + x0 * sin;
    }
}

#[inline]
pub fn fast_tanh(x: f32) -> f32 {
    let abs_x = x.abs();
    if abs_x >= 4.7 {
        x.signum()
    } else {
        let x2 = x * x;
        let num = x * (135135.0 + x2 * (17325.0 + x2 * (378.0 + x2)));
        let den = 135135.0 + x2 * (62370.0 + x2 * (3150.0 + x2 * 28.0));
        num / den
    }
}

pub fn argmax(values: &[f32]) -> usize {
    values
        .iter()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.total_cmp(b))
        .map(|(idx, _)| idx)
        .unwrap_or(0)
}

pub fn block_sparse_attention_linear(
    q: &[f32],
    k_pages: &[f32],
    v_pages: &[f32],
    block_size: usize,
    head_dim: usize,
) -> Vec<f32> {
    let num_blocks = (k_pages.len() / head_dim) / block_size.max(1);
    let mut out = vec![0.0_f32; head_dim];
    if num_blocks == 0 {
        return out;
    }
    let scale = 1.0 / (head_dim as f32).sqrt();
    let mut total_weight = 0.0_f32;
    for b in (0..num_blocks).rev().take(4) {
        let base = b * block_size * head_dim;
        let mut score = 0.0_f32;
        for i in 0..head_dim {
            score += q[i] * k_pages[base + i];
        }
        let weight = (score * scale).exp();
        total_weight += weight;
        for i in 0..head_dim {
            out[i] += weight * v_pages[base + i];
        }
    }
    if total_weight > 0.0 {
        let inv = 1.0 / total_weight;
        for v in &mut out {
            *v *= inv;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rms_norm_matches_manual_two_values() {
        let out = rms_norm(&[3.0, 4.0], &[1.0, 1.0], 0.0);
        let scale = 1.0 / ((25.0_f32 / 2.0).sqrt());
        assert!((out[0] - 3.0 * scale).abs() < 1e-6);
        assert!((out[1] - 4.0 * scale).abs() < 1e-6);
    }

    #[test]
    fn rms_norm_large_matches_scalar_reference() {
        let len = 1537;
        let x = (0..len)
            .map(|idx| ((idx as f32 * 0.013).sin() * 0.5) + ((idx as f32 * 0.029).cos() * 0.75))
            .collect::<Vec<_>>();
        let weight = (0..len)
            .map(|idx| 0.75 + (idx % 17) as f32 * 0.01)
            .collect::<Vec<_>>();
        let eps = 1e-6;
        let mean_square = x.iter().map(|v| v * v).sum::<f32>() / len as f32;
        let scale = 1.0 / (mean_square + eps).sqrt();
        let reference = x
            .iter()
            .zip(&weight)
            .map(|(value, weight)| value * scale * weight)
            .collect::<Vec<_>>();

        let out = rms_norm(&x, &weight, eps);
        let mut in_place = x.clone();
        rms_norm_in_place(&mut in_place, &weight, eps);

        for (expected, got) in reference.iter().zip(&out) {
            assert!((expected - got).abs() < 1e-5);
        }
        for (expected, got) in reference.iter().zip(&in_place) {
            assert!((expected - got).abs() < 1e-5);
        }
    }

    #[test]
    fn softmax_sums_to_one() {
        let mut xs = [1.0, 2.0, 3.0];
        softmax_in_place(&mut xs);
        let sum: f32 = xs.iter().sum();
        assert!((sum - 1.0).abs() < 1e-6);
        assert!(xs[2] > xs[1] && xs[1] > xs[0]);
    }

    #[test]
    fn matvec3_matches_individual_matvecs() {
        let a = Matrix::from_row_major(2, 2, vec![1.0, 2.0, 3.0, 4.0]);
        let b = Matrix::from_row_major(1, 2, vec![0.5, -1.0]);
        let c = Matrix::from_row_major(2, 2, vec![2.0, 0.25, -2.0, 1.5]);
        let x = [2.0, 3.0];
        let (got_a, got_b, got_c) = matvec3(&a, &b, &c, &x);
        assert_eq!(got_a, matvec(&a, &x));
        assert_eq!(got_b, matvec(&b, &x));
        assert_eq!(got_c, matvec(&c, &x));
    }

    #[test]
    fn matvec2_matches_individual_matvecs() {
        let a = Matrix::from_row_major(2, 2, vec![1.0, 2.0, 3.0, 4.0]);
        let b = Matrix::from_row_major(1, 2, vec![0.5, -1.0]);
        let x = [2.0, 3.0];
        let (got_a, got_b) = matvec2(&a, &b, &x);
        assert_eq!(got_a, matvec(&a, &x));
        assert_eq!(got_b, matvec(&b, &x));
    }

    #[test]
    fn rope_preserves_pair_norms() {
        let mut xs = [1.0, 2.0, 3.0, 4.0];
        let before = xs[0] * xs[0] + xs[1] * xs[1];
        apply_rope_pairwise(&mut xs, 17, 10_000.0);
        let after = xs[0] * xs[0] + xs[1] * xs[1];
        assert!((before - after).abs() < 1e-5);
    }

    #[test]
    fn fast_tanh_precision_check() {
        for i in -600..600 {
            let x = i as f32 * 0.01;
            let actual = x.tanh();
            let approx = fast_tanh(x);
            let diff = (actual - approx).abs();
            assert!(
                diff < 0.0003,
                "failed at x={}, actual={}, approx={}, diff={}",
                x,
                actual,
                approx,
                diff
            );
        }
    }

    #[test]
    fn softcap_in_place_matches_scalar_fast_tanh() {
        let softcap = 30.0;
        let mut values: Vec<f32> = (-256..256).map(|i| i as f32 * 0.5).collect();
        let expected: Vec<f32> = values
            .iter()
            .map(|value| fast_tanh(*value / softcap) * softcap)
            .collect();
        softcap_in_place(&mut values, softcap);
        for (actual, expected) in values.iter().zip(expected) {
            assert!((actual - expected).abs() < 1e-4);
        }
    }

    #[test]
    fn cached_rope_split_half_matches_exactly() {
        let mut v1 = vec![0.5, -0.2, 1.1, 0.4, 0.9, -0.3, 0.8, -0.1];
        let mut v2 = v1.clone();
        let head_dim = v1.len();
        let max_position = 32;
        let position = 13;
        let rope_theta = 10000.0;
        let rotary_fraction = 0.5;

        apply_rope_split_half(&mut v1, position, rope_theta, rotary_fraction);

        let table = RopeTrigTable::new(max_position, head_dim, rotary_fraction, rope_theta);
        apply_rope_split_half_cached(&mut v2, position, &table);

        for (x, y) in v1.iter().zip(&v2) {
            assert!((x - y).abs() < 1e-5);
        }
    }

    #[test]
    fn cached_rope_falls_back_exactly_past_cached_positions() {
        let mut v1 = vec![0.5, -0.2, 1.1, 0.4, 0.9, -0.3, 0.8, -0.1];
        let mut v2 = v1.clone();
        let head_dim = v1.len();
        let position = 8;
        let rope_theta = 10000.0;
        let rotary_fraction = 1.0;

        apply_rope_split_half(&mut v1, position, rope_theta, rotary_fraction);

        let table = RopeTrigTable::new(4, head_dim, rotary_fraction, rope_theta);
        apply_rope_split_half_cached(&mut v2, position, &table);

        for (x, y) in v1.iter().zip(&v2) {
            assert!((x - y).abs() < 1e-5);
        }
    }

    #[test]
    fn block_sparse_linear_attention_scales() {
        let q = vec![1.0, 0.5, -0.5, 0.2];
        let k_pages = vec![1.0, 0.5, -0.5, 0.2, 0.5, -0.5, 0.2, 1.0];
        let v_pages = vec![0.8, -0.4, 0.2, 0.1, -0.1, 0.3, 0.5, -0.2];
        let out = block_sparse_attention_linear(&q, &k_pages, &v_pages, 1, 4);
        assert_eq!(out.len(), 4);
    }
}
