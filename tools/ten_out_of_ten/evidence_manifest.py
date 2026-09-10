#!/usr/bin/env python3
# Copyright © 2026 Zymatica
# SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
# See LICENSE for terms.
"""Create a cryptographic manifest for a Zymatica evidence directory with source tree binding."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path


def sha256_file(path: Path) -> str:
    raw = path.read_bytes()
    if path.suffix in {".json", ".txt", ".md", ".csv", ".spdx", ".lean"} or path.name == "SHA256SUMS":
        raw = raw.replace(b"\r\n", b"\n")
    return hashlib.sha256(raw).hexdigest()


def compute_source_tree_hash(root: Path) -> str:
    """Compute deterministic SHA-256 tree hash over tracked versioned source files using git ls-files."""
    h = hashlib.sha256()
    try:
        res = subprocess.run(["git", "ls-files"], cwd=root, capture_output=True, text=True, check=True)
        tracked_files = res.stdout.splitlines()
    except Exception:
        # Fallback if git not available
        tracked_files = []

    skip_prefixes = ("evidence/", "target/", "node_modules/", "build/", ".git/")
    for rel_str in sorted(tracked_files):
        if rel_str.startswith(skip_prefixes):
            continue
        p = root / rel_str
        if p.is_file():
            h.update(rel_str.encode("utf-8"))
            h.update(sha256_file(p).encode("utf-8"))
    return h.hexdigest()


def command_output(command: list[str], cwd: Path) -> str | None:
    env = os.environ.copy()
    cargo_bin = os.path.expanduser("~/.cargo/bin")
    if os.path.isdir(cargo_bin) and cargo_bin not in env.get("PATH", ""):
        env["PATH"] = f"{cargo_bin}{os.pathsep}{env.get('PATH', '')}"
    cmd = list(command)
    exe = shutil.which(cmd[0], path=env.get("PATH"))
    if exe:
        cmd[0] = exe
    try:
        result = subprocess.run(cmd, cwd=cwd, env=env, text=True, capture_output=True, check=True)
        return result.stdout.strip()
    except Exception:
        return None


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("evidence_dir", type=Path)
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    repo = args.repo.resolve()
    evidence_dir = args.evidence_dir.resolve()
    if not evidence_dir.is_dir():
        raise SystemExit(f"not a directory: {evidence_dir}")

    files = []
    for path in sorted(p for p in evidence_dir.rglob("*") if p.is_file()):
        if args.output and path.resolve() == args.output.resolve():
            continue
        if path.name in {"MANIFEST.json", "SHA256SUMS", "release_attestation.json"}:
            continue
        files.append(
            {
                "path": path.relative_to(evidence_dir).as_posix(),
                "size": path.stat().st_size,
                "sha256": sha256_file(path),
            }
        )

    source_tree_sha = compute_source_tree_hash(repo)
    git_head = command_output(["git", "rev-parse", "HEAD"], repo)
    git_tree = command_output(["git", "rev-parse", "HEAD^{tree}"], repo)

    payload = {
        "schema": "zymatica.evidence-manifest.v2",
        "created_utc": datetime.now(timezone.utc).isoformat(),
        "source_commit_sha": git_head,
        "source_git_tree_sha": git_tree,
        "source_tree_sha256": source_tree_sha,
        "git_head": git_head,
        "git_tree": git_tree,
        "git_status_porcelain": command_output(["git", "status", "--porcelain"], repo),
        "rustc": command_output(["rustc", "--version"], repo),
        "cargo": command_output(["cargo", "--version"], repo),
        "python": sys.version,
        "platform": platform.platform(),
        "files": files,
    }
    output = args.output or (evidence_dir / "MANIFEST.json")
    with open(output, "w", encoding="utf-8", newline="\n") as f:
        f.write(json.dumps(payload, indent=2, sort_keys=True) + "\n")
    print(output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
