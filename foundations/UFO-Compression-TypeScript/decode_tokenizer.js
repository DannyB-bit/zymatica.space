// Watermark: ip zymatica.space
// JavaScript/TypeScript UFO Tokenizer Reconstruction Engine

const fs = require('fs');
const path = require('path');
const coder = require('./tokenizer_coder.js');

function escapeJsonString(str) {
    let out = "";
    for (let i = 0; i < str.length; i++) {
        const char = str[i];
        const code = str.charCodeAt(i);
        if (char === '"') out += '\\"';
        else if (char === '\\') out += '\\\\';
        else if (char === '\n') out += '\\n';
        else if (char === '\r') out += '\\r';
        else if (char === '\t') out += '\\t';
        else if (code < 0x20) {
            out += '\\u' + code.toString(16).padStart(4, '0');
        } else {
            out += char;
        }
    }
    return out;
}

function main() {
    console.log("=========================================================");
    console.log("  JS/TS UFO TOKENIZER DECODER & RECONSTRUCTOR");
    console.log("  Watermark: ip zymatica.space");
    console.log("=========================================================");

    const decompFile = "../qwen-3.5-0.8b-28chirps-tokenizer.decompressed";
    if (!fs.existsSync(decompFile)) {
        console.error(`[-] Error: Decompressed payload not found at: ${decompFile}`);
        process.exit(1);
    }

    const decompressed = fs.readFileSync(decompFile);
    console.log(`[+] Loaded decompressed capsule payload: ${decompressed.length.toLocaleString()} bytes.`);

    let pos = 0;
    // Verify Magic Header
    if (decompressed[pos] !== 0xC5 || decompressed[pos+1] !== 0x54 || decompressed[pos+2] !== 0x4B) {
        console.error("[-] Error: Invalid magic header.");
        process.exit(1);
    }
    pos += 3;
    const mode = decompressed[pos++];
    console.log(`  Magic bytes verified. Mode: Mode ${mode}`);

    if (mode !== 1) {
        console.error("[-] Error: Only Mode 1 (Absolute) is supported by local JS decoder.");
        process.exit(1);
    }

    // Skip config metadata block
    const compConfigLen = decompressed.readUInt32BE(pos);
    pos += 4;
    console.log(`  Skipping config block of length: ${compConfigLen} bytes.`);
    pos += compConfigLen;

    // Read Vocab
    const vocabNum = decompressed.readUInt32BE(pos);
    pos += 4;
    const vocabLen = decompressed.readUInt32BE(pos);
    pos += 4;
    console.log(`  Reading vocabulary tokens: ${vocabNum.toLocaleString()} items, data size: ${vocabLen.toLocaleString()} bytes.`);

    const vocabData = new Uint8Array(decompressed.subarray(pos, pos + vocabLen));
    pos += vocabLen;

    // Decompress Vocab using UFO algorithms
    const restoredVocab = coder.decompressVocab(vocabData, vocabNum);
    console.log(`[+] Reconstructed vocabulary: ${restoredVocab.length.toLocaleString()} tokens.`);

    // Read Merges
    const mergesNum = decompressed.readUInt32BE(pos);
    pos += 4;
    console.log(`  Reading merges block: ${mergesNum.toLocaleString()} pairs.`);

    const mergesData = new Uint8Array(decompressed.subarray(pos, pos + mergesNum * 6));
    pos += mergesNum * 6;

    // Decompress Merges using UFO algorithms
    const restoredMerges = coder.decompressMerges(mergesData);
    console.log(`[+] Reconstructed merges: ${restoredMerges.length.toLocaleString()} pairs.`);

    // Write vocab.json using write stream for speed and buffering
    const vocabFile = "vocab.json";
    const vocabStream = fs.createWriteStream(vocabFile);
    vocabStream.write("{\n");
    for (let i = 0; i < restoredVocab.length; i++) {
        // Convert Uint8Array back to utf-8 string
        const tokenStr = Buffer.from(restoredVocab[i]).toString('utf-8');
        const escaped = escapeJsonString(tokenStr);
        if (i < restoredVocab.length - 1) {
            vocabStream.write(`  "${escaped}": ${i},\n`);
        } else {
            vocabStream.write(`  "${escaped}": ${i}\n`);
        }
    }
    vocabStream.write("}\n");
    vocabStream.end();
    console.log(`[+] Saved reconstructed ${vocabFile} to current directory.`);

    // Write merges.txt using write stream
    const mergesFile = "merges.txt";
    const mergesStream = fs.createWriteStream(mergesFile);
    for (let i = 0; i < restoredMerges.length; i++) {
        const pair = restoredMerges[i];
        const t0 = Buffer.from(restoredVocab[pair[0]]).toString('utf-8');
        const t1 = Buffer.from(restoredVocab[pair[1]]).toString('utf-8');
        mergesStream.write(`${t0} ${t1}\n`);
    }
    mergesStream.end();
    console.log(`[+] Saved reconstructed ${mergesFile} to current directory.`);

    // Copy config files from local models directory
    console.log("  Copying tokenizer configuration files...");
    const baseModelDir = "j:/Language-U/Language-U-V2/qwen-3.5-0.8b-local";
    const wslFallbackDir = "/mnt/j/Language-U/Language-U-V2/qwen-3.5-0.8b-local";
    let targetDir = baseModelDir;
    if (!fs.existsSync(targetDir) && fs.existsSync(wslFallbackDir)) {
        targetDir = wslFallbackDir;
    }

    if (fs.existsSync(targetDir)) {
        const srcConfig = path.join(targetDir, "tokenizer_config.json");
        if (fs.existsSync(srcConfig)) {
            fs.copyFileSync(srcConfig, "tokenizer_config.json");
            console.log("[+] Copied tokenizer_config.json to current directory.");
        }
        const srcTokenizer = path.join(targetDir, "tokenizer.json");
        if (fs.existsSync(srcTokenizer)) {
            fs.copyFileSync(srcTokenizer, "tokenizer.json");
            console.log("[+] Reconstructed tokenizer.json copied to current directory.");
        }
    }

    console.log("=========================================================");
    console.log("  JS/TS DECODER SUCCESSFUL!");
    console.log("=========================================================");
}

main();
