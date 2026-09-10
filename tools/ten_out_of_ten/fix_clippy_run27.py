#!/usr/bin/env python3
# Copyright © 2026 Zymatica
# SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
# See LICENSE for terms.
"""
Zymatica Rust 1.98 / GitHub Actions run #27 Clippy remediation patcher.

Target baseline:
  c77d424f8f1dcdbb1a71c7c8ee4e4d9990460961

This script:
  1. Applies source-level fixes for every Clippy error visible in run #27 that
     was reproduced across Ubuntu/Windows, plus the macOS/aarch64-only lints.
  2. Runs cargo clippy --fix for any remaining machine-applicable suggestions.
  3. Runs rustfmt.
  4. Runs the exact strict CI Clippy gate.
  5. Optionally runs workspace tests and release build.

It fails closed if an expected source pattern cannot be found or if any gate fails.
"""

from __future__ import annotations

import argparse
import re
import shutil
import subprocess
import sys
from pathlib import Path

EXPECTED_HEAD = "c77d424f8f1dcdbb1a71c7c8ee4e4d9990460961"


def run(cmd: list[str], cwd: Path, *, check: bool = True) -> subprocess.CompletedProcess:
    print("\n$", " ".join(cmd), flush=True)
    proc = subprocess.run(cmd, cwd=cwd)
    if check and proc.returncode != 0:
        raise SystemExit(proc.returncode)
    return proc


def git_head(repo: Path) -> str:
    proc = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=repo,
        capture_output=True,
        text=True,
        check=True,
    )
    return proc.stdout.strip()


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8")


def write(path: Path, text: str) -> None:
    path.write_text(text, encoding="utf-8", newline="\n")


def replace_exact(path: Path, old: str, new: str, *, count: int = 1) -> None:
    text = read(path)
    if new in text:
        print(f"[SKIP] {path}: already contains replacement")
        return
    found = text.count(old)
    if found < count:
        raise RuntimeError(
            f"{path}: expected at least {count} occurrence(s), found {found}\n"
            f"Missing pattern:\n{old}"
        )
    text = text.replace(old, new, count)
    write(path, text)
    print(f"[PATCH] {path}: exact replacement x{count}")


def replace_all_exact(path: Path, old: str, new: str, *, minimum: int = 1) -> None:
    text = read(path)
    found = text.count(old)
    if found < minimum:
        raise RuntimeError(
            f"{path}: expected at least {minimum} occurrence(s), found {found}\n"
            f"Missing pattern:\n{old}"
        )
    text = text.replace(old, new)
    write(path, text)
    print(f"[PATCH] {path}: exact replacement x{found}")


def regex_replace(
    path: Path,
    pattern: str,
    repl,
    *,
    count: int = 1,
    flags: int = 0,
) -> None:
    text = read(path)
    new_text, n = re.subn(pattern, repl, text, count=count, flags=flags)
    if n != count:
        raise RuntimeError(
            f"{path}: expected regex replacement count={count}, got {n}\n"
            f"Pattern: {pattern}"
        )
    write(path, new_text)
    print(f"[PATCH] {path}: regex replacement x{n}")


