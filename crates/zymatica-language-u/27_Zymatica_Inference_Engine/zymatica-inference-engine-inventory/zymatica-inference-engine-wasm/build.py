# Watermark: ip zymatica.space | astronautshe.com
# Copyright (c) 2026 Zymatica.
# SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
# See LICENSE for terms.

import subprocess
import sys
import os

def main():
    script_dir = os.path.dirname(os.path.abspath(__file__))
    zig_file = os.path.join(script_dir, "proof.zig")
    
    print("[*] Launching Zig compiler to cross-compile to WebAssembly...")
    cmd = [
        "zig", "build-exe", zig_file,
        "-target", "wasm32-freestanding",
        "-O", "ReleaseFast",
        "--name", "proof_wasm",
        "--export=run_verification"
    ]
    
    try:
        res = subprocess.run(cmd, cwd=script_dir, capture_output=True, text=True)
        if res.returncode == 0:
            print("[+] WebAssembly compiled successfully: proof_wasm.wasm is ready.")
        else:
            print(f"[-] Zig compilation failed:\n{res.stderr}")
            sys.exit(1)
    except Exception as e:
        print(f"[-] Subprocess compile error: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
