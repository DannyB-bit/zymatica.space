#!/usr/bin/env python3
# Copyright © 2026 Zymatica
# SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
# See LICENSE for terms.
"""Objective research-evidence gate used for the assistant's 10.00 engineering threshold.

The thresholds are intentionally demanding. They are not claims that the current algorithms already
meet them. A PASS means the committed evidence files demonstrate the specified minimum behavior on
real models and held-out prompts.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any


def load(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def fail(failures: list[str], condition: bool, message: str) -> None:
    if not condition:
        failures.append(message)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--evidence-dir", type=Path, required=True)
    args = parser.parse_args()
    root = args.evidence_dir
    failures: list[str] = []

    hyper_path = root / "hyper_kv.json"
    worm_path = root / "wormhole.json"
    epi_path = root / "epigenetic.json"
    mcts_path = root / "mcts.json"
    for path in (hyper_path, worm_path, epi_path, mcts_path):
        fail(failures, path.exists(), f"missing evidence file: {path}")
    if failures:
        print(json.dumps({"status": "FAIL", "failures": failures}, indent=2))
        return 1

    hyper = load(hyper_path)
    worm = load(worm_path)
    epi = load(epi_path)
    mcts = load(mcts_path)

    fail(failures, hyper.get("prompt_count", 0) >= 32, "Hyper-KV must evaluate at least 32 real prompts")
    fail(failures, hyper.get("injection_supported") is True, "Hyper-KV reconstructed cache must be reinjected into the model")
    fail(failures, (hyper.get("mean_compression_ratio") or 0) >= 2.0, "Hyper-KV mean measured compression ratio must be >= 2.0x")
    fail(failures, (hyper.get("next_step_top1_parity") or 0) >= 0.90, "Hyper-KV next-step top-1 parity must be >= 90%")
    fail(failures, (hyper.get("next_step_kl_mean") if hyper.get("next_step_kl_mean") is not None else 999) <= 0.10, "Hyper-KV mean next-step KL must be <= 0.10")

    fail(failures, worm.get("training_samples", 0) >= 100, "Z-WORMHOLE requires at least 100 training prompts")
    fail(failures, worm.get("heldout_samples", 0) >= 25, "Z-WORMHOLE requires at least 25 held-out prompts")
    fail(failures, (worm.get("heldout_cosine_mean") or 0) >= 0.75, "Z-WORMHOLE held-out target-state cosine must be >= 0.75")
    fail(failures, (worm.get("heldout_top1_agreement") or 0) >= 0.60, "Z-WORMHOLE held-out LM-head top-1 agreement must be >= 60%")

    fail(failures, epi.get("calibration_activation_rows", 0) >= 128, "Class 31 needs >=128 real calibration activation rows")
    fail(failures, (epi.get("max_abs_calibration_activation_delta") or 999) <= 1.0e-4, "Class 31 calibration activation max delta must be <=1e-4")
    fail(failures, (epi.get("adapt_loss_relative_change") if epi.get("adapt_loss_relative_change") is not None else 1) < 0.0, "Class 31 projected update must improve adaptation loss")
    fail(failures, (epi.get("base_loss_relative_change") if epi.get("base_loss_relative_change") is not None else 1) <= 0.02, "Class 31 base loss degradation must be <=2%")

    fail(failures, mcts.get("prompt_count", 0) >= 16, "Z-MCTS needs >=16 real-model prompts")
    fail(failures, (mcts.get("success_fraction") or 0) >= 0.60, "Z-MCTS must improve the real LM-head objective on >=60% of prompts")
    fail(failures, (mcts.get("median_reference_logprob_improvement") or 0) > 0.0, "Z-MCTS median real-model log-probability improvement must be positive")

    report = {"status": "PASS" if not failures else "FAIL", "failures": failures}
    print(json.dumps(report, indent=2))
    return 0 if not failures else 1


if __name__ == "__main__":
    raise SystemExit(main())
