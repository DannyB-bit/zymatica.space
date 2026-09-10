use std::sync::atomic::{AtomicU8, Ordering};

const THERMAL_UNKNOWN: u8 = 2;
const THERMAL_LOW: u8 = 0;
const THERMAL_HIGH: u8 = 1;
const Q4_DOT2_MIN_FUSED_LEN: usize = 1024;
const Q8_DOT2_MIN_FUSED_LEN: usize = 1536;

static THERMAL_PRESSURE_HIGH: AtomicU8 = AtomicU8::new(THERMAL_UNKNOWN);

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
static X86_AVX2_AVAILABLE: AtomicU8 = AtomicU8::new(THERMAL_UNKNOWN);

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
static X86_AVX2_FMA_AVAILABLE: AtomicU8 = AtomicU8::new(THERMAL_UNKNOWN);

#[cfg(target_arch = "x86_64")]
static X86_AVX512F_BW_AVAILABLE: AtomicU8 = AtomicU8::new(THERMAL_UNKNOWN);

#[inline]
pub fn f32_dot(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    #[cfg(target_arch = "aarch64")]
    {
        if a.len() >= 16 {
            // SAFETY: both slices have equal length and the NEON routine only reads within them.
            return unsafe { f32_dot_neon(a.as_ptr(), b.as_ptr(), a.len()) };
        }
    }
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if a.len() >= 32 && is_x86_avx2_fma_available() {
            // SAFETY: AVX2+FMA availability is checked and both slices have equal length.
            return unsafe { f32_dot_avx2_fma(a.as_ptr(), b.as_ptr(), a.len()) };
        }
        if a.len() >= 32 && is_x86_avx2_available() {
            // SAFETY: AVX2 availability is checked and both slices have equal length.
            return unsafe { f32_dot_avx2(a.as_ptr(), b.as_ptr(), a.len()) };
        }
    }
    f32_dot_scalar(a, b)
}

#[inline]
pub fn f32_dot4(
    row_a: &[f32],
    row_b: &[f32],
    row_c: &[f32],
    row_d: &[f32],
    x: &[f32],
) -> (f32, f32, f32, f32) {
    debug_assert_eq!(row_a.len(), x.len());
    debug_assert_eq!(row_b.len(), x.len());
    debug_assert_eq!(row_c.len(), x.len());
    debug_assert_eq!(row_d.len(), x.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if x.len() >= 16 && is_x86_avx2_fma_available() {
            return unsafe {
                f32_dot4_avx2_fma(
                    row_a.as_ptr(),
                    row_b.as_ptr(),
                    row_c.as_ptr(),
                    row_d.as_ptr(),
                    x.as_ptr(),
                    x.len(),
                )
            };
        }
    }
    (
        f32_dot(row_a, x),
        f32_dot(row_b, x),
        f32_dot(row_c, x),
        f32_dot(row_d, x),
    )
}

#[inline]
#[allow(clippy::too_many_arguments)]
pub fn f32_dot8(
    row_a: &[f32],
    row_b: &[f32],
    row_c: &[f32],
    row_d: &[f32],
    row_e: &[f32],
    row_f: &[f32],
    row_g: &[f32],
    row_h: &[f32],
    x: &[f32],
) -> (f32, f32, f32, f32, f32, f32, f32, f32) {
    debug_assert_eq!(row_a.len(), x.len());
    debug_assert_eq!(row_b.len(), x.len());
    debug_assert_eq!(row_c.len(), x.len());
    debug_assert_eq!(row_d.len(), x.len());
    debug_assert_eq!(row_e.len(), x.len());
    debug_assert_eq!(row_f.len(), x.len());
    debug_assert_eq!(row_g.len(), x.len());
    debug_assert_eq!(row_h.len(), x.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if x.len() >= 16 && is_x86_avx2_fma_available() {
            return unsafe {
                f32_dot8_avx2_fma(
                    row_a.as_ptr(),
                    row_b.as_ptr(),
                    row_c.as_ptr(),
                    row_d.as_ptr(),
                    row_e.as_ptr(),
                    row_f.as_ptr(),
                    row_g.as_ptr(),
                    row_h.as_ptr(),
                    x.as_ptr(),
                    x.len(),
                )
            };
        }
    }
    (
        f32_dot(row_a, x),
        f32_dot(row_b, x),
        f32_dot(row_c, x),
        f32_dot(row_d, x),
        f32_dot(row_e, x),
        f32_dot(row_f, x),
        f32_dot(row_g, x),
        f32_dot(row_h, x),
    )
}

#[inline]
#[allow(clippy::too_many_arguments)]
pub fn q8_u8_dot4_f32_scaled(
    row_a: &[u8],
    row_b: &[u8],
    row_c: &[u8],
    row_d: &[u8],
    x: &[f32],
    scale_a: f32,
    scale_b: f32,
    scale_c: f32,
    scale_d: f32,
) -> (f32, f32, f32, f32) {
    debug_assert_eq!(row_a.len(), x.len());
    debug_assert_eq!(row_b.len(), x.len());
    debug_assert_eq!(row_c.len(), x.len());
    debug_assert_eq!(row_d.len(), x.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if is_x86_avx2_fma_available() {
            return unsafe {
                q8_i8_dot4_f32_avx2_fma(
                    row_a.as_ptr().cast::<i8>(),
                    row_b.as_ptr().cast::<i8>(),
                    row_c.as_ptr().cast::<i8>(),
                    row_d.as_ptr().cast::<i8>(),
                    x.as_ptr(),
                    x.len(),
                    scale_a,
                    scale_b,
                    scale_c,
                    scale_d,
                )
            };
        }
    }
    (
        q8_u8_dot_f32_scaled(row_a, x, scale_a),
        q8_u8_dot_f32_scaled(row_b, x, scale_b),
        q8_u8_dot_f32_scaled(row_c, x, scale_c),
        q8_u8_dot_f32_scaled(row_d, x, scale_d),
    )
}

#[inline]
pub fn q8_i8_dot_f32_scaled(row: &[i8], x: &[f32], scale: f32) -> f32 {
    debug_assert_eq!(row.len(), x.len());
    if thermal_pressure_high() {
        return q8_i8_dot_f32_scaled_thermal_high(row, x, scale);
    }
    #[cfg(target_arch = "aarch64")]
    {
        // SAFETY: slices are bounds-checked by length equality above; the NEON routine only reads
        // within row.len() and x.len().
        unsafe { q8_dot_f32_neon(row.as_ptr(), x.as_ptr(), row.len(), scale) }
    }
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_avx512f_bw_available() {
                // SAFETY: AVX-512 availability is checked at runtime; the routine only reads
                // within the provided slice lengths and preserves the f32 activation path.
                return unsafe { q8_dot_f32_avx512(row.as_ptr(), x.as_ptr(), row.len(), scale) };
            }
        }
        if is_x86_avx2_fma_available() {
            // SAFETY: AVX2+FMA availability is checked at runtime; the routine only reads within
            // the provided slice lengths.
            return unsafe { q8_dot_f32_avx2_fma(row.as_ptr(), x.as_ptr(), row.len(), scale) };
        }
        if is_x86_avx2_available() {
            // SAFETY: AVX2 availability is checked at runtime; the routine only reads within the
            // provided slice lengths.
            return unsafe { q8_dot_f32_avx2(row.as_ptr(), x.as_ptr(), row.len(), scale) };
        }
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        q8_dot_f32_scalar(row.iter().copied(), x, scale)
    }
}

#[inline]
fn thermal_pressure_high() -> bool {
    match THERMAL_PRESSURE_HIGH.load(Ordering::Relaxed) {
        THERMAL_LOW => return false,
        THERMAL_HIGH => return true,
        _ => {}
    }
    let high = std::env::var("ZYMATICA_THERMAL_PRESSURE")
        .map(|v| v == "high")
        .unwrap_or(false);
    THERMAL_PRESSURE_HIGH.store(
        if high { THERMAL_HIGH } else { THERMAL_LOW },
        Ordering::Relaxed,
    );
    high
}

pub fn refresh_thermal_pressure_cache() -> bool {
    THERMAL_PRESSURE_HIGH.store(THERMAL_UNKNOWN, Ordering::Relaxed);
    thermal_pressure_high()
}

#[inline]
pub fn softcap_pade_in_place(values: &mut [f32], softcap: f32) {
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if values.len() >= 32 && is_x86_avx2_fma_available() {
            // SAFETY: AVX2+FMA availability is checked at runtime and the kernel writes only
            // within the supplied mutable slice.
            unsafe {
                softcap_pade_avx2_fma(values.as_mut_ptr(), values.len(), softcap);
            }
            return;
        }
    }

    for value in values {
        *value = fast_tanh_pade_scalar(*value / softcap) * softcap;
    }
}

#[inline]
fn fast_tanh_pade_scalar(x: f32) -> f32 {
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

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2,fma")]
unsafe fn softcap_pade_avx2_fma(values: *mut f32, len: usize, softcap: f32) {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let inv_softcap = _mm256_set1_ps(1.0 / softcap);
    let softcap_v = _mm256_set1_ps(softcap);
    let sign_mask = _mm256_set1_ps(-0.0);
    let threshold = _mm256_set1_ps(4.7);
    let zero = _mm256_setzero_ps();
    let one = _mm256_set1_ps(1.0);
    let neg_one = _mm256_set1_ps(-1.0);
    let c_28 = _mm256_set1_ps(28.0);
    let c_378 = _mm256_set1_ps(378.0);
    let c_3150 = _mm256_set1_ps(3150.0);
    let c_17325 = _mm256_set1_ps(17325.0);
    let c_62370 = _mm256_set1_ps(62370.0);
    let c_135135 = _mm256_set1_ps(135135.0);

    let mut i = 0_usize;
    while i + 8 <= len {
        let raw = unsafe { _mm256_loadu_ps(values.add(i)) };
        let x = _mm256_mul_ps(raw, inv_softcap);
        let x2 = _mm256_mul_ps(x, x);

        let num_inner = _mm256_fmadd_ps(x2, one, c_378);
        let num_mid = _mm256_fmadd_ps(x2, num_inner, c_17325);
        let num_factor = _mm256_fmadd_ps(x2, num_mid, c_135135);
        let num = _mm256_mul_ps(x, num_factor);

        let den_inner = _mm256_fmadd_ps(x2, c_28, c_3150);
        let den_mid = _mm256_fmadd_ps(x2, den_inner, c_62370);
        let den = _mm256_fmadd_ps(x2, den_mid, c_135135);

        let approx = _mm256_div_ps(num, den);
        let abs_x = _mm256_andnot_ps(sign_mask, x);
        let sat_mask = _mm256_cmp_ps(abs_x, threshold, _CMP_GE_OQ);
        let positive_mask = _mm256_cmp_ps(x, zero, _CMP_GE_OQ);
        let sign = _mm256_blendv_ps(neg_one, one, positive_mask);
        let tanh = _mm256_blendv_ps(approx, sign, sat_mask);
        let out = _mm256_mul_ps(tanh, softcap_v);
        unsafe {
            _mm256_storeu_ps(values.add(i), out);
        }
        i += 8;
    }

    while i < len {
        let value = unsafe { *values.add(i) };
        unsafe {
            *values.add(i) = fast_tanh_pade_scalar(value / softcap) * softcap;
        }
        i += 1;
    }
}

#[inline]
fn f32_dot_scalar(a: &[f32], b: &[f32]) -> f32 {
    let mut sum = 0.0;
    for i in 0..a.len() {
        sum += a[i] * b[i];
    }
    sum
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn f32_dot_avx2(a: *const f32, b: *const f32, len: usize) -> f32 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let mut i = 0_usize;
    let mut acc0 = _mm256_setzero_ps();
    let mut acc1 = _mm256_setzero_ps();
    let mut acc2 = _mm256_setzero_ps();
    let mut acc3 = _mm256_setzero_ps();

    while i + 32 <= len {
        let a0 = unsafe { _mm256_loadu_ps(a.add(i)) };
        let b0 = unsafe { _mm256_loadu_ps(b.add(i)) };
        let a1 = unsafe { _mm256_loadu_ps(a.add(i + 8)) };
        let b1 = unsafe { _mm256_loadu_ps(b.add(i + 8)) };
        let a2 = unsafe { _mm256_loadu_ps(a.add(i + 16)) };
        let b2 = unsafe { _mm256_loadu_ps(b.add(i + 16)) };
        let a3 = unsafe { _mm256_loadu_ps(a.add(i + 24)) };
        let b3 = unsafe { _mm256_loadu_ps(b.add(i + 24)) };
        acc0 = _mm256_add_ps(acc0, _mm256_mul_ps(a0, b0));
        acc1 = _mm256_add_ps(acc1, _mm256_mul_ps(a1, b1));
        acc2 = _mm256_add_ps(acc2, _mm256_mul_ps(a2, b2));
        acc3 = _mm256_add_ps(acc3, _mm256_mul_ps(a3, b3));
        i += 32;
    }

    acc0 = _mm256_add_ps(acc0, acc1);
    acc2 = _mm256_add_ps(acc2, acc3);
    acc0 = _mm256_add_ps(acc0, acc2);
    let mut lanes = [0.0_f32; 8];
    unsafe { _mm256_storeu_ps(lanes.as_mut_ptr(), acc0) };
    let mut sum = lanes.iter().sum::<f32>();
    while i < len {
        sum += unsafe { *a.add(i) } * unsafe { *b.add(i) };
        i += 1;
    }
    sum
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2,fma")]
unsafe fn f32_dot_avx2_fma(a: *const f32, b: *const f32, len: usize) -> f32 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let mut i = 0_usize;
    let mut acc0 = _mm256_setzero_ps();
    let mut acc1 = _mm256_setzero_ps();
    let mut acc2 = _mm256_setzero_ps();
    let mut acc3 = _mm256_setzero_ps();

    while i + 32 <= len {
        let a0 = unsafe { _mm256_loadu_ps(a.add(i)) };
        let b0 = unsafe { _mm256_loadu_ps(b.add(i)) };
        let a1 = unsafe { _mm256_loadu_ps(a.add(i + 8)) };
        let b1 = unsafe { _mm256_loadu_ps(b.add(i + 8)) };
        let a2 = unsafe { _mm256_loadu_ps(a.add(i + 16)) };
        let b2 = unsafe { _mm256_loadu_ps(b.add(i + 16)) };
        let a3 = unsafe { _mm256_loadu_ps(a.add(i + 24)) };
        let b3 = unsafe { _mm256_loadu_ps(b.add(i + 24)) };
        acc0 = _mm256_fmadd_ps(a0, b0, acc0);
        acc1 = _mm256_fmadd_ps(a1, b1, acc1);
        acc2 = _mm256_fmadd_ps(a2, b2, acc2);
        acc3 = _mm256_fmadd_ps(a3, b3, acc3);
        i += 32;
    }

    acc0 = _mm256_add_ps(acc0, acc1);
    acc2 = _mm256_add_ps(acc2, acc3);
    acc0 = _mm256_add_ps(acc0, acc2);
    let mut lanes = [0.0_f32; 8];
    unsafe { _mm256_storeu_ps(lanes.as_mut_ptr(), acc0) };
    let mut sum = lanes.iter().sum::<f32>();
    while i < len {
        sum += unsafe { *a.add(i) } * unsafe { *b.add(i) };
        i += 1;
    }
    sum
}

#[cfg(target_arch = "aarch64")]
#[inline]
unsafe fn f32_dot_neon(a: *const f32, b: *const f32, len: usize) -> f32 {
    use std::arch::aarch64::*;

    let mut i = 0_usize;
    let mut acc0 = unsafe { vdupq_n_f32(0.0) };
    let mut acc1 = unsafe { vdupq_n_f32(0.0) };
    let mut acc2 = unsafe { vdupq_n_f32(0.0) };
    let mut acc3 = unsafe { vdupq_n_f32(0.0) };
    while i + 16 <= len {
        let a0 = unsafe { vld1q_f32(a.add(i)) };
        let b0 = unsafe { vld1q_f32(b.add(i)) };
        let a1 = unsafe { vld1q_f32(a.add(i + 4)) };
        let b1 = unsafe { vld1q_f32(b.add(i + 4)) };
        let a2 = unsafe { vld1q_f32(a.add(i + 8)) };
        let b2 = unsafe { vld1q_f32(b.add(i + 8)) };
        let a3 = unsafe { vld1q_f32(a.add(i + 12)) };
        let b3 = unsafe { vld1q_f32(b.add(i + 12)) };
        acc0 = unsafe { vfmaq_f32(acc0, a0, b0) };
        acc1 = unsafe { vfmaq_f32(acc1, a1, b1) };
        acc2 = unsafe { vfmaq_f32(acc2, a2, b2) };
        acc3 = unsafe { vfmaq_f32(acc3, a3, b3) };
        i += 16;
    }

    let mut sum =
        unsafe { vaddvq_f32(acc0) + vaddvq_f32(acc1) + vaddvq_f32(acc2) + vaddvq_f32(acc3) };
    while i < len {
        sum += unsafe { *a.add(i) } * unsafe { *b.add(i) };
        i += 1;
    }
    sum
}

#[inline]
pub(crate) fn q8_i8_dot_f32_scaled_thermal_high(row: &[i8], x: &[f32], scale: f32) -> f32 {
    debug_assert_eq!(row.len(), x.len());
    let mut sum = 0.0;
    let len = row.len();
    let mut i = 0;
    while i < len {
        if (i / 16) % 2 == 0 {
            let limit = (i + 16).min(len);
            for j in i..limit {
                sum += row[j] as f32 * x[j];
            }
        }
        i += 16;
    }
    sum * scale * 2.0
}

#[inline]
pub fn q8_u8_dot_f32_scaled(row: &[u8], x: &[f32], scale: f32) -> f32 {
    debug_assert_eq!(row.len(), x.len());
    #[cfg(target_arch = "aarch64")]
    {
        // SAFETY: .zq8 stores two's-complement i8 payload bytes. Casting the read-only u8 pointer
        // to i8 preserves the byte pattern and the NEON routine only reads within the slice.
        unsafe { q8_dot_f32_neon(row.as_ptr().cast::<i8>(), x.as_ptr(), row.len(), scale) }
    }
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        #[cfg(target_arch = "x86_64")]
        {
            if is_x86_avx512f_bw_available() {
                // SAFETY: .zq8 stores two's-complement i8 payload bytes. Casting the read-only u8
                // pointer to i8 preserves the byte pattern; AVX-512 availability is checked.
                return unsafe {
                    q8_dot_f32_avx512(row.as_ptr().cast::<i8>(), x.as_ptr(), row.len(), scale)
                };
            }
        }
        if is_x86_avx2_fma_available() {
            // SAFETY: .zq8 stores two's-complement i8 payload bytes. Casting the read-only u8
            // pointer to i8 preserves the byte pattern; AVX2+FMA availability is checked.
            return unsafe {
                q8_dot_f32_avx2_fma(row.as_ptr().cast::<i8>(), x.as_ptr(), row.len(), scale)
            };
        }
        if is_x86_avx2_available() {
            // SAFETY: .zq8 stores two's-complement i8 payload bytes. Casting the read-only u8
            // pointer to i8 preserves the byte pattern; AVX2 availability is checked at runtime.
            return unsafe {
                q8_dot_f32_avx2(row.as_ptr().cast::<i8>(), x.as_ptr(), row.len(), scale)
            };
        }
    }
    #[cfg(not(target_arch = "aarch64"))]
    {
        q8_dot_f32_scalar(row.iter().map(|v| *v as i8), x, scale)
    }
}

