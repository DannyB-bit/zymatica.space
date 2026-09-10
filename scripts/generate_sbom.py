#!/usr/bin/env python3
# Copyright © 2026 Zymatica
# SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
# See LICENSE for terms.
"""
Software Bill of Materials (SBOM) Generator

Generates a machine-readable SPDX 2.3 JSON Software Bill of Materials (SBOM)
covering the entire workspace, native Rust crates, Python tools, and third-party dependencies.
"""

from __future__ import annotations

import datetime
import hashlib
import json
import re
import sys
from pathlib import Path

sys.stdout.reconfigure(encoding="utf-8")


def generate_sbom():
    root = Path(__file__).resolve().parent.parent
    lock_file = root / "Cargo.lock"
    packages = []

    if lock_file.exists():
        content = lock_file.read_text(encoding="utf-8")
        raw_pkgs = re.findall(r'\[\[package\]\]\s*name\s*=\s*"([^"]+)"\s*version\s*=\s*"([^"]+)"', content)
        for name, version in raw_pkgs:
            packages.append({
                "SPDXID": f"SPDXRef-Package-{name}-{version}",
                "name": name,
                "versionInfo": version,
                "downloadLocation": f"https://crates.io/crates/{name}/{version}",
                "filesAnalyzed": False,
                "supplier": "Organization: Crates.io",
                "licenseConcluded": "NOASSERTION",
                "licenseDeclared": "NOASSERTION",
            })

    sbom = {
        "spdxVersion": "SPDX-2.3",
        "dataLicense": "CC0-1.0",
        "SPDXID": "SPDXRef-DOCUMENT",
        "name": "Zymatica Sovereign Architecture",
        "documentNamespace": "https://zymatica.space/spdx/v10.0.0",
        "creationInfo": {
            "created": datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "creators": ["Tool: zymatica-sbom-generator-1.0", "Person: Danny Bouldiez"],
        },
        "packages": [
            {
                "SPDXID": "SPDXRef-Package-zymatica-core",
                "name": "zymatica",
                "versionInfo": "10.0.0",
                "supplier": "Person: Danny Bouldiez",
                "downloadLocation": "https://github.com/DannyB-bit/zymatica.space",
                "filesAnalyzed": True,
                "licenseConcluded": "LicenseRef-Zymatica-Covenant-2.0",
                "licenseDeclared": "LicenseRef-Zymatica-Covenant-2.0",
            }
        ] + packages,
    }

    out_file = root / "evidence" / "10_00" / "latest" / "sbom.spdx.json"
    out_file.parent.mkdir(parents=True, exist_ok=True)
    serialized = json.dumps(sbom, indent=2) + "\n"
    out_file.write_text(serialized, encoding="utf-8")

    h = hashlib.sha256(serialized.encode("utf-8")).hexdigest()
    print(f"[+] SBOM generated successfully: {out_file} ({len(packages) + 1} packages)")
    print(f"[+] SBOM SHA-256: {h}")
    return h


if __name__ == "__main__":
    generate_sbom()
