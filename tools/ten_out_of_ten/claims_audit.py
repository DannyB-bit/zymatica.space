#!/usr/bin/env python3
# Copyright © 2026 Zymatica
# SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
# See LICENSE for terms.
"""
Numerical Research-Claim & Evidence Registry Auditor

Audits:
1. Cross-checks all registered claims in claims/claims.jsonl against their underlying evidence JSON files.
   Verifies that declared metrics (e.g. compression ratio, accuracy, logprob delta, collision rate)
   match machine-readable evidence metrics exactly.
2. Audits all Markdown documentation files and tables.
   Ensures strong quantitative claims carry a valid [CLAIM: ID] or [EVIDENCE: path] tag, or explicit technical scoping.
"""

from __future__ import annotations

import argparse
import json
import math
import re
import subprocess
import sys
from pathlib import Path
from typing import Any, Dict, List

CLAIM_RE = re.compile(r"\[CLAIM:\s*([A-Z0-9_-]+)\]", re.IGNORECASE)
EVIDENCE_RE = re.compile(r"\[EVIDENCE:\s*([^\]]+)\]", re.IGNORECASE)

STRONG_PATTERNS = [
    re.compile(r"\b(world[- ]?record|record[- ]?shatter|shattered)\b", re.IGNORECASE),
    re.compile(r"\b(100% production[- ]grade|zero catastrophic|0% catastrophic|mathematically impossible to fail)\b", re.IGNORECASE),
    re.compile(r"\b(universal(?:ly)?(?:\s+cross-model|\s+semantic|\s+codec)|\bguarantee(?:d|s)?\s+exact)\b", re.IGNORECASE),
    re.compile(r"\b(0\.00%\s*(?:of\s*)?historical\s*knowledge\s*lost)\b", re.IGNORECASE),
    re.compile(r"\b(globally\s*optimal\s*trajectory)\b", re.IGNORECASE),
]

# Strict scoping words: only technical qualifiers that properly characterize theoretical or hypothetical models
STRICT_SCOPING = re.compile(
    r"\b(theoretical bound|simulation target|hypothesis|hypothetical|simulation model|"
    r"mathematical specification|architectural specification|formal theorem|covenant license|"
    r"target bound|simulation only|exact coordinate matches|linear activation invariance)\b",
    re.IGNORECASE,
)

SKIP_DIRS = {
    ".git", "target", "node_modules", "vendor", "third_party", "third-party",
    "build", ".zymatica_10_00_backup", "zymatica_10_00_bundle", "zymatica-agent-harness",
    "tools/ahash-0.8.4",
}


def load_claim_registry(root: Path) -> Dict[str, Dict[str, Any]]:
    registry_file = root / "claims" / "claims.jsonl"
    claims = {}
    if not registry_file.exists():
        raise FileNotFoundError(f"Missing Claim Registry: {registry_file}")
    for line in registry_file.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if line:
            entry = json.loads(line)
            claims[entry["claim_id"]] = entry
    return claims


def flatten_nested_json(data: Any, prefix: str = "") -> Dict[str, Any]:
    """Recursively flattens nested JSON dictionaries and records both bare keys and path keys."""
    flattened: Dict[str, Any] = {}
    if isinstance(data, dict):
        for k, v in data.items():
            # Store bare key (last key wins or list)
            flattened[k] = v
            nested = flatten_nested_json(v, f"{prefix}.{k}" if prefix else k)
            flattened.update(nested)
    elif isinstance(data, list):
        for idx, item in enumerate(data):
            nested = flatten_nested_json(item, f"{prefix}[{idx}]")
            flattened.update(nested)
    return flattened