#[inline]
pub fn q8_i8_dot2_f32_scaled(
    row_a: &[i8],
    row_b: &[i8],
    x: &[f32],
    scale_a: f32,
    scale_b: f32,
) -> (f32, f32) {
    debug_assert_eq!(row_a.len(), x.len());
    debug_assert_eq!(row_b.len(), x.len());
    if x.len() < Q8_DOT2_MIN_FUSED_LEN {
        return (
            q8_i8_dot_f32_scaled(row_a, x, scale_a),
            q8_i8_dot_f32_scaled(row_b, x, scale_b),
        );
    }
    if thermal_pressure_high() {
        return (
            q8_i8_dot_f32_scaled_thermal_high(row_a, x, scale_a),
            q8_i8_dot_f32_scaled_thermal_high(row_b, x, scale_b),
        );
    }
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if is_x86_avx2_fma_available() {
            // SAFETY: AVX2+FMA availability is checked at runtime. Both rows are validated to
            // match x length above and the routine only reads within those bounds.
            return unsafe {
                q8_i8_dot2_f32_avx2_fma(
                    row_a.as_ptr(),
                    row_b.as_ptr(),
                    x.as_ptr(),
                    x.len(),
                    scale_a,
                    scale_b,
                )
            };
        }
    }
    (
        q8_i8_dot_f32_scaled(row_a, x, scale_a),
        q8_i8_dot_f32_scaled(row_b, x, scale_b),
    )
}

#[inline]
#[allow(clippy::too_many_arguments)]
pub fn q8_i8_dot4_f32_scaled(
    row_a: &[i8],
    row_b: &[i8],
    row_c: &[i8],
    row_d: &[i8],
    x: &[f32],
    scale_a: f32,
    scale_b: f32,
    scale_c: f32,
    scale_d: f32,
) -> (f32, f32, f32, f32) {
    debug_assert_eq!(row_a.len(), x.len());
    debug_assert_eq!(row_b.len(), x.len());
    debug_assert_eq!(row_c.len(), x.len());
    debug_assert_eq!(row_d.len(), x.len());
    if x.len() < Q8_DOT2_MIN_FUSED_LEN {
        return (
            q8_i8_dot_f32_scaled(row_a, x, scale_a),
            q8_i8_dot_f32_scaled(row_b, x, scale_b),
            q8_i8_dot_f32_scaled(row_c, x, scale_c),
            q8_i8_dot_f32_scaled(row_d, x, scale_d),
        );
    }
    if thermal_pressure_high() {
        return (
            q8_i8_dot_f32_scaled_thermal_high(row_a, x, scale_a),
            q8_i8_dot_f32_scaled_thermal_high(row_b, x, scale_b),
            q8_i8_dot_f32_scaled_thermal_high(row_c, x, scale_c),
            q8_i8_dot_f32_scaled_thermal_high(row_d, x, scale_d),
        );
    }
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if is_x86_avx2_fma_available() {
            return unsafe {
                q8_i8_dot4_f32_avx2_fma(
                    row_a.as_ptr(),
                    row_b.as_ptr(),
                    row_c.as_ptr(),
                    row_d.as_ptr(),
                    x.as_ptr(),
                    x.len(),
                    scale_a,
                    scale_b,
                    scale_c,
                    scale_d,
                )
            };
        }
    }
    (
        q8_i8_dot_f32_scaled(row_a, x, scale_a),
        q8_i8_dot_f32_scaled(row_b, x, scale_b),
        q8_i8_dot_f32_scaled(row_c, x, scale_c),
        q8_i8_dot_f32_scaled(row_d, x, scale_d),
    )
}

#[inline]
pub fn q8_i8_dot_i8_scaled(lhs: &[i8], rhs: &[i8], lhs_scale: f32, rhs_scale: f32) -> f32 {
    debug_assert_eq!(lhs.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if lhs.len() >= 32 && is_x86_avx2_available() {
            // SAFETY: AVX2 availability is checked at runtime and both slices have equal len.
            return unsafe {
                q8_i8_dot_i8_avx2(lhs.as_ptr(), rhs.as_ptr(), lhs.len(), lhs_scale * rhs_scale)
            };
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("dotprod") {
            // SAFETY: dotprod availability is checked at runtime and both slices have equal len.
            return unsafe {
                q8_i8_dot_i8_sdot(lhs.as_ptr(), rhs.as_ptr(), lhs.len(), lhs_scale * rhs_scale)
            };
        }
    }
    q8_i8_dot_i8_scalar(lhs, rhs, lhs_scale * rhs_scale)
}

#[inline]
pub fn q8_u8_dot_i8_scaled(lhs: &[u8], rhs: &[i8], lhs_scale: f32, rhs_scale: f32) -> f32 {
    debug_assert_eq!(lhs.len(), rhs.len());
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if lhs.len() >= 32 && is_x86_avx2_available() {
            // SAFETY: .zq8 stores two's-complement i8 payload bytes. Casting preserves the byte
            // pattern, AVX2 availability is checked, and both slices have equal len.
            return unsafe {
                q8_i8_dot_i8_avx2(
                    lhs.as_ptr().cast::<i8>(),
                    rhs.as_ptr(),
                    lhs.len(),
                    lhs_scale * rhs_scale,
                )
            };
        }
    }
    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("dotprod") {
            // SAFETY: .zq8 stores two's-complement i8 payload bytes. Casting preserves the byte
            // pattern, dotprod availability is checked, and both slices have equal len.
            return unsafe {
                q8_i8_dot_i8_sdot(
                    lhs.as_ptr().cast::<i8>(),
                    rhs.as_ptr(),
                    lhs.len(),
                    lhs_scale * rhs_scale,
                )
            };
        }
    }
    q8_i8_dot_i8_scalar_cast(lhs, rhs, lhs_scale * rhs_scale)
}

#[inline]
fn q8_i8_dot_i8_scalar(lhs: &[i8], rhs: &[i8], scale: f32) -> f32 {
    lhs.iter()
        .zip(rhs)
        .map(|(a, b)| *a as f32 * *b as f32)
        .sum::<f32>()
        * scale
}

#[inline]
fn q8_i8_dot_i8_scalar_cast(lhs: &[u8], rhs: &[i8], scale: f32) -> f32 {
    lhs.iter()
        .zip(rhs)
        .map(|(a, b)| (*a as i8) as f32 * *b as f32)
        .sum::<f32>()
        * scale
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn q8_i8_dot_i8_avx2(lhs: *const i8, rhs: *const i8, len: usize, scale: f32) -> f32 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let mut i = 0_usize;
    let mut acc = _mm256_setzero_si256();
    while i + 16 <= len {
        // Widen signed i8 lanes to i16, then madd adjacent pairs into i32 lanes.
        let a128 = unsafe { _mm_loadu_si128(lhs.add(i).cast::<__m128i>()) };
        let b128 = unsafe { _mm_loadu_si128(rhs.add(i).cast::<__m128i>()) };
        let a16 = _mm256_cvtepi8_epi16(a128);
        let b16 = _mm256_cvtepi8_epi16(b128);
        acc = _mm256_add_epi32(acc, _mm256_madd_epi16(a16, b16));
        i += 16;
    }

    let mut lanes = [0_i32; 8];
    unsafe { _mm256_storeu_si256(lanes.as_mut_ptr().cast::<__m256i>(), acc) };
    let mut sum = lanes.iter().copied().sum::<i32>();
    while i < len {
        sum += unsafe { *lhs.add(i) } as i32 * unsafe { *rhs.add(i) } as i32;
        i += 1;
    }
    sum as f32 * scale
}

#[cfg(target_arch = "aarch64")]
#[target_feature(enable = "dotprod")]
unsafe fn q8_i8_dot_i8_sdot(lhs: *const i8, rhs: *const i8, len: usize, scale: f32) -> f32 {
    use std::arch::aarch64::*;
    use std::arch::asm;

    let mut i = 0_usize;
    let mut acc = vdupq_n_s32(0);
    while i + 16 <= len {
        let a = unsafe { vld1q_s8(lhs.add(i)) };
        let b = unsafe { vld1q_s8(rhs.add(i)) };
        unsafe {
            asm!(
                "sdot {acc:v}.4s, {a:v}.16b, {b:v}.16b",
                acc = inout(vreg) acc,
                a = in(vreg) a,
                b = in(vreg) b,
                options(nostack, preserves_flags)
            );
        }
        i += 16;
    }

    let mut sum = vaddvq_s32(acc);
    while i < len {
        sum += unsafe { *lhs.add(i) } as i32 * unsafe { *rhs.add(i) } as i32;
        i += 1;
    }
    sum as f32 * scale
}

#[inline]
#[cfg(not(target_arch = "aarch64"))]
fn q8_dot_f32_scalar<I>(row: I, x: &[f32], scale: f32) -> f32
where
    I: IntoIterator<Item = i8>,
{
    row.into_iter()
        .zip(x)
        .map(|(q, xv)| q as f32 * scale * xv)
        .sum()
}

