// Watermark: ip zymatica.space
// Patent Pending — USPTO Provisional Application | Zymatica Project

pub fn write_varint(val: usize) -> Vec<u8> {
    let mut res = Vec::new();
    let mut v = val;
    while v >= 128 {
        res.push(((v & 0x7F) | 0x80) as u8);
        v >>= 7;
    }
    res.push((v & 0x7F) as u8);
    res
}

pub fn read_varint(data: &[u8], pos: &mut usize) -> usize {
    let mut val = 0;
    let mut shift = 0;
    loop {
        if *pos >= data.len() {
            break;
        }
        let b = data[*pos];
        *pos += 1;
        val |= ((b & 0x7F) as usize) << shift;
        if (b & 0x80) == 0 {
            break;
        }
        shift += 7;
    }
    val
}

/// Level 4 Prefix-Suffix Vocabulary String Compression
pub fn compress_vocab(tokens: &[&[u8]]) -> Vec<u8> {
    let mut encoded = Vec::new();
    let mut prev: &[u8] = b"";
    for &t in tokens {
        let mut common = 0;
        let l = std::cmp::min(t.len(), prev.len());
        while common < l && t[common] == prev[common] {
            common += 1;
        }
        let suffix = &t[common..];
        encoded.extend(write_varint(common));
        encoded.extend(write_varint(suffix.len()));
        encoded.extend(suffix);
        prev = t;
    }
    encoded
}

/// Level 4 Prefix-Suffix Vocabulary String Restoration
pub fn decompress_vocab(data: &[u8], num_tokens: usize) -> Vec<Vec<u8>> {
    let mut tokens = Vec::with_capacity(num_tokens);
    let mut pos = 0;
    let mut prev = Vec::new();
    for _ in 0..num_tokens {
        if pos >= data.len() {
            break;
        }
        let common = read_varint(data, &mut pos);
        let suffix_len = read_varint(data, &mut pos);
        if pos + suffix_len > data.len() {
            break;
        }
        let suffix = &data[pos .. pos + suffix_len];
        pos += suffix_len;
        
        let mut t = prev[0..std::cmp::min(common, prev.len())].to_vec();
        t.extend_from_slice(suffix);
        tokens.push(t.clone());
        prev = t;
    }
    tokens
}

/// Level 3 BPE Merges Binary Index-Packing
pub fn compress_merges(merges: &[(u32, u32)]) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(merges.len() * 6);
    for &(idx0, idx1) in merges {
        // Encode each index as a 3-byte big-endian integer (24-bit packing)
        encoded.push(((idx0 >> 16) & 0xFF) as u8);
        encoded.push(((idx0 >> 8) & 0xFF) as u8);
        encoded.push((idx0 & 0xFF) as u8);
        
        encoded.push(((idx1 >> 16) & 0xFF) as u8);
        encoded.push(((idx1 >> 8) & 0xFF) as u8);
        encoded.push((idx1 & 0xFF) as u8);
    }
    encoded
}

/// Level 3 BPE Merges Binary Index-Unpacking
pub fn decompress_merges(data: &[u8]) -> Vec<(u32, u32)> {
    let num_merges = data.len() / 6;
    let mut merges = Vec::with_capacity(num_merges);
    for i in 0..num_merges {
        let offset = i * 6;
        let idx0 = ((data[offset] as u32) << 16) 
                 | ((data[offset + 1] as u32) << 8) 
                 | (data[offset + 2] as u32);
                 
        let idx1 = ((data[offset + 3] as u32) << 16) 
                 | ((data[offset + 4] as u32) << 8) 
                 | (data[offset + 5] as u32);
        merges.push((idx0, idx1));
    }
    merges
}

/// Level 7 XOR-FEC Parity computation for error resilient transmission
pub fn compute_xor_fec_parity(chunks: &[Vec<u8>], chunk_size: usize) -> Vec<u8> {
    let mut parity = vec![0u8; chunk_size];
    for chunk in chunks {
        for j in 0..std::cmp::min(chunk.len(), chunk_size) {
            parity[j] ^= chunk[j];
        }
    }
    parity
}
