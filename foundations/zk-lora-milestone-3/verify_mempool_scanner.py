#!/usr/bin/env python3
"""
ZK-LoRa Milestone 3 verification runner.

This verifies the scanner's decrypted-event payout matching path. Live Zcash
mode must provide the same event schema from a wallet/light-client adapter via
ZK_LORA_DECRYPTED_EVENT_JSON or ZK_LORA_DECRYPTED_EVENT_PATH.
"""

import os
import subprocess
import sys
from pathlib import Path


def main() -> None:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8", errors="backslashreplace")

    print("=" * 80)
    print("ZK-LoRa Milestone 3: Decrypted Payment Event Verification Suite")
    print("=" * 80)

    rust_dir = Path("Full_Projects") / "rust"
    fixture_path = Path("fixtures") / "decrypted_payment_event.json"
    if not fixture_path.exists():
        print(f"    * Error: Decrypted payment fixture not found at {fixture_path}")
        sys.exit(1)

    print("\n[1] Compiling ZK-LoRa Rust Operator Daemon...")
    try:
        subprocess.run(["cargo", "build"], cwd=rust_dir, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        print("    * Compilation status: [PASS] SUCCESS")
    except subprocess.CalledProcessError as exc:
        print("    * Compilation status: [FAIL] FAILED")
        print(exc.stderr.decode(encoding="utf-8", errors="ignore"))
        sys.exit(1)

    print("\n[2] Executing decrypted-event scanner test suite...")
    exe_name = "zk-lora-operator.exe" if os.name == "nt" else "zk-lora-operator"
    exe_path = rust_dir / "target" / "debug" / exe_name
    if not exe_path.exists():
        print(f"    * Error: Compiled binary not found at {exe_path}")
        sys.exit(1)

    env = os.environ.copy()
    env["ZK_LORA_DECRYPTED_EVENT_PATH"] = str(fixture_path)
    try:
        result = subprocess.run(
            [str(exe_path), "--test"],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            encoding="utf-8",
            errors="backslashreplace",
            env=env,
        )
    except subprocess.CalledProcessError as exc:
        print("    * Execution status: [FAIL] FAILED")
        print(f"    * Stdout:\n{exc.stdout}")
        print(f"    * Stderr:\n{exc.stderr}")
        sys.exit(1)

    print(result.stdout)
    if "SUCCESS: All modules verified successfully" not in result.stdout:
        print("    * Error: Test execution did not print completion logs.")
        sys.exit(1)

    print("=" * 80)
    print("MILESTONE 3 VERIFICATION PASSED SUCCESSFULLY!")
    print("   Decrypted payment event fixture validated with 2% dev fee split.")
    print("   Live mode requires wallet/light-client decrypted event input.")
    print("=" * 80)


if __name__ == "__main__":
    main()
