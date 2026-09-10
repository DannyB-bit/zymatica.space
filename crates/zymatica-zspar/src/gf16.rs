// Copyright © 2026 Zymatica
// SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
// See LICENSE for terms.

//! Galois Field GF(16) Arithmetic with primitive polynomial p(x) = x^4 + x + 1 (0x13)
//! Generator element alpha = 2

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gf16;

impl Gf16 {
    pub const EXP: [u8; 32] = [
        1, 2, 4, 8, 3, 6, 12, 11, 5, 10, 7, 14, 15, 13, 9, 1, 2, 4, 8, 3, 6, 12, 11, 5, 10, 7, 14,
        15, 13, 9, 1, 2,
    ];

    pub const LOG: [u8; 16] = [0, 0, 1, 4, 2, 8, 5, 10, 3, 14, 9, 7, 6, 13, 11, 12];

    #[inline]
    pub const fn add(a: u8, b: u8) -> u8 {
        (a ^ b) & 0x0f
    }

    #[inline]
    pub const fn sub(a: u8, b: u8) -> u8 {
        (a ^ b) & 0x0f
    }

    #[inline]
    pub fn mul(a: u8, b: u8) -> u8 {
        let a = a & 0x0f;
        let b = b & 0x0f;
        if a == 0 || b == 0 {
            0
        } else {
            let log_sum = (Self::LOG[a as usize] as usize) + (Self::LOG[b as usize] as usize);
            Self::EXP[log_sum % 15]
        }
    }

    #[inline]
    pub fn div(a: u8, b: u8) -> Option<u8> {
        let a = a & 0x0f;
        let b = b & 0x0f;
        if b == 0 {
            return None;
        }
        if a == 0 {
            return Some(0);
        }
        let log_diff = (Self::LOG[a as usize] as i32) - (Self::LOG[b as usize] as i32) + 15;
        Some(Self::EXP[(log_diff as usize) % 15])
    }

    #[inline]
    pub fn inv(a: u8) -> Option<u8> {
        Self::div(1, a)
    }

    #[inline]
    pub fn alpha_pow(p: u32) -> u8 {
        Self::EXP[(p as usize) % 15]
    }

    #[inline]
    pub fn poly_eval(poly: &[u8], x: u8) -> u8 {
        let mut acc = 0u8;
        for &c in poly {
            acc = Self::add(Self::mul(acc, x), c);
        }
        acc
    }
}
