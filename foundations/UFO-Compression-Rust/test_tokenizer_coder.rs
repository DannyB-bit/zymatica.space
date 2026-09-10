mod tokenizer_coder;

fn main() {
    println!("=========================================================");
    println!("  RUNNING RUST UFO TOKENIZER CODER VERIFICATION");
    println!("  Watermark: ip zymatica.space");
    println!("=========================================================");

    // 1. Test Vocab Coder
    println!("\n[Test 1] Prefix-Suffix Vocab Coder...");
    let original_vocab: Vec<&[u8]> = vec![
        b"hello",
        b"hell",
        b"heaven",
        b"heavy",
        b"world",
        b"word",
        b"work",
        b"worker",
        b"working"
    ];

    let compressed_vocab = tokenizer_coder::compress_vocab(&original_vocab);
    println!("  Original vocab items: {}", original_vocab.len());
    println!("  Compressed vocab size: {} bytes", compressed_vocab.len());

    let restored_vocab = tokenizer_coder::decompress_vocab(&compressed_vocab, original_vocab.len());
    println!("  Restored vocab items: {}", restored_vocab.len());

    assert_eq!(original_vocab.len(), restored_vocab.len());
    for i in 0..original_vocab.len() {
        assert_eq!(original_vocab[i], restored_vocab[i]);
    }
    println!("  [+] Vocab round-trip: SUCCESS (100% Match)");

    // 2. Test BPE Merges Coder
    println!("\n[Test 2] BPE Merges Binary Index Coder...");
    let original_merges: Vec<(u32, u32)> = vec![
        (1015, 2030),
        (45, 12),
        (16777215, 50000),
        (0, 1),
        (100000, 200000)
    ];

    let compressed_merges = tokenizer_coder::compress_merges(&original_merges);
    println!("  Original merges items: {}", original_merges.len());
    println!("  Compressed merges size: {} bytes", compressed_merges.len());

    let restored_merges = tokenizer_coder::decompress_merges(&compressed_merges);
    println!("  Restored merges items: {}", restored_merges.len());

    assert_eq!(original_merges.len(), restored_merges.len());
    for i in 0..original_merges.len() {
        assert_eq!(original_merges[i], restored_merges[i]);
    }
    println!("  [+] Merges round-trip: SUCCESS (100% Match)");

    // 3. Test XOR-FEC Parity
    println!("\n[Test 3] XOR-FEC Parity Calculation...");
    let c1 = vec![0xAA, 0xBB, 0xCC, 0xDD];
    let c2 = vec![0x11, 0x22, 0x33, 0x44];
    let c3 = vec![0x55, 0x66, 0x77, 0x88];
    let chunks = vec![c1, c2, c3];

    let parity = tokenizer_coder::compute_xor_fec_parity(&chunks, 4);
    let expected_parity = vec![
        0xAA ^ 0x11 ^ 0x55,
        0xBB ^ 0x22 ^ 0x66,
        0xCC ^ 0x33 ^ 0x77,
        0xDD ^ 0x44 ^ 0x88
    ];
    assert_eq!(parity, expected_parity);
    println!("  [+] XOR-FEC computation: SUCCESS");

    println!("\n=========================================================");
    println!("  ALL RUST TESTS PASSED SUCCESSFULLY!");
    println!("=========================================================");
}
