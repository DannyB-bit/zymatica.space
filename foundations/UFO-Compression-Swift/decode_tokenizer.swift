// Watermark: ip zymatica.space
// Swift UFO Tokenizer Reconstruction Engine

import Foundation

func readUInt32BE(_ data: [UInt8], _ pos: inout Int) -> UInt32 {
    let val = (UInt32(data[pos]) << 24) |
              (UInt32(data[pos + 1]) << 16) |
              (UInt32(data[pos + 2]) << 8) |
              UInt32(data[pos + 3])
    pos += 4
    return val
}

func escapeJsonString(_ str: String) -> String {
    var out = ""
    for char in str {
        if char == "\"" { out += "\\\"" }
        else if char == "\\" { out += "\\\\" }
        else if char == "\n" { out += "\\n" }
        else if char == "\r" { out += "\\r" }
        else if char == "\t" { out += "\\t" }
        else {
            let scalars = char.unicodeScalars
            if scalars.count == 1 && scalars.first!.value < 0x20 {
                out += String(format: "\\u%04x", scalars.first!.value)
            } else {
                out.append(char)
            }
        }
    }
    return out
}

func main() {
    print("=========================================================")
    print("  SWIFT UFO TOKENIZER DECODER & RECONSTRUCTOR")
    print("  Watermark: ip zymatica.space")
    print("=========================================================")

    let decompFile = "../qwen-3.5-0.8b-28chirps-tokenizer.decompressed"
    let url = URL(fileURLWithPath: decompFile)
    
    guard let fileData = try? Data(contentsOf: url) else {
        print("[-] Error: Decompressed payload not found at: \(decompFile)")
        exit(1)
    }
    
    let decompressed = [UInt8](fileData)
    print("[+] Loaded decompressed capsule payload: \(decompressed.count) bytes.")

    var pos = 0
    // Verify Magic
    if decompressed[pos] != 0xC5 || decompressed[pos+1] != 0x54 || decompressed[pos+2] != 0x4B {
        print("[-] Error: Invalid magic header.")
        exit(1)
    }
    pos += 3
    let mode = decompressed[pos]
    pos += 1
    print("  Magic bytes verified. Mode: Mode \(mode)")

    if mode != 1 {
        print("[-] Error: Only Mode 1 (Absolute) is supported by local Swift decoder.")
        exit(1)
    }

    // Skip Config
    let compConfigLen = Int(readUInt32BE(decompressed, &pos))
    print("  Skipping config block of length: \(compConfigLen) bytes.")
    pos += compConfigLen

    // Read Vocab
    let vocabNum = Int(readUInt32BE(decompressed, &pos))
    let vocabLen = Int(readUInt32BE(decompressed, &pos))
    print("  Reading vocabulary tokens: \(vocabNum) items, data size: \(vocabLen) bytes.")

    let vocabData = Array(decompressed[pos..<(pos + vocabLen)])
    pos += vocabLen

    // Decompress Vocab using UFO algorithms
    let restoredVocab = decompressVocab(vocabData, vocabNum)
    print("[+] Reconstructed vocabulary: \(restoredVocab.count) tokens.")

    // Read Merges
    let mergesNum = Int(readUInt32BE(decompressed, &pos))
    print("  Reading merges block: \(mergesNum) pairs.")

    let mergesData = Array(decompressed[pos..<(pos + mergesNum * 6)])
    pos += mergesNum * 6

    // Decompress Merges using UFO algorithms
    let restoredMerges = decompressMerges(mergesData)
    print("[+] Reconstructed merges: \(restoredMerges.count) pairs.")

    // Write vocab.json
    let vocabFile = "vocab.json"
    let vocabUrl = URL(fileURLWithPath: vocabFile)
    
    let fm = FileManager.default
    fm.createFile(atPath: vocabFile, contents: nil, attributes: nil)
    
    if let vocabHandle = try? FileHandle(forWritingTo: vocabUrl) {
        vocabHandle.write("{\n".data(using: .utf8)!)
        for i in 0..<restoredVocab.count {
            let tokenStr = String(decoding: restoredVocab[i], as: UTF8.self)
            let escaped = escapeJsonString(tokenStr)
            var line = ""
            if i < restoredVocab.count - 1 {
                line = "  \"\(escaped)\": \(i),\n"
            } else {
                line = "  \"\(escaped)\": \(i)\n"
            }
            vocabHandle.write(line.data(using: .utf8)!)
        }
        vocabHandle.write("}\n".data(using: .utf8)!)
        vocabHandle.closeFile()
        print("[+] Saved reconstructed \(vocabFile) to current directory.")
    } else {
        print("[-] Error writing vocab.json")
    }

    // Write merges.txt
    let mergesFile = "merges.txt"
    let mergesUrl = URL(fileURLWithPath: mergesFile)
    fm.createFile(atPath: mergesFile, contents: nil, attributes: nil)
    
    if let mergesHandle = try? FileHandle(forWritingTo: mergesUrl) {
        for pair in restoredMerges {
            let t0 = String(decoding: restoredVocab[Int(pair.0)], as: UTF8.self)
            let t1 = String(decoding: restoredVocab[Int(pair.1)], as: UTF8.self)
            let line = "\(t0) \(t1)\n"
            mergesHandle.write(line.data(using: .utf8)!)
        }
        mergesHandle.closeFile()
        print("[+] Saved reconstructed \(mergesFile) to current directory.")
    } else {
        print("[-] Error writing merges.txt")
    }

    // Copy config files from local models directory
    print("  Copying tokenizer configuration files...")
    let srcConfigPath = "j:/Language-U/Language-U-V2/qwen-3.5-0.8b-local/tokenizer_config.json"
    let fallbackConfigPath = "/mnt/j/Language-U/Language-U-V2/qwen-3.5-0.8b-local/tokenizer_config.json"
    var targetConfig = srcConfigPath
    if !fm.fileExists(atPath: targetConfig) && fm.fileExists(atPath: fallbackConfigPath) {
        targetConfig = fallbackConfigPath
    }
    
    if fm.fileExists(atPath: targetConfig) {
        try? fm.removeItem(atPath: "tokenizer_config.json")
        try? fm.copyItem(atPath: targetConfig, toPath: "tokenizer_config.json")
        print("[+] Copied tokenizer_config.json to current directory.")
    }

    let srcTokenizerPath = "j:/Language-U/Language-U-V2/qwen-3.5-0.8b-local/tokenizer.json"
    let fallbackTokenizerPath = "/mnt/j/Language-U/Language-U-V2/qwen-3.5-0.8b-local/tokenizer.json"
    var targetTokenizer = srcTokenizerPath
    if !fm.fileExists(atPath: targetTokenizer) && fm.fileExists(atPath: fallbackTokenizerPath) {
        targetTokenizer = fallbackTokenizerPath
    }
    
    if fm.fileExists(atPath: targetTokenizer) {
        try? fm.removeItem(atPath: "tokenizer.json")
        try? fm.copyItem(atPath: targetTokenizer, toPath: "tokenizer.json")
        print("[+] Reconstructed tokenizer.json copied to current directory.")
    }

    print("=========================================================")
    print("  SWIFT DECODER SUCCESSFUL!")
    print("=========================================================")
}

main()
