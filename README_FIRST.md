# Zymatica 10.00 Hardening Bundle

**Target base inspected:** `DannyB-bit/zymatica.space` at commit `53c05c6c45a7f4cb9eb4f8221dfc7cec445410fd`.

This bundle is intentionally stricter than “make CI green.” It contains the code I would want to see before rating the repository 10.00: exact Rust 1.98 workspace hardening, stronger production implementations for Classes 29/31/34/35, real-model evidence generation, fail-closed ZK verifier changes, cross-platform CI, evidence/claim/license gates, branch protection, and signed-release tooling.

## What the bundle replaces

Full drop-in Rust source replacements:

- `crates/zymatica-engine/src/hyper_manifold_kv_folding.rs`
  - keeps the legacy LUT type for compatibility
  - adds a real deterministic low-rank f16 KV block representation with measured error and measured compression ratio
- `crates/zymatica-engine/src/epigenetic_manifold_crystallizer.rs`
  - keeps the 70-byte wire format
  - adds checked two-pass MGS, matrix-row update projection, and explicit numerical orthogonality metrics
- `crates/zymatica-engine/src/z_wormhole.rs`
  - keeps the deterministic constructor only as an explicitly untrained compatibility path
  - adds learned bridge JSON loading, strict dimension validation, training fingerprints and real target-reference evaluation
- `crates/zymatica-engine/src/z_mcts.rs`
  - adds a pluggable `LatentEvaluator` so real LM-head/reward/verifier objectives drive MCTS
  - keeps geometric goal search only as a reference evaluator

CI replacement:

- `.github/workflows/production_verification_ci.yml`
  - pins Rust **1.98.0** exactly
  - uses `fail-fast: false`
  - runs fmt, workspace Clippy `-D warnings`, tests, release build, C++ Z-SPAR, Python syntax and existing proof scripts on the matrix
- `.github/workflows/ten_out_of_ten_release_gate.yml`
  - validates committed real-model evidence, research claims, licensing consistency, evidence hashes and RustSec audit

Additional code:

- `tools/ten_out_of_ten/real_model_validation.py`
- `tools/ten_out_of_ten/apply_clippy_1_98_fixes.py`
- `tools/ten_out_of_ten/acceptance_gate.py`
- `tools/ten_out_of_ten/license_audit.py`
- `tools/ten_out_of_ten/claims_audit.py`
- `tools/ten_out_of_ten/evidence_manifest.py`
- `patches/zk_fail_closed_and_mutation_tests.patch`
- branch-protection and signed-release scripts

## Install on your Windows mirror

From outside the repo:

```powershell
python .\zymatica_10_00_bundle\install_bundle.py --repo C:\zymatica-kernel
cd C:\zymatica-kernel
```

Inspect the diff first:

```powershell
git diff --stat
git diff
```

Apply the ZK fail-closed/mutation-test patch:

```powershell
git apply --check patches/zk_fail_closed_and_mutation_tests.patch
git apply patches/zk_fail_closed_and_mutation_tests.patch
```

Then run the exact Rust 1.98 cleanup pass that matches GitHub:

```powershell
python tools/ten_out_of_ten/apply_clippy_1_98_fixes.py --repo . --allow-dirty
```

Then execute the full local gate:

```powershell
.\verify_10_00.ps1
```

Do **not** call the repository 10.00 yet. At this stage the build/reproducibility code should be hardened, but Classes 29/31/34/35 still need committed real-model evidence.

## Generate the real-model evidence

Read `docs/REAL_MODEL_EVIDENCE_README.md`, create non-cherry-picked prompt corpora, then produce:

```text
evidence/10_00/latest/hyper_kv.json
evidence/10_00/latest/wormhole.json
evidence/10_00/latest/epigenetic.json
evidence/10_00/latest/mcts.json
```

Validate them:

```powershell
python tools/ten_out_of_ten/acceptance_gate.py --evidence-dir evidence/10_00/latest
python tools/ten_out_of_ten/evidence_manifest.py evidence/10_00/latest --repo . --output evidence/10_00/latest/MANIFEST.json
```

Then run:

```powershell
.\verify_10_00.ps1 -EvidenceDir evidence/10_00/latest
```

## Protect GitHub after CI is green

```powershell
.\scripts\configure_branch_protection.ps1 -Repository DannyB-bit/zymatica.space -Branch main
```

Optionally enforce signed commits if your GitHub plan/repository settings support it:

```powershell
.\scripts\configure_branch_protection.ps1 -Repository DannyB-bit/zymatica.space -Branch main -RequireSignedCommits
```

Create a signed release only from a clean, verified commit:

```bash
scripts/create_signed_release.sh v1.0.0
```

## Important boundary

A code bundle cannot manufacture a 10.00 rating. The remaining proof is empirical and operational: the exact live commit must go green on GitHub, the real-model evidence must pass the objective gate, strong README claims must be evidence-linked or narrowed, the license/provenance map must be defensible, and branch/release controls must actually be enabled.