#[cfg(target_arch = "aarch64")]
#[inline]
unsafe fn q8_dot_f32_neon(row: *const i8, x: *const f32, len: usize, scale: f32) -> f32 {
    use std::arch::aarch64::*;

    let mut i = 0_usize;
    let mut acc0 = unsafe { vdupq_n_f32(0.0) };
    let mut acc1 = unsafe { vdupq_n_f32(0.0) };
    let mut acc2 = unsafe { vdupq_n_f32(0.0) };
    let mut acc3 = unsafe { vdupq_n_f32(0.0) };

    while i + 16 <= len {
        let q8 = unsafe { vld1q_s8(row.add(i)) };
        let low16 = unsafe { vmovl_s8(vget_low_s8(q8)) };
        let high16 = unsafe { vmovl_s8(vget_high_s8(q8)) };

        let q0 = unsafe { vcvtq_f32_s32(vmovl_s16(vget_low_s16(low16))) };
        let q1 = unsafe { vcvtq_f32_s32(vmovl_s16(vget_high_s16(low16))) };
        let q2 = unsafe { vcvtq_f32_s32(vmovl_s16(vget_low_s16(high16))) };
        let q3 = unsafe { vcvtq_f32_s32(vmovl_s16(vget_high_s16(high16))) };

        let x0 = unsafe { vld1q_f32(x.add(i)) };
        let x1 = unsafe { vld1q_f32(x.add(i + 4)) };
        let x2 = unsafe { vld1q_f32(x.add(i + 8)) };
        let x3 = unsafe { vld1q_f32(x.add(i + 12)) };

        acc0 = unsafe { vfmaq_f32(acc0, q0, x0) };
        acc1 = unsafe { vfmaq_f32(acc1, q1, x1) };
        acc2 = unsafe { vfmaq_f32(acc2, q2, x2) };
        acc3 = unsafe { vfmaq_f32(acc3, q3, x3) };

        i += 16;
    }

    let mut sum =
        unsafe { vaddvq_f32(acc0) + vaddvq_f32(acc1) + vaddvq_f32(acc2) + vaddvq_f32(acc3) };
    while i < len {
        sum += unsafe { *row.add(i) } as f32 * unsafe { *x.add(i) };
        i += 1;
    }
    sum * scale
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[inline]
fn is_x86_avx2_available() -> bool {
    match X86_AVX2_AVAILABLE.load(Ordering::Relaxed) {
        THERMAL_LOW => return false,
        THERMAL_HIGH => return true,
        _ => {}
    }
    #[cfg(target_arch = "x86")]
    {
        let available = std::is_x86_feature_detected!("avx2");
        X86_AVX2_AVAILABLE.store(
            if available { THERMAL_HIGH } else { THERMAL_LOW },
            Ordering::Relaxed,
        );
        available
    }
    #[cfg(target_arch = "x86_64")]
    {
        let available = std::is_x86_feature_detected!("avx2");
        X86_AVX2_AVAILABLE.store(
            if available { THERMAL_HIGH } else { THERMAL_LOW },
            Ordering::Relaxed,
        );
        available
    }
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[inline]
fn is_x86_avx2_fma_available() -> bool {
    match X86_AVX2_FMA_AVAILABLE.load(Ordering::Relaxed) {
        THERMAL_LOW => return false,
        THERMAL_HIGH => return true,
        _ => {}
    }
    let available = std::is_x86_feature_detected!("avx2") && std::is_x86_feature_detected!("fma");
    X86_AVX2_FMA_AVAILABLE.store(
        if available { THERMAL_HIGH } else { THERMAL_LOW },
        Ordering::Relaxed,
    );
    available
}

#[cfg(target_arch = "x86_64")]
#[inline]
fn is_x86_avx512f_bw_available() -> bool {
    match X86_AVX512F_BW_AVAILABLE.load(Ordering::Relaxed) {
        THERMAL_LOW => return false,
        THERMAL_HIGH => return true,
        _ => {}
    }
    let available =
        std::is_x86_feature_detected!("avx512f") && std::is_x86_feature_detected!("avx512bw");
    X86_AVX512F_BW_AVAILABLE.store(
        if available { THERMAL_HIGH } else { THERMAL_LOW },
        Ordering::Relaxed,
    );
    available
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn q8_dot_f32_avx2(row: *const i8, x: *const f32, len: usize, scale: f32) -> f32 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let mut i = 0_usize;
    let mut acc = _mm256_setzero_ps();
    while i + 8 <= len {
        let bytes = unsafe { _mm_loadl_epi64(row.add(i).cast::<__m128i>()) };
        let q32 = _mm256_cvtepi8_epi32(bytes);
        let qf = _mm256_cvtepi32_ps(q32);
        let xf = unsafe { _mm256_loadu_ps(x.add(i)) };
        acc = _mm256_add_ps(acc, _mm256_mul_ps(qf, xf));
        i += 8;
    }

    let mut lanes = [0.0_f32; 8];
    unsafe { _mm256_storeu_ps(lanes.as_mut_ptr(), acc) };
    let mut sum = lanes.iter().sum::<f32>();
    while i < len {
        sum += unsafe { *row.add(i) } as f32 * unsafe { *x.add(i) };
        i += 1;
    }
    sum * scale
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2,fma")]
unsafe fn q8_dot_f32_avx2_fma(row: *const i8, x: *const f32, len: usize, scale: f32) -> f32 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let mut i = 0_usize;
    let mut acc = _mm256_setzero_ps();
    while i + 8 <= len {
        let bytes = unsafe { _mm_loadl_epi64(row.add(i).cast::<__m128i>()) };
        let q32 = _mm256_cvtepi8_epi32(bytes);
        let qf = _mm256_cvtepi32_ps(q32);
        let xf = unsafe { _mm256_loadu_ps(x.add(i)) };
        acc = _mm256_fmadd_ps(qf, xf, acc);
        i += 8;
    }

    let mut lanes = [0.0_f32; 8];
    unsafe { _mm256_storeu_ps(lanes.as_mut_ptr(), acc) };
    let mut sum = lanes.iter().sum::<f32>();
    while i < len {
        sum += unsafe { *row.add(i) } as f32 * unsafe { *x.add(i) };
        i += 1;
    }
    sum * scale
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2,fma")]
#[allow(unused_unsafe)]
unsafe fn q8_i8_dot2_f32_avx2_fma(
    row_a: *const i8,
    row_b: *const i8,
    x: *const f32,
    len: usize,
    scale_a: f32,
    scale_b: f32,
) -> (f32, f32) {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let mut i = 0_usize;
    let mut acc_a = unsafe { _mm256_setzero_ps() };
    let mut acc_b = unsafe { _mm256_setzero_ps() };

    while i + 16 <= len {
        let a0 = unsafe { _mm_loadl_epi64(row_a.add(i).cast::<__m128i>()) };
        let b0 = unsafe { _mm_loadl_epi64(row_b.add(i).cast::<__m128i>()) };
        let x0 = unsafe { _mm256_loadu_ps(x.add(i)) };
        let qa0 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(a0)) };
        let qb0 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(b0)) };
        acc_a = unsafe { _mm256_fmadd_ps(qa0, x0, acc_a) };
        acc_b = unsafe { _mm256_fmadd_ps(qb0, x0, acc_b) };

        let a1 = unsafe { _mm_loadl_epi64(row_a.add(i + 8).cast::<__m128i>()) };
        let b1 = unsafe { _mm_loadl_epi64(row_b.add(i + 8).cast::<__m128i>()) };
        let x1 = unsafe { _mm256_loadu_ps(x.add(i + 8)) };
        let qa1 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(a1)) };
        let qb1 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(b1)) };
        acc_a = unsafe { _mm256_fmadd_ps(qa1, x1, acc_a) };
        acc_b = unsafe { _mm256_fmadd_ps(qb1, x1, acc_b) };

        i += 16;
    }

    let mut lanes_a = [0.0_f32; 8];
    let mut lanes_b = [0.0_f32; 8];
    unsafe {
        _mm256_storeu_ps(lanes_a.as_mut_ptr(), acc_a);
        _mm256_storeu_ps(lanes_b.as_mut_ptr(), acc_b);
    }
    let mut sum_a = lanes_a.iter().sum::<f32>();
    let mut sum_b = lanes_b.iter().sum::<f32>();

    while i < len {
        let xv = unsafe { *x.add(i) };
        sum_a += unsafe { *row_a.add(i) } as f32 * xv;
        sum_b += unsafe { *row_b.add(i) } as f32 * xv;
        i += 1;
    }

    (sum_a * scale_a, sum_b * scale_b)
}

/// # Safety
///
/// `row_a`, `row_b`, `row_c`, `row_d`, and `x` must point to valid arrays of `f32` of at least `len` elements.
/// The host CPU must support AVX2 and FMA instructions.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2,fma")]
#[allow(unused_unsafe, clippy::too_many_arguments)]
pub unsafe fn f32_dot4_avx2_fma(
    row_a: *const f32,
    row_b: *const f32,
    row_c: *const f32,
    row_d: *const f32,
    x: *const f32,
    len: usize,
) -> (f32, f32, f32, f32) {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let mut i = 0_usize;
    let mut acc_a = unsafe { _mm256_setzero_ps() };
    let mut acc_b = unsafe { _mm256_setzero_ps() };
    let mut acc_c = unsafe { _mm256_setzero_ps() };
    let mut acc_d = unsafe { _mm256_setzero_ps() };

    while i + 8 <= len {
        let x0 = unsafe { _mm256_loadu_ps(x.add(i)) };
        let a0 = unsafe { _mm256_loadu_ps(row_a.add(i)) };
        let b0 = unsafe { _mm256_loadu_ps(row_b.add(i)) };
        let c0 = unsafe { _mm256_loadu_ps(row_c.add(i)) };
        let d0 = unsafe { _mm256_loadu_ps(row_d.add(i)) };

        acc_a = unsafe { _mm256_fmadd_ps(a0, x0, acc_a) };
        acc_b = unsafe { _mm256_fmadd_ps(b0, x0, acc_b) };
        acc_c = unsafe { _mm256_fmadd_ps(c0, x0, acc_c) };
        acc_d = unsafe { _mm256_fmadd_ps(d0, x0, acc_d) };

        i += 8;
    }

    let mut lanes_a = [0.0_f32; 8];
    let mut lanes_b = [0.0_f32; 8];
    let mut lanes_c = [0.0_f32; 8];
    let mut lanes_d = [0.0_f32; 8];
    unsafe {
        _mm256_storeu_ps(lanes_a.as_mut_ptr(), acc_a);
        _mm256_storeu_ps(lanes_b.as_mut_ptr(), acc_b);
        _mm256_storeu_ps(lanes_c.as_mut_ptr(), acc_c);
        _mm256_storeu_ps(lanes_d.as_mut_ptr(), acc_d);
    }
    let mut sum_a = lanes_a.iter().sum::<f32>();
    let mut sum_b = lanes_b.iter().sum::<f32>();
    let mut sum_c = lanes_c.iter().sum::<f32>();
    let mut sum_d = lanes_d.iter().sum::<f32>();

    while i < len {
        let xv = unsafe { *x.add(i) };
        sum_a += unsafe { *row_a.add(i) } * xv;
        sum_b += unsafe { *row_b.add(i) } * xv;
        sum_c += unsafe { *row_c.add(i) } * xv;
        sum_d += unsafe { *row_d.add(i) } * xv;
        i += 1;
    }

    (sum_a, sum_b, sum_c, sum_d)
}

/// # Safety
///
/// `row_a` through `row_h` and `x` must point to valid arrays of `f32` of at least `len` elements.
/// The host CPU must support AVX2 and FMA instructions.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2,fma")]
#[allow(unused_unsafe, clippy::too_many_arguments)]
pub unsafe fn f32_dot8_avx2_fma(
    row_a: *const f32,
    row_b: *const f32,
    row_c: *const f32,
    row_d: *const f32,
    row_e: *const f32,
    row_f: *const f32,
    row_g: *const f32,
    row_h: *const f32,
    x: *const f32,
    len: usize,
) -> (f32, f32, f32, f32, f32, f32, f32, f32) {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let mut i = 0_usize;
    let mut acc_a = unsafe { _mm256_setzero_ps() };
    let mut acc_b = unsafe { _mm256_setzero_ps() };
    let mut acc_c = unsafe { _mm256_setzero_ps() };
    let mut acc_d = unsafe { _mm256_setzero_ps() };
    let mut acc_e = unsafe { _mm256_setzero_ps() };
    let mut acc_f = unsafe { _mm256_setzero_ps() };
    let mut acc_g = unsafe { _mm256_setzero_ps() };
    let mut acc_h = unsafe { _mm256_setzero_ps() };

    while i + 8 <= len {
        let x0 = unsafe { _mm256_loadu_ps(x.add(i)) };
        let a0 = unsafe { _mm256_loadu_ps(row_a.add(i)) };
        let b0 = unsafe { _mm256_loadu_ps(row_b.add(i)) };
        let c0 = unsafe { _mm256_loadu_ps(row_c.add(i)) };
        let d0 = unsafe { _mm256_loadu_ps(row_d.add(i)) };
        let e0 = unsafe { _mm256_loadu_ps(row_e.add(i)) };
        let f0 = unsafe { _mm256_loadu_ps(row_f.add(i)) };
        let g0 = unsafe { _mm256_loadu_ps(row_g.add(i)) };
        let h0 = unsafe { _mm256_loadu_ps(row_h.add(i)) };

        acc_a = unsafe { _mm256_fmadd_ps(a0, x0, acc_a) };
        acc_b = unsafe { _mm256_fmadd_ps(b0, x0, acc_b) };
        acc_c = unsafe { _mm256_fmadd_ps(c0, x0, acc_c) };
        acc_d = unsafe { _mm256_fmadd_ps(d0, x0, acc_d) };
        acc_e = unsafe { _mm256_fmadd_ps(e0, x0, acc_e) };
        acc_f = unsafe { _mm256_fmadd_ps(f0, x0, acc_f) };
        acc_g = unsafe { _mm256_fmadd_ps(g0, x0, acc_g) };
        acc_h = unsafe { _mm256_fmadd_ps(h0, x0, acc_h) };

        i += 8;
    }

    let mut lanes_a = [0.0_f32; 8];
    let mut lanes_b = [0.0_f32; 8];
    let mut lanes_c = [0.0_f32; 8];
    let mut lanes_d = [0.0_f32; 8];
    let mut lanes_e = [0.0_f32; 8];
    let mut lanes_f = [0.0_f32; 8];
    let mut lanes_g = [0.0_f32; 8];
    let mut lanes_h = [0.0_f32; 8];
    unsafe {
        _mm256_storeu_ps(lanes_a.as_mut_ptr(), acc_a);
        _mm256_storeu_ps(lanes_b.as_mut_ptr(), acc_b);
        _mm256_storeu_ps(lanes_c.as_mut_ptr(), acc_c);
        _mm256_storeu_ps(lanes_d.as_mut_ptr(), acc_d);
        _mm256_storeu_ps(lanes_e.as_mut_ptr(), acc_e);
        _mm256_storeu_ps(lanes_f.as_mut_ptr(), acc_f);
        _mm256_storeu_ps(lanes_g.as_mut_ptr(), acc_g);
        _mm256_storeu_ps(lanes_h.as_mut_ptr(), acc_h);
    }
    let mut sum_a = lanes_a.iter().sum::<f32>();
    let mut sum_b = lanes_b.iter().sum::<f32>();
    let mut sum_c = lanes_c.iter().sum::<f32>();
    let mut sum_d = lanes_d.iter().sum::<f32>();
    let mut sum_e = lanes_e.iter().sum::<f32>();
    let mut sum_f = lanes_f.iter().sum::<f32>();
    let mut sum_g = lanes_g.iter().sum::<f32>();
    let mut sum_h = lanes_h.iter().sum::<f32>();

    while i < len {
        let xv = unsafe { *x.add(i) };
        sum_a += unsafe { *row_a.add(i) } * xv;
        sum_b += unsafe { *row_b.add(i) } * xv;
        sum_c += unsafe { *row_c.add(i) } * xv;
        sum_d += unsafe { *row_d.add(i) } * xv;
        sum_e += unsafe { *row_e.add(i) } * xv;
        sum_f += unsafe { *row_f.add(i) } * xv;
        sum_g += unsafe { *row_g.add(i) } * xv;
        sum_h += unsafe { *row_h.add(i) } * xv;
        i += 1;
    }

    (sum_a, sum_b, sum_c, sum_d, sum_e, sum_f, sum_g, sum_h)
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2,fma")]
#[allow(unused_unsafe, clippy::too_many_arguments, clippy::missing_safety_doc)]
unsafe fn q8_i8_dot4_f32_avx2_fma(
    row_a: *const i8,
    row_b: *const i8,
    row_c: *const i8,
    row_d: *const i8,
    x: *const f32,
    len: usize,
    scale_a: f32,
    scale_b: f32,
    scale_c: f32,
    scale_d: f32,
) -> (f32, f32, f32, f32) {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let mut i = 0_usize;
    let mut acc_a = unsafe { _mm256_setzero_ps() };
    let mut acc_b = unsafe { _mm256_setzero_ps() };
    let mut acc_c = unsafe { _mm256_setzero_ps() };
    let mut acc_d = unsafe { _mm256_setzero_ps() };

    while i + 16 <= len {
        let a0 = unsafe { _mm_loadl_epi64(row_a.add(i).cast::<__m128i>()) };
        let b0 = unsafe { _mm_loadl_epi64(row_b.add(i).cast::<__m128i>()) };
        let c0 = unsafe { _mm_loadl_epi64(row_c.add(i).cast::<__m128i>()) };
        let d0 = unsafe { _mm_loadl_epi64(row_d.add(i).cast::<__m128i>()) };
        let x0 = unsafe { _mm256_loadu_ps(x.add(i)) };
        acc_a = unsafe { _mm256_fmadd_ps(_mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(a0)), x0, acc_a) };
        acc_b = unsafe { _mm256_fmadd_ps(_mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(b0)), x0, acc_b) };
        acc_c = unsafe { _mm256_fmadd_ps(_mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(c0)), x0, acc_c) };
        acc_d = unsafe { _mm256_fmadd_ps(_mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(d0)), x0, acc_d) };

        let a1 = unsafe { _mm_loadl_epi64(row_a.add(i + 8).cast::<__m128i>()) };
        let b1 = unsafe { _mm_loadl_epi64(row_b.add(i + 8).cast::<__m128i>()) };
        let c1 = unsafe { _mm_loadl_epi64(row_c.add(i + 8).cast::<__m128i>()) };
        let d1 = unsafe { _mm_loadl_epi64(row_d.add(i + 8).cast::<__m128i>()) };
        let x1 = unsafe { _mm256_loadu_ps(x.add(i + 8)) };
        acc_a = unsafe { _mm256_fmadd_ps(_mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(a1)), x1, acc_a) };
        acc_b = unsafe { _mm256_fmadd_ps(_mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(b1)), x1, acc_b) };
        acc_c = unsafe { _mm256_fmadd_ps(_mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(c1)), x1, acc_c) };
        acc_d = unsafe { _mm256_fmadd_ps(_mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(d1)), x1, acc_d) };

        i += 16;
    }

    let mut lanes_a = [0.0_f32; 8];
    let mut lanes_b = [0.0_f32; 8];
    let mut lanes_c = [0.0_f32; 8];
    let mut lanes_d = [0.0_f32; 8];
    unsafe {
        _mm256_storeu_ps(lanes_a.as_mut_ptr(), acc_a);
        _mm256_storeu_ps(lanes_b.as_mut_ptr(), acc_b);
        _mm256_storeu_ps(lanes_c.as_mut_ptr(), acc_c);
        _mm256_storeu_ps(lanes_d.as_mut_ptr(), acc_d);
    }
    let mut sum_a = lanes_a.iter().sum::<f32>();
    let mut sum_b = lanes_b.iter().sum::<f32>();
    let mut sum_c = lanes_c.iter().sum::<f32>();
    let mut sum_d = lanes_d.iter().sum::<f32>();

    while i < len {
        let xv = unsafe { *x.add(i) };
        sum_a += unsafe { *row_a.add(i) } as f32 * xv;
        sum_b += unsafe { *row_b.add(i) } as f32 * xv;
        sum_c += unsafe { *row_c.add(i) } as f32 * xv;
        sum_d += unsafe { *row_d.add(i) } as f32 * xv;
        i += 1;
    }

    (
        sum_a * scale_a,
        sum_b * scale_b,
        sum_c * scale_c,
        sum_d * scale_d,
    )
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx512f,avx512bw")]
unsafe fn q8_dot_f32_avx512(row: *const i8, x: *const f32, len: usize, scale: f32) -> f32 {
    use std::arch::x86_64::*;

    let mut i = 0_usize;
    let mut acc = _mm512_setzero_ps();
    while i + 16 <= len {
        let bytes = unsafe { _mm_loadu_si128(row.add(i).cast::<__m128i>()) };
        let q32 = _mm512_cvtepi8_epi32(bytes);
        let qf = _mm512_cvtepi32_ps(q32);
        let xf = unsafe { _mm512_loadu_ps(x.add(i)) };
        acc = _mm512_add_ps(acc, _mm512_mul_ps(qf, xf));
        i += 16;
    }

    let mut lanes = [0.0_f32; 16];
    unsafe { _mm512_storeu_ps(lanes.as_mut_ptr(), acc) };
    let mut sum = lanes.iter().sum::<f32>();
    while i < len {
        sum += unsafe { *row.add(i) } as f32 * unsafe { *x.add(i) };
        i += 1;
    }
    sum * scale
}

