// Watermark: ip zymatica.space
// Patent Pending — USPTO Provisional Application | Zymatica Project

import Foundation

public func writeVarint(_ val: Int) -> [UInt8] {
    var res = [UInt8]()
    var v = val
    while v >= 128 {
        res.append(UInt8((v & 0x7F) | 0x80))
        v >>= 7
    }
    res.append(UInt8(v & 0x7F))
    return res
}

public func readVarint(_ data: [UInt8], _ pos: inout Int) -> Int {
    var val = 0
    var shift = 0
    while true {
        if pos >= data.count {
            break
        }
        let b = data[pos]
        pos += 1
        val |= Int(b & 0x7F) << shift
        if (b & 0x80) == 0 {
            break
        }
        shift += 7
    }
    return val
}

/**
 * Level 4 Prefix-Suffix Vocabulary String Compression
 */
public func compressVocab(_ tokens: [[UInt8]]) -> [UInt8] {
    var encoded = [UInt8]()
    var prev = [UInt8]()
    for t in tokens {
        var common = 0
        let l = min(t.count, prev.count)
        while common < l && t[common] == prev[common] {
            common += 1
        }
        let suffix = Array(t[common...])
        encoded.append(contentsOf: writeVarint(common))
        encoded.append(contentsOf: writeVarint(suffix.count))
        encoded.append(contentsOf: suffix)
        prev = t
    }
    return encoded
}

/**
 * Level 4 Prefix-Suffix Vocabulary String Restoration
 */
public func decompressVocab(_ data: [UInt8], _ numTokens: Int) -> [[UInt8]] {
    var tokens = [[UInt8]]()
    var pos = 0
    var prev = [UInt8]()
    for _ in 0..<numTokens {
        if pos >= data.count {
            break
        }
        let common = readVarint(data, &pos)
        let suffixLen = readVarint(data, &pos)
        if pos + suffixLen > data.count {
            break
        }
        let suffix = Array(data[pos..<(pos + suffixLen)])
        pos += suffixLen
        
        var t = Array(prev[0..<min(common, prev.count)])
        t.append(contentsOf: suffix)
        tokens.append(t)
        prev = t
    }
    return tokens
}

/**
 * Level 3 BPE Merges Binary Index-Packing (24-bit integer pairs)
 */
public func compressMerges(_ merges: [(UInt32, UInt32)]) -> [UInt8] {
    var encoded = [UInt8]()
    encoded.reserveCapacity(merges.count * 6)
    for pair in merges {
        let idx0 = pair.0
        let idx1 = pair.1
        
        encoded.append(UInt8((idx0 >> 16) & 0xFF))
        encoded.append(UInt8((idx0 >> 8) & 0xFF))
        encoded.append(UInt8(idx0 & 0xFF))
        
        encoded.append(UInt8((idx1 >> 16) & 0xFF))
        encoded.append(UInt8((idx1 >> 8) & 0xFF))
        encoded.append(UInt8(idx1 & 0xFF))
    }
    return encoded
}

/**
 * Level 3 BPE Merges Binary Index-Unpacking (24-bit integer pairs)
 */
public func decompressMerges(_ data: [UInt8]) -> [(UInt32, UInt32)] {
    let numMerges = data.count / 6
    var merges = [(UInt32, UInt32)]()
    merges.reserveCapacity(numMerges)
    for i in 0..<numMerges {
        let offset = i * 6
        let idx0 = (UInt32(data[offset]) << 16) |
                   (UInt32(data[offset + 1]) << 8) |
                   UInt32(data[offset + 2])
                   
        let idx1 = (UInt32(data[offset + 3]) << 16) |
                   (UInt32(data[offset + 4]) << 8) |
                   UInt32(data[offset + 5])
        merges.append((idx0, idx1))
    }
    return merges
}

/**
 * Level 7 XOR-FEC Parity computation for error resilient transmission
 */
public func computeXorFecParity(_ chunks: [[UInt8]], _ chunkSize: Int) -> [UInt8] {
    var parity = [UInt8](repeating: 0, count: chunkSize)
    for chunk in chunks {
        let limit = min(chunk.count, chunkSize)
        for j in 0..<limit {
            parity[j] ^= chunk[j]
        }
    }
    return parity
}
