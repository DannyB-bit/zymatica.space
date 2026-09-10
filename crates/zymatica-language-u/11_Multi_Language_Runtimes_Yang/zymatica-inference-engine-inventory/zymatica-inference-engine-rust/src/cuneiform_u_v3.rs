// Watermark: ip zymatica.space | astronautshe.com
// Copyright © 2026 Zymatica
// SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
// See LICENSE for terms.

use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Concept6D {
    pub domain: u8,
    pub subdomain: u8,
    pub operation: u8,
    pub modality: u8,
    pub depth: u8,
    pub polarity: u8,
}

pub struct RadicalPredictor {
    pub alpha: u32,
    pub weight: u32,
    pub trans_rc: HashMap<u8, HashMap<u8, u32>>,
    pub trans_rf: HashMap<u16, HashMap<u8, u32>>,
    pub trans_ra: HashMap<u32, HashMap<u8, u32>>,
    pub prev_rc: u8,
    pub prev_rf: u8,
    pub prev_ra: u8,
}

impl RadicalPredictor {
    pub fn new(alpha: u32, weight: u32) -> Self {
        Self {
            alpha,
            weight,
            trans_rc: HashMap::new(),
            trans_rf: HashMap::new(),
            trans_ra: HashMap::new(),
            prev_rc: 0,
            prev_rf: 0,
            prev_ra: 0,
        }
    }

    pub fn observe(&mut self, rc: u8, rf: u8, ra: u8) {
        let w = self.weight;
        let key_rc = self.prev_rc;
        self.trans_rc
            .entry(key_rc)
            .or_insert_with(HashMap::new)
            .entry(rc)
            .and_modify(|c| *c += w)
            .or_insert(w);

        let key_rf = ((rc as u16) << 8) | (self.prev_rf as u16);
        self.trans_rf
            .entry(key_rf)
            .or_insert_with(HashMap::new)
            .entry(rf)
            .and_modify(|c| *c += w)
            .or_insert(w);

        let key_ra = ((rc as u32) << 16) | ((rf as u32) << 8) | (self.prev_ra as u32);
        self.trans_ra
            .entry(key_ra)
            .or_insert_with(HashMap::new)
            .entry(ra)
            .and_modify(|c| *c += w)
            .or_insert(w);

        self.prev_rc = rc;
        self.prev_rf = rf;
        self.prev_ra = ra;
    }

    pub fn get_cum_freqs_rc(&self, prev_rc: u8) -> Vec<u32> {
        let mut freqs = vec![self.alpha; 256];
        if let Some(map) = self.trans_rc.get(&prev_rc) {
            for (&sym, &count) in map {
                freqs[sym as usize] += count;
            }
        }
        let mut cum_freqs = vec![0; 257];
        for i in 0..256 {
            cum_freqs[i + 1] = cum_freqs[i] + freqs[i];
        }
        cum_freqs
    }

    pub fn get_cum_freqs_rf(&self, curr_rc: u8, prev_rf: u8) -> Vec<u32> {
        let mut freqs = vec![self.alpha; 256];
        let key = ((curr_rc as u16) << 8) | (prev_rf as u16);
        if let Some(map) = self.trans_rf.get(&key) {
            for (&sym, &count) in map {
                freqs[sym as usize] += count;
            }
        }
        let mut cum_freqs = vec![0; 257];
        for i in 0..256 {
            cum_freqs[i + 1] = cum_freqs[i] + freqs[i];
        }
        cum_freqs
    }

    pub fn get_cum_freqs_ra(&self, curr_rc: u8, curr_rf: u8, prev_ra: u8) -> Vec<u32> {
        let mut freqs = vec![self.alpha; 256];
        let key = ((curr_rc as u32) << 16) | ((curr_rf as u32) << 8) | (prev_ra as u32);
        if let Some(map) = self.trans_ra.get(&key) {
            for (&sym, &count) in map {
                freqs[sym as usize] += count;
            }
        }
        let mut cum_freqs = vec![0; 257];
        for i in 0..256 {
            cum_freqs[i + 1] = cum_freqs[i] + freqs[i];
        }
        cum_freqs
    }
}

