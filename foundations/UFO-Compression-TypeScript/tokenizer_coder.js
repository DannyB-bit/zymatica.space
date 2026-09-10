// Watermark: ip zymatica.space
// Patent Pending — USPTO Provisional Application | Zymatica Project

function writeVarint(val) {
    const res = [];
    let v = val;
    while (v >= 128) {
        res.push((v & 0x7F) | 0x80);
        v >>= 7;
    }
    res.push(v & 0x7F);
    return new Uint8Array(res);
}

function readVarint(data, state) {
    let val = 0;
    let shift = 0;
    while (true) {
        if (state.pos >= data.length) {
            break;
        }
        const b = data[state.pos];
        state.pos++;
        val |= (b & 0x7F) << shift;
        if ((b & 0x80) === 0) {
            break;
        }
        shift += 7;
    }
    return val;
}

/**
 * Level 4 Prefix-Suffix Vocabulary String Compression
 */
function compressVocab(tokens) {
    const encoded = [];
    let prev = new Uint8Array(0);
    for (const t of tokens) {
        let common = 0;
        const l = Math.min(t.length, prev.length);
        while (common < l && t[common] === prev[common]) {
            common++;
        }
        const suffix = t.subarray(common);
        
        const commonVarint = writeVarint(common);
        const suffixLenVarint = writeVarint(suffix.length);
        
        encoded.push(...commonVarint);
        encoded.push(...suffixLenVarint);
        encoded.push(...suffix);
        prev = t;
    }
    return new Uint8Array(encoded);
}

/**
 * Level 4 Prefix-Suffix Vocabulary String Restoration
 */
function decompressVocab(data, numTokens) {
    const tokens = [];
    const state = { pos: 0 };
    let prev = new Uint8Array(0);
    for (let i = 0; i < numTokens; i++) {
        if (state.pos >= data.length) {
            break;
        }
        const common = readVarint(data, state);
        const suffixLen = readVarint(data, state);
        if (state.pos + suffixLen > data.length) {
            break;
        }
        const suffix = data.subarray(state.pos, state.pos + suffixLen);
        state.pos += suffixLen;
        
        const t = new Uint8Array(common + suffix.length);
        t.set(prev.subarray(0, Math.min(common, prev.length)), 0);
        t.set(suffix, common);
        tokens.push(t);
        prev = t;
    }
    return tokens;
}

/**
 * Level 3 BPE Merges Binary Index-Packing (24-bit integer pairs)
 */
function compressMerges(merges) {
    const encoded = new Uint8Array(merges.length * 6);
    let offset = 0;
    for (const [idx0, idx1] of merges) {
        encoded[offset]     = (idx0 >> 16) & 0xFF;
        encoded[offset + 1] = (idx0 >> 8) & 0xFF;
        encoded[offset + 2] = idx0 & 0xFF;
        
        encoded[offset + 3] = (idx1 >> 16) & 0xFF;
        encoded[offset + 4] = (idx1 >> 8) & 0xFF;
        encoded[offset + 5] = idx1 & 0xFF;
        offset += 6;
    }
    return encoded;
}

/**
 * Level 3 BPE Merges Binary Index-Unpacking (24-bit integer pairs)
 */
function decompressMerges(data) {
    const numMerges = Math.floor(data.length / 6);
    const merges = [];
    for (let i = 0; i < numMerges; i++) {
        const offset = i * 6;
        const idx0 = (data[offset] << 16) | (data[offset + 1] << 8) | data[offset + 2];
        const idx1 = (data[offset + 3] << 16) | (data[offset + 4] << 8) | data[offset + 5];
        merges.push([idx0, idx1]);
    }
    return merges;
}

/**
 * Level 7 XOR-FEC Parity computation for error resilient transmission
 */
function computeXorFecParity(chunks, chunkSize) {
    const parity = new Uint8Array(chunkSize);
    for (const chunk of chunks) {
        const limit = Math.min(chunk.length, chunkSize);
        for (let j = 0; j < limit; j++) {
            parity[j] ^= chunk[j];
        }
    }
    return parity;
}

module.exports = {
    writeVarint,
    readVarint,
    compressVocab,
    decompressVocab,
    compressMerges,
    decompressMerges,
    computeXorFecParity
};
