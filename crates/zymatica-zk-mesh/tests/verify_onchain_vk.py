# Watermark: ip zymatica.space | astronautshe.com
# Copyright © 2026 Zymatica
# SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
# See LICENSE for terms.
"""
Verify that the on-chain Solana verifier key constants match the Groth16 circuit.

This catches stale or mistyped VK coordinates after circuit changes.
"""

import os
import re
import subprocess
import sys


VK_ARRAYS = ("vk_alpha", "vk_beta", "vk_gamma", "vk_delta")
VK_LINE_RE = re.compile(r"^(vk_[^:]+|vk_ic\[\d+\]):\s*([0-9a-fA-F]+)$")


def repo_root() -> str:
    return os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def hex_array(block: str) -> str:
    return "".join(match.group(1).lower() for match in re.finditer(r"0x([0-9a-fA-F]{2})", block))


def expected_vk(root: str) -> dict[str, str]:
    env = os.environ.copy()
    result = subprocess.run(
        ["cargo", "run", "--quiet", "-p", "zk-lorawan-groth16", "--", "print-vk"],
        cwd=root,
        capture_output=True,
        text=True,
        timeout=120,
        env=env,
    )
    if result.returncode != 0:
        raise RuntimeError(result.stderr.strip() or "print-vk failed")

    expected: dict[str, str] = {}
    for line in result.stdout.splitlines():
        match = VK_LINE_RE.match(line.strip())
        if match:
            expected[match.group(1)] = match.group(2).lower()
    return expected


def committed_vk(root: str) -> dict[str, str]:
    lib_path = os.path.join(root, "programs", "zk_lorawan", "src", "lib.rs")
    with open(lib_path, "r", encoding="utf-8") as handle:
        source = handle.read()

    actual: dict[str, str] = {}
    for name in VK_ARRAYS:
        match = re.search(rf"let {name}: \[u8; \d+\] = \[(.*?)\];", source, re.S)
        if not match:
            raise RuntimeError(f"Could not find {name} in on-chain program")
        actual[name] = hex_array(match.group(1))

    match = re.search(r"let vk_ic: \[\[u8; 64\]; 9\] = \[(.*?)\];", source, re.S)
    if not match:
        raise RuntimeError("Could not find vk_ic in on-chain program")
    joined_ic = hex_array(match.group(1))
    if len(joined_ic) != 9 * 64 * 2:
        raise RuntimeError(f"vk_ic has {len(joined_ic) // 2} bytes, expected 576")
    for index in range(9):
        actual[f"vk_ic[{index}]"] = joined_ic[index * 128:(index + 1) * 128]
    return actual


def main() -> int:
    root = repo_root()
    expected = expected_vk(root)
    actual = committed_vk(root)

    missing = sorted(set(expected) - set(actual))
    mismatches = [
        name for name in sorted(expected)
        if name in actual and expected[name] != actual[name]
    ]

    if missing or mismatches:
        for name in missing:
            print(f"MISSING {name}")
        for name in mismatches:
            print(f"MISMATCH {name}")
            print(f"  expected: {expected[name]}")
            print(f"  actual:   {actual[name]}")
        return 1

    print(f"On-chain verifier key constants match Groth16 print-vk output ({len(expected)} points).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
