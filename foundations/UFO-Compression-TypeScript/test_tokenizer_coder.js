// Watermark: ip zymatica.space
// JavaScript/TypeScript Verification Suite for UFO Tokenizer Compression

const coder = require('./tokenizer_coder.js');
const assert = require('assert');

function runTests() {
    console.log("=========================================================");
    console.log("  RUNNING JS/TS UFO TOKENIZER CODER VERIFICATION");
    console.log("  Watermark: ip zymatica.space");
    console.log("=========================================================");

    // 1. Test Vocab Prefix-Suffix coder
    console.log("\n[Test 1] Prefix-Suffix Vocab Coder...");
    const originalVocabStrings = [
        "hello",
        "hell",
        "heaven",
        "heavy",
        "world",
        "word",
        "work",
        "worker",
        "working"
    ];
    const originalVocab = originalVocabStrings.map(s => Buffer.from(s, 'utf-8'));

    const compressedVocab = coder.compressVocab(originalVocab);
    console.log(`  Original vocab items: ${originalVocab.length}`);
    console.log(`  Compressed vocab size: ${compressedVocab.length} bytes`);

    const restoredVocab = coder.decompressVocab(compressedVocab, originalVocab.length);
    console.log(`  Restored vocab items: ${restoredVocab.length}`);

    assert.strictEqual(originalVocab.length, restoredVocab.length);
    for (let i = 0; i < originalVocab.length; i++) {
        const origStr = originalVocab[i].toString('utf-8');
        const restStr = Buffer.from(restoredVocab[i]).toString('utf-8');
        assert.strictEqual(origStr, restStr);
    }
    console.log("  [+] Vocab round-trip: SUCCESS (100% Match)");

    // 2. Test BPE Merges index pack/unpack
    console.log("\n[Test 2] BPE Merges Binary Index Coder...");
    const originalMerges = [
        [1015, 2030],
        [45, 12],
        [16777215, 50000],
        [0, 1],
        [100000, 200000]
    ];

    const compressedMerges = coder.compressMerges(originalMerges);
    console.log(`  Original merges items: ${originalMerges.length}`);
    console.log(`  Compressed merges size: ${compressedMerges.length} bytes`);

    const restoredMerges = coder.decompressMerges(compressedMerges);
    console.log(`  Restored merges items: ${restoredMerges.length}`);

    assert.strictEqual(originalMerges.length, restoredMerges.length);
    for (let i = 0; i < originalMerges.length; i++) {
        assert.strictEqual(originalMerges[i][0], restoredMerges[i][0]);
        assert.strictEqual(originalMerges[i][1], restoredMerges[i][1]);
    }
    console.log("  [+] Merges round-trip: SUCCESS (100% Match)");

    // 3. Test XOR-FEC Parity
    console.log("\n[Test 3] XOR-FEC Parity Calculation...");
    const c1 = new Uint8Array([0xAA, 0xBB, 0xCC, 0xDD]);
    const c2 = new Uint8Array([0x11, 0x22, 0x33, 0x44]);
    const c3 = new Uint8Array([0x55, 0x66, 0x77, 0x88]);
    const chunks = [c1, c2, c3];

    const parity = coder.computeXorFecParity(chunks, 4);
    const expectedParity = new Uint8Array([
        0xAA ^ 0x11 ^ 0x55,
        0xBB ^ 0x22 ^ 0x66,
        0xCC ^ 0x33 ^ 0x77,
        0xDD ^ 0x44 ^ 0x88
    ]);

    assert.deepStrictEqual(parity, expectedParity);
    console.log("  [+] XOR-FEC computation: SUCCESS");

    console.log("\n=========================================================");
    console.log("  ALL JS/TS TESTS PASSED SUCCESSFULLY!");
    console.log("=========================================================");
}

runTests();