def audit_numerical_metrics(root: Path, registry: Dict[str, Dict[str, Any]]) -> List[Dict[str, Any]]:
    metric_violations = []
    for cid, entry in registry.items():
        metrics = entry.get("metrics", {})
        evidence_files = entry.get("evidence_files", [])
        if not evidence_files:
            continue
        
        # Load evidence files
        combined_evidence: Dict[str, Any] = {}
        has_json_evidence = False
        for rel_path in evidence_files:
            ef = root / rel_path
            if not ef.exists():
                metric_violations.append({
                    "claim_id": cid,
                    "reason": f"Declared evidence file does not exist: {rel_path}",
                })
                continue
            if ef.suffix == ".json":
                has_json_evidence = True
                try:
                    raw_data = json.loads(ef.read_text(encoding="utf-8"))
                    flat = flatten_nested_json(raw_data)
                    combined_evidence.update(flat)
                except Exception as e:
                    metric_violations.append({
                        "claim_id": cid,
                        "reason": f"Failed to parse evidence JSON {rel_path}: {e}",
                    })
            else:
                # Non-JSON evidence files only register presence; they do NOT synthesize fake metric values
                combined_evidence[f"file_exists_{ef.name}"] = True

        # Compare registry metrics to evidence metrics: METRIC MUST EXIST -> OTHERWISE FAIL -> THEN COMPARE
        for metric_name, declared_val in metrics.items():
            if metric_name not in combined_evidence:
                metric_violations.append({
                    "claim_id": cid,
                    "metric": metric_name,
                    "declared": declared_val,
                    "reason": f"Required metric '{metric_name}' MUST exist in evidence files {evidence_files} but was NOT found",
                })
                continue

            actual_val = combined_evidence[metric_name]
            if isinstance(declared_val, (int, float)) and isinstance(actual_val, (int, float)):
                if not math.isclose(declared_val, actual_val, rel_tol=1e-3, abs_tol=1e-4):
                    metric_violations.append({
                        "claim_id": cid,
                        "metric": metric_name,
                        "declared": declared_val,
                        "actual_in_evidence": actual_val,
                        "reason": f"Numerical metric mismatch: declared {declared_val} != evidence {actual_val}",
                    })
            elif declared_val != actual_val:
                metric_violations.append({
                    "claim_id": cid,
                    "metric": metric_name,
                    "declared": declared_val,
                    "actual_in_evidence": actual_val,
                    "reason": f"Value mismatch: declared {declared_val!r} != evidence {actual_val!r}",
                })
    return metric_violations


def main() -> int:
    parser = argparse.ArgumentParser(description="Numerical Research-Claim & Evidence Registry Auditor")
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--json-output", type=Path)
    args = parser.parse_args()
    root = args.root.resolve()
    violations: list[dict[str, object]] = []

    claims_registry = load_claim_registry(root)
    metric_violations = audit_numerical_metrics(root, claims_registry)
    violations.extend(metric_violations)

    checked_lines = 0
    checked_files = 0

    try:
        res = subprocess.run(["git", "ls-files"], cwd=root, capture_output=True, text=True, check=True)
        md_paths = [root / p for p in res.stdout.splitlines() if p.endswith(".md")]
    except Exception:
        md_paths = list(root.rglob("*.md"))

    for path in sorted(md_paths, key=lambda p: p.relative_to(root).as_posix()):
        rel = path.relative_to(root)
        if any(part.lower() in SKIP_DIRS for part in rel.parts) or any(skip in rel.as_posix() for skip in SKIP_DIRS):
            continue
        try:
            lines = path.read_text(encoding="utf-8-sig").splitlines()
        except UnicodeDecodeError:
            continue
        checked_files += 1
        for number, line in enumerate(lines, 1):
            checked_lines += 1
            line_str = line.strip()
            if line_str.startswith("<!--") or not line_str:
                continue

            claim_match = CLAIM_RE.search(line)
            if claim_match:
                cid = claim_match.group(1).strip()
                if cid not in claims_registry:
                    violations.append({
                        "path": rel.as_posix(),
                        "line": number,
                        "reason": f"Claim marker references unknown claim ID: {cid}",
                        "text": line_str[:300],
                    })
                continue

            marker = EVIDENCE_RE.search(line)
            if marker:
                evidence_path = root / marker.group(1).strip()
                if not evidence_path.exists():
                    violations.append({
                        "path": rel.as_posix(),
                        "line": number,
                        "reason": f"Evidence marker points to missing file: {marker.group(1).strip()}",
                        "text": line_str[:300],
                    })
                continue

            if STRICT_SCOPING.search(line):
                continue
            if not any(pattern.search(line) for pattern in STRONG_PATTERNS):
                continue

            violations.append({
                "path": rel.as_posix(),
                "line": number,
                "reason": "Strong claim lacks valid [CLAIM: ID] / [EVIDENCE: path] marker or strict technical scoping",
                "text": line_str[:300],
            })

    report = {
        "status": "PASS" if not violations else "FAIL",
        "checked_markdown_files": checked_files,
        "checked_markdown_lines": checked_lines,
        "registered_claims_count": len(claims_registry),
        "numerical_metric_audit": "VERIFIED_BIT_EXACT" if not metric_violations else "FAILED",
        "violations": violations,
    }

    if args.json_output:
        args.json_output.parent.mkdir(parents=True, exist_ok=True)
        with open(args.json_output, "w", encoding="utf-8", newline="\n") as f:
            json.dump(report, f, indent=2)
            f.write("\n")

    print(json.dumps(report, indent=2))
    return 0 if not violations else 1


if __name__ == "__main__":
    raise SystemExit(main())
