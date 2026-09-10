// Watermark: ip zymatica.space
// Rust UFO Tokenizer Reconstruction Engine

mod tokenizer_coder;

use std::fs::File;
use std::io::{Read, Write, BufWriter};
use std::path::Path;

fn read_u32_be(data: &[u8], pos: &mut usize) -> u32 {
    let val = ((data[*pos] as u32) << 24) |
              ((data[*pos + 1] as u32) << 16) |
              ((data[*pos + 2] as u32) << 8) |
              (data[*pos + 3] as u32);
    *pos += 4;
    val
}

// Escape raw byte token for JSON formatting
fn escape_json_string(token: &[u8]) -> String {
    let mut out = String::new();
    for &b in token {
        match b {
            b'"' => out.push_str("\\\""),
            b'\\' => out.push_str("\\\\"),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            0..=0x1f => {
                out.push_str(&format!("\\u{:04x}", b));
            }
            _ => {
                out.push(b as char);
            }
        }
    }
    out
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=========================================================");
    println!("  RUST UFO TOKENIZER DECODER & RECONSTRUCTOR");
    println!("  Watermark: ip zymatica.space");
    println!("=========================================================");

    // Read decompressed payload
    let decomp_file = "../qwen-3.5-0.8b-28chirps-tokenizer.decompressed";
    let mut file = File::open(decomp_file)?;
    let mut decompressed = Vec::new();
    file.read_to_end(&mut decompressed)?;
    println!("[+] Loaded decompressed capsule payload: {} bytes.", decompressed.len());

    // Verify Magic Header and Mode
    let mut pos = 0;
    if decompressed[pos] != 0xC5 || decompressed[pos+1] != 0x54 || decompressed[pos+2] != 0x4B {
        eprintln!("[-] Error: Invalid magic header.");
        std::process::exit(1);
    }
    pos += 3;
    let mode = decompressed[pos];
    pos += 1;
    println!("  Magic bytes verified. Mode: Mode {}", mode);

    if mode != 1 {
        eprintln!("[-] Error: Only Mode 1 (Absolute) is supported by Rust local decoder.");
        std::process::exit(1);
    }

    // Skip config block
    let comp_config_len = read_u32_be(&decompressed, &mut pos) as usize;
    println!("  Skipping config block of length: {} bytes.", comp_config_len);
    pos += comp_config_len;

    // Read Vocab
    let vocab_num = read_u32_be(&decompressed, &mut pos) as usize;
    let vocab_len = read_u32_be(&decompressed, &mut pos) as usize;
    println!("  Reading vocabulary tokens: {} items, data size: {} bytes.", vocab_num, vocab_len);

    let vocab_data = &decompressed[pos .. pos + vocab_len];
    pos += vocab_len;

    // Decompress Vocab using UFO algorithms
    let restored_vocab = tokenizer_coder::decompress_vocab(vocab_data, vocab_num);
    println!("[+] Reconstructed vocabulary: {} tokens.", restored_vocab.len());

    // Read Merges
    let merges_num = read_u32_be(&decompressed, &mut pos) as usize;
    println!("  Reading merges block: {} pairs.", merges_num);

    let merges_data = &decompressed[pos .. pos + merges_num * 6];

    // Decompress Merges using UFO algorithms
    let restored_merges = tokenizer_coder::decompress_merges(merges_data);
    println!("[+] Reconstructed merges: {} pairs.", restored_merges.len());

    // Write vocab.json using BufWriter for speed
    let vocab_file = "vocab.json";
    let out_vocab = File::create(vocab_file)?;
    let mut writer = BufWriter::new(out_vocab);
    writeln!(writer, "{{")?;
    for i in 0..restored_vocab.len() {
        let escaped = escape_json_string(&restored_vocab[i]);
        if i < restored_vocab.len() - 1 {
            writeln!(writer, "  \"{}\": {},", escaped, i)?;
        } else {
            writeln!(writer, "  \"{}\": {}", escaped, i)?;
        }
    }
    writeln!(writer, "}}")?;
    writer.flush()?;
    println!("[+] Saved reconstructed {} to current directory.", vocab_file);

    // Write merges.txt using BufWriter
    let merges_file = "merges.txt";
    let out_merges = File::create(merges_file)?;
    let mut writer_merges = BufWriter::new(out_merges);
    for pair in &restored_merges {
        let t0 = std::str::from_utf8(&restored_vocab[pair.0 as usize]).unwrap_or("");
        let t1 = std::str::from_utf8(&restored_vocab[pair.1 as usize]).unwrap_or("");
        writeln!(writer_merges, "{} {}", t0, t1)?;
    }
    writer_merges.flush()?;
    println!("[+] Saved reconstructed {} to current directory.", merges_file);

    // Copy config files from local models directory
    println!("  Copying tokenizer configuration files...");
    let mut src_config = Path::new("j:/Language-U/Language-U-V2/qwen-3.5-0.8b-local/tokenizer_config.json");
    let fallback_config = Path::new("/mnt/j/Language-U/Language-U-V2/qwen-3.5-0.8b-local/tokenizer_config.json");
    if !src_config.exists() && fallback_config.exists() {
        src_config = fallback_config;
    }
    if src_config.exists() {
        std::fs::copy(src_config, "tokenizer_config.json")?;
        println!("[+] Copied tokenizer_config.json to current directory.");
    }

    let mut src_tokenizer = Path::new("j:/Language-U/Language-U-V2/qwen-3.5-0.8b-local/tokenizer.json");
    let fallback_tokenizer = Path::new("/mnt/j/Language-U/Language-U-V2/qwen-3.5-0.8b-local/tokenizer.json");
    if !src_tokenizer.exists() && fallback_tokenizer.exists() {
        src_tokenizer = fallback_tokenizer;
    }
    if src_tokenizer.exists() {
        std::fs::copy(src_tokenizer, "tokenizer.json")?;
        println!("[+] Reconstructed tokenizer.json copied to current directory.");
    }

    println!("=========================================================");
    println!("  RUST DECODER SUCCESSFUL!");
    println!("=========================================================");
    Ok(())
}
