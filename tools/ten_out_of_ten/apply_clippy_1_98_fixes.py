#!/usr/bin/env python3
# Copyright © 2026 Zymatica
# SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
# See LICENSE for terms.
"""Deterministic Rust 1.98 Clippy hardening driver.

Run from the repository root *after* copying the replacement Class 29/31/34/35 modules from this
bundle.  The script uses the same pinned compiler as the proposed CI, applies Clippy's own
machine-applicable rewrites, adds the small number of intentionally explicit API lint annotations /
Default impls, formats the workspace, and finally requires a zero-warning `-D warnings` pass.

It never suppresses a lint crate-wide.
"""

from __future__ import annotations

import argparse
import os
import subprocess
import sys
from pathlib import Path

TOOLCHAIN = "1.98.0"

DEFAULT_TYPES = {
    "crates/zymatica-engine/src/agent_cron.rs": "CronSchedulerEngine",
    "crates/zymatica-engine/src/agent_gateway.rs": "GatewayEngine",
    "crates/zymatica-engine/src/agent_memory.rs": "MemoryStore",
    "crates/zymatica-engine/src/agent_plugin_loader.rs": "PluginLoader",
    "crates/zymatica-engine/src/agent_skills.rs": "SkillStore",
    "crates/zymatica-engine/src/agent_tools.rs": "ToolRegistry",
    "crates/zymatica-engine/src/agent_voice.rs": "VoiceEngine",
}


def run(command: list[str], cwd: Path, check: bool = True) -> subprocess.CompletedProcess[str]:
    print("+", " ".join(command), flush=True)
    return subprocess.run(command, cwd=cwd, text=True, check=check)


def ensure_default_impl(path: Path, type_name: str) -> bool:
    text = path.read_text(encoding="utf-8")
    if f"impl Default for {type_name}" in text:
        return False
    marker = f"impl {type_name} {{"
    if marker not in text:
        raise RuntimeError(f"cannot locate {marker!r} in {path}")
    default_impl = (
        f"impl Default for {type_name} {{\n"
        "    fn default() -> Self {\n"
        "        Self::new()\n"
        "    }\n"
        "}\n\n"
    )
    path.write_text(text.replace(marker, default_impl + marker, 1), encoding="utf-8")
    return True


def ensure_too_many_args_allow(path: Path) -> bool:
    text = path.read_text(encoding="utf-8")
    needle = "    pub const fn new(\n"
    if needle not in text:
        needle = "    pub const fn new("
    if needle not in text:
        raise RuntimeError(f"cannot locate embedded 8D constructor in {path}")
    prefix = "    #[allow(clippy::too_many_arguments)]\n"
    index = text.index(needle)
    if text[max(0, index - len(prefix)) : index] == prefix:
        return False
    text = text[:index] + prefix + text[index:]
    path.write_text(text, encoding="utf-8")
    return True


def assert_clean_start(repo: Path, allow_dirty: bool) -> None:
    if allow_dirty:
        return
    result = subprocess.run(
        ["git", "status", "--porcelain"], cwd=repo, text=True, capture_output=True, check=True
    )
    if result.stdout.strip():
        raise RuntimeError(
            "working tree is not clean. Commit/stash current work, or pass --allow-dirty intentionally"
        )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--repo", type=Path, default=Path.cwd())
    parser.add_argument("--allow-dirty", action="store_true")
    parser.add_argument("--skip-rustup-install", action="store_true")
    args = parser.parse_args()
    repo = args.repo.resolve()
    if not (repo / "Cargo.toml").exists():
        raise RuntimeError(f"{repo} does not look like the Zymatica repository root")

    assert_clean_start(repo, args.allow_dirty)
    if not args.skip_rustup_install:
        run(
            [
                "rustup",
                "toolchain",
                "install",
                TOOLCHAIN,
                "--profile",
                "minimal",
                "--component",
                "rustfmt",
                "--component",
                "clippy",
            ],
            repo,
        )

    # First pass: let Clippy make only its machine-applicable source-preserving suggestions.
    run(
        [
            "cargo",
            f"+{TOOLCHAIN}",
            "clippy",
            "--workspace",
            "--all-targets",
            "--fix",
            "--allow-dirty",
            "--allow-staged",
        ],
        repo,
        check=False,
    )

    changed: list[str] = []
    for relative, type_name in DEFAULT_TYPES.items():
        path = repo / relative
        if path.exists() and ensure_default_impl(path, type_name):
            changed.append(relative)

    embedded = repo / "crates/zymatica-engine/src/no_std_embedded_8d.rs"
    if embedded.exists() and ensure_too_many_args_allow(embedded):
        changed.append(str(embedded.relative_to(repo)))

    print(f"manual hardening touched {len(changed)} file(s)")
    for item in changed:
        print("  ", item)

    # Second fix pass catches lints exposed by the newly generated code and all test targets.
    run(
        [
            "cargo",
            f"+{TOOLCHAIN}",
            "clippy",
            "--workspace",
            "--all-targets",
            "--fix",
            "--allow-dirty",
            "--allow-staged",
        ],
        repo,
        check=False,
    )
    run(["cargo", f"+{TOOLCHAIN}", "fmt", "--all"], repo)

    # Final gate is fail-closed. If any lint remains, the script exits non-zero and no claim of
    # completion is printed.
    run(
        [
            "cargo",
            f"+{TOOLCHAIN}",
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
        repo,
    )
    print("PASS: Rust 1.98.0 workspace Clippy is clean under -D warnings")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:
        print(f"ERROR: {exc}", file=sys.stderr)
        raise SystemExit(1)
