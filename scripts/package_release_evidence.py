#!/usr/bin/env python3
# Copyright © 2026 Zymatica
# SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
# See LICENSE for terms.
"""
Evidence Archival & Release Package Builder

Assembles permanent research evidence files, regenerates MANIFEST.json and SHA256SUMS.
"""

from __future__ import annotations

import hashlib
import json
import subprocess
import sys
from pathlib import Path

sys.stdout.reconfigure(encoding="utf-8")


def sha256_file(path: Path) -> str:
    raw = path.read_bytes()
    if path.suffix in {".json", ".txt", ".md", ".csv", ".spdx", ".lean"} or path.name == "SHA256SUMS":
        raw = raw.replace(b"\r\n", b"\n")
    return hashlib.sha256(raw).hexdigest()


def package_evidence():
    root = Path(__file__).resolve().parent.parent
    evidence_dir = root / "evidence" / "10_00" / "latest"

    # Step 1: Run evidence_manifest.py
    subprocess.run([sys.executable, str(root / "tools" / "ten_out_of_ten" / "evidence_manifest.py"), str(evidence_dir)], check=True)

    # Step 2: Generate SHA256SUMS for immutable evidence artifacts
    lines = []
    for p in sorted(evidence_dir.rglob("*")):
        if p.is_file() and p.name not in {"SHA256SUMS", "release_attestation.json"}:
            rel = p.relative_to(evidence_dir).as_posix()
            lines.append(f"{sha256_file(p)}  {rel}")

    sums_file = evidence_dir / "SHA256SUMS"
    with open(sums_file, "w", encoding="utf-8", newline="\n") as f:
        f.write("\n".join(lines) + "\n")
    print(f"[+] Generated SHA256SUMS with {len(lines)} files at {sums_file}")


if __name__ == "__main__":
    package_evidence()
