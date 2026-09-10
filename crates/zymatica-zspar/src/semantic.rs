// Copyright © 2026 Zymatica
// SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
// See LICENSE for terms.

use crate::crc32c::crc32c;
use crate::rs12_8::{Parity, Rs12_8, Syndromes};
use crate::sha256::{sha256_128, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
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
    pub const fn new(
        domain: u8,
        subdomain: u8,
        operation: u8,
        modality: u8,
        strength: u8,
        polarity: u8,
        temporal_horizon: u8,
        epistemic_certainty: u8,
    ) -> Self {
        Self {
            domain: domain & 0x0f,
            subdomain: subdomain & 0x0f,
            operation: operation & 0x0f,
            modality: modality & 0x0f,
            strength: strength & 0x0f,
            polarity: polarity & 0x0f,
            temporal_horizon: temporal_horizon & 0x0f,
            epistemic_certainty: epistemic_certainty & 0x0f,
        }
    }

    pub const fn symbols(&self) -> [u8; 8] {
        [
            self.domain,
            self.subdomain,
            self.operation,
            self.modality,
            self.strength,
            self.polarity,
            self.temporal_horizon,
            self.epistemic_certainty,
        ]
    }

    pub const fn from_symbols(s: [u8; 8]) -> Self {
        Self::new(s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7])
    }

    pub const fn to_dword(&self) -> u32 {
        ((self.domain as u32) << 28)
            | ((self.subdomain as u32) << 24)
            | ((self.operation as u32) << 20)
            | ((self.modality as u32) << 16)
            | ((self.strength as u32) << 12)
            | ((self.polarity as u32) << 8)
            | ((self.temporal_horizon as u32) << 4)
            | self.epistemic_certainty as u32
    }

    pub const fn from_dword(dword: u32) -> Self {
        Self::new(
            ((dword >> 28) & 0x0f) as u8,
            ((dword >> 24) & 0x0f) as u8,
            ((dword >> 20) & 0x0f) as u8,
            ((dword >> 16) & 0x0f) as u8,
            ((dword >> 12) & 0x0f) as u8,
            ((dword >> 8) & 0x0f) as u8,
            ((dword >> 4) & 0x0f) as u8,
            (dword & 0x0f) as u8,
        )
    }
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InvariantKind {
    Entity = 1,
    Quantity = 2,
    Unit = 3,
    Negation = 4,
    Tool = 5,
    Argument = 6,
    Policy = 7,
    Custom = 255,
}

impl InvariantKind {
    fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Entity,
            2 => Self::Quantity,
            3 => Self::Unit,
            4 => Self::Negation,
            5 => Self::Tool,
            6 => Self::Argument,
            7 => Self::Policy,
            _ => Self::Custom,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct InvariantRecord {
    pub kind: InvariantKind,
    pub key: u16,
    pub value: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InvariantSet {
    records: Vec<InvariantRecord>,
}

impl InvariantSet {
    pub const MAX_RECORDS: usize = 32;

    pub fn new(mut records: Vec<InvariantRecord>) -> Result<Self, &'static str> {
        if records.len() > Self::MAX_RECORDS {
            return Err("too many semantic invariant records");
        }
        records.sort_unstable();
        Ok(Self { records })
    }

    pub fn add(&mut self, record: InvariantRecord) -> Result<(), &'static str> {
        if self.records.len() >= Self::MAX_RECORDS {
            return Err("semantic invariant record limit exceeded");
        }
        self.records.push(record);
        self.records.sort_unstable();
        Ok(())
    }

    pub fn records(&self) -> &[InvariantRecord] {
        &self.records
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(1 + self.records.len() * 11);
        out.push(self.records.len() as u8);
        for record in &self.records {
            out.push(record.kind as u8);
            out.extend_from_slice(&record.key.to_be_bytes());
            out.extend_from_slice(&record.value.to_be_bytes());
        }
        out
    }

    pub fn parse_canonical(bytes: &[u8]) -> Option<Self> {
        let (&count, rest) = bytes.split_first()?;
        let count = count as usize;
        if count > Self::MAX_RECORDS || rest.len() != count * 11 {
            return None;
        }
        let mut records = Vec::with_capacity(count);
        let (chunks, _) = rest.as_chunks::<11>();
        for chunk in chunks {
            let kind = InvariantKind::from_u8(chunk[0]);
            let key = u16::from_be_bytes([chunk[1], chunk[2]]);
            let value = i64::from_be_bytes([
                chunk[3], chunk[4], chunk[5], chunk[6], chunk[7], chunk[8], chunk[9], chunk[10],
            ]);
            records.push(InvariantRecord { kind, key, value });
        }
        Self::new(records).ok()
    }
}

