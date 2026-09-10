# Watermark: ip zymatica.space | astronautshe.com
# Copyright © 2026 Zymatica
# SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
# See LICENSE for terms.
"""
ZK-LoRaWAN Master Verification Harness
========================================
Runs all module self-tests and reports pass/fail status.
"""

import subprocess
import sys
import os
import time

MODULES = [
    {
        "name": "LLD-AC Proof Compression",
        "component": "Component 08",
        "script": "proof_compression/lld_ac_encoder.py",
    },
    {
        "name": "XOR-FEC Crypto-Reconstruction",
        "component": "Component 06",
        "script": "fec/xor_fec.py",
    },
    {
        "name": "ZK Semantic Gating (Range Proofs)",
        "component": "Component 02",
        "script": "semantic_gating/range_proof.py",
    },
    {
        "name": "Private Gateway Reputation",
        "component": "Components 12 & 15",
        "script": "reputation/pedersen_reputation.py",
    },
    {
        "name": "microByte JIT VK Compression",
        "component": "Component 19",
        "script": "vk_compression/microbyte_jit.py",
    },
    {
        "name": "UFO Semantic Codec",
        "component": "Component 09",
        "script": "semantic_codec/semantic_codec.py",
    },
    {
        "name": "LoRa Gateway Engine",
        "component": "Gateway Software",
        "script": "gateway/gateway.py",
        "pre_command": ["cargo", "build", "-p", "zk-lorawan-groth16"],
        "pre_timeout": 120,
        "required_output": [
            "Chirps verified:  3",
            "Chirps rejected:  0",
        ],
    },
    {
        "name": "On-Chain Verifier Key Consistency",
        "component": "Groth16/Solana Bridge",
        "script": "tests/verify_onchain_vk.py",
    },
]


def run_module_test(module: dict, base_dir: str) -> bool:
    """Run a single module's self-test and return pass/fail."""
    script_path = os.path.join(base_dir, module["script"])

    if not os.path.exists(script_path):
        print(f"    ⚠️  Script not found: {script_path}")
        return False

    try:
        pre_command = module.get("pre_command")
        if pre_command:
            print(f"    Preflight: {' '.join(pre_command)}")
            preflight = subprocess.run(
                pre_command,
                capture_output=True, text=True,
                timeout=module.get("pre_timeout", 30),
                cwd=base_dir
            )
            if preflight.returncode != 0:
                print(f"    preflight stderr: {preflight.stderr[:500]}")
                return False

        env = os.environ.copy()
        if module["script"] == "gateway/gateway.py":
            env["ZK_LORAWAN_TEST_CHIRPS"] = "3"

        result = subprocess.run(
            [sys.executable, script_path],
            capture_output=True, text=True, timeout=120,
            cwd=base_dir,
            env=env
        )
        output = result.stdout + result.stderr

        if result.returncode != 0:
            print(f"    stderr: {result.stderr[:200]}")
            return False

        # Print the module's output (indented)
        for line in result.stdout.strip().split('\n'):
            print(f"    {line}")

        missing_markers = [
            marker for marker in module.get("required_output", [])
            if marker not in output
        ]
        if missing_markers:
            print(f"    Missing required output marker(s): {', '.join(missing_markers)}")
            return False

        return True

    except subprocess.TimeoutExpired:
        print(f"    [!] TIMEOUT after 120 seconds")
        return False
    except Exception as e:
        print(f"    [!] Error: {e}")
        return False


def main():
    print("=" * 70)
    print("ZK-LoRaWAN | Master Verification Harness")
    print("=" * 70)
    print()

    base_dir = os.path.dirname(os.path.abspath(__file__))
    # If running from tests/ subdirectory, go up one level
    if os.path.basename(base_dir) == "tests":
        base_dir = os.path.dirname(base_dir)

    passed = 0
    failed = 0
    results = []

    for module in MODULES:
        print(f"  +-- {module['name']} ({module['component']})")
        print(f"  |   Script: {module['script']}")
        print(f"  |")

        success = run_module_test(module, base_dir)

        if success:
            passed += 1
            status = "PASSED"
        else:
            failed += 1
            status = "FAILED"

        results.append((module["name"], status))
        print(f"  +-- {status}")
        print()

    # Summary
    total = passed + failed
    print("=" * 70)
    print(f"  RESULTS: {passed} passed, {failed} failed, {total} total")
    print("=" * 70)
    print()

    for name, status in results:
        print(f"    [{status}]  {name}")

    print()
    if failed == 0:
        print("  ALL MODULES PASSED - ZK-LoRaWAN verification suite is operational!")
    else:
        print(f"  WARNING: {failed} module(s) failed. Review output above.")

    print()
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
