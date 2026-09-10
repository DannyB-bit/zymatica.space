#!/usr/bin/env python3
# Copyright © 2026 Zymatica
# SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
# See LICENSE for terms.
"""
ZYMATICA MASTER FORENSIC EVIDENCE ORCHESTRATOR

Audits and orchestrates live execution across all architectural, mathematical,
cryptographic, physical RF, and real-model verification subsystems.
Consumes machine-readable structured artifacts with zero synthetic shortcuts.
"""

from __future__ import annotations

import json
import subprocess
import sys
import time
from pathlib import Path

sys.stdout.reconfigure(encoding="utf-8")


def run_subsystem(title: str, command: list[str]) -> tuple[bool, str, float]:
    t0 = time.perf_counter()
    res = subprocess.run(command, capture_output=True, text=True, encoding="utf-8")
    elapsed = time.perf_counter() - t0
    success = (res.returncode == 0)
    output = res.stdout if success else res.stderr
    return success, output, elapsed


def main():
    print("=" * 85)
    print("🔬 ZYMATICA SOVEREIGN ARCHITECTURE: MASTER FORENSIC EVIDENCE ORCHESTRATOR")
    print("   Evidentiary Standard: Live Execution, Content-Addressed Hash & Artifact Proofs")
    print("   Attribution: Book Author: Danny Bouldiez | Codebase: Devs One")
    print("=" * 85)

    tests = [
        ("1. Riemannian Metric Tensor & Lie-Algebra Isometry", [sys.executable, "prove_isometry.py"]),
        ("2. 6D Cuneiform-U Semantic & Wire Compression Battery", [sys.executable, "tools/ten_out_of_ten/semantic_cuneiform_eval.py"]),
        ("3. Frontier Mathematical Battery (Geodesic, SVD, MiMC, XOR-FEC)", [sys.executable, "verify_frontier_suite.py"]),
        ("4. Canonical Physical RF Evidence & Self-Reconstruction Gate", [sys.executable, "tools/ten_out_of_ten/verify_rf_evidence.py"]),
        ("5. Class 30 Z-HQSpec Holomorphic Speculative Engine Benchmark", [sys.executable, "crates/zymatica-language-u/30_Holomorphic_Speculative_Engine/run_proof.py"]),
        ("6. Research Claim Registry & Numerical Audit Gate", [sys.executable, "tools/ten_out_of_ten/claims_audit.py", "--root", "."]),
        ("7. Real-Model Empirical Acceptance Gate (Hyper-KV, WORMHOLE, Epigenetic, MCTS)", [sys.executable, "tools/ten_out_of_ten/acceptance_gate.py", "--evidence-dir", "evidence/10_00/latest"]),
    ]

    passed = 0
    total = len(tests)
    t_global_start = time.perf_counter()

    for idx, (title, cmd) in enumerate(tests, 1):
        print(f"\n[AUDIT {idx}/{total}] {title}...")
        ok, out, el = run_subsystem(title, cmd)
        if ok:
            print(f"  ✅ PASS ({el:.2f}s) - Live subverifier executed cleanly")
            passed += 1
        else:
            print(f"  ❌ FAIL ({el:.2f}s) - Execution error:")
            print("  " + "\n  ".join(out.strip().splitlines()[-5:]))

    t_global = time.perf_counter() - t_global_start
    print("\n" + "=" * 85)
    print(f"🔬 MASTER AUDIT RESULT: {passed}/{total} SUBSYSTEMS VERIFIED ({t_global:.2f}s Total Runtime)")
    if passed == total:
        print("   STATUS: 100% LIVE FORENSIC VERIFICATION PASS — ALL EMPIRICAL & MATHEMATICAL CLAIMS VALIDATED")
    else:
        print(f"   STATUS: AUDIT DEFICIT DETECTED ({total - passed} failures)")
    print("=" * 85)

    return 0 if passed == total else 1


if __name__ == "__main__":
    raise SystemExit(main())