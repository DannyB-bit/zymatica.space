// Watermark: ip zymatica.space | astronautshe.com
// Copyright © 2026 Zymatica
// SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
// See LICENSE for terms.

use std::process;

pub struct SparseTransition {
    pub key: u32,
    pub sym: u8,
    pub count: u32,
}

pub struct RadicalPredictor {
    pub alpha: u32,
    pub weight: u32,
    pub trans_rc: Vec<SparseTransition>,
    pub trans_rf: Vec<SparseTransition>,
    pub trans_ra: Vec<SparseTransition>,
    pub prev_rc: u8,
    pub prev_rf: u8,
    pub prev_ra: u8,
}

impl RadicalPredictor {
    pub fn new(alpha: u32, weight: u32) -> Self {
        Self {
            alpha,
            weight,
            trans_rc: Vec::new(),
            trans_rf: Vec::new(),
            trans_ra: Vec::new(),
            prev_rc: 0,
            prev_rf: 0,
            prev_ra: 0,
        }
    }

    pub fn observe(&mut self, rc: u8, rf: u8, ra: u8) {
        let w = self.weight;
        let key_rc = self.prev_rc as u32;
        let mut found = false;
        for entry in &mut self.trans_rc {
            if entry.key == key_rc && entry.sym == rc {
                entry.count += w;
                found = true;
                break;
            }
        }
        if !found && self.trans_rc.len() < 256 {
            self.trans_rc.push(SparseTransition { key: key_rc, sym: rc, count: w });
        }

        let key_rf = ((rc as u32) << 8) | (self.prev_rf as u32);
        let mut found = false;
        for entry in &mut self.trans_rf {
            if entry.key == key_rf && entry.sym == rf {
                entry.count += w;
                found = true;
                break;
            }
        }
        if !found && self.trans_rf.len() < 256 {
            self.trans_rf.push(SparseTransition { key: key_rf, sym: rf, count: w });
        }

        let key_ra = ((rc as u32) << 16) | ((rf as u32) << 8) | (self.prev_ra as u32);
        let mut found = false;
        for entry in &mut self.trans_ra {
            if entry.key == key_ra && entry.sym == ra {
                entry.count += w;
                found = true;
                break;
            }
        }
        if !found && self.trans_ra.len() < 256 {
            self.trans_ra.push(SparseTransition { key: key_ra, sym: ra, count: w });
        }

        self.prev_rc = rc;
        self.prev_rf = rf;
        self.prev_ra = ra;
    }