#[inline]
pub fn q4_dot_f32_scaled(row_packed: &[u8], x: &[f32], scale: f32) -> f32 {
    let kernel = select_q4_dot_kernel();
    q4_dot_f32_scaled_with_kernel(row_packed, x, scale, kernel)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Q4DotKernel {
    ThermalHigh,
    #[cfg(target_arch = "aarch64")]
    Neon,
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    Avx2Fma,
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    Avx2,
    Scalar,
}

#[inline]
pub fn select_q4_dot_kernel() -> Q4DotKernel {
    if thermal_pressure_high() {
        Q4DotKernel::ThermalHigh
    } else {
        #[cfg(target_arch = "aarch64")]
        {
            Q4DotKernel::Neon
        }
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            if is_x86_avx2_fma_available() {
                Q4DotKernel::Avx2Fma
            } else if is_x86_avx2_available() {
                Q4DotKernel::Avx2
            } else {
                Q4DotKernel::Scalar
            }
        }
        #[cfg(not(any(target_arch = "aarch64", target_arch = "x86", target_arch = "x86_64")))]
        {
            Q4DotKernel::Scalar
        }
    }
}

#[inline]
pub fn q4_dot_f32_scaled_with_kernel(
    row_packed: &[u8],
    x: &[f32],
    scale: f32,
    kernel: Q4DotKernel,
) -> f32 {
    debug_assert_eq!(row_packed.len(), x.len().div_ceil(2));
    match kernel {
        Q4DotKernel::ThermalHigh => q4_dot_f32_scaled_thermal_high(row_packed, x, scale),
        #[cfg(target_arch = "aarch64")]
        Q4DotKernel::Neon => unsafe {
            q4_dot_f32_neon(row_packed.as_ptr(), x.as_ptr(), x.len(), scale)
        },
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        Q4DotKernel::Avx2Fma => unsafe {
            q4_dot_f32_avx2_fma(row_packed.as_ptr(), x.as_ptr(), x.len(), scale)
        },
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        Q4DotKernel::Avx2 => unsafe {
            q4_dot_f32_avx2(row_packed.as_ptr(), x.as_ptr(), x.len(), scale)
        },
        Q4DotKernel::Scalar => q4_dot_f32_scalar(row_packed, x, scale),
    }
}

#[inline]
pub fn q4_dot2_f32_scaled(
    row_a_packed: &[u8],
    row_b_packed: &[u8],
    x: &[f32],
    scale_a: f32,
    scale_b: f32,
) -> (f32, f32) {
    let kernel = select_q4_dot_kernel();
    q4_dot2_f32_scaled_with_kernel(row_a_packed, row_b_packed, x, scale_a, scale_b, kernel)
}

#[inline]
pub fn q4_dot2_f32_scaled_with_kernel(
    row_a_packed: &[u8],
    row_b_packed: &[u8],
    x: &[f32],
    scale_a: f32,
    scale_b: f32,
    kernel: Q4DotKernel,
) -> (f32, f32) {
    debug_assert_eq!(row_a_packed.len(), x.len().div_ceil(2));
    debug_assert_eq!(row_b_packed.len(), x.len().div_ceil(2));
    if x.len() < Q4_DOT2_MIN_FUSED_LEN {
        return (
            q4_dot_f32_scaled_with_kernel(row_a_packed, x, scale_a, kernel),
            q4_dot_f32_scaled_with_kernel(row_b_packed, x, scale_b, kernel),
        );
    }
    match kernel {
        Q4DotKernel::ThermalHigh => (
            q4_dot_f32_scaled_thermal_high(row_a_packed, x, scale_a),
            q4_dot_f32_scaled_thermal_high(row_b_packed, x, scale_b),
        ),
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        Q4DotKernel::Avx2Fma => {
            // SAFETY: AVX2+FMA availability is checked at runtime. The packed row lengths are
            // validated against x above and the routine only reads within those ranges.
            unsafe {
                q4_dot2_f32_avx2_fma(
                    row_a_packed.as_ptr(),
                    row_b_packed.as_ptr(),
                    x.as_ptr(),
                    x.len(),
                    scale_a,
                    scale_b,
                )
            }
        }
        _ => (
            q4_dot_f32_scaled_with_kernel(row_a_packed, x, scale_a, kernel),
            q4_dot_f32_scaled_with_kernel(row_b_packed, x, scale_b, kernel),
        ),
    }
}

#[inline]
#[allow(clippy::too_many_arguments)]
pub fn q4_dot4_f32_scaled(
    row_a_packed: &[u8],
    row_b_packed: &[u8],
    row_c_packed: &[u8],
    row_d_packed: &[u8],
    x: &[f32],
    scale_a: f32,
    scale_b: f32,
    scale_c: f32,
    scale_d: f32,
) -> (f32, f32, f32, f32) {
    let kernel = select_q4_dot_kernel();
    q4_dot4_f32_scaled_with_kernel(
        row_a_packed,
        row_b_packed,
        row_c_packed,
        row_d_packed,
        x,
        scale_a,
        scale_b,
        scale_c,
        scale_d,
        kernel,
    )
}

#[inline]
#[allow(clippy::too_many_arguments)]
pub fn q4_dot4_f32_scaled_with_kernel(
    row_a_packed: &[u8],
    row_b_packed: &[u8],
    row_c_packed: &[u8],
    row_d_packed: &[u8],
    x: &[f32],
    scale_a: f32,
    scale_b: f32,
    scale_c: f32,
    scale_d: f32,
    kernel: Q4DotKernel,
) -> (f32, f32, f32, f32) {
    debug_assert_eq!(row_a_packed.len(), x.len().div_ceil(2));
    debug_assert_eq!(row_b_packed.len(), x.len().div_ceil(2));
    debug_assert_eq!(row_c_packed.len(), x.len().div_ceil(2));
    debug_assert_eq!(row_d_packed.len(), x.len().div_ceil(2));
    if x.len() < Q4_DOT2_MIN_FUSED_LEN {
        return (
            q4_dot_f32_scaled_with_kernel(row_a_packed, x, scale_a, kernel),
            q4_dot_f32_scaled_with_kernel(row_b_packed, x, scale_b, kernel),
            q4_dot_f32_scaled_with_kernel(row_c_packed, x, scale_c, kernel),
            q4_dot_f32_scaled_with_kernel(row_d_packed, x, scale_d, kernel),
        );
    }
    match kernel {
        Q4DotKernel::ThermalHigh => (
            q4_dot_f32_scaled_thermal_high(row_a_packed, x, scale_a),
            q4_dot_f32_scaled_thermal_high(row_b_packed, x, scale_b),
            q4_dot_f32_scaled_thermal_high(row_c_packed, x, scale_c),
            q4_dot_f32_scaled_thermal_high(row_d_packed, x, scale_d),
        ),
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        Q4DotKernel::Avx2Fma => unsafe {
            q4_dot4_f32_avx2_fma(
                row_a_packed.as_ptr(),
                row_b_packed.as_ptr(),
                row_c_packed.as_ptr(),
                row_d_packed.as_ptr(),
                x.as_ptr(),
                x.len(),
                scale_a,
                scale_b,
                scale_c,
                scale_d,
            )
        },
        _ => (
            q4_dot_f32_scaled_with_kernel(row_a_packed, x, scale_a, kernel),
            q4_dot_f32_scaled_with_kernel(row_b_packed, x, scale_b, kernel),
            q4_dot_f32_scaled_with_kernel(row_c_packed, x, scale_c, kernel),
            q4_dot_f32_scaled_with_kernel(row_d_packed, x, scale_d, kernel),
        ),
    }
}

#[inline]
pub(crate) fn q4_dot_f32_scaled_thermal_high(row_packed: &[u8], x: &[f32], scale: f32) -> f32 {
    debug_assert_eq!(row_packed.len(), x.len().div_ceil(2));
    let mut sum = 0.0;
    let len = x.len();
    let mut i = 0;
    while i < len {
        if (i / 16) % 2 == 0 {
            let limit = (i + 16).min(len);
            for (j, value) in x.iter().enumerate().take(limit).skip(i) {
                let byte_idx = j / 2;
                let byte = row_packed[byte_idx];
                let val = if j % 2 == 0 {
                    (byte & 0x0F) as i8 - 8
                } else {
                    ((byte >> 4) & 0x0F) as i8 - 8
                };
                sum += val as f32 * *value;
            }
        }
        i += 16;
    }
    sum * scale * 2.0
}

