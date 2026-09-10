// Copyright © 2026 Zymatica
// SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
// See LICENSE for terms.

//! Systematic Reed-Solomon RS(12,8) Codec over GF(16)
//! Generator polynomial g(x) = (x-a)(x-a^2)(x-a^3)(x-a^4) = [1, 13, 12, 8, 7]
//! Bounded distance decoding: 2*e + s <= 4

use crate::gf16::Gf16;

pub const K_DATA_SYMBOLS: usize = 8;
pub const K_PARITY_SYMBOLS: usize = 4;
pub const K_CODE_SYMBOLS: usize = 12;
pub const K_MAX_SYMBOL_ERRORS: usize = 2;
pub const K_MAX_ERASURES: usize = 4;

pub type Data = [u8; K_DATA_SYMBOLS];
pub type Parity = [u8; K_PARITY_SYMBOLS];
pub type Codeword = [u8; K_CODE_SYMBOLS];
pub type Syndromes = [u8; K_PARITY_SYMBOLS];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeStatus {
    Clean,
    Corrected,
    Uncorrectable,
    InvalidInput,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodeResult {
    pub status: DecodeStatus,
    pub codeword: Codeword,
    pub syndromes_before: Syndromes,
    pub corrected_positions: Vec<u8>,
    pub erasure_positions: Vec<u8>,
}

impl DecodeResult {
    pub fn success(&self) -> bool {
        self.status == DecodeStatus::Clean || self.status == DecodeStatus::Corrected
    }
}

pub struct Rs12_8;

impl Rs12_8 {
    pub const GENERATOR: [u8; 5] = [1, 13, 12, 8, 7];

    pub fn encode(data: &Data) -> Result<Codeword, &'static str> {
        for &d in data {
            if d > 15 {
                return Err("Symbol out of GF(16) range");
            }
        }

        let mut work = [0u8; K_CODE_SYMBOLS];
        work[..K_DATA_SYMBOLS].copy_from_slice(data);

        for i in 0..K_DATA_SYMBOLS {
            let coef = work[i];
            if coef == 0 {
                continue;
            }
            for j in 1..Self::GENERATOR.len() {
                work[i + j] = Gf16::add(work[i + j], Gf16::mul(Self::GENERATOR[j], coef));
            }
        }

        let mut out = [0u8; K_CODE_SYMBOLS];
        out[..K_DATA_SYMBOLS].copy_from_slice(data);
        out[K_DATA_SYMBOLS..].copy_from_slice(&work[K_DATA_SYMBOLS..]);
        Ok(out)
    }

    pub fn parity_for(data: &Data) -> Result<Parity, &'static str> {
        let cw = Self::encode(data)?;
        let mut p = [0u8; K_PARITY_SYMBOLS];
        p.copy_from_slice(&cw[K_DATA_SYMBOLS..]);
        Ok(p)
    }

    pub fn syndromes(codeword: &Codeword) -> Syndromes {
        let mut out = [0u8; K_PARITY_SYMBOLS];
        for (s, slot) in out.iter_mut().enumerate() {
            let x = Gf16::alpha_pow((s + 1) as u32);
            let mut acc = 0u8;
            for &sym in codeword {
                acc = Gf16::add(Gf16::mul(acc, x), sym);
            }
            *slot = acc;
        }
        out
    }

    pub fn pack_parity(parity: &Parity) -> Option<[u8; 2]> {
        for &p in parity {
            if p > 15 {
                return None;
            }
        }
        Some([
            (parity[0] << 4) | (parity[1] & 0x0f),
            (parity[2] << 4) | (parity[3] & 0x0f),
        ])
    }

    pub fn unpack_parity(bytes: &[u8; 2]) -> Parity {
        [
            (bytes[0] >> 4) & 0x0f,
            bytes[0] & 0x0f,
            (bytes[1] >> 4) & 0x0f,
            bytes[1] & 0x0f,
        ]
    }

    pub fn decode(received: Codeword, erasures: &[u8]) -> DecodeResult {
        for &sym in &received {
            if sym > 15 {
                return DecodeResult {
                    status: DecodeStatus::InvalidInput,
                    codeword: received,
                    syndromes_before: [0; 4],
                    corrected_positions: Vec::new(),
                    erasure_positions: erasures.to_vec(),
                };
            }
        }

        let mut cleaned_erasures = Vec::new();
        for &p in erasures {
            if (p as usize) < K_CODE_SYMBOLS && !cleaned_erasures.contains(&p) {
                cleaned_erasures.push(p);
            }
        }

        let s_before = Self::syndromes(&received);
        if s_before == [0; 4] && cleaned_erasures.is_empty() {
            return DecodeResult {
                status: DecodeStatus::Clean,
                codeword: received,
                syndromes_before: s_before,
                corrected_positions: Vec::new(),
                erasure_positions: cleaned_erasures,
            };
        }

        let num_erasures = cleaned_erasures.len();
        if num_erasures > K_MAX_ERASURES {
            return DecodeResult {
                status: DecodeStatus::Uncorrectable,
                codeword: received,
                syndromes_before: s_before,
                corrected_positions: Vec::new(),
                erasure_positions: cleaned_erasures,
            };
        }

        // Try decoding with 0, 1, or 2 additional unknown errors
        let max_additional_errors = (4 - num_erasures) / 2;

        for num_errors in 0..=max_additional_errors {
            if let Some((corrected, err_pos)) =
                Self::try_solve(&received, &cleaned_erasures, num_errors, s_before)
            {
                let mut all_pos = cleaned_erasures.clone();
                for &ep in &err_pos {
                    if !all_pos.contains(&ep) {
                        all_pos.push(ep);
                    }
                }
                all_pos.sort_unstable();

                let status = if all_pos.is_empty() {
                    DecodeStatus::Clean
                } else {
                    DecodeStatus::Corrected
                };

                return DecodeResult {
                    status,
                    codeword: corrected,
                    syndromes_before: s_before,
                    corrected_positions: all_pos,
                    erasure_positions: cleaned_erasures,
                };
            }
        }

        DecodeResult {
            status: DecodeStatus::Uncorrectable,
            codeword: received,
            syndromes_before: s_before,
            corrected_positions: Vec::new(),
            erasure_positions: cleaned_erasures,
        }
    }

    fn try_solve(
        received: &Codeword,
        erasures: &[u8],
        num_errors: usize,
        s_before: Syndromes,
    ) -> Option<(Codeword, Vec<u8>)> {
        let available_positions: Vec<u8> = (0..K_CODE_SYMBOLS as u8)
            .filter(|p| !erasures.contains(p))
            .collect();

        if num_errors == 0 {
            let error_positions: Vec<u8> = erasures.to_vec();
            if let Some(corrected) = Self::solve_positions(received, &error_positions, s_before) {
                return Some((corrected, Vec::new()));
            }
            return None;
        }

        if num_errors == 1 {
            for &p1 in &available_positions {
                let mut pos = erasures.to_vec();
                pos.push(p1);
                if let Some(corrected) = Self::solve_positions(received, &pos, s_before) {
                    return Some((corrected, vec![p1]));
                }
            }
            return None;
        }

        if num_errors == 2 {
            for i in 0..available_positions.len() {
                for j in (i + 1)..available_positions.len() {
                    let p1 = available_positions[i];
                    let p2 = available_positions[j];
                    let mut pos = erasures.to_vec();
                    pos.push(p1);
                    pos.push(p2);
                    if let Some(corrected) = Self::solve_positions(received, &pos, s_before) {
                        return Some((corrected, vec![p1, p2]));
                    }
                }
            }
            return None;
        }

        None
    }

    fn solve_positions(
        received: &Codeword,
        positions: &[u8],
        syndromes: Syndromes,
    ) -> Option<Codeword> {
        let m = positions.len();
        if m == 0 {
            return if syndromes == [0; 4] {
                Some(*received)
            } else {
                None
            };
        }
        if m > 4 {
            return None;
        }

        // Matrix A: A[s][j] = (alpha^(s+1))^(11 - pos[j])
        let mut a = vec![vec![0u8; m]; 4];
        for (s, a_row) in a.iter_mut().enumerate() {
            let x = Gf16::alpha_pow((s + 1) as u32);
            for (j, &pos) in positions.iter().enumerate() {
                let power = 11 - (pos as usize);
                let mut val = 1u8;
                for _ in 0..power {
                    val = Gf16::mul(val, x);
                }
                a_row[j] = val;
            }
        }

        let mut b = syndromes.to_vec();

        // Gaussian elimination on m x m subsystem
        for col in 0..m {
            let mut pivot_row = None;
            for (row, a_row) in a.iter().enumerate().skip(col) {
                if a_row[col] != 0 {
                    pivot_row = Some(row);
                    break;
                }
            }
            let pr = pivot_row?;
            if pr != col {
                a.swap(pr, col);
                b.swap(pr, col);
            }

            let inv_pivot = Gf16::inv(a[col][col])?;
            for val in a[col].iter_mut().take(m).skip(col) {
                *val = Gf16::mul(*val, inv_pivot);
            }
            b[col] = Gf16::mul(b[col], inv_pivot);

            for row in 0..4 {
                if row != col && a[row][col] != 0 {
                    let factor = a[row][col];
                    let col_slice = a[col][col..m].to_vec();
                    for (c, &col_val) in col_slice.iter().enumerate() {
                        let factor_a = Gf16::mul(factor, col_val);
                        a[row][col + c] = Gf16::add(a[row][col + c], factor_a);
                    }
                    b[row] = Gf16::add(b[row], Gf16::mul(factor, b[col]));
                }
            }
        }

        // Check remaining equations
        for &val in b.iter().take(4).skip(m) {
            if val != 0 {
                return None;
            }
        }

        let mut corrected = *received;
        for (j, &pos) in positions.iter().enumerate() {
            let error_val = b[j];
            if error_val > 15 {
                return None;
            }
            corrected[pos as usize] = Gf16::sub(corrected[pos as usize], error_val);
        }

        // Verify syndromes after correction
        if Self::syndromes(&corrected) == [0; 4] {
            Some(corrected)
        } else {
            None
        }
    }
}