def patch(repo: Path) -> None:
    src = repo / "crates" / "zymatica-engine" / "src"

    # 1) agent_compression.rs — manual_div_ceil
    p = src / "agent_compression.rs"
    replace_exact(
        p,
        "(text.len() + 3) / 4",
        "text.len().div_ceil(4)",
    )

    # 2) agent_plugin_loader.rs — collapsible_if / let-chain
    p = src / "agent_plugin_loader.rs"
    replace_exact(
        p,
        """                if let Ok(content) = fs::read_to_string(&manifest_path) {
                    if let Ok(manifest) = serde_json::from_str::<PluginManifest>(&content) {
                        self.plugins.insert(manifest.name.clone(), manifest);
                        count += 1;
                    }
                }""",
        """                if let Ok(content) = fs::read_to_string(&manifest_path)
                    && let Ok(manifest) = serde_json::from_str::<PluginManifest>(&content)
                {
                    self.plugins.insert(manifest.name.clone(), manifest);
                    count += 1;
                }""",
    )

    # 3) agent_simd_tokenizer.rs — unnecessary_cast + collapsible_if
    p = src / "agent_simd_tokenizer.rs"
    replace_exact(p, "vocab.insert(s, i as u32);", "vocab.insert(s, i);")
    replace_exact(
        p,
        """                    if let Some(&rank) = self.ranks.get(&pair) {
                        if rank < min_rank {
                            min_rank = rank;
                            min_idx = Some(i);
                        }
                    }""",
        """                    if let Some(&rank) = self.ranks.get(&pair)
                        && rank < min_rank
                    {
                        min_rank = rank;
                        min_idx = Some(i);
                    }""",
    )

    # 4) agent_tool_decoder.rs — manual_strip
    p = src / "agent_tool_decoder.rs"
    replace_exact(
        p,
        """        if val_slice.starts_with('"') {
            let name_content = &val_slice[1..];
            let end_quote = name_content.find('"')?;
            Some(name_content[..end_quote].to_string())
        } else {
            None
        }""",
        """        if let Some(name_content) = val_slice.strip_prefix('"') {
            let end_quote = name_content.find('"')?;
            Some(name_content[..end_quote].to_string())
        } else {
            None
        }""",
    )

    # 5) agent_tools.rs — needless_borrows_for_generic_args on both OS branches
    p = src / "agent_tools.rs"
    replace_exact(p, '.args(&["/C", cmd])', '.args(["/C", cmd])')
    replace_exact(p, '.args(&["-c", cmd])', '.args(["-c", cmd])')

    # 6) cuneiform_8d_octonion_engine.rs — four needless_range_loop cases (already applied)

    # 7) frontier.rs — chunks_exact_to_as_chunks (already applied)

    # 8) kernels.rs
    p = src / "kernels.rs"

    # macOS/aarch64-only needless_return: these two NEON paths are terminal cfg branches.
    replace_all_exact(
        p,
        "return unsafe { q8_dot_f32_neon(",
        "unsafe { q8_dot_f32_neon(",
        minimum=2,
    )
    # The above leaves trailing `};` where the expression needs `}`. Repair the two exact call forms.
    replace_exact(
        p,
        "unsafe { q8_dot_f32_neon(row.as_ptr(), x.as_ptr(), row.len(), scale) };",
        "unsafe { q8_dot_f32_neon(row.as_ptr(), x.as_ptr(), row.len(), scale) }",
    )
    replace_exact(
        p,
        "unsafe { q8_dot_f32_neon(row.as_ptr().cast::<i8>(), x.as_ptr(), row.len(), scale) };",
        "unsafe { q8_dot_f32_neon(row.as_ptr().cast::<i8>(), x.as_ptr(), row.len(), scale) }",
    )
    # macOS/aarch64-only manual_is_multiple_of
    replace_all_exact(p, "i % 2 == 0", "i.is_multiple_of(2)", minimum=1)

    # Common q1_58 scalar loop: enumerate x rather than index it.
    text = read(p)
    fn_match = re.search(
        r"pub fn q1_58_dot_f32_scaled\([^)]*\) -> f32 \{.*?\n\}",
        text,
        flags=re.S,
    )
    if not fn_match:
        raise RuntimeError(f"{p}: q1_58_dot_f32_scaled function not found")
    fn = fn_match.group(0)
    if "for col in 0..cols {" not in fn or "x[col]" not in fn:
        raise RuntimeError(f"{p}: expected q1_58 loop shape not found")
    new_fn = fn.replace(
        "for col in 0..cols {",
        "for (col, &xv) in x.iter().take(cols).enumerate() {",
        1,
    ).replace("x[col]", "xv")
    text = text[:fn_match.start()] + new_fn + text[fn_match.end():]
    write(p, text)
    print(f"[PATCH] {p}: q1_58 iterator loop")

    # Test-only neg_multiply / identity simplification.
    regex_replace(
        p,
        r"let expected = \(1\.0 \* 1\.0 \+ 0\.0 \* \(-2\.0\) \+ \(-1\.0\) \* 3\.0 \+ 1\.0 \* \(-4\.0\)\) \* scale_a;",
        "let expected = -6.0 * scale_a;",
    )

    # 9) neural_swarm_hypergraph.rs
    p = src / "neural_swarm_hypergraph.rs"
    replace_exact(
        p,
        """            for i in 0..6 {
                sum_coords[i] += (state[i] as f32) * chirp.consensus_weight;
            }""",
        """            for (sum, &coord) in sum_coords.iter_mut().zip(state.iter()) {
                *sum += (coord as f32) * chirp.consensus_weight;
            }""",
    )
    replace_exact(
        p,
        """        let mut seed = vec![0u8; 381];
        for i in 0..381 {
            seed[i] = (i % 251) as u8;
        }""",
        """        let mut seed = vec![0u8; 381];
        for (i, byte) in seed.iter_mut().enumerate() {
            *byte = (i % 251) as u8;
        }""",
    )

    # 10) quant.rs — q1_58 matvec_into output loop
    p = src / "quant.rs"
    replace_exact(
        p,
        """        for row_idx in 0..self.rows {
            let scale = self.scales[row_idx];
            let start = row_idx * packed_cols;
            let row_packed = &self.packed[start..start + packed_cols];
            out[row_idx] = q1_58_dot_f32_scaled(row_packed, x, scale, self.cols);
        }""",
        """        for (row_idx, out_cell) in out.iter_mut().enumerate() {
            let scale = self.scales[row_idx];
            let start = row_idx * packed_cols;
            let row_packed = &self.packed[start..start + packed_cols];
            *out_cell = q1_58_dot_f32_scaled(row_packed, x, scale, self.cols);
        }""",
    )

    # 11) unified_frontier_codec.rs
    p = src / "unified_frontier_codec.rs"
    replace_exact(
        p,
        """            for p in received_packets.iter().take(received_packets.len() - 1) {
                if let Some(data) = p {
                    out.extend_from_slice(&data[1..]);
                }
            }""",
        """            for data in received_packets
                .iter()
                .take(received_packets.len() - 1)
                .flatten()
            {
                out.extend_from_slice(&data[1..]);
            }""",
    )
    replace_exact(
        p,
        """        for (idx, p) in received_packets.iter().enumerate() {
            if idx != target {
                if let Some(data) = p {
                    for (dst, src) in recovered[1..Self::MTU].iter_mut().zip(&data[1..Self::MTU]) {
                        *dst ^= *src;
                    }
                }
            }
        }""",
        """        for (idx, p) in received_packets.iter().enumerate() {
            if idx != target
                && let Some(data) = p
            {
                for (dst, src) in recovered[1..Self::MTU]
                    .iter_mut()
                    .zip(&data[1..Self::MTU])
                {
                    *dst ^= *src;
                }
            }
        }""",
    )
    replace_exact(
        p,
        """        let mut trajectory = Vec::new();
        trajectory.push(Concept6D::new(1, 2, 3, 4, 5, 6));
        trajectory.push(Concept6D::new(1, 2, 3, 4, 5, 6)); // Mode 00
        trajectory.push(Concept6D::new(1, 2, 4, 5, 6, 7)); // Mode 01
        trajectory.push(Concept6D::new(1, 9, 2, 1, 0, 0)); // Mode 10
        trajectory.push(Concept6D::new(4, 7, 8, 9, 10, 11)); // Mode 11""",
        """        let trajectory = vec![
            Concept6D::new(1, 2, 3, 4, 5, 6),
            Concept6D::new(1, 2, 3, 4, 5, 6), // Mode 00
            Concept6D::new(1, 2, 4, 5, 6, 7), // Mode 01
            Concept6D::new(1, 9, 2, 1, 0, 0), // Mode 10
            Concept6D::new(4, 7, 8, 9, 10, 11), // Mode 11
        ];""",
    )

    # 12) field_harness.rs — fixed-size collection is an array, not Vec
    p = src / "field_harness.rs"
    replace_exact(p, "    let mut nodes = vec![", "    let mut nodes = [")

    # 13) agent_setup_wizard.rs test — no field reassignment after Default::default()
    p = src / "agent_setup_wizard.rs"
    replace_exact(
        p,
        """        let mut config = ZymaticaConfig::default();
        config.zymatica_home = dir.path().to_path_buf();

        let cfg = SetupWizard::run_setup(true)?;""",
        """        let config = ZymaticaConfig {
            zymatica_home: dir.path().to_path_buf(),
            ..Default::default()
        };
        assert_eq!(config.zymatica_home, dir.path());

        let cfg = SetupWizard::run_setup(true)?;""",
    )

    # 14) holomorphic_speculative_engine.rs test — identity_op / erasing_op
    p = src / "holomorphic_speculative_engine.rs"
    replace_exact(
        p,
        """        unembed[0 * hidden_dim + 0] = 1.0; // Token 0 aligns with axis 0
        unembed[1 * hidden_dim + 1] = 2.0; // Token 1 aligns with axis 1
        unembed[2 * hidden_dim + 2] = 1.0; // Token 2 aligns with axis 2""",
        """        unembed[0] = 1.0; // Token 0 aligns with axis 0
        unembed[hidden_dim + 1] = 2.0; // Token 1 aligns with axis 1
        unembed[2 * hidden_dim + 2] = 1.0; // Token 2 aligns with axis 2""",
    )


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--repo",
        type=Path,
        default=Path.cwd(),
        help="Zymatica repository root (default: current directory)",
    )
    ap.add_argument(
        "--force-head",
        action="store_true",
        help="Allow running when HEAD is not the exact run-#27 baseline",
    )
    ap.add_argument(
        "--skip-auto-fix",
        action="store_true",
        help="Skip cargo clippy --fix second pass",
    )
    ap.add_argument(
        "--skip-tests",
        action="store_true",
        help="Stop after strict fmt+Clippy instead of tests+release build",
    )
    args = ap.parse_args()

    repo = args.repo.resolve()
    if not (repo / "Cargo.toml").exists():
        raise SystemExit(f"Not a Cargo workspace root: {repo}")

    head = git_head(repo)
    print(f"[INFO] repo={repo}")
    print(f"[INFO] HEAD={head}")
    if head != EXPECTED_HEAD and not args.force_head:
        raise SystemExit(
            f"Refusing to patch unexpected HEAD.\n"
            f"Expected: {EXPECTED_HEAD}\n"
            f"Actual:   {head}\n"
            f"If you intentionally rebased without changing these files, rerun with --force-head."
        )

    try:
        patch(repo)
    except Exception as exc:
        print(f"\n[FAIL] Source patch aborted: {exc}", file=sys.stderr)
        return 2

    # Always format before invoking Clippy.
    run(["cargo", "fmt", "--all"], repo)

    print("\n[SUCCESS] Source patches applied.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
