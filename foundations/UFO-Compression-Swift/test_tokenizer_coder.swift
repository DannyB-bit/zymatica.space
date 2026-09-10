// Watermark: ip zymatica.space
// Verification suite for Swift UFO Tokenizer Coder

import Foundation

func runTests() {
    print("=========================================================")
    print("  RUNNING SWIFT UFO TOKENIZER CODER VERIFICATION")
    print("  Watermark: ip zymatica.space")
    print("=========================================================")

    // 1. Test Vocab Prefix-Suffix Coder
    print("\n[Test 1] Prefix-Suffix Vocab Coder...")
    let originalVocabStrings = [
        "hello",
        "hell",
        "heaven",
        "heavy",
        "world",
        "word",
        "work",
        "worker",
        "working"
    ]
    let originalVocab = originalVocabStrings.map { [UInt8]($0.utf8) }

    let compressedVocab = compressVocab(originalVocab)
    print("  Original vocab items: \(originalVocab.count)")
    print("  Compressed vocab size: \(compressedVocab.count) bytes")

    let restoredVocab = decompressVocab(compressedVocab, originalVocab.count)
    print("  Restored vocab items: \(restoredVocab.count)")

    assert(originalVocab.count == restoredVocab.count)
    for i in 0..<originalVocab.count {
        assert(originalVocab[i] == restoredVocab[i], "Mismatch at index \(i)")
    }
    print("  [+] Vocab round-trip: SUCCESS (100% Match)")

    // 2. Test BPE Merges index pack/unpack
    print("\n[Test 2] BPE Merges Binary Index Coder...")
    let originalMerges: [(UInt32, UInt32)] = [
        (1015, 2030),
        (45, 12),
        (16777215, 50000),
        (0, 1),
        (100000, 200000)
    ]

    let compressedMerges = compressMerges(originalMerges)
    print("  Original merges items: \(originalMerges.count)")
    print("  Compressed merges size: \(compressedMerges.count) bytes")

    let restoredMerges = decompressMerges(compressedMerges)
    print("  Restored merges items: \(restoredMerges.count)")

    assert(originalMerges.count == restoredMerges.count)
    for i in 0..<originalMerges.count {
        assert(originalMerges[i].0 == restoredMerges[i].0)
        assert(originalMerges[i].1 == restoredMerges[i].1)
    }
    print("  [+] Merges round-trip: SUCCESS (100% Match)")

    // 3. Test XOR-FEC Parity
    print("\n[Test 3] XOR-FEC Parity Calculation...")
    let c1: [UInt8] = [0xAA, 0xBB, 0xCC, 0xDD]
    let c2: [UInt8] = [0x11, 0x22, 0x33, 0x44]
    let c3: [UInt8] = [0x55, 0x66, 0x77, 0x88]
    let chunks = [c1, c2, c3]

    let parity = computeXorFecParity(chunks, 4)
    let expectedParity: [UInt8] = [
        0xAA ^ 0x11 ^ 0x55,
        0xBB ^ 0x22 ^ 0x66,
        0xCC ^ 0x33 ^ 0x77,
        0xDD ^ 0x44 ^ 0x88
    ]

    assert(parity == expectedParity)
    print("  [+] XOR-FEC computation: SUCCESS")

    print("\n=========================================================")
    print("  ALL SWIFT TESTS PASSED SUCCESSFULLY!")
    print("=========================================================")
}

runTests()
