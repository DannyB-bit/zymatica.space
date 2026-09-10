// Copyright © 2026 Zymatica
// SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
// See LICENSE for terms.

//! # Invention Class 32: Zymatica 8D Semantic DWORD & Octonion Hypercube Engine (Z-8D)
//!
//! Dual Architecture:
//! 1. Discrete 8D Semantic Tuple (packed 32-bit DWORD) with lossless geodesic stream delta codec.
//! 2. Continuous 8D Non-Associative Octonion Division Algebra ($\mathbb{O}$) with Cayley-Dickson multiplication,
//!    Fano plane basis relations, norms, conjugates, and continuous manifold embeddings.

use std::ops::{Add, Mul, Neg, Sub};

/// 8-Axis Discrete Semantic Concept packed into a 32-bit DWORD
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Concept8D {
    pub domain: u8,
    pub subdomain: u8,
    pub operation: u8,
    pub modality: u8,
    pub strength: u8,
    pub polarity: u8,
    pub temporal_horizon: u8,
    pub epistemic_certainty: u8,
}

impl Concept8D {
    #[allow(clippy::too_many_arguments)]
    pub fn new(d: u8, sub: u8, op: u8, mod_: u8, st: u8, pol: u8, temp: u8, cert: u8) -> Self {
        Self {
            domain: d & 0x0F,
            subdomain: sub & 0x0F,
            operation: op & 0x0F,
            modality: mod_ & 0x0F,
            strength: st & 0x0F,
            polarity: pol & 0x0F,
            temporal_horizon: temp & 0x0F,
            epistemic_certainty: cert & 0x0F,
        }
    }

    /// Pack into 32-bit DWORD: [R_C (8b), R_F (8b), R_A (8b), R_T (8b)]
    pub fn to_dword(&self) -> u32 {
        let rc = (self.domain << 4) | (self.subdomain & 0x0F);
        let rf = (self.operation << 4) | (self.modality & 0x0F);
        let ra = (self.strength << 4) | (self.polarity & 0x0F);
        let rt = (self.temporal_horizon << 4) | (self.epistemic_certainty & 0x0F);
        ((rc as u32) << 24) | ((rf as u32) << 16) | ((ra as u32) << 8) | (rt as u32)
    }

    /// Unpack from 32-bit DWORD
    pub fn from_dword(dword: u32) -> Self {
        Self {
            domain: ((dword >> 28) & 0x0F) as u8,
            subdomain: ((dword >> 24) & 0x0F) as u8,
            operation: ((dword >> 20) & 0x0F) as u8,
            modality: ((dword >> 16) & 0x0F) as u8,
            strength: ((dword >> 12) & 0x0F) as u8,
            polarity: ((dword >> 8) & 0x0F) as u8,
            temporal_horizon: ((dword >> 4) & 0x0F) as u8,
            epistemic_certainty: (dword & 0x0F) as u8,
        }
    }

    /// Embed 8D discrete concept into the continuous Octonion algebra ($\mathbb{O}$)
    pub fn to_octonion(&self) -> Octonion {
        Octonion::new([
            (self.domain as f32) / 15.0,
            (self.subdomain as f32) / 15.0,
            (self.operation as f32) / 15.0,
            (self.modality as f32) / 15.0,
            (self.strength as f32) / 15.0,
            (self.polarity as f32) / 15.0,
            (self.temporal_horizon as f32) / 15.0,
            (self.epistemic_certainty as f32) / 15.0,
        ])
    }
}

/// Continuous 8D Octonion Normed Division Algebra ($\mathbb{O}$)
///
/// Implements Cayley-Dickson multiplication over the 8-dimensional basis $(e_0, e_1, \dots, e_7)$
/// satisfying the non-associative Fano plane multiplication rules:
/// $e_0 = 1$, $e_i^2 = -1$ for $i \in \{1,\dots,7\}$,
/// oriented triples: $(1,2,4), (2,3,5), (3,4,6), (4,5,7), (5,6,1), (6,7,2), (7,1,3)$.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Octonion {
    pub c: [f32; 8],
}

impl Octonion {
    pub const ZERO: Self = Self { c: [0.0; 8] };
    pub const ONE: Self = Self {
        c: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
    };

    pub fn new(c: [f32; 8]) -> Self {
        Self { c }
    }

