#!/usr/bin/env python3
# Copyright © 2026 Zymatica
# SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
# See LICENSE for terms.
"""Fail-closed license *consistency* audit without guessing third-party ownership.

The audit never auto-relicenses a file.  It fails on contradictory/invalid headers and optionally
requires Covenant headers for paths explicitly listed by the owner in LICENSE_SCOPE.txt.

LICENSE_SCOPE.txt syntax: one repository-relative glob per line, e.g.
    crates/zymatica-engine/src/*.rs
    crates/zymatica-zspar/src/*.rs
Only add a path after confirming Zymatica owns the relevant source rights.
"""

from __future__ import annotations

import argparse
import fnmatch
import json
import subprocess
from pathlib import Path

SOURCE_EXTENSIONS = {
    ".rs", ".py", ".c", ".cc", ".cpp", ".cxx", ".h", ".hh", ".hpp", ".hxx",
    ".go", ".java", ".kt", ".swift", ".js", ".mjs", ".cjs", ".ts", ".tsx", ".jsx",
}
EXCLUDED_PARTS = {
    ".git", "target", "node_modules", "vendor", "third_party", "third-party",
    "dist", "build", ".zymatica_10_00_backup", "zymatica_10_00_bundle", "zymatica-agent-harness",
}
_PREFIX = "SPDX" + "-License-Identifier: "
COVENANT = _PREFIX + "LicenseRef-Zymatica-Covenant-2.0"
KNOWN_SPDX = (
    COVENANT,
    _PREFIX + "MIT",
    _PREFIX + "Apache-2.0",
    _PREFIX + "MIT OR Apache-2.0",
    _PREFIX + "Apache-2.0 OR MIT",
)


def load_lines(path: Path) -> list[str]:
    if not path.exists():
        return []
    return [
        line.strip().replace("\\", "/")
        for line in path.read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    ]


def scoped(rel: str, globs: list[str]) -> bool:
    return any(fnmatch.fnmatch(rel, pattern) for pattern in globs)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=Path.cwd())
    parser.add_argument("--json-output", type=Path)
    args = parser.parse_args()
    root = args.root.resolve()
    covenant_scope = load_lines(root / "LICENSE_SCOPE.txt")
    third_party_allowlist = set(load_lines(root / "THIRD_PARTY_LICENSE_ALLOWLIST.txt"))
    violations: list[dict[str, str]] = []
    warnings: list[dict[str, str]] = []
    checked = 0

    try:
        res = subprocess.run(["git", "ls-files"], cwd=root, capture_output=True, text=True, check=True)
        candidate_paths = [root / p for p in res.stdout.splitlines()]
    except Exception:
        candidate_paths = sorted(root.rglob("*"))

    for path in sorted(candidate_paths, key=lambda p: p.relative_to(root).as_posix()):
        if not path.is_file() or path.suffix.lower() not in SOURCE_EXTENSIONS:
            continue
        rel_path = path.relative_to(root)
        if any(part.lower() in EXCLUDED_PARTS for part in rel_path.parts):
            continue
        rel = rel_path.as_posix()
        try:
            text = path.read_text(encoding="utf-8-sig")
        except UnicodeDecodeError:
            continue
        checked += 1
        head = "\n".join(text.splitlines()[:40])

        if path.suffix.lower() == ".py" and "// SPDX-License-Identifier:" in head:
            violations.append({"path": rel, "reason": "Python file uses invalid // SPDX syntax"})
        if path.suffix.lower() == ".rs" and "# SPDX-License-Identifier:" in head:
            violations.append({"path": rel, "reason": "Rust file uses invalid # SPDX syntax"})

        markers = [marker for marker in KNOWN_SPDX if marker in head]
        has_covenant = COVENANT in head
        has_open = any("MIT" in marker or "Apache-2.0" in marker for marker in markers)
        legacy_apache = "Licensed under Apache License 2.0" in head or "Apache License, Version 2.0" in head

        if has_covenant and (has_open or legacy_apache):
            violations.append({"path": rel, "reason": "file contains both Covenant and MIT/Apache licensing markers"})

        if scoped(rel, covenant_scope) and rel not in third_party_allowlist and not has_covenant:
            violations.append({"path": rel, "reason": "owner-declared Covenant scope is missing Covenant SPDX marker"})

        if not markers and not legacy_apache:
            warnings.append({"path": rel, "reason": "no file-level SPDX marker; repository-level license may still govern"})

    report = {
        "checked_files": checked,
        "covenant_scope_globs": covenant_scope,
        "violations": sorted(violations, key=lambda v: (v["path"], v.get("reason", ""))),
        "warnings": sorted(warnings, key=lambda w: (w["path"], w.get("reason", ""))),
        "status": "PASS" if not violations else "FAIL",
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