pub struct BitWriter {
    pub buffer: Vec<u8>,
    pub current_byte: u8,
    pub bit_count: usize,
}

impl BitWriter {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            current_byte: 0,
            bit_count: 0,
        }
    }

    pub fn write_bit(&mut self, bit: u8) {
        self.current_byte = (self.current_byte << 1) | (bit & 1);
        self.bit_count += 1;
        if self.bit_count % 8 == 0 {
            self.buffer.push(self.current_byte);
            self.current_byte = 0;
        }
    }

    pub fn write_bit_helper(&mut self, underflow_bits: &mut u32, bit: u8) {
        self.write_bit(bit);
        while *underflow_bits > 0 {
            self.write_bit(1 - bit);
            *underflow_bits -= 1;
        }
    }

    pub fn flush(&mut self) -> Vec<u8> {
        if self.bit_count % 8 != 0 {
            let padding_bits = 8 - (self.bit_count % 8);
            self.current_byte <<= padding_bits;
            self.buffer.push(self.current_byte);
            self.current_byte = 0;
            self.bit_count += padding_bits;
        }
        self.buffer.clone()
    }
}

pub struct BitReader {
    pub data: Vec<u8>,
    pub byte_index: usize,
    pub bit_index: usize,
    pub total_bits: usize,
}

impl BitReader {
    pub fn new(data: Vec<u8>) -> Self {
        let total_bits = data.len() * 8;
        Self {
            data,
            byte_index: 0,
            bit_index: 0,
            total_bits,
        }
    }

    pub fn read_bit(&mut self) -> u8 {
        if self.byte_index >= self.data.len() {
            return 0;
        }
        let bit = (self.data[self.byte_index] >> (7 - self.bit_index)) & 1;
        self.bit_index += 1;
        if self.bit_index == 8 {
            self.bit_index = 0;
            self.byte_index += 1;
        }
        bit
    }
}

pub fn cuneiform_u_v3_encode(
    concepts: &[Concept6D],
    alpha: u32,
    weight: u32,
) -> Vec<u8> {
    let mut pred = RadicalPredictor::new(alpha, weight);
    let mut w = BitWriter::new();

    let mut low: u32 = 0;
    let mut high: u32 = 0xFFFFFFFF;
    let mut underflow_bits: u32 = 0;

    for c in concepts {
        let rc = (c.domain << 4) | c.subdomain;
        let rf = (c.operation << 4) | c.modality;
        let ra = (c.depth << 4) | c.polarity;

        let symbols = [rc, rf, ra];
        let prev_rc = pred.prev_rc;
        let prev_rf = pred.prev_rf;
        let prev_ra = pred.prev_ra;

        for step in 0..3 {
            let cum_freqs = match step {
                0 => pred.get_cum_freqs_rc(prev_rc),
                1 => pred.get_cum_freqs_rf(symbols[0], prev_rf),
                _ => pred.get_cum_freqs_ra(symbols[0], symbols[1], prev_ra),
            };

            let sym = symbols[step] as usize;
            let total = cum_freqs[256];
            let cum_low = cum_freqs[sym];
            let cum_high = cum_freqs[sym + 1];

            let range_width = (high as u64) - (low as u64) + 1;
            high = low + ((range_width * cum_high as u64) / total as u64) as u32 - 1;
            low = low + ((range_width * cum_low as u64) / total as u64) as u32;

            // Renormalize
            loop {
                if high < 0x80000000 {
                    w.write_bit_helper(&mut underflow_bits, 0);
                    low <<= 1;
                    high = (high << 1) | 1;
                } else if low >= 0x80000000 {
                    w.write_bit_helper(&mut underflow_bits, 1);
                    low = (low - 0x80000000) << 1;
                    high = ((high - 0x80000000) << 1) | 1;
                } else if low >= 0x40000000 && high < 0xC0000000 {
                    underflow_bits += 1;
                    low = (low - 0x40000000) << 1;
                    high = ((high - 0x40000000) << 1) | 1;
                } else {
                    break;
                }
            }
        }
        pred.observe(rc, rf, ra);
    }

    underflow_bits += 1;
    if low < 0x40000000 {
        w.write_bit_helper(&mut underflow_bits, 0);
    } else {
        w.write_bit_helper(&mut underflow_bits, 1);
    }

    w.flush()
}