#[inline]
#[cfg_attr(target_arch = "aarch64", allow(dead_code))]
fn q4_dot_f32_scalar(row_packed: &[u8], x: &[f32], scale: f32) -> f32 {
    let mut acc = 0.0f32;
    let len = x.len();
    let packed_len = len / 2;
    for i in 0..packed_len {
        let byte = row_packed[i];
        let q0 = (byte & 0x0f) as i8 - 8;
        let q1 = (byte >> 4) as i8 - 8;
        acc += (q0 as f32) * x[i * 2] + (q1 as f32) * x[i * 2 + 1];
    }
    if !len.is_multiple_of(2) {
        let byte = row_packed[packed_len];
        let q0 = (byte & 0x0f) as i8 - 8;
        acc += (q0 as f32) * x[packed_len * 2];
    }
    acc * scale
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
#[allow(unused_unsafe)]
unsafe fn q4_dot_f32_avx2(row_packed: *const u8, x: *const f32, len: usize, scale: f32) -> f32 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let mut i = 0_usize;
    let mut acc = unsafe { _mm256_setzero_ps() };
    let low_mask = unsafe { _mm_set1_epi8(0x0f) };
    let eight = unsafe { _mm_set1_epi8(8) };

    while i + 32 <= len {
        // Load 16 bytes (32 elements)
        let bytes = unsafe { _mm_loadu_si128(row_packed.add(i / 2).cast::<__m128i>()) };

        // Extract low and high nibbles
        let low_nibbles = unsafe { _mm_and_si128(bytes, low_mask) };
        let high_nibbles = unsafe { _mm_and_si128(_mm_srli_epi16(bytes, 4), low_mask) };

        // Subtract 8
        let q_low = unsafe { _mm_sub_epi8(low_nibbles, eight) };
        let q_high = unsafe { _mm_sub_epi8(high_nibbles, eight) };

        // Interleave low and high nibbles
        let int_low = unsafe { _mm_unpacklo_epi8(q_low, q_high) };
        let int_high = unsafe { _mm_unpackhi_epi8(q_low, q_high) };

        // Convert and multiply elements 0..7
        let f0 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(int_low)) };
        let x0 = unsafe { _mm256_loadu_ps(x.add(i)) };
        acc = unsafe { _mm256_add_ps(acc, _mm256_mul_ps(f0, x0)) };

        // Convert and multiply elements 8..15
        let f1 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(_mm_srli_si128(int_low, 8))) };
        let x1 = unsafe { _mm256_loadu_ps(x.add(i + 8)) };
        acc = unsafe { _mm256_add_ps(acc, _mm256_mul_ps(f1, x1)) };

        // Convert and multiply elements 16..23
        let f2 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(int_high)) };
        let x2 = unsafe { _mm256_loadu_ps(x.add(i + 16)) };
        acc = unsafe { _mm256_add_ps(acc, _mm256_mul_ps(f2, x2)) };

        // Convert and multiply elements 24..31
        let f3 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(_mm_srli_si128(int_high, 8))) };
        let x3 = unsafe { _mm256_loadu_ps(x.add(i + 24)) };
        acc = unsafe { _mm256_add_ps(acc, _mm256_mul_ps(f3, x3)) };

        i += 32;
    }

    let mut lanes = [0.0_f32; 8];
    unsafe {
        _mm256_storeu_ps(lanes.as_mut_ptr(), acc);
    }
    let mut sum = lanes.iter().sum::<f32>();

    while i < len {
        let byte = unsafe { *row_packed.add(i / 2) };
        let nibble = if i.is_multiple_of(2) {
            byte & 0x0f
        } else {
            byte >> 4
        };
        let q = nibble as i8 - 8;
        sum += q as f32 * unsafe { *x.add(i) };
        i += 1;
    }
    sum * scale
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2,fma")]
#[allow(unused_unsafe)]
unsafe fn q4_dot_f32_avx2_fma(row_packed: *const u8, x: *const f32, len: usize, scale: f32) -> f32 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let mut i = 0_usize;
    let mut acc = unsafe { _mm256_setzero_ps() };
    let low_mask = unsafe { _mm_set1_epi8(0x0f) };
    let eight = unsafe { _mm_set1_epi8(8) };

    while i + 32 <= len {
        let bytes = unsafe { _mm_loadu_si128(row_packed.add(i / 2).cast::<__m128i>()) };

        let low_nibbles = unsafe { _mm_and_si128(bytes, low_mask) };
        let high_nibbles = unsafe { _mm_and_si128(_mm_srli_epi16(bytes, 4), low_mask) };

        let q_low = unsafe { _mm_sub_epi8(low_nibbles, eight) };
        let q_high = unsafe { _mm_sub_epi8(high_nibbles, eight) };

        let int_low = unsafe { _mm_unpacklo_epi8(q_low, q_high) };
        let int_high = unsafe { _mm_unpackhi_epi8(q_low, q_high) };

        let f0 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(int_low)) };
        let x0 = unsafe { _mm256_loadu_ps(x.add(i)) };
        acc = unsafe { _mm256_fmadd_ps(f0, x0, acc) };

        let f1 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(_mm_srli_si128(int_low, 8))) };
        let x1 = unsafe { _mm256_loadu_ps(x.add(i + 8)) };
        acc = unsafe { _mm256_fmadd_ps(f1, x1, acc) };

        let f2 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(int_high)) };
        let x2 = unsafe { _mm256_loadu_ps(x.add(i + 16)) };
        acc = unsafe { _mm256_fmadd_ps(f2, x2, acc) };

        let f3 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(_mm_srli_si128(int_high, 8))) };
        let x3 = unsafe { _mm256_loadu_ps(x.add(i + 24)) };
        acc = unsafe { _mm256_fmadd_ps(f3, x3, acc) };

        i += 32;
    }

    let mut lanes = [0.0_f32; 8];
    unsafe {
        _mm256_storeu_ps(lanes.as_mut_ptr(), acc);
    }
    let mut sum = lanes.iter().sum::<f32>();

    while i < len {
        let byte = unsafe { *row_packed.add(i / 2) };
        let nibble = if i.is_multiple_of(2) {
            byte & 0x0f
        } else {
            byte >> 4
        };
        let q = nibble as i8 - 8;
        sum += q as f32 * unsafe { *x.add(i) };
        i += 1;
    }
    sum * scale
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2,fma")]
#[allow(unused_unsafe)]
unsafe fn q4_dot2_f32_avx2_fma(
    row_a_packed: *const u8,
    row_b_packed: *const u8,
    x: *const f32,
    len: usize,
    scale_a: f32,
    scale_b: f32,
) -> (f32, f32) {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let mut i = 0_usize;
    let mut acc_a = unsafe { _mm256_setzero_ps() };
    let mut acc_b = unsafe { _mm256_setzero_ps() };
    let low_mask = unsafe { _mm_set1_epi8(0x0f) };
    let eight = unsafe { _mm_set1_epi8(8) };

    while i + 32 <= len {
        let bytes_a = unsafe { _mm_loadu_si128(row_a_packed.add(i / 2).cast::<__m128i>()) };
        let bytes_b = unsafe { _mm_loadu_si128(row_b_packed.add(i / 2).cast::<__m128i>()) };

        let low_a = unsafe { _mm_and_si128(bytes_a, low_mask) };
        let high_a = unsafe { _mm_and_si128(_mm_srli_epi16(bytes_a, 4), low_mask) };
        let q_low_a = unsafe { _mm_sub_epi8(low_a, eight) };
        let q_high_a = unsafe { _mm_sub_epi8(high_a, eight) };
        let int_low_a = unsafe { _mm_unpacklo_epi8(q_low_a, q_high_a) };
        let int_high_a = unsafe { _mm_unpackhi_epi8(q_low_a, q_high_a) };

        let low_b = unsafe { _mm_and_si128(bytes_b, low_mask) };
        let high_b = unsafe { _mm_and_si128(_mm_srli_epi16(bytes_b, 4), low_mask) };
        let q_low_b = unsafe { _mm_sub_epi8(low_b, eight) };
        let q_high_b = unsafe { _mm_sub_epi8(high_b, eight) };
        let int_low_b = unsafe { _mm_unpacklo_epi8(q_low_b, q_high_b) };
        let int_high_b = unsafe { _mm_unpackhi_epi8(q_low_b, q_high_b) };

        let x0 = unsafe { _mm256_loadu_ps(x.add(i)) };
        let x1 = unsafe { _mm256_loadu_ps(x.add(i + 8)) };
        let x2 = unsafe { _mm256_loadu_ps(x.add(i + 16)) };
        let x3 = unsafe { _mm256_loadu_ps(x.add(i + 24)) };

        let a0 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(int_low_a)) };
        let a1 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(_mm_srli_si128(int_low_a, 8))) };
        let a2 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(int_high_a)) };
        let a3 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(_mm_srli_si128(int_high_a, 8))) };
        acc_a = unsafe { _mm256_fmadd_ps(a0, x0, acc_a) };
        acc_a = unsafe { _mm256_fmadd_ps(a1, x1, acc_a) };
        acc_a = unsafe { _mm256_fmadd_ps(a2, x2, acc_a) };
        acc_a = unsafe { _mm256_fmadd_ps(a3, x3, acc_a) };

        let b0 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(int_low_b)) };
        let b1 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(_mm_srli_si128(int_low_b, 8))) };
        let b2 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(int_high_b)) };
        let b3 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(_mm_srli_si128(int_high_b, 8))) };
        acc_b = unsafe { _mm256_fmadd_ps(b0, x0, acc_b) };
        acc_b = unsafe { _mm256_fmadd_ps(b1, x1, acc_b) };
        acc_b = unsafe { _mm256_fmadd_ps(b2, x2, acc_b) };
        acc_b = unsafe { _mm256_fmadd_ps(b3, x3, acc_b) };

        i += 32;
    }

    let mut lanes_a = [0.0_f32; 8];
    let mut lanes_b = [0.0_f32; 8];
    unsafe {
        _mm256_storeu_ps(lanes_a.as_mut_ptr(), acc_a);
        _mm256_storeu_ps(lanes_b.as_mut_ptr(), acc_b);
    }
    let mut sum_a = lanes_a.iter().sum::<f32>();
    let mut sum_b = lanes_b.iter().sum::<f32>();

    while i < len {
        let byte_a = unsafe { *row_a_packed.add(i / 2) };
        let byte_b = unsafe { *row_b_packed.add(i / 2) };
        let nibble_a = if i.is_multiple_of(2) {
            byte_a & 0x0f
        } else {
            byte_a >> 4
        };
        let nibble_b = if i.is_multiple_of(2) {
            byte_b & 0x0f
        } else {
            byte_b >> 4
        };
        let xv = unsafe { *x.add(i) };
        sum_a += (nibble_a as i8 - 8) as f32 * xv;
        sum_b += (nibble_b as i8 - 8) as f32 * xv;
        i += 1;
    }
    (sum_a * scale_a, sum_b * scale_b)
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2,fma")]
#[allow(unused_unsafe, clippy::too_many_arguments, clippy::missing_safety_doc)]
unsafe fn q4_dot4_f32_avx2_fma(
    row_a_packed: *const u8,
    row_b_packed: *const u8,
    row_c_packed: *const u8,
    row_d_packed: *const u8,
    x: *const f32,
    len: usize,
    scale_a: f32,
    scale_b: f32,
    scale_c: f32,
    scale_d: f32,
) -> (f32, f32, f32, f32) {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let mut i = 0_usize;
    let mut acc_a = unsafe { _mm256_setzero_ps() };
    let mut acc_b = unsafe { _mm256_setzero_ps() };
    let mut acc_c = unsafe { _mm256_setzero_ps() };
    let mut acc_d = unsafe { _mm256_setzero_ps() };
    let low_mask = unsafe { _mm_set1_epi8(0x0f) };
    let eight = unsafe { _mm_set1_epi8(8) };

    while i + 32 <= len {
        let bytes_a = unsafe { _mm_loadu_si128(row_a_packed.add(i / 2).cast::<__m128i>()) };
        let bytes_b = unsafe { _mm_loadu_si128(row_b_packed.add(i / 2).cast::<__m128i>()) };
        let bytes_c = unsafe { _mm_loadu_si128(row_c_packed.add(i / 2).cast::<__m128i>()) };
        let bytes_d = unsafe { _mm_loadu_si128(row_d_packed.add(i / 2).cast::<__m128i>()) };

        let low_a = unsafe { _mm_and_si128(bytes_a, low_mask) };
        let high_a = unsafe { _mm_and_si128(_mm_srli_epi16(bytes_a, 4), low_mask) };
        let q_low_a = unsafe { _mm_sub_epi8(low_a, eight) };
        let q_high_a = unsafe { _mm_sub_epi8(high_a, eight) };
        let int_low_a = unsafe { _mm_unpacklo_epi8(q_low_a, q_high_a) };
        let int_high_a = unsafe { _mm_unpackhi_epi8(q_low_a, q_high_a) };

        let low_b = unsafe { _mm_and_si128(bytes_b, low_mask) };
        let high_b = unsafe { _mm_and_si128(_mm_srli_epi16(bytes_b, 4), low_mask) };
        let q_low_b = unsafe { _mm_sub_epi8(low_b, eight) };
        let q_high_b = unsafe { _mm_sub_epi8(high_b, eight) };
        let int_low_b = unsafe { _mm_unpacklo_epi8(q_low_b, q_high_b) };
        let int_high_b = unsafe { _mm_unpackhi_epi8(q_low_b, q_high_b) };

        let low_c = unsafe { _mm_and_si128(bytes_c, low_mask) };
        let high_c = unsafe { _mm_and_si128(_mm_srli_epi16(bytes_c, 4), low_mask) };
        let q_low_c = unsafe { _mm_sub_epi8(low_c, eight) };
        let q_high_c = unsafe { _mm_sub_epi8(high_c, eight) };
        let int_low_c = unsafe { _mm_unpacklo_epi8(q_low_c, q_high_c) };
        let int_high_c = unsafe { _mm_unpackhi_epi8(q_low_c, q_high_c) };

        let low_d = unsafe { _mm_and_si128(bytes_d, low_mask) };
        let high_d = unsafe { _mm_and_si128(_mm_srli_epi16(bytes_d, 4), low_mask) };
        let q_low_d = unsafe { _mm_sub_epi8(low_d, eight) };
        let q_high_d = unsafe { _mm_sub_epi8(high_d, eight) };
        let int_low_d = unsafe { _mm_unpacklo_epi8(q_low_d, q_high_d) };
        let int_high_d = unsafe { _mm_unpackhi_epi8(q_low_d, q_high_d) };

        let x0 = unsafe { _mm256_loadu_ps(x.add(i)) };
        let x1 = unsafe { _mm256_loadu_ps(x.add(i + 8)) };
        let x2 = unsafe { _mm256_loadu_ps(x.add(i + 16)) };
        let x3 = unsafe { _mm256_loadu_ps(x.add(i + 24)) };

        let a0 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(int_low_a)) };
        let a1 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(_mm_srli_si128(int_low_a, 8))) };
        let a2 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(int_high_a)) };
        let a3 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(_mm_srli_si128(int_high_a, 8))) };
        acc_a = unsafe { _mm256_fmadd_ps(a0, x0, acc_a) };
        acc_a = unsafe { _mm256_fmadd_ps(a1, x1, acc_a) };
        acc_a = unsafe { _mm256_fmadd_ps(a2, x2, acc_a) };
        acc_a = unsafe { _mm256_fmadd_ps(a3, x3, acc_a) };

        let b0 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(int_low_b)) };
        let b1 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(_mm_srli_si128(int_low_b, 8))) };
        let b2 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(int_high_b)) };
        let b3 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(_mm_srli_si128(int_high_b, 8))) };
        acc_b = unsafe { _mm256_fmadd_ps(b0, x0, acc_b) };
        acc_b = unsafe { _mm256_fmadd_ps(b1, x1, acc_b) };
        acc_b = unsafe { _mm256_fmadd_ps(b2, x2, acc_b) };
        acc_b = unsafe { _mm256_fmadd_ps(b3, x3, acc_b) };

        let c0 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(int_low_c)) };
        let c1 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(_mm_srli_si128(int_low_c, 8))) };
        let c2 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(int_high_c)) };
        let c3 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(_mm_srli_si128(int_high_c, 8))) };
        acc_c = unsafe { _mm256_fmadd_ps(c0, x0, acc_c) };
        acc_c = unsafe { _mm256_fmadd_ps(c1, x1, acc_c) };
        acc_c = unsafe { _mm256_fmadd_ps(c2, x2, acc_c) };
        acc_c = unsafe { _mm256_fmadd_ps(c3, x3, acc_c) };

        let d0 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(int_low_d)) };
        let d1 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(_mm_srli_si128(int_low_d, 8))) };
        let d2 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(int_high_d)) };
        let d3 = unsafe { _mm256_cvtepi32_ps(_mm256_cvtepi8_epi32(_mm_srli_si128(int_high_d, 8))) };
        acc_d = unsafe { _mm256_fmadd_ps(d0, x0, acc_d) };
        acc_d = unsafe { _mm256_fmadd_ps(d1, x1, acc_d) };
        acc_d = unsafe { _mm256_fmadd_ps(d2, x2, acc_d) };
        acc_d = unsafe { _mm256_fmadd_ps(d3, x3, acc_d) };

        i += 32;
    }

    let mut lanes_a = [0.0_f32; 8];
    let mut lanes_b = [0.0_f32; 8];
    let mut lanes_c = [0.0_f32; 8];
    let mut lanes_d = [0.0_f32; 8];
    unsafe {
        _mm256_storeu_ps(lanes_a.as_mut_ptr(), acc_a);
        _mm256_storeu_ps(lanes_b.as_mut_ptr(), acc_b);
        _mm256_storeu_ps(lanes_c.as_mut_ptr(), acc_c);
        _mm256_storeu_ps(lanes_d.as_mut_ptr(), acc_d);
    }
    let mut sum_a = lanes_a.iter().sum::<f32>();
    let mut sum_b = lanes_b.iter().sum::<f32>();
    let mut sum_c = lanes_c.iter().sum::<f32>();
    let mut sum_d = lanes_d.iter().sum::<f32>();

    while i < len {
        let byte_a = unsafe { *row_a_packed.add(i / 2) };
        let byte_b = unsafe { *row_b_packed.add(i / 2) };
        let byte_c = unsafe { *row_c_packed.add(i / 2) };
        let byte_d = unsafe { *row_d_packed.add(i / 2) };
        let nibble_a = if i.is_multiple_of(2) {
            byte_a & 0x0f
        } else {
            byte_a >> 4
        };
        let nibble_b = if i.is_multiple_of(2) {
            byte_b & 0x0f
        } else {
            byte_b >> 4
        };
        let nibble_c = if i.is_multiple_of(2) {
            byte_c & 0x0f
        } else {
            byte_c >> 4
        };
        let nibble_d = if i.is_multiple_of(2) {
            byte_d & 0x0f
        } else {
            byte_d >> 4
        };
        let xv = unsafe { *x.add(i) };
        sum_a += (nibble_a as i8 - 8) as f32 * xv;
        sum_b += (nibble_b as i8 - 8) as f32 * xv;
        sum_c += (nibble_c as i8 - 8) as f32 * xv;
        sum_d += (nibble_d as i8 - 8) as f32 * xv;
        i += 1;
    }
    (
        sum_a * scale_a,
        sum_b * scale_b,
        sum_c * scale_c,
        sum_d * scale_d,
    )
}