    /// Basis element $e_k$ for $k \in 0..8$
    pub fn basis(k: usize) -> Self {
        assert!(k < 8);
        let mut c = [0.0f32; 8];
        c[k] = 1.0;
        Self { c }
    }

    /// Real scalar part $e_0$
    pub fn real(&self) -> f32 {
        self.c[0]
    }

    /// Imaginary 7D vector part $(e_1, \dots, e_7)$
    pub fn imaginary(&self) -> [f32; 7] {
        [
            self.c[1], self.c[2], self.c[3], self.c[4], self.c[5], self.c[6], self.c[7],
        ]
    }

    /// Octonionic Conjugate: $\bar{x} = (x_0, -x_1, -x_2, \dots, -x_7)$
    pub fn conjugate(&self) -> Self {
        Self {
            c: [
                self.c[0], -self.c[1], -self.c[2], -self.c[3], -self.c[4], -self.c[5], -self.c[6],
                -self.c[7],
            ],
        }
    }

    /// Squared Euclidean Norm: $\|x\|^2 = \sum_{k=0}^7 x_k^2$
    pub fn norm_squared(&self) -> f32 {
        self.c.iter().map(|&x| x * x).sum()
    }

    /// Euclidean Norm: $\|x\| = \sqrt{\|x\|^2}$
    pub fn norm(&self) -> f32 {
        self.norm_squared().sqrt()
    }

    /// Multiplicative Inverse: $x^{-1} = \frac{\bar{x}}{\|x\|^2}$
    pub fn inverse(&self) -> Option<Self> {
        let n_sq = self.norm_squared();
        if n_sq < 1e-15 {
            None
        } else {
            let conj = self.conjugate();
            let inv_n = 1.0 / n_sq;
            let mut c = [0.0f32; 8];
            for (i, value) in c.iter_mut().enumerate() {
                *value = conj.c[i] * inv_n;
            }
            Some(Self { c })
        }
    }

    /// Euclidean Inner Product: $\langle a, b \rangle = \sum_{k=0}^7 a_k b_k = \text{Re}(a \bar{b})$
    pub fn dot(&self, other: &Self) -> f32 {
        self.c.iter().zip(&other.c).map(|(&a, &b)| a * b).sum()
    }

    /// Project onto discrete Concept8D
    pub fn to_concept8d(&self) -> Concept8D {
        let clamp_nibble = |v: f32| -> u8 {
            let scaled = (v * 15.0).round();
            if scaled < 0.0 {
                0
            } else if scaled > 15.0 {
                15
            } else {
                scaled as u8
            }
        };
        Concept8D::new(
            clamp_nibble(self.c[0]),
            clamp_nibble(self.c[1]),
            clamp_nibble(self.c[2]),
            clamp_nibble(self.c[3]),
            clamp_nibble(self.c[4]),
            clamp_nibble(self.c[5]),
            clamp_nibble(self.c[6]),
            clamp_nibble(self.c[7]),
        )
    }
}

impl Add for Octonion {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        let mut c = [0.0f32; 8];
        for (i, value) in c.iter_mut().enumerate() {
            *value = self.c[i] + rhs.c[i];
        }
        Self { c }
    }
}

impl Sub for Octonion {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        let mut c = [0.0f32; 8];
        for (i, value) in c.iter_mut().enumerate() {
            *value = self.c[i] - rhs.c[i];
        }
        Self { c }
    }
}

impl Neg for Octonion {
    type Output = Self;
    fn neg(self) -> Self {
        let mut c = [0.0f32; 8];
        for (i, value) in c.iter_mut().enumerate() {
            *value = -self.c[i];
        }
        Self { c }
    }
}

/// Cayley-Dickson / Fano-plane Octonionic Non-Associative Product
impl Mul for Octonion {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        let a = self.c;
        let b = rhs.c;

        // Cayley-Dickson multiplication of two quaternions (a_L, a_R) and (b_L, b_R):
        // (a_L + a_R * e_4)(b_L + b_R * e_4) = (a_L b_L - \bar{b}_R a_R) + (b_R a_L + a_R \bar{b}_L) e_4
        let mut c = [0.0f32; 8];