pub fn stable_text_id(text: &str) -> u64 {
    let digest = Sha256::digest(text.as_bytes());
    u64::from_be_bytes([
        digest[0], digest[1], digest[2], digest[3], digest[4], digest[5], digest[6], digest[7],
    ])
}

pub fn semantic_tag(sequence: u64, state: &Concept8D, invariants: &InvariantSet) -> [u8; 16] {
    const DOMAIN: &[u8] = b"ZSPAR-SEMANTIC-V1";
    let inv = invariants.canonical_bytes();
    let mut material = Vec::with_capacity(DOMAIN.len() + 8 + 4 + inv.len());
    material.extend_from_slice(DOMAIN);
    material.extend_from_slice(&sequence.to_be_bytes());
    material.extend_from_slice(&state.to_dword().to_be_bytes());
    material.extend_from_slice(&inv);
    sha256_128(&material)
}

pub fn axis_diff_mask(a: &Concept8D, b: &Concept8D) -> u8 {
    let x = a.symbols();
    let y = b.symbols();
    let mut mask = 0u8;
    for i in 0..8 {
        if x[i] != y[i] {
            mask |= 1u8 << i;
        }
    }
    mask
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairStatus {
    Clean,
    Corrected,
    Uncorrectable,
    SemanticTagMismatch,
    InvalidFrame,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairResult {
    pub status: RepairStatus,
    pub state: Concept8D,
    pub corrected_axis_mask: u8,
    pub syndromes: Syndromes,
}

impl RepairResult {
    pub fn success(&self) -> bool {
        matches!(self.status, RepairStatus::Clean | RepairStatus::Corrected)
    }
}

fn append_crc(bytes: &mut Vec<u8>) {
    let crc = crc32c(bytes);
    bytes.extend_from_slice(&crc.to_be_bytes());
}

fn verify_crc(bytes: &[u8]) -> bool {
    if bytes.len() < 4 {
        return false;
    }
    let body_len = bytes.len() - 4;
    let expected = u32::from_be_bytes([
        bytes[body_len],
        bytes[body_len + 1],
        bytes[body_len + 2],
        bytes[body_len + 3],
    ]);
    crc32c(&bytes[..body_len]) == expected
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystematicFrame {
    pub sequence: u64,
    pub state: Concept8D,
    pub parity: Parity,
    pub tag: [u8; 16],
    pub has_invariants: bool,
}

impl SystematicFrame {
    pub const MAGIC: [u8; 4] = *b"ZSPS";
    pub const VERSION: u8 = 1;
    pub const WIRE_SIZE: usize = 40;

    pub fn create(sequence: u64, state: Concept8D, invariants: &InvariantSet) -> Self {
        let parity = Rs12_8::parity_for(&state.symbols()).expect("Concept8D symbols are nibbles");
        let tag = semantic_tag(sequence, &state, invariants);
        Self {
            sequence,
            state,
            parity,
            tag,
            has_invariants: !invariants.is_empty(),
        }
    }

    pub fn serialize(&self) -> [u8; 40] {
        let mut out = Vec::with_capacity(Self::WIRE_SIZE);
        out.extend_from_slice(&Self::MAGIC);
        out.push(Self::VERSION);
        out.push(if self.has_invariants { 1 } else { 0 });
        out.extend_from_slice(&self.sequence.to_be_bytes());
        out.extend_from_slice(&self.state.to_dword().to_be_bytes());
        out.extend_from_slice(&Rs12_8::pack_parity(&self.parity).expect("parity is nibble-sized"));
        out.extend_from_slice(&self.tag);
        append_crc(&mut out);
        let mut arr = [0u8; Self::WIRE_SIZE];
        arr.copy_from_slice(&out);
        arr
    }

    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != Self::WIRE_SIZE || !verify_crc(bytes) || bytes[..4] != Self::MAGIC {
            return None;
        }
        if bytes[4] != Self::VERSION {
            return None;
        }
        let sequence = u64::from_be_bytes(bytes[6..14].try_into().ok()?);
        let state = Concept8D::from_dword(u32::from_be_bytes(bytes[14..18].try_into().ok()?));
        let parity = Rs12_8::unpack_parity(&[bytes[18], bytes[19]]);
        let mut tag = [0u8; 16];
        tag.copy_from_slice(&bytes[20..36]);
        Some(Self {
            sequence,
            state,
            parity,
            tag,
            has_invariants: (bytes[5] & 1) != 0,
        })
    }

    pub fn verify_semantics(&self, invariants: &InvariantSet) -> bool {
        semantic_tag(self.sequence, &self.state, invariants) == self.tag
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParityOnlyFrame {
    pub sequence: u64,
    pub parity: Parity,
    pub tag: [u8; 16],
    pub has_invariants: bool,
}

impl ParityOnlyFrame {
    pub const MAGIC: [u8; 4] = *b"ZSPP";
    pub const VERSION: u8 = 1;
    pub const WIRE_SIZE: usize = 36;

    pub fn create(sequence: u64, authoritative: Concept8D, invariants: &InvariantSet) -> Self {
        let parity =
            Rs12_8::parity_for(&authoritative.symbols()).expect("Concept8D symbols are nibbles");
        let tag = semantic_tag(sequence, &authoritative, invariants);
        Self {
            sequence,
            parity,
            tag,
            has_invariants: !invariants.is_empty(),
        }
    }

    pub fn serialize(&self) -> [u8; 36] {
        let mut out = Vec::with_capacity(Self::WIRE_SIZE);
        out.extend_from_slice(&Self::MAGIC);
        out.push(Self::VERSION);
        out.push(if self.has_invariants { 1 } else { 0 });
        out.extend_from_slice(&self.sequence.to_be_bytes());
        out.extend_from_slice(&Rs12_8::pack_parity(&self.parity).expect("parity is nibble-sized"));
        out.extend_from_slice(&self.tag);
        append_crc(&mut out);
        let mut arr = [0u8; Self::WIRE_SIZE];
        arr.copy_from_slice(&out);
        arr
    }

    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != Self::WIRE_SIZE || !verify_crc(bytes) || bytes[..4] != Self::MAGIC {
            return None;
        }
        if bytes[4] != Self::VERSION {
            return None;
        }
        let sequence = u64::from_be_bytes(bytes[6..14].try_into().ok()?);
        let parity = Rs12_8::unpack_parity(&[bytes[14], bytes[15]]);
        let mut tag = [0u8; 16];
        tag.copy_from_slice(&bytes[16..32]);
        Some(Self {
            sequence,
            parity,
            tag,
            has_invariants: (bytes[5] & 1) != 0,
        })
    }

    pub fn repair_prediction(
        &self,
        predicted: Concept8D,
        predicted_invariants: &InvariantSet,
        erasure_axis_mask: u8,
    ) -> RepairResult {
        let mut received = [0u8; 12];
        received[..8].copy_from_slice(&predicted.symbols());
        received[8..].copy_from_slice(&self.parity);

        let mut erasures = Vec::new();
        for axis in 0..8u8 {
            if (erasure_axis_mask & (1u8 << axis)) != 0 {
                erasures.push(axis);
            }
        }

        let decoded = Rs12_8::decode(received, &erasures);
        if !decoded.success() {
            return RepairResult {
                status: RepairStatus::Uncorrectable,
                state: predicted,
                corrected_axis_mask: 0,
                syndromes: decoded.syndromes_before,
            };
        }

        let mut corrected_symbols = [0u8; 8];
        corrected_symbols.copy_from_slice(&decoded.codeword[..8]);
        let corrected = Concept8D::from_symbols(corrected_symbols);
        let corrected_axis_mask = axis_diff_mask(&predicted, &corrected);

        if semantic_tag(self.sequence, &corrected, predicted_invariants) != self.tag {
            return RepairResult {
                status: RepairStatus::SemanticTagMismatch,
                state: corrected,
                corrected_axis_mask,
                syndromes: decoded.syndromes_before,
            };
        }

        RepairResult {
            status: if corrected == predicted && erasure_axis_mask == 0 {
                RepairStatus::Clean
            } else {
                RepairStatus::Corrected
            },
            state: corrected,
            corrected_axis_mask,
            syndromes: decoded.syndromes_before,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairRequest {
    pub sequence: u64,
    pub predicted: Concept8D,
    pub erasure_axis_mask: u8,
    pub syndromes: Syndromes,
    pub expected_tag: [u8; 16],
}

impl RepairRequest {
    pub const MAGIC: [u8; 4] = *b"ZSRQ";
    pub const VERSION: u8 = 1;
    pub const WIRE_SIZE: usize = 41;

    pub fn from_failed_prediction(
        frame: &ParityOnlyFrame,
        predicted: Concept8D,
        erasure_axis_mask: u8,
    ) -> Self {
        let mut received = [0u8; 12];
        received[..8].copy_from_slice(&predicted.symbols());
        received[8..].copy_from_slice(&frame.parity);
        let syndromes = Rs12_8::syndromes(&received);
        Self {
            sequence: frame.sequence,
            predicted,
            erasure_axis_mask,
            syndromes,
            expected_tag: frame.tag,
        }
    }

    pub fn serialize(&self) -> [u8; 41] {
        let mut out = Vec::with_capacity(Self::WIRE_SIZE);
        out.extend_from_slice(&Self::MAGIC);
        out.push(Self::VERSION);
        out.push(0);
        out.extend_from_slice(&self.sequence.to_be_bytes());
        out.extend_from_slice(&self.predicted.to_dword().to_be_bytes());
        out.push(self.erasure_axis_mask);
        out.push((self.syndromes[0] << 4) | self.syndromes[1]);
        out.push((self.syndromes[2] << 4) | self.syndromes[3]);
        out.extend_from_slice(&self.expected_tag);
        append_crc(&mut out);
        let mut arr = [0u8; Self::WIRE_SIZE];
        arr.copy_from_slice(&out);
        arr
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairResponse {
    pub sequence: u64,
    pub authoritative: Concept8D,
    pub parity: Parity,
    pub tag: [u8; 16],
    pub has_invariants: bool,
}

impl RepairResponse {
    pub const MAGIC: [u8; 4] = *b"ZSRP";
    pub const VERSION: u8 = 1;
    pub const WIRE_SIZE: usize = 40;

    pub fn create(sequence: u64, authoritative: Concept8D, invariants: &InvariantSet) -> Self {
        Self {
            sequence,
            authoritative,
            parity: Rs12_8::parity_for(&authoritative.symbols())
                .expect("Concept8D symbols are nibbles"),
            tag: semantic_tag(sequence, &authoritative, invariants),
            has_invariants: !invariants.is_empty(),
        }
    }

    pub fn serialize(&self) -> [u8; 40] {
        let mut out = Vec::with_capacity(Self::WIRE_SIZE);
        out.extend_from_slice(&Self::MAGIC);
        out.push(Self::VERSION);
        out.push(if self.has_invariants { 1 } else { 0 });
        out.extend_from_slice(&self.sequence.to_be_bytes());
        out.extend_from_slice(&self.authoritative.to_dword().to_be_bytes());
        out.extend_from_slice(&Rs12_8::pack_parity(&self.parity).expect("parity is nibble-sized"));
        out.extend_from_slice(&self.tag);
        append_crc(&mut out);
        let mut arr = [0u8; Self::WIRE_SIZE];
        arr.copy_from_slice(&out);
        arr
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvariantPatchFrame {
    pub sequence: u64,
    pub authoritative: Concept8D,
    pub invariants: InvariantSet,
    pub tag: [u8; 16],
}

impl InvariantPatchFrame {
    pub const MAGIC: [u8; 4] = *b"ZSIP";
    pub const VERSION: u8 = 1;

    pub fn create(sequence: u64, authoritative: Concept8D, invariants: InvariantSet) -> Self {
        let tag = semantic_tag(sequence, &authoritative, &invariants);
        Self {
            sequence,
            authoritative,
            invariants,
            tag,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let inv = self.invariants.canonical_bytes();
        let mut out = Vec::with_capacity(4 + 1 + 1 + 8 + 4 + inv.len() + 16 + 4);
        out.extend_from_slice(&Self::MAGIC);
        out.push(Self::VERSION);
        out.push(0);
        out.extend_from_slice(&self.sequence.to_be_bytes());
        out.extend_from_slice(&self.authoritative.to_dword().to_be_bytes());
        out.extend_from_slice(&inv);
        out.extend_from_slice(&self.tag);
        append_crc(&mut out);
        out
    }

    pub fn parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 39
            || !verify_crc(bytes)
            || bytes[..4] != Self::MAGIC
            || bytes[4] != Self::VERSION
        {
            return None;
        }
        let inv_start = 18usize;
        let count = *bytes.get(inv_start)? as usize;
        if count > InvariantSet::MAX_RECORDS {
            return None;
        }
        let inv_len = 1 + count * 11;
        let expected_size = inv_start + inv_len + 16 + 4;
        if bytes.len() != expected_size {
            return None;
        }
        let sequence = u64::from_be_bytes(bytes[6..14].try_into().ok()?);
        let authoritative =
            Concept8D::from_dword(u32::from_be_bytes(bytes[14..18].try_into().ok()?));
        let invariants = InvariantSet::parse_canonical(&bytes[inv_start..inv_start + inv_len])?;
        let mut tag = [0u8; 16];
        tag.copy_from_slice(&bytes[inv_start + inv_len..inv_start + inv_len + 16]);
        if semantic_tag(sequence, &authoritative, &invariants) != tag {
            return None;
        }
        Some(Self {
            sequence,
            authoritative,
            invariants,
            tag,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn invariants() -> InvariantSet {
        InvariantSet::new(vec![
            InvariantRecord {
                kind: InvariantKind::Entity,
                key: 1,
                value: stable_text_id("VALVE-7") as i64,
            },
            InvariantRecord {
                kind: InvariantKind::Quantity,
                key: 2,
                value: 50,
            },
            InvariantRecord {
                kind: InvariantKind::Unit,
                key: 2,
                value: stable_text_id("PSI") as i64,
            },
            InvariantRecord {
                kind: InvariantKind::Negation,
                key: 9,
                value: 1,
            },
        ])
        .unwrap()
    }

    #[test]
    fn golden_wire_vectors_match_cpp() {
        let state = Concept8D::new(1, 4, 12, 1, 0, 15, 2, 13);
        let inv = invariants();
        let seq = 0x0102_0304_0506_0708u64;
        let sf = SystematicFrame::create(seq, state, &inv);
        let pf = ParityOnlyFrame::create(seq, state, &inv);
        assert_eq!(state.to_dword(), 0x14c1_0f2d);
        assert_eq!(sf.parity, [15, 5, 15, 14]);
        assert_eq!(
            sf.tag,
            [
                0xbd, 0x50, 0x0f, 0x86, 0x24, 0x70, 0x8b, 0x9c, 0x0a, 0x5d, 0x1e, 0x51, 0x40, 0xd3,
                0x48, 0x83,
            ]
        );
        assert_eq!(
            sf.serialize(),
            [
                0x5a, 0x53, 0x50, 0x53, 0x01, 0x01, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
                0x14, 0xc1, 0x0f, 0x2d, 0xf5, 0xfe, 0xbd, 0x50, 0x0f, 0x86, 0x24, 0x70, 0x8b, 0x9c,
                0x0a, 0x5d, 0x1e, 0x51, 0x40, 0xd3, 0x48, 0x83, 0x30, 0x3d, 0x0c, 0x0b,
            ]
        );
        assert_eq!(
            pf.serialize(),
            [
                0x5a, 0x53, 0x50, 0x50, 0x01, 0x01, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08,
                0xf5, 0xfe, 0xbd, 0x50, 0x0f, 0x86, 0x24, 0x70, 0x8b, 0x9c, 0x0a, 0x5d, 0x1e, 0x51,
                0x40, 0xd3, 0x48, 0x83, 0x6f, 0x11, 0x38, 0xb7,
            ]
        );
    }

    #[test]
    fn parity_only_repairs_two_axis_model_drift() {
        let authoritative = Concept8D::new(2, 5, 9, 14, 15, 3, 1, 12);
        let inv = invariants();
        let frame = ParityOnlyFrame::create(77, authoritative, &inv);
        let mut predicted = authoritative;
        predicted.operation ^= 0x07;
        predicted.modality ^= 0x03;
        let repaired = frame.repair_prediction(predicted, &inv, 0);
        assert!(repaired.success());
        assert_eq!(repaired.state, authoritative);
    }
}
