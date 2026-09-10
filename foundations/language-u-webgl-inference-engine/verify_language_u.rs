// ZYMATICA | Language-U Cross-Language Verification Engine (Rust)
// Watermark: ip zymatica.space | astronautshe.com

use std::fs::File;
use std::io::Read;
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
pub struct ConceptRadicals {
    pub rc: u8,
    pub rf: u8,
    pub ra: u8,
}

pub fn read_varint(data: &[u8], state: &mut usize) -> usize {
    let mut val = 0;
    let mut shift = 0;
    loop {
        if *state >= data.len() {
            break;
        }
        let b = data[*state] as usize;
        *state += 1;
        val |= (b & 0x7F) << shift;
        if (b & 0x80) == 0 {
            break;
        }
        shift += 7;
    }
    val
}

pub fn decompress_vocab(data: &[u8], num_tokens: usize) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut state = 0;
    let mut prev = String::new();
    for _ in 0..num_tokens {
        if state >= data.len() {
            break;
        }
        let common = read_varint(data, &mut state);
        let suffix_len = read_varint(data, &mut state);
        if state + suffix_len > data.len() {
            break;
        }
        let suffix_bytes = &data[state..state + suffix_len];
        state += suffix_len;
        
        let suffix = String::from_utf8_lossy(suffix_bytes).into_owned();
        let prefix = if common < prev.len() {
            &prev[0..common]
        } else {
            &prev
        };
        let token = format!("{}{}", prefix, suffix);
        tokens.push(token.clone());
        prev = token;
    }
    tokens
}

pub fn decode(encoded_bytes: Vec<u8>, num_concepts: usize, alpha: u32, weight: u32) -> Vec<ConceptRadicals> {
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

            high = low.wrapping_add(((range_width * cum_high as u64) / total) as u32).wrapping_sub(1);
            low = low.wrapping_add(((range_width * cum_low as u64) / total) as u32);

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

        decoded.push(ConceptRadicals {
            rc: symbols[0],
            rf: symbols[1],
            ra: symbols[2],
        });
        pred.observe(symbols[0], symbols[1], symbols[2]);
    }
    decoded
}

fn main() {
    println!("======================================================================");
    println!("ZYMATICA | Cross-Language Rust Decompressor & Range-Decoder");
    println!("======================================================================\n");

    let mut names_file = File::open("Language-U-Browser/frameworks_names.bin")
        .or_else(|_| File::open("frameworks_names.bin"))
        .unwrap_or_else(|_| {
            eprintln!("[!] Error: Binary transport files not found. Run run_ultimate_pipeline.py first.");
            process::exit(1);
        });
    let mut coords_file = File::open("Language-U-Browser/frameworks_coordinates.bin")
        .or_else(|_| File::open("frameworks_coordinates.bin"))
        .unwrap_or_else(|_| {
            eprintln!("[!] Error: Binary transport files not found.");
            process::exit(1);
        });

    let mut names_bytes = Vec::new();
    names_file.read_to_end(&mut names_bytes).unwrap();

    let mut coords_bytes = Vec::new();
    coords_file.read_to_end(&mut coords_bytes).unwrap();

    // 1. Decompress vocab
    let names = decompress_vocab(&names_bytes, 49);
    println!("[1] Rust Vocab Decompression: SUCCESS ({} names restored).", names.len());

    // 2. Formulate expected radicals
    let mut expected = Vec::with_capacity(49);
    for name in &names {
        let mut domain = 1u8;
        let lower = name.to_lowercase();
        if lower.contains("pixi") || lower.contains("phaser") || lower.contains("away") || lower.contains("p5") {
            domain = 2;
        } else if lower.contains("scenejs") || lower.contains("glam") || lower.contains("deck") || lower.contains("cesium") || lower.contains("luma") || lower.contains("philo") {
            domain = 7;
        }
        let rc = (domain << 4) | 2;
        let rf = (1 << 4) | 2;
        let ra = (15 << 4) | 12;
        expected.push(ConceptRadicals { rc, rf, ra });
    }

    // 3. Decode radicals
    let decoded = decode(coords_bytes, 49, 1, 128);
    println!("[2] Rust Yang Range Decoder execution: SUCCESS.");

    // 4. Match check
    let mut match_ok = true;
    for i in 0..49 {
        if expected[i] != decoded[i] {
            eprintln!("[!] Mismatch at index {} ({}): Expected RC={:02X}, RF={:02X}, RA={:02X} | Decoded RC={:02X}, RF={:02X}, RA={:02X}",
                i, names[i], expected[i].rc, expected[i].rf, expected[i].ra, decoded[i].rc, decoded[i].rf, decoded[i].ra);
            match_ok = false;
            break;
        }
    }

    if match_ok {
        println!("\n[SUCCESS] Rust range-decoder verification: 100% MATCH!");
    } else {
        eprintln!("\n[ERROR] Rust dynamic coordinate check failed!");
        process::exit(1);
    }
}