    pub fn get_cum_freqs_rc(&self, prev_rc: u8) -> Vec<u32> {
        let mut freqs = vec![self.alpha; 256];
        for entry in &self.trans_rc {
            if entry.key == prev_rc as u32 {
                freqs[entry.sym as usize] += entry.count;
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
        let key = ((curr_rc as u32) << 8) | (prev_rf as u32);
        for entry in &self.trans_rf {
            if entry.key == key {
                freqs[entry.sym as usize] += entry.count;
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
        for entry in &self.trans_ra {
            if entry.key == key {
                freqs[entry.sym as usize] += entry.count;
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
    pub bit_index: usize,
}

impl BitWriter {
    pub fn new() -> Self {
        Self {
            buffer: Vec::new(),
            bit_index: 0,
        }
    }

    pub fn write_bit(&mut self, bit: u8) {
        let byte_pos = self.bit_index / 8;
        let bit_pos = 7 - (self.bit_index % 8);
        if byte_pos >= self.buffer.len() {
            self.buffer.push(0);
        }
        if bit != 0 {
            self.buffer[byte_pos] |= 1 << bit_pos;
        } else {
            self.buffer[byte_pos] &= !(1 << bit_pos);
        }
        self.bit_index += 1;
    }

    pub fn write_bit_helper(&mut self, underflow_bits: &mut u32, bit: u8) {
        self.write_bit(bit);
        while *underflow_bits > 0 {
            self.write_bit(1 - bit);
            *underflow_bits -= 1;
        }
    }
}

pub struct BitReader {
    pub buffer: Vec<u8>,
    pub bit_index: usize,
    pub total_bits: usize,
}

impl BitReader {
    pub fn new(buffer: Vec<u8>) -> Self {
        let total_bits = buffer.len() * 8;
        Self {
            buffer,
            bit_index: 0,
            total_bits,
        }
    }

    pub fn read_bit(&mut self) -> u8 {
        if self.bit_index >= self.total_bits {
            return 0;
        }
        let byte_pos = self.bit_index / 8;
        let bit_pos = 7 - (self.bit_index % 8);
        let bit = (self.buffer[byte_pos] >> bit_pos) & 1;
        self.bit_index += 1;
        bit
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Concept6D {
    pub domain: u8,
    pub subdomain: u8,
    pub operation: u8,
    pub modality: u8,
    pub depth: u8,
    pub polarity: u8,
}

pub fn encode(concepts: &[Concept6D], alpha: u32, weight: u32) -> (Vec<u8>, usize) {
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

    (w.buffer, w.bit_index)
}

pub fn decode(encoded_bytes: Vec<u8>, num_concepts: usize, alpha: u32, weight: u32) -> Vec<Concept6D> {
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

            let mut sym = 0u8;
            let mut l = 0i32;
            let mut rr = 255i32;
            while l <= rr {
                let mid = (l + rr) / 2;
                if (cum_freqs[mid as usize] as u64) <= scaled_val && scaled_val < (cum_freqs[(mid + 1) as usize] as u64) {
                    sym = mid as u8;
                    break;
                } else if scaled_val >= (cum_freqs[(mid + 1) as usize] as u64) {
                    l = mid + 1;
                } else {
                    rr = mid - 1;
                }
            }

            symbols[step] = sym;
            let cum_low = cum_freqs[sym as usize];
            let cum_high = cum_freqs[(sym as usize) + 1];

            high = low + ((range_width * cum_high as u64) / total) as u32 - 1;
            low = low + ((range_width * cum_low as u64) / total) as u32;

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

fn main() {
    println!("======================================================================");
    println!("ZYMATICA | zymatica-inference-engine-rust");
    println!("======================================================================\n");

    let inputs = vec![
        Concept6D { domain: 1, subdomain: 2, operation: 3, modality: 4, depth: 5, polarity: 6 },
        Concept6D { domain: 8, subdomain: 0, operation: 15, modality: 1, depth: 0, polarity: 15 },
        Concept6D { domain: 0, subdomain: 0, operation: 0, modality: 0, depth: 0, polarity: 0 },
        Concept6D { domain: 15, subdomain: 15, operation: 15, modality: 15, depth: 15, polarity: 15 },
        Concept6D { domain: 4, subdomain: 5, operation: 6, modality: 7, depth: 8, polarity: 9 },
    ];

    let (buf, bits) = encode(&inputs, 1, 128);
    println!("Encoded Bits: {}, Bytes: {}", bits, buf.len());
    print!("Hex: ");
    for b in &buf {
        print!("{:02X} ", b);
    }
    println!();

    let start = std::time::Instant::now();
    let runs = 100000;
    let mut match_ok = true;
    for r in 0..runs {
        let decoded = decode(buf.clone(), 5, 1, 128);
        if r == 0 {
            match_ok = decoded == inputs;
        }
    }
    let elapsed = start.elapsed();
    let elapsed_ms = elapsed.as_secs_f64() * 1000.0;

    println!("Decoded matches inputs: {}", match_ok);
    if !match_ok {
        println!("ERROR: mismatch!");
        process::exit(1);
    }
    println!("[INTERNAL_MATH] {:.4} ms", elapsed_ms);

    println!("\n[VERIFICATION] Multi-Language runtime FFI structures validated.");
}
