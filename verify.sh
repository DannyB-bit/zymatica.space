#!/usr/bin/env bash
# ==============================================================================
# ZYMATICA MASTER REPRODUCIBILITY & VERIFICATION SUITE
# Author: Danny Bouldiez | Codebase by Devs One
# ==============================================================================
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
echo "================================================================================"
echo " [+] ZYMATICA FULL-STACK REPRODUCIBILITY & VERIFICATION BATTERY"
echo "================================================================================"

# Validate toolchain availability (Fail-Closed Strict Verification)
for tool in cargo cmake; do
    if ! command -v "$tool" >/dev/null 2>&1; then
        echo "ERROR: Mandatory verification tool '$tool' is not installed." >&2
        echo "Strict release verification requires cargo, cmake, and python." >&2
        exit 1
    fi
done

PYTHON_BIN="python3"
if ! command -v python3 >/dev/null 2>&1; then
    if command -v python >/dev/null 2>&1; then
        PYTHON_BIN="python"
    else
        echo "ERROR: Mandatory verification tool 'python3'/'python' is not installed." >&2
        exit 1
    fi
fi

# 1. Rust Formatter Check
echo "[1/6] Verifying Rust formatting..."
cargo fmt --all -- --check
echo "  [PASS] Rustfmt verified."

# 2. Rust Clippy Linters
echo "[2/6] Verifying Rust Clippy lints (-D warnings)..."
cargo clippy --workspace --all-targets -- -D warnings
echo "  [PASS] Clippy verified."

# 3. Native Rust Workspace Tests
echo "[3/6] Running native Rust workspace tests..."
cargo test --workspace --locked --verbose
echo "  [PASS] Rust workspace tests passed."

# 4. C++20 Z-SPAR Native Tests
echo "[4/6] Building and running C++20 Z-SPAR tests..."
cmake -S "$ROOT/crates/zymatica-language-u/33_Z_SPAR_Semantic_Parity" -B "$ROOT/build_zspar" -DCMAKE_BUILD_TYPE=Release
cmake --build "$ROOT/build_zspar" -j
ctest --test-dir "$ROOT/build_zspar" --output-on-failure
echo "  [PASS] C++20 Z-SPAR test suite passed."

# 5. Independent Golden Vectors
echo "[5/6] Validating Z-SPAR cross-language golden vectors..."
$PYTHON_BIN "$ROOT/crates/zymatica-language-u/33_Z_SPAR_Semantic_Parity/tools/reference_vectors.py" > "$ROOT/.golden.tmp.json"
diff -u "$ROOT/crates/zymatica-language-u/33_Z_SPAR_Semantic_Parity/GOLDEN_VECTORS.json" "$ROOT/.golden.tmp.json"
rm -f "$ROOT/.golden.tmp.json"
echo "  [PASS] Golden test vectors match 100%."

# 6. Python Algorithmic Proofs (Classes 28-35)
echo "[6/6] Executing multi-class algorithmic verification proofs..."
$PYTHON_BIN "$ROOT/crates/zymatica-language-u/unified_polyglot_pillars/unified_four_pillars_engine.py"
$PYTHON_BIN "$ROOT/crates/zymatica-language-u/31_Epigenetic_Weight_Crystallizer/run_proof.py"
$PYTHON_BIN "$ROOT/crates/zymatica-language-u/32_8D_Octonion_Hypercube/run_proof.py"
$PYTHON_BIN "$ROOT/crates/zymatica-language-u/34_Z_WORMHOLE_Latent_Transfer/run_proof.py"
$PYTHON_BIN "$ROOT/crates/zymatica-language-u/35_Z_MCTS_Latent_Reasoning/run_proof.py"
echo "  [PASS] Algorithmic proofs verified."

echo "================================================================================"
echo " [SUCCESS] ALL REPRODUCIBILITY & EVIDENCE GATES PASSED (100% GREEN)"
echo "================================================================================"