#[cfg(target_arch = "aarch64")]
#[inline]
unsafe fn q4_dot_f32_neon(row_packed: *const u8, x: *const f32, len: usize, scale: f32) -> f32 {
    use std::arch::aarch64::*;
    let mut i = 0_usize;
    unsafe {
        let mut acc0 = vdupq_n_f32(0.0);
        let mut acc1 = vdupq_n_f32(0.0);
        let eight = vdup_n_u8(8);

        while i + 16 <= len {
            let packed = vld1_u8(row_packed.add(i / 2));
            let low_mask = vdup_n_u8(0x0f);
            let low_nibbles = vand_u8(packed, low_mask);
            let high_nibbles = vshr_n_u8(packed, 4);

            let q_low = vsub_s8(vreinterpret_s8_u8(low_nibbles), vreinterpret_s8_u8(eight));
            let q_high = vsub_s8(vreinterpret_s8_u8(high_nibbles), vreinterpret_s8_u8(eight));

            let q_low_16 = vmovl_s8(q_low);
            let q_high_16 = vmovl_s8(q_high);

            let q_low_f_0 = vcvtq_f32_s32(vmovl_s16(vget_low_s16(q_low_16)));
            let q_low_f_1 = vcvtq_f32_s32(vmovl_s16(vget_high_s16(q_low_16)));

            let q_high_f_0 = vcvtq_f32_s32(vmovl_s16(vget_low_s16(q_high_16)));
            let q_high_f_1 = vcvtq_f32_s32(vmovl_s16(vget_high_s16(q_high_16)));

            let x_a = vld1q_f32(x.add(i));
            let x_b = vld1q_f32(x.add(i + 4));
            let x_c = vld1q_f32(x.add(i + 8));
            let x_d = vld1q_f32(x.add(i + 12));

            let unzip_ab = vuzpq_f32(x_a, x_b);
            let unzip_cd = vuzpq_f32(x_c, x_d);

            acc0 = vfmaq_f32(acc0, q_low_f_0, unzip_ab.0);
            acc0 = vfmaq_f32(acc0, q_low_f_1, unzip_cd.0);

            acc1 = vfmaq_f32(acc1, q_high_f_0, unzip_ab.1);
            acc1 = vfmaq_f32(acc1, q_high_f_1, unzip_cd.1);

            i += 16;
        }

        let mut sum = vaddvq_f32(acc0) + vaddvq_f32(acc1);
        while i < len {
            let byte = *row_packed.add(i / 2);
            let nibble = if i.is_multiple_of(2) {
                byte & 0x0f
            } else {
                byte >> 4
            };
            let q = nibble as i8 - 8;
            sum += q as f32 * *x.add(i);
            i += 1;
        }
        sum * scale
    }
}

#[inline]
pub fn q5_dot_f32_scaled(row_packed: &[u8], start_bit: usize, x: &[f32], scale: f32) -> f32 {
    debug_assert!(row_packed.len() * 8 >= start_bit + x.len() * 5);
    if thermal_pressure_high() {
        return q5_dot_f32_scaled_thermal_high(row_packed, start_bit, x, scale);
    }
    if start_bit == 0 {
        #[cfg(target_arch = "aarch64")]
        {
            // SAFETY: row_packed covers x.len() 5-bit codes by the assertion above.
            return unsafe {
                q5_dot_f32_neon_aligned(row_packed.as_ptr(), x.as_ptr(), x.len(), scale)
            };
        }
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        {
            if is_x86_avx2_available() {
                // SAFETY: AVX2 availability is checked at runtime and the routine only reads
                // complete 5-byte/8-code chunks plus a scalar tail inside the provided slices.
                return unsafe {
                    q5_dot_f32_avx2_aligned(row_packed.as_ptr(), x.as_ptr(), x.len(), scale)
                };
            }
        }
        #[cfg(not(target_arch = "aarch64"))]
        {
            return q5_dot_f32_aligned_scalar(row_packed, x, scale);
        }
    }
    q5_dot_f32_bitstream_scalar(row_packed, start_bit, x, scale)
}

#[inline]
pub(crate) fn q5_dot_f32_scaled_thermal_high(
    row_packed: &[u8],
    start_bit: usize,
    x: &[f32],
    scale: f32,
) -> f32 {
    debug_assert!(row_packed.len() * 8 >= start_bit + x.len() * 5);
    let mut sum = 0.0;
    let len = x.len();
    let mut i = 0;
    while i < len {
        if (i / 16) % 2 == 0 {
            let limit = (i + 16).min(len);
            for (j, value) in x.iter().enumerate().take(limit).skip(i) {
                let bit_offset = start_bit + j * 5;
                let byte_idx = bit_offset / 8;
                let bit_idx = bit_offset % 8;
                let mut val = 0_u16;
                if byte_idx < row_packed.len() {
                    val |= row_packed[byte_idx] as u16;
                }
                if byte_idx + 1 < row_packed.len() {
                    val |= (row_packed[byte_idx + 1] as u16) << 8;
                }
                let code = ((val >> bit_idx) & 0x1F) as i8 - 16;
                sum += code as f32 * *value;
            }
        }
        i += 16;
    }
    sum * scale * 2.0
}

#[inline]
#[allow(dead_code)]
fn decode_q5_aligned_8(src: &[u8]) -> [i32; 8] {
    let b0 = src[0];
    let b1 = src[1];
    let b2 = src[2];
    let b3 = src[3];
    let b4 = src[4];
    [
        ((b0 & 0x1f) as i32) - 16,
        (((b0 >> 5) | (b1 << 3)) & 0x1f) as i32 - 16,
        ((b1 >> 2) & 0x1f) as i32 - 16,
        (((b1 >> 7) | (b2 << 1)) & 0x1f) as i32 - 16,
        (((b2 >> 4) | (b3 << 4)) & 0x1f) as i32 - 16,
        ((b3 >> 1) & 0x1f) as i32 - 16,
        (((b3 >> 6) | (b4 << 2)) & 0x1f) as i32 - 16,
        ((b4 >> 3) & 0x1f) as i32 - 16,
    ]
}

#[inline]
#[allow(dead_code)]
fn q5_dot_f32_aligned_scalar(row_packed: &[u8], x: &[f32], scale: f32) -> f32 {
    let mut acc = 0.0;
    let mut col = 0_usize;
    let mut byte = 0_usize;
    while col + 8 <= x.len() {
        let q = decode_q5_aligned_8(&row_packed[byte..byte + 5]);
        acc += q[0] as f32 * x[col]
            + q[1] as f32 * x[col + 1]
            + q[2] as f32 * x[col + 2]
            + q[3] as f32 * x[col + 3]
            + q[4] as f32 * x[col + 4]
            + q[5] as f32 * x[col + 5]
            + q[6] as f32 * x[col + 6]
            + q[7] as f32 * x[col + 7];
        col += 8;
        byte += 5;
    }
    while col < x.len() {
        let bit_index = col * 5;
        let mut code = 0_u8;
        for bit in 0..5 {
            let src = bit_index + bit;
            code |= ((row_packed[src / 8] >> (src % 8)) & 1) << bit;
        }
        acc += (code as i8 - 16) as f32 * x[col];
        col += 1;
    }
    acc * scale
}