pub fn cuneiform_u_v3_decode(
    encoded_bytes: Vec<u8>,
    num_concepts: usize,
    alpha: u32,
    weight: u32,
) -> Vec<Concept6D> {
    let mut pred = RadicalPredictor::new(alpha, weight);
    let mut r = BitReader::new(encoded_bytes);

    let mut value: u32 = 0;
    for _ in 0..32 {
        value = (value << 1) | (r.read_bit() as u32);
    }

    let mut low: u32 = 0;
    let mut high: u32 = 0xFFFFFFFF;
    let mut decoded = Vec::with_capacity(num_concepts);

    for _ in 0..num_concepts {
        let prev_rc = pred.prev_rc;
        let prev_rf = pred.prev_rf;
        let prev_ra = pred.prev_ra;

        let mut symbols = [0u8; 3];

        for step in 0..3 {
            let cum_freqs = match step {
                0 => pred.get_cum_freqs_rc(prev_rc),
                1 => pred.get_cum_freqs_rf(symbols[0], prev_rf),
                _ => pred.get_cum_freqs_ra(symbols[0], symbols[1], prev_ra),
            };

            let total = cum_freqs[256] as u64;
            let range_width = (high as u64) - (low as u64) + 1;
            let scaled_val = (((value as u64 - low as u64) + 1) * total - 1) / range_width;

            // Binary search
            let mut sym = 0u8;
            let mut l = 0i32;
            let mut rr = 255i32;
            while l <= rr {
                let mid = (l + rr) / 2;
                let cum_mid = cum_freqs[mid as usize] as u64;
                let cum_mid_next = cum_freqs[(mid + 1) as usize] as u64;
                if cum_mid <= scaled_val && scaled_val < cum_mid_next {
                    sym = mid as u8;
                    break;
                } else if scaled_val >= cum_mid_next {
                    l = mid + 1;
                } else {
                    rr = mid - 1;
                }
            }

            symbols[step] = sym;

            let cum_low = cum_freqs[sym as usize];
            let cum_high = cum_freqs[(sym + 1) as usize];

            high = low + ((range_width * cum_high as u64) / total) as u32 - 1;
            low = low + ((range_width * cum_low as u64) / total) as u32;

            // Renormalize
            loop {
                if high < 0x80000000 {
                    low <<= 1;
                    high = (high << 1) | 1;
                    value = (value << 1) | (r.read_bit() as u32);
                } else if low >= 0x80000000 {
                    low = (low - 0x80000000) << 1;
                    high = ((high - 0x80000000) << 1) | 1;
                    value = ((value - 0x80000000) << 1) | (r.read_bit() as u32);
                } else if low >= 0x40000000 && high < 0xC0000000 {
                    low = (low - 0x40000000) << 1;
                    high = ((high - 0x40000000) << 1) | 1;
                    value = ((value - 0x40000000) << 1) | (r.read_bit() as u32);
                } else {
                    break;
                }
            }
        }

        decoded.push(Concept6D {
            domain: symbols[0] >> 4,
            subdomain: symbols[0] & 0x0F,
            operation: symbols[1] >> 4,
            modality: symbols[1] & 0x0F,
            depth: symbols[2] >> 4,
            polarity: symbols[2] & 0x0F,
        });

        pred.observe(symbols[0], symbols[1], symbols[2]);
    }

    decoded
}
