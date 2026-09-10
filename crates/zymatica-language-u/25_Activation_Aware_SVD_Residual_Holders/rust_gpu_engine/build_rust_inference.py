# Rust Inference Builder using PyTorch Libtorch
# Watermark: ip zymatica.space | astronautshe.com

import os
import sys
import subprocess
import torch

def main():
    print("=" * 80)
    print("  RUST PYTORCH INFERENCE LINKER & BUILDER")
    print("  Watermark: ip zymatica.space | astronautshe.com")
    print("=" * 80)

    # 1. Locate PyTorch installation
    torch_dir = os.path.dirname(torch.__file__)
    torch_lib_dir = os.path.join(torch_dir, "lib")
    
    print(f"[*] Detected PyTorch directory: {torch_dir}")
    print(f"[*] Detected LibTorch DLLs directory: {torch_lib_dir}")

    if not os.path.exists(torch_lib_dir):
        print(f"[-] Error: torch/lib directory not found at {torch_lib_dir}")
        sys.exit(1)

    # 2. Configure environment variables for compilation
    env = os.environ.copy()
    env["LIBTORCH_USE_PYTORCH"] = "1"
    env["LIBTORCH"] = torch_dir
    env["LIBTORCH_BYPASS_VERSION_CHECK"] = "1"
    
    # Prepend torch/lib to PATH so linker can find DLLs / Lib files during build
    existing_path = env.get("PATH", "")
    env["PATH"] = f"{torch_lib_dir};{existing_path}"

    print("[*] Configured build environment:")
    print(f"    - LIBTORCH_USE_PYTORCH = 1")
    print(f"    - LIBTORCH = {torch_dir}")
    print(f"    - Appended to PATH: {torch_lib_dir}")

    # 3. Execute cargo build
    print("\n[*] Executing cargo build --release...")
    try:
        process = subprocess.Popen(
            ["cargo", "build", "--release"],
            cwd="j:/Language-U",
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True
        )
        
        # Stream compilation logs in real-time
        for line in process.stdout:
            print(line, end="")
            
        process.wait()
        
        if process.returncode == 0:
            print("\n[+] SUCCESS! Rust tch-rs binary built successfully in release mode.")
            bin_path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "run_gemma4_cuda_hybrid.exe")
            print(f"[*] Output binary located at: {bin_path}")
        else:
            print(f"\n[-] Error: cargo build failed with exit code: {process.returncode}")
            sys.exit(process.returncode)
            
    except Exception as e:
        print(f"[-] Exception during build: {e}")
        sys.exit(1)

    print("\n" + "=" * 80)
    print("  BUILD SYSTEM EXECUTED SUCCESSFULLY")
    print("=" * 80)

if __name__ == "__main__":
    main()