#[inline]
fn q5_dot_f32_bitstream_scalar(row_packed: &[u8], start_bit: usize, x: &[f32], scale: f32) -> f32 {
    let mut acc = 0.0;
    for (col, xv) in x.iter().enumerate() {
        let bit_index = start_bit + col * 5;
        let mut code = 0_u8;
        for bit in 0..5 {
            let src = bit_index + bit;
            let byte_idx = src / 8;
            let bit_idx = src % 8;
            code |= ((row_packed[byte_idx] >> bit_idx) & 1) << bit;
        }
        let q = code as i8 - 16;
        acc += q as f32 * scale * xv;
    }
    acc
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2")]
unsafe fn q5_dot_f32_avx2_aligned(
    row_packed: *const u8,
    x: *const f32,
    len: usize,
    scale: f32,
) -> f32 {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let mut col = 0_usize;
    let mut byte = 0_usize;
    let mut acc = _mm256_setzero_ps();
    while col + 8 <= len {
        let src = unsafe { std::slice::from_raw_parts(row_packed.add(byte), 5) };
        let q = decode_q5_aligned_8(src);
        let qi = _mm256_setr_epi32(q[0], q[1], q[2], q[3], q[4], q[5], q[6], q[7]);
        let qf = _mm256_cvtepi32_ps(qi);
        let xf = unsafe { _mm256_loadu_ps(x.add(col)) };
        acc = _mm256_add_ps(acc, _mm256_mul_ps(qf, xf));
        col += 8;
        byte += 5;
    }

    let mut lanes = [0.0_f32; 8];
    unsafe { _mm256_storeu_ps(lanes.as_mut_ptr(), acc) };
    let mut sum = lanes.iter().sum::<f32>();
    while col < len {
        let bit_index = col * 5;
        let mut code = 0_u8;
        for bit in 0..5 {
            let src = bit_index + bit;
            let packed_byte = unsafe { *row_packed.add(src / 8) };
            code |= ((packed_byte >> (src % 8)) & 1) << bit;
        }
        sum += (code as i8 - 16) as f32 * unsafe { *x.add(col) };
        col += 1;
    }
    sum * scale
}

#[cfg(target_arch = "aarch64")]
#[inline]
unsafe fn q5_dot_f32_neon_aligned(
    row_packed: *const u8,
    x: *const f32,
    len: usize,
    scale: f32,
) -> f32 {
    use std::arch::aarch64::*;

    let mut col = 0_usize;
    let mut byte = 0_usize;
    let mut acc0 = unsafe { vdupq_n_f32(0.0) };
    let mut acc1 = unsafe { vdupq_n_f32(0.0) };
    while col + 8 <= len {
        let src = unsafe { std::slice::from_raw_parts(row_packed.add(byte), 5) };
        let q = decode_q5_aligned_8(src);
        let q0 = unsafe { vcvtq_f32_s32(vld1q_s32(q.as_ptr())) };
        let q1 = unsafe { vcvtq_f32_s32(vld1q_s32(q.as_ptr().add(4))) };
        let x0 = unsafe { vld1q_f32(x.add(col)) };
        let x1 = unsafe { vld1q_f32(x.add(col + 4)) };
        acc0 = unsafe { vfmaq_f32(acc0, q0, x0) };
        acc1 = unsafe { vfmaq_f32(acc1, q1, x1) };
        col += 8;
        byte += 5;
    }

    let mut sum = unsafe { vaddvq_f32(acc0) + vaddvq_f32(acc1) };
    while col < len {
        let bit_index = col * 5;
        let mut code = 0_u8;
        for bit in 0..5 {
            let src = bit_index + bit;
            let packed_byte = unsafe { *row_packed.add(src / 8) };
            code |= ((packed_byte >> (src % 8)) & 1) << bit;
        }
        sum += (code as i8 - 16) as f32 * unsafe { *x.add(col) };
        col += 1;
    }
    sum * scale
}

#[cfg(target_arch = "aarch64")]
pub fn rms_norm_neon(x: &[f32], weight: &[f32], eps: f32) -> Vec<f32> {
    use std::arch::aarch64::*;
    let len = x.len();
    let mut i = 0_usize;

    let mut acc = unsafe { vdupq_n_f32(0.0) };
    while i + 4 <= len {
        let vx = unsafe { vld1q_f32(x.as_ptr().add(i)) };
        acc = unsafe { vfmaq_f32(acc, vx, vx) };
        i += 4;
    }
    let mut sum_sq = unsafe { vaddvq_f32(acc) };
    while i < len {
        sum_sq += x[i] * x[i];
        i += 1;
    }

    let mean_square = sum_sq / len as f32;
    let scale = 1.0 / (mean_square + eps).sqrt();

    let mut out = vec![0.0f32; len];
    let mut i = 0_usize;
    let vscale = unsafe { vdupq_n_f32(scale) };
    while i + 4 <= len {
        let vx = unsafe { vld1q_f32(x.as_ptr().add(i)) };
        let vw = unsafe { vld1q_f32(weight.as_ptr().add(i)) };
        let vout = unsafe { vmulq_f32(vmulq_f32(vx, vscale), vw) };
        unsafe { vst1q_f32(out.as_mut_ptr().add(i), vout) };
        i += 4;
    }
    while i < len {
        out[i] = x[i] * scale * weight[i];
        i += 1;
    }
    out
}

#[cfg(target_arch = "aarch64")]
pub fn rms_norm_unit_neon(x: &[f32], eps: f32) -> Vec<f32> {
    use std::arch::aarch64::*;
    let len = x.len();
    let mut i = 0_usize;

    let mut acc = unsafe { vdupq_n_f32(0.0) };
    while i + 4 <= len {
        let vx = unsafe { vld1q_f32(x.as_ptr().add(i)) };
        acc = unsafe { vfmaq_f32(acc, vx, vx) };
        i += 4;
    }
    let mut sum_sq = unsafe { vaddvq_f32(acc) };
    while i < len {
        sum_sq += x[i] * x[i];
        i += 1;
    }

    let mean_square = sum_sq / len as f32;
    let scale = 1.0 / (mean_square + eps).sqrt();

    let mut out = vec![0.0f32; len];
    let mut i = 0_usize;
    let vscale = unsafe { vdupq_n_f32(scale) };
    while i + 4 <= len {
        let vx = unsafe { vld1q_f32(x.as_ptr().add(i)) };
        let vout = unsafe { vmulq_f32(vx, vscale) };
        unsafe { vst1q_f32(out.as_mut_ptr().add(i), vout) };
        i += 4;
    }
    while i < len {
        out[i] = x[i] * scale;
        i += 1;
    }
    out
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn rms_norm_x86_avx2_fma(x: &[f32], weight: &[f32], eps: f32) -> Option<Vec<f32>> {
    if x.len() < 32 || !is_x86_avx2_fma_available() {
        return None;
    }
    // SAFETY: AVX2+FMA availability is checked at runtime and both slices are bounds-checked by
    // the caller in ops::rms_norm.
    Some(unsafe { rms_norm_avx2_fma_impl(x.as_ptr(), weight.as_ptr(), x.len(), eps) })
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub fn rms_norm_in_place_x86_avx2_fma(values: &mut [f32], weight: &[f32], eps: f32) -> bool {
    if values.len() < 32 || !is_x86_avx2_fma_available() {
        return false;
    }
    // SAFETY: AVX2+FMA availability is checked at runtime and both slices are bounds-checked by
    // the caller in ops::rms_norm_in_place.
    unsafe {
        rms_norm_in_place_avx2_fma_impl(values.as_mut_ptr(), weight.as_ptr(), values.len(), eps)
    };
    true
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2,fma")]
#[allow(unused_unsafe)]
unsafe fn rms_norm_avx2_fma_impl(
    x: *const f32,
    weight: *const f32,
    len: usize,
    eps: f32,
) -> Vec<f32> {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let mut i = 0_usize;
    let mut acc = unsafe { _mm256_setzero_ps() };
    while i + 8 <= len {
        let vx = unsafe { _mm256_loadu_ps(x.add(i)) };
        acc = unsafe { _mm256_fmadd_ps(vx, vx, acc) };
        i += 8;
    }
    let mut lanes = [0.0_f32; 8];
    unsafe { _mm256_storeu_ps(lanes.as_mut_ptr(), acc) };
    let mut sum_sq = lanes.iter().sum::<f32>();
    while i < len {
        let value = unsafe { *x.add(i) };
        sum_sq += value * value;
        i += 1;
    }

    let scale = 1.0 / (sum_sq / len as f32 + eps).sqrt();
    let vscale = unsafe { _mm256_set1_ps(scale) };
    let mut out = vec![0.0_f32; len];
    i = 0;
    while i + 8 <= len {
        let vx = unsafe { _mm256_loadu_ps(x.add(i)) };
        let vw = unsafe { _mm256_loadu_ps(weight.add(i)) };
        let scaled = unsafe { _mm256_mul_ps(vx, vscale) };
        let vout = unsafe { _mm256_mul_ps(scaled, vw) };
        unsafe { _mm256_storeu_ps(out.as_mut_ptr().add(i), vout) };
        i += 8;
    }
    while i < len {
        out[i] = unsafe { *x.add(i) } * scale * unsafe { *weight.add(i) };
        i += 1;
    }
    out
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2,fma")]
#[allow(unused_unsafe)]
unsafe fn rms_norm_in_place_avx2_fma_impl(
    values: *mut f32,
    weight: *const f32,
    len: usize,
    eps: f32,
) {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let mut i = 0_usize;
    let mut acc = unsafe { _mm256_setzero_ps() };
    while i + 8 <= len {
        let vx = unsafe { _mm256_loadu_ps(values.add(i)) };
        acc = unsafe { _mm256_fmadd_ps(vx, vx, acc) };
        i += 8;
    }
    let mut lanes = [0.0_f32; 8];
    unsafe { _mm256_storeu_ps(lanes.as_mut_ptr(), acc) };
    let mut sum_sq = lanes.iter().sum::<f32>();
    while i < len {
        let value = unsafe { *values.add(i) };
        sum_sq += value * value;
        i += 1;
    }

    let scale = 1.0 / (sum_sq / len as f32 + eps).sqrt();
    let vscale = unsafe { _mm256_set1_ps(scale) };
    i = 0;
    while i + 8 <= len {
        let vx = unsafe { _mm256_loadu_ps(values.add(i)) };
        let vw = unsafe { _mm256_loadu_ps(weight.add(i)) };
        let scaled = unsafe { _mm256_mul_ps(vx, vscale) };
        let vout = unsafe { _mm256_mul_ps(scaled, vw) };
        unsafe { _mm256_storeu_ps(values.add(i), vout) };
        i += 8;
    }
    while i < len {
        unsafe {
            *values.add(i) *= scale * *weight.add(i);
        }
        i += 1;
    }
}

/// # Safety
///
/// `row_a` and `row_b` must point to valid arrays of `i8` of at least `len` elements.
/// `x` must point to a valid array of `f32` of at least `len` elements.
/// `out_a` and `out_b` must be valid mutable references.
#[cfg(target_arch = "aarch64")]
#[inline]
#[allow(clippy::too_many_arguments)]
pub unsafe fn q8_gemv_row_pair_neon(
    row_a: *const i8,
    row_b: *const i8,
    x: *const f32,
    len: usize,
    scale_a: f32,
    scale_b: f32,
    out_a: &mut f32,
    out_b: &mut f32,
) {
    use std::arch::aarch64::*;

    unsafe {
        let mut i = 0_usize;
        let mut acc_a0 = vdupq_n_f32(0.0);
        let mut acc_a1 = vdupq_n_f32(0.0);
        let mut acc_b0 = vdupq_n_f32(0.0);
        let mut acc_b1 = vdupq_n_f32(0.0);

        while i + 8 <= len {
            let qa = vld1_s8(row_a.add(i));
            let qb = vld1_s8(row_b.add(i));

            let qa_s16 = vmovl_s8(qa);
            let qb_s16 = vmovl_s8(qb);

            let qa_f32_0 = vcvtq_f32_s32(vmovl_s16(vget_low_s16(qa_s16)));
            let qa_f32_1 = vcvtq_f32_s32(vmovl_s16(vget_high_s16(qa_s16)));

            let qb_f32_0 = vcvtq_f32_s32(vmovl_s16(vget_low_s16(qb_s16)));
            let qb_f32_1 = vcvtq_f32_s32(vmovl_s16(vget_high_s16(qb_s16)));

            let x0 = vld1q_f32(x.add(i));
            let x1 = vld1q_f32(x.add(i + 4));

            acc_a0 = vfmaq_f32(acc_a0, qa_f32_0, x0);
            acc_a1 = vfmaq_f32(acc_a1, qa_f32_1, x1);

            acc_b0 = vfmaq_f32(acc_b0, qb_f32_0, x0);
            acc_b1 = vfmaq_f32(acc_b1, qb_f32_1, x1);

            i += 8;
        }

        let mut sum_a = (vaddvq_f32(acc_a0) + vaddvq_f32(acc_a1)) * scale_a;
        let mut sum_b = (vaddvq_f32(acc_b0) + vaddvq_f32(acc_b1)) * scale_b;

        while i < len {
            sum_a += (*row_a.add(i)) as f32 * (*x.add(i)) * scale_a;
            sum_b += (*row_b.add(i)) as f32 * (*x.add(i)) * scale_b;
            i += 1;
        }

        *out_a = sum_a;
        *out_b = sum_b;
    }
}

#[inline]
#[allow(clippy::too_many_arguments)]
pub fn q3_dot4_f32_scaled(
    row_a_packed: &[u8],
    row_b_packed: &[u8],
    row_c_packed: &[u8],
    row_d_packed: &[u8],
    x: &[f32],
    scale_a: f32,
    scale_b: f32,
    scale_c: f32,
    scale_d: f32,
) -> (f32, f32, f32, f32) {
    let packed_len = x.len().saturating_mul(3).div_ceil(8);
    assert!(row_a_packed.len() >= packed_len);
    assert!(row_b_packed.len() >= packed_len);
    assert!(row_c_packed.len() >= packed_len);
    assert!(row_d_packed.len() >= packed_len);

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    if std::arch::is_x86_feature_detected!("avx2") && std::arch::is_x86_feature_detected!("fma") {
        // SAFETY: both required CPU features were checked above, and the packed-row bounds were
        // validated for every value read by the implementation.
        return unsafe {
            q3_dot4_f32_scaled_avx2_fma(
                row_a_packed,
                row_b_packed,
                row_c_packed,
                row_d_packed,
                x,
                scale_a,
                scale_b,
                scale_c,
                scale_d,
            )
        };
    }

    q3_dot4_f32_scaled_scalar(
        row_a_packed,
        row_b_packed,
        row_c_packed,
        row_d_packed,
        x,
        scale_a,
        scale_b,
        scale_c,
        scale_d,
    )
}

#[inline]
fn load_q3_group_bits(row_packed: &[u8], byte_index: usize) -> u32 {
    u32::from(row_packed[byte_index])
        | (u32::from(row_packed[byte_index + 1]) << 8)
        | (u32::from(row_packed[byte_index + 2]) << 16)
}

#[inline]
fn decode_q3_group(row_packed: &[u8], byte_index: usize) -> [i8; 8] {
    let bits = load_q3_group_bits(row_packed, byte_index);
    std::array::from_fn(|lane| ((bits >> (lane * 3)) & 0x07) as i8 - 4)
}

#[inline]
fn decode_q3_value(row_packed: &[u8], col: usize) -> i8 {
    let group = col / 8;
    let rem = col % 8;
    let byte_index = group * 3;
    let p0 = row_packed[byte_index];
    let code = match rem {
        0 => p0 & 0x07,
        1 => (p0 >> 3) & 0x07,
        2 => ((p0 >> 6) & 0x03) | ((row_packed[byte_index + 1] & 0x01) << 2),
        3 => (row_packed[byte_index + 1] >> 1) & 0x07,
        4 => (row_packed[byte_index + 1] >> 4) & 0x07,
        5 => {
            ((row_packed[byte_index + 1] >> 7) & 0x01) | ((row_packed[byte_index + 2] & 0x03) << 1)
        }
        6 => (row_packed[byte_index + 2] >> 2) & 0x07,
        7 => (row_packed[byte_index + 2] >> 5) & 0x07,
        _ => unreachable!(),
    };
    code as i8 - 4
}

#[inline]
#[allow(clippy::too_many_arguments)]
fn q3_dot4_f32_scaled_scalar(
    row_a_packed: &[u8],
    row_b_packed: &[u8],
    row_c_packed: &[u8],
    row_d_packed: &[u8],
    x: &[f32],
    scale_a: f32,
    scale_b: f32,
    scale_c: f32,
    scale_d: f32,
) -> (f32, f32, f32, f32) {
    let len = x.len();
    let num_groups = len / 8;
    let mut sum_a = 0.0_f32;
    let mut sum_b = 0.0_f32;
    let mut sum_c = 0.0_f32;
    let mut sum_d = 0.0_f32;

    for g in 0..num_groups {
        let b_idx = g * 3;
        let base_x = g * 8;
        let qa = decode_q3_group(row_a_packed, b_idx);
        let qb = decode_q3_group(row_b_packed, b_idx);
        let qc = decode_q3_group(row_c_packed, b_idx);
        let qd = decode_q3_group(row_d_packed, b_idx);

        for i in 0..8 {
            let xv = x[base_x + i];
            sum_a += qa[i] as f32 * xv;
            sum_b += qb[i] as f32 * xv;
            sum_c += qc[i] as f32 * xv;
            sum_d += qd[i] as f32 * xv;
        }
    }

    for (col, &xv) in x.iter().enumerate().skip(num_groups * 8) {
        sum_a += decode_q3_value(row_a_packed, col) as f32 * xv;
        sum_b += decode_q3_value(row_b_packed, col) as f32 * xv;
        sum_c += decode_q3_value(row_c_packed, col) as f32 * xv;
        sum_d += decode_q3_value(row_d_packed, col) as f32 * xv;
    }

    (
        sum_a * scale_a,
        sum_b * scale_b,
        sum_c * scale_c,
        sum_d * scale_d,
    )
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[target_feature(enable = "avx2,fma")]
#[allow(clippy::too_many_arguments, unused_unsafe)]
unsafe fn q3_dot4_f32_scaled_avx2_fma(
    row_a_packed: &[u8],
    row_b_packed: &[u8],
    row_c_packed: &[u8],
    row_d_packed: &[u8],
    x: &[f32],
    scale_a: f32,
    scale_b: f32,
    scale_c: f32,
    scale_d: f32,
) -> (f32, f32, f32, f32) {
    #[cfg(target_arch = "x86")]
    use std::arch::x86::*;
    #[cfg(target_arch = "x86_64")]
    use std::arch::x86_64::*;

    let mut acc_a = unsafe { _mm256_setzero_ps() };
    let mut acc_b = unsafe { _mm256_setzero_ps() };
    let mut acc_c = unsafe { _mm256_setzero_ps() };
    let mut acc_d = unsafe { _mm256_setzero_ps() };
    let shifts = unsafe { _mm256_setr_epi32(0, 3, 6, 9, 12, 15, 18, 21) };
    let code_mask = unsafe { _mm256_set1_epi32(0x07) };
    let zero_point = unsafe { _mm256_set1_epi32(4) };
    macro_rules! decode_group {
        ($row:expr, $index:expr) => {{
            let packed = unsafe { _mm256_set1_epi32(load_q3_group_bits($row, $index) as i32) };
            let codes = unsafe { _mm256_and_si256(_mm256_srlv_epi32(packed, shifts), code_mask) };
            unsafe { _mm256_cvtepi32_ps(_mm256_sub_epi32(codes, zero_point)) }
        }};
    }
    let mut col = 0_usize;
    let mut byte_index = 0_usize;
    while col + 8 <= x.len() {
        let va = decode_group!(row_a_packed, byte_index);
        let vb = decode_group!(row_b_packed, byte_index);
        let vc = decode_group!(row_c_packed, byte_index);
        let vd = decode_group!(row_d_packed, byte_index);
        let vx = unsafe { _mm256_loadu_ps(x.as_ptr().add(col)) };
        acc_a = unsafe { _mm256_fmadd_ps(va, vx, acc_a) };
        acc_b = unsafe { _mm256_fmadd_ps(vb, vx, acc_b) };
        acc_c = unsafe { _mm256_fmadd_ps(vc, vx, acc_c) };
        acc_d = unsafe { _mm256_fmadd_ps(vd, vx, acc_d) };
        col += 8;
        byte_index += 3;
    }

    let mut lanes_a = [0.0_f32; 8];
    let mut lanes_b = [0.0_f32; 8];
    let mut lanes_c = [0.0_f32; 8];
    let mut lanes_d = [0.0_f32; 8];
    unsafe {
        _mm256_storeu_ps(lanes_a.as_mut_ptr(), acc_a);
        _mm256_storeu_ps(lanes_b.as_mut_ptr(), acc_b);
        _mm256_storeu_ps(lanes_c.as_mut_ptr(), acc_c);
        _mm256_storeu_ps(lanes_d.as_mut_ptr(), acc_d);
    }
    let mut sum_a = lanes_a.iter().sum::<f32>();
    let mut sum_b = lanes_b.iter().sum::<f32>();
    let mut sum_c = lanes_c.iter().sum::<f32>();
    let mut sum_d = lanes_d.iter().sum::<f32>();
    for (tail_col, &xv) in x.iter().enumerate().skip(col) {
        sum_a += decode_q3_value(row_a_packed, tail_col) as f32 * xv;
        sum_b += decode_q3_value(row_b_packed, tail_col) as f32 * xv;
        sum_c += decode_q3_value(row_c_packed, tail_col) as f32 * xv;
        sum_d += decode_q3_value(row_d_packed, tail_col) as f32 * xv;
    }

    (
        sum_a * scale_a,
        sum_b * scale_b,
        sum_c * scale_c,
        sum_d * scale_d,
    )
}

#[inline]
pub fn q3_dot_f32_scaled(row_packed: &[u8], x: &[f32], scale: f32) -> f32 {
    let packed_len = x.len().saturating_mul(3).div_ceil(8);
    assert!(row_packed.len() >= packed_len);
    let mut sum = 0.0_f32;
    let full_groups = x.len() / 8;
    for group in 0..full_groups {
        let values = decode_q3_group(row_packed, group * 3);
        let base = group * 8;
        for lane in 0..8 {
            sum += values[lane] as f32 * x[base + lane];
        }
    }
    for (col, &xv) in x.iter().enumerate().skip(full_groups * 8) {
        sum += decode_q3_value(row_packed, col) as f32 * xv;
    }
    sum * scale
}

#[inline]
pub fn q1_58_dot_f32_scaled(row_packed: &[u8], x: &[f32], scale: f32, cols: usize) -> f32 {
    let mut sum = 0.0_f32;
    for (col, &xv) in x.iter().take(cols).enumerate() {
        let byte = row_packed[col / 4];
        let code = (byte >> ((col % 4) * 2)) & 0x03;
        match code {
            0 => sum -= xv,
            2 => sum += xv,
            _ => {}
        }
    }
    sum * scale
}

#[inline]
#[allow(clippy::too_many_arguments)]
pub fn q1_58_dot4_f32_scaled(
    row_a_packed: &[u8],
    row_b_packed: &[u8],
    row_c_packed: &[u8],
    row_d_packed: &[u8],
    x: &[f32],
    scale_a: f32,
    scale_b: f32,
    scale_c: f32,
    scale_d: f32,
    cols: usize,
) -> (f32, f32, f32, f32) {
    let mut sum_a = 0.0_f32;
    let mut sum_b = 0.0_f32;
    let mut sum_c = 0.0_f32;
    let mut sum_d = 0.0_f32;

    for (col, &xv) in x.iter().enumerate().take(cols) {
        let byte_idx = col / 4;
        let shift = (col % 4) * 2;

        let code_a = (row_a_packed[byte_idx] >> shift) & 0x03;
        let code_b = (row_b_packed[byte_idx] >> shift) & 0x03;
        let code_c = (row_c_packed[byte_idx] >> shift) & 0x03;
        let code_d = (row_d_packed[byte_idx] >> shift) & 0x03;

        match code_a {
            0 => sum_a -= xv,
            2 => sum_a += xv,
            _ => {}
        }
        match code_b {
            0 => sum_b -= xv,
            2 => sum_b += xv,
            _ => {}
        }
        match code_c {
            0 => sum_c -= xv,
            2 => sum_c += xv,
            _ => {}
        }
        match code_d {
            0 => sum_d -= xv,
            2 => sum_d += xv,
            _ => {}
        }
    }

    (
        sum_a * scale_a,
        sum_b * scale_b,
        sum_c * scale_c,
        sum_d * scale_d,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack_q3_for_test(values: &[i8]) -> Vec<u8> {
        let mut packed = vec![0_u8; values.len().saturating_mul(3).div_ceil(8)];
        for (col, &value) in values.iter().enumerate() {
            let code = (value + 4) as u8 & 0x07;
            for bit in 0..3 {
                if ((code >> bit) & 1) != 0 {
                    let dst = col * 3 + bit;
                    packed[dst / 8] |= 1 << (dst % 8);
                }
            }
        }
        packed
    }

    #[allow(dead_code)]
    fn pack_q5_for_test(values: &[i8], start_bit: usize) -> Vec<u8> {
        let total_bits = start_bit + values.len() * 5;
        let mut packed = vec![0_u8; total_bits.div_ceil(8)];
        for (col, value) in values.iter().enumerate() {
            let code = (*value + 16) as u8 & 0x1f;
            let bit_index = start_bit + col * 5;
            for bit in 0..5 {
                if ((code >> bit) & 1) != 0 {
                    let dst = bit_index + bit;
                    packed[dst / 8] |= 1 << (dst % 8);
                }
            }
        }
        packed
    }

    #[test]
    fn q8_kernel_matches_scalar_reference() {
        let row = [
            -127_i8, -3, 0, 4, 12, 31, 64, 126, -90, 17, 1, -1, 88, -45, 6, 7, 9,
        ];
        let x = [
            0.5, -1.0, 2.0, 0.25, -0.75, 1.5, 0.125, -2.0, 0.9, 0.8, -0.7, 0.6, -0.5, 0.4, -0.3,
            0.2, 0.1,
        ];
        let scale = 0.03125;
        let reference: f32 = row
            .iter()
            .zip(x)
            .map(|(q, xv)| *q as f32 * scale * xv)
            .sum();
        assert!((q8_i8_dot_f32_scaled(&row, &x, scale) - reference).abs() < 1e-5);

        let as_bytes: Vec<u8> = row.iter().map(|v| *v as u8).collect();
        assert!((q8_u8_dot_f32_scaled(&as_bytes, &x, scale) - reference).abs() < 1e-5);
    }

    #[test]
    fn q3_single_and_four_row_kernels_include_partial_groups() {
        for len in 1..24 {
            let rows = (0..4)
                .map(|row| {
                    (0..len)
                        .map(|col| ((row * 5 + col * 3) % 8) as i8 - 4)
                        .collect::<Vec<_>>()
                })
                .collect::<Vec<_>>();
            let packed = rows
                .iter()
                .map(|row| pack_q3_for_test(row))
                .collect::<Vec<_>>();
            let x = (0..len)
                .map(|col| ((col as f32 * 0.17).sin() * 0.7) - 0.2)
                .collect::<Vec<_>>();
            let scales = [0.125_f32, 0.25, 0.5, 0.75];
            let expected = rows
                .iter()
                .zip(scales)
                .map(|(row, scale)| {
                    row.iter()
                        .zip(&x)
                        .map(|(&value, &xv)| value as f32 * xv * scale)
                        .sum::<f32>()
                })
                .collect::<Vec<_>>();

            let got = q3_dot4_f32_scaled(
                &packed[0], &packed[1], &packed[2], &packed[3], &x, scales[0], scales[1],
                scales[2], scales[3],
            );
            let got = [got.0, got.1, got.2, got.3];
            let scalar = q3_dot4_f32_scaled_scalar(
                &packed[0], &packed[1], &packed[2], &packed[3], &x, scales[0], scales[1],
                scales[2], scales[3],
            );
            let scalar = [scalar.0, scalar.1, scalar.2, scalar.3];
            for row in 0..4 {
                assert!(
                    (got[row] - expected[row]).abs() < 1.0e-5,
                    "four-row mismatch at len={len} row={row}: got={} expected={}",
                    got[row],
                    expected[row]
                );
                assert!((scalar[row] - expected[row]).abs() < 1.0e-5);
                let single = q3_dot_f32_scaled(&packed[row], &x, scales[row]);
                assert!(
                    (single - expected[row]).abs() < 1.0e-5,
                    "single-row mismatch at len={len} row={row}: got={single} expected={}",
                    expected[row]
                );
            }
        }
    }

    #[test]
    fn q8_dot2_kernel_matches_scalar_reference() {
        let len = Q8_DOT2_MIN_FUSED_LEN + 19;
        let row_a = (0..len)
            .map(|idx| ((idx * 13 + 17) % 255) as i16 - 127)
            .map(|value| value as i8)
            .collect::<Vec<_>>();
        let row_b = (0..len)
            .map(|idx| ((idx * 19 + 23) % 255) as i16 - 127)
            .map(|value| value as i8)
            .collect::<Vec<_>>();
        let x = (0..len)
            .map(|idx| ((idx as f32 * 0.013).sin() * 0.5) + ((idx as f32 * 0.029).cos() * 0.75))
            .collect::<Vec<_>>();
        let scale_a = 0.03125;
        let scale_b = 0.0625;
        let reference_a = q8_i8_dot_f32_scaled(&row_a, &x, scale_a);
        let reference_b = q8_i8_dot_f32_scaled(&row_b, &x, scale_b);
        let (got_a, got_b) = q8_i8_dot2_f32_scaled(&row_a, &row_b, &x, scale_a, scale_b);

        assert!((got_a - reference_a).abs() < 1e-4);
        assert!((got_b - reference_b).abs() < 1e-4);
    }

    #[test]
    fn q8_integer_dot_kernel_matches_scalar_reference() {
        let lhs = [
            -12_i8, -3, 0, 4, 11, 37, -64, 99, 5, -7, 13, -21, 44, -55, 66, -77, 88,
        ];
        let rhs = [9_i8, -8, 7, -6, 5, -4, 3, -2, 1, 0, -1, 2, -3, 4, -5, 6, -7];
        let reference: f32 = lhs
            .iter()
            .zip(rhs)
            .map(|(a, b)| *a as f32 * b as f32 * 0.125 * 0.25)
            .sum();
        assert!((q8_i8_dot_i8_scaled(&lhs, &rhs, 0.125, 0.25) - reference).abs() < 1e-5);

        let lhs_bytes: Vec<u8> = lhs.iter().map(|v| *v as u8).collect();
        assert!((q8_u8_dot_i8_scaled(&lhs_bytes, &rhs, 0.125, 0.25) - reference).abs() < 1e-5);
    }

    #[test]
    fn q4_kernel_matches_direct_reference() {
        let q = [-7_i8, -3, 0, 4, 7, -6, 2, 1, -1, 5, -4, 3, -2, 6, 0, -5, 4];
        let x = [
            0.5, -1.0, 2.0, 0.25, -0.75, 1.5, 0.125, -2.0, 0.9, 0.8, -0.7, 0.6, -0.5, 0.4, -0.3,
            0.2, 0.1,
        ];
        let scale = 0.125;
        let reference: f32 = q
            .iter()
            .zip(x)
            .map(|(qv, xv)| *qv as f32 * scale * xv)
            .sum();
        let mut packed = vec![0_u8; q.len().div_ceil(2)];
        for (idx, value) in q.iter().enumerate() {
            let nibble = (*value + 8) as u8 & 0x0f;
            if idx.is_multiple_of(2) {
                packed[idx / 2] |= nibble;
            } else {
                packed[idx / 2] |= nibble << 4;
            }
        }

        assert!((q4_dot_f32_scaled(&packed, &x, scale) - reference).abs() < 1e-5);
    }

    #[test]
    fn q4_planned_kernel_matches_default_dispatch() {
        let q = (0..97)
            .map(|idx| ((idx * 7 + 5) % 15) as i8 - 7)
            .collect::<Vec<_>>();
        let x = (0..q.len())
            .map(|idx| ((idx as f32 * 0.017).sin() * 0.75) + ((idx as f32 * 0.031).cos() * 0.25))
            .collect::<Vec<_>>();
        let mut packed = vec![0_u8; q.len().div_ceil(2)];
        for (idx, value) in q.iter().enumerate() {
            let nibble = (*value + 8) as u8 & 0x0f;
            if idx.is_multiple_of(2) {
                packed[idx / 2] |= nibble;
            } else {
                packed[idx / 2] |= nibble << 4;
            }
        }

        let scale = 0.0625;
        let kernel = select_q4_dot_kernel();
        let planned = q4_dot_f32_scaled_with_kernel(&packed, &x, scale, kernel);
        let default = q4_dot_f32_scaled(&packed, &x, scale);
        assert!((planned - default).abs() < 1e-6);
    }

    #[test]
    fn q4_dot2_kernel_matches_direct_reference() {
        let len = Q4_DOT2_MIN_FUSED_LEN + 17;
        let q_a = (0..len)
            .map(|idx| ((idx * 7 + 3) % 15) as i8 - 7)
            .collect::<Vec<_>>();
        let q_b = (0..len)
            .map(|idx| ((idx * 11 + 5) % 15) as i8 - 7)
            .collect::<Vec<_>>();
        let x = (0..len)
            .map(|idx| ((idx as f32 * 0.017).sin() * 0.75) + ((idx as f32 * 0.031).cos() * 0.25))
            .collect::<Vec<_>>();
        let scale_a = 0.125;
        let scale_b = 0.25;
        let reference_a: f32 = q_a
            .iter()
            .zip(&x)
            .map(|(qv, xv)| *qv as f32 * scale_a * xv)
            .sum();
        let reference_b: f32 = q_b
            .iter()
            .zip(&x)
            .map(|(qv, xv)| *qv as f32 * scale_b * xv)
            .sum();
        let mut packed_a = vec![0_u8; q_a.len().div_ceil(2)];
        let mut packed_b = vec![0_u8; q_b.len().div_ceil(2)];
        for (idx, value) in q_a.iter().enumerate() {
            let nibble = (*value + 8) as u8 & 0x0f;
            if idx.is_multiple_of(2) {
                packed_a[idx / 2] |= nibble;
            } else {
                packed_a[idx / 2] |= nibble << 4;
            }
        }
        for (idx, value) in q_b.iter().enumerate() {
            let nibble = (*value + 8) as u8 & 0x0f;
            if idx.is_multiple_of(2) {
                packed_b[idx / 2] |= nibble;
            } else {
                packed_b[idx / 2] |= nibble << 4;
            }
        }

        let (got_a, got_b) = q4_dot2_f32_scaled(&packed_a, &packed_b, &x, scale_a, scale_b);
        assert!((got_a - reference_a).abs() < 1e-5);
        assert!((got_b - reference_b).abs() < 1e-5);
        let (got4_a, got4_b, got4_c, got4_d) = q4_dot4_f32_scaled(
            &packed_a, &packed_b, &packed_a, &packed_b, &x, scale_a, scale_b, scale_a, scale_b,
        );
        assert!((got4_a - reference_a).abs() < 1e-5);
        assert!((got4_b - reference_b).abs() < 1e-5);
        assert!((got4_c - reference_a).abs() < 1e-5);
        assert!((got4_d - reference_b).abs() < 1e-5);
    }

    #[test]
    fn q1_58_dot4_kernel_parity() {
        let x = [1.0, -2.0, 3.0, -4.0];
        let q_a = [1_i8, 0, -1, 1]; // ternary values
        let scale_a = 0.5;

        let mut packed_a = vec![0_u8; 1];
        for (col, &val) in q_a.iter().enumerate() {
            let code = (val + 1) as u8 & 0x03;
            packed_a[0] |= code << (col * 2);
        }

        let res = q1_58_dot_f32_scaled(&packed_a, &x, scale_a, 4);
        let expected = -6.0 * scale_a;
        assert!((res - expected).abs() < 1e-5);

        let (r_a, r_b, r_c, r_d) = q1_58_dot4_f32_scaled(
            &packed_a, &packed_a, &packed_a, &packed_a, &x, scale_a, scale_a, scale_a, scale_a, 4,
        );
        assert!((r_a - expected).abs() < 1e-5);
        assert!((r_b - expected).abs() < 1e-5);
        assert!((r_c - expected).abs() < 1e-5);
        assert!((r_d - expected).abs() < 1e-5);
    }
}