        c[0] = a[0] * b[0]
            - a[1] * b[1]
            - a[2] * b[2]
            - a[3] * b[3]
            - a[4] * b[4]
            - a[5] * b[5]
            - a[6] * b[6]
            - a[7] * b[7];
        c[1] = a[0] * b[1] + a[1] * b[0] + a[2] * b[4] + a[3] * b[7] - a[4] * b[2] + a[5] * b[6]
            - a[6] * b[5]
            - a[7] * b[3];
        c[2] = a[0] * b[2] - a[1] * b[4] + a[2] * b[0] + a[3] * b[5] + a[4] * b[1] - a[5] * b[3]
            + a[6] * b[7]
            - a[7] * b[6];
        c[3] = a[0] * b[3] - a[1] * b[7] - a[2] * b[5] + a[3] * b[0] + a[4] * b[6] + a[5] * b[2]
            - a[6] * b[4]
            + a[7] * b[1];
        c[4] = a[0] * b[4] + a[1] * b[2] - a[2] * b[1] - a[3] * b[6]
            + a[4] * b[0]
            + a[5] * b[7]
            + a[6] * b[3]
            - a[7] * b[5];
        c[5] = a[0] * b[5] - a[1] * b[6] + a[2] * b[3] - a[3] * b[2] - a[4] * b[7]
            + a[5] * b[0]
            + a[6] * b[1]
            + a[7] * b[4];
        c[6] = a[0] * b[6] + a[1] * b[5] - a[2] * b[7] + a[3] * b[4] - a[4] * b[3] - a[5] * b[1]
            + a[6] * b[0]
            + a[7] * b[2];
        c[7] = a[0] * b[7] + a[1] * b[3] + a[2] * b[6] - a[3] * b[1] + a[4] * b[5]
            - a[5] * b[4]
            - a[6] * b[2]
            + a[7] * b[0];

        Self { c }
    }
}

/// Hardened Lossless 8D Geodesic Stream Codec
pub struct Geodesic8DCodecHardened;

impl Geodesic8DCodecHardened {
    pub const KEYFRAME_INTERVAL: usize = 16;

    /// Losslessly encode a stream of Concept8D coordinates into a compressed byte stream
    pub fn encode_trajectory(trajectory: &[Concept8D]) -> Vec<u8> {
        if trajectory.is_empty() {
            return Vec::new();
        }
        let mut out = Vec::with_capacity(trajectory.len() * 4);

        let mut prev = trajectory[0];
        for (i, curr) in trajectory.iter().enumerate() {
            if i % Self::KEYFRAME_INTERVAL == 0 {
                // Keyframe: 0xC0 + 4-byte DWORD (5 bytes total)
                out.push(0xC0);
                out.extend_from_slice(&curr.to_dword().to_be_bytes());
            } else if *curr == prev {
                // Mode 00: Repeat previous coordinate (1 byte total)
                out.push(0x00);
            } else if curr.domain == prev.domain && curr.subdomain == prev.subdomain {
                // Mode 01: Domain & Subdomain unchanged; transmit remaining 6 nibbles losslessly (4 bytes total)
                out.push(0x40);
                out.push((curr.operation << 4) | (curr.modality & 0x0F));
                out.push((curr.strength << 4) | (curr.polarity & 0x0F));
                out.push((curr.temporal_horizon << 4) | (curr.epistemic_certainty & 0x0F));
            } else {
                // Mode 11: Global coordinate transition (5 bytes total)
                out.push(0xC0);
                out.extend_from_slice(&curr.to_dword().to_be_bytes());
            }
            prev = *curr;
        }
        out
    }

    /// Losslessly decode compressed byte stream back to exact Concept8D sequence
    pub fn decode_trajectory(bytes: &[u8]) -> Result<Vec<Concept8D>, &'static str> {
        if bytes.is_empty() {
            return Ok(Vec::new());
        }
        if bytes.len() < 5 || bytes[0] != 0xC0 {
            return Err("Stream must begin with a 0xC0 Keyframe header");
        }

        let mut out = Vec::new();
        let mut idx = 0;
        let mut curr =
            Concept8D::from_dword(u32::from_be_bytes([bytes[1], bytes[2], bytes[3], bytes[4]]));
        out.push(curr);
        idx += 5;

