// Watermark: ip zymatica.space | astronautshe.com
// Copyright (c) 2026 Zymatica.
// SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
// See LICENSE for terms.

const fs = require('fs');
const path = require('path');

function main() {
    console.log("======================================================================");
    console.log("ZYMATICA | zymatica-inference-engine-wasm (WebAssembly)");
    console.log("======================================================================\n");

    const wasmPath = path.join(__dirname, "proof_wasm.wasm");
    if (!fs.existsSync(wasmPath)) {
        console.error(`[-] Error: WebAssembly binary not found at ${wasmPath}`);
        console.error("[-] Please run 'python build.py' first to compile.");
        process.exit(1);
    }

    const wasmBuffer = fs.readFileSync(wasmPath);
    WebAssembly.instantiate(new Uint8Array(wasmBuffer), {})
        .then(wasmModule => {
            const exports = wasmModule.instance.exports;
            
            const start = performance.now();
            const runs = 100000;
            let result = 1;
            for (let i = 0; i < runs; i++) {
                result = exports.run_verification();
                if (result !== 1) break;
            }
            const duration = (performance.now() - start).toFixed(4);
            
            console.log(`[*] Executed WASM binary loop in ${duration} ms.`);
            console.log(`[*] Verification return code: ${result}`);
            
            if (result === 1) {
                console.log(`[INTERNAL_MATH] ${duration} ms`);
                console.log("\n[VERIFICATION] Multi-Language runtime FFI structures validated.");
            } else {
                console.error("\n[-] ERROR: Decoded sequence mismatch inside WASM context!");
                process.exit(1);
            }
        })
        .catch(err => {
            console.error(`[-] Instantiation error: ${err}`);
            process.exit(1);
        });
}

main();