        while idx < bytes.len() {
            let mode = bytes[idx];
            if mode == 0x00 {
                // Repeat
                out.push(curr);
                idx += 1;
            } else if mode == 0x40 {
                // Local update (3 data bytes)
                if idx + 3 >= bytes.len() {
                    return Err("Unexpected EOF in Mode 01 8D payload");
                }
                let b1 = bytes[idx + 1];
                let b2 = bytes[idx + 2];
                let b3 = bytes[idx + 3];
                curr.operation = (b1 >> 4) & 0x0F;
                curr.modality = b1 & 0x0F;
                curr.strength = (b2 >> 4) & 0x0F;
                curr.polarity = b2 & 0x0F;
                curr.temporal_horizon = (b3 >> 4) & 0x0F;
                curr.epistemic_certainty = b3 & 0x0F;
                out.push(curr);
                idx += 4;
            } else if mode == 0xC0 {
                // Keyframe / Escape (4 data bytes)
                if idx + 4 >= bytes.len() {
                    return Err("Unexpected EOF in Mode 11 8D Keyframe");
                }
                let dw = u32::from_be_bytes([
                    bytes[idx + 1],
                    bytes[idx + 2],
                    bytes[idx + 3],
                    bytes[idx + 4],
                ]);
                curr = Concept8D::from_dword(dw);
                out.push(curr);
                idx += 5;
            } else {
                return Err("Invalid 8D stream mode prefix");
            }
        }

        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_concept8d_dword_roundtrip() {
        for d in 0..16 {
            for sub in 0..16 {
                let c = Concept8D::new(d, sub, 15 - d, (sub * 3) % 16, 12, 5, 8, 14);
                let dw = c.to_dword();
                let decoded = Concept8D::from_dword(dw);
                assert_eq!(c, decoded);
            }
        }
    }

    #[test]
    fn test_octonion_algebra_basis_multiplication() {
        // e_0 is identity
        let e0 = Octonion::basis(0);
        let e1 = Octonion::basis(1);
        let e2 = Octonion::basis(2);
        let e3 = Octonion::basis(3);
        let e4 = Octonion::basis(4);
        let e5 = Octonion::basis(5);
        let e6 = Octonion::basis(6);
        let _e7 = Octonion::basis(7);

        assert_eq!(e0 * e1, e1);
        assert_eq!(e1 * e0, e1);

        // e_i^2 = -1 for i > 0
        for k in 1..8 {
            let ek = Octonion::basis(k);
            assert_eq!(ek * ek, -e0);
        }

        // Fano plane oriented multiplication: e1 * e2 = e4
        assert_eq!(e1 * e2, e4);
        assert_eq!(e2 * e1, -e4);

        // e2 * e3 = e5
        assert_eq!(e2 * e3, e5);
        assert_eq!(e3 * e2, -e5);

        // Non-associativity: (e1 * e2) * e3 != e1 * (e2 * e3)
        let left = (e1 * e2) * e3; // e4 * e3 = -e6
        let right = e1 * (e2 * e3); // e1 * e5 = +e6
        assert_eq!(left, -e6);
        assert_eq!(right, e6);
        assert_ne!(left, right);
    }

    #[test]
    fn test_octonion_norm_composition_property() {
        // ||a * b|| == ||a|| * ||b|| (Division Algebra Hurwitz theorem)
        let a = Octonion::new([1.0, 2.0, -1.0, 0.5, 3.0, -2.5, 1.2, -0.8]);
        let b = Octonion::new([-0.5, 1.2, 3.0, -1.5, 0.8, 2.1, -1.0, 0.4]);

        let prod = a * b;
        let norm_prod = prod.norm();
        let expected = a.norm() * b.norm();
        assert!((norm_prod - expected).abs() < 1e-4);
    }

    #[test]
    fn test_geodesic_8d_codec_hardened_lossless_property() {
        let mut trajectory = Vec::new();
        for i in 0..200 {
            trajectory.push(Concept8D::new(
                ((i / 20) % 16) as u8,
                ((i / 5) % 16) as u8,
                (i % 16) as u8,
                ((i * 3) % 16) as u8,
                ((i * 7) % 16) as u8,
                ((i * 11) % 16) as u8,
                ((i * 13) % 16) as u8,
                ((i * 5) % 16) as u8,
            ));
        }

        let encoded = Geodesic8DCodecHardened::encode_trajectory(&trajectory);
        let decoded =
            Geodesic8DCodecHardened::decode_trajectory(&encoded).expect("Decode should succeed");
        assert_eq!(trajectory, decoded);
    }
}
