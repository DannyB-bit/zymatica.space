#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
EVIDENCE_DIR="${1:-}"
TOOLCHAIN="1.98.0"

for tool in git rustup cargo cmake python3; do
  command -v "$tool" >/dev/null || { echo "ERROR: missing mandatory tool: $tool" >&2; exit 1; }
done

rustup toolchain install "$TOOLCHAIN" --profile minimal --component rustfmt --component clippy

echo "[1/11] Toolchain"
rustc +"$TOOLCHAIN" --version --verbose
cargo +"$TOOLCHAIN" --version

echo "[2/11] Formatting"
cargo +"$TOOLCHAIN" fmt --all -- --check

echo "[3/11] Clippy -D warnings"
cargo +"$TOOLCHAIN" clippy --workspace --all-targets --locked -- -D warnings

echo "[4/11] Rust workspace tests"
cargo +"$TOOLCHAIN" test --workspace --locked --verbose

echo "[5/11] Release build"
cargo +"$TOOLCHAIN" build --workspace --release --locked

echo "[6/11] C++20 Z-SPAR"
cmake -S "$ROOT/crates/zymatica-language-u/33_Z_SPAR_Semantic_Parity" -B "$ROOT/build_zspar" -DCMAKE_BUILD_TYPE=Release
cmake --build "$ROOT/build_zspar" --parallel
ctest --test-dir "$ROOT/build_zspar" --output-on-failure

echo "[7/11] Python syntax"
python3 -m compileall -q "$ROOT/tools/ten_out_of_ten" "$ROOT/crates/zymatica-language-u"

echo "[8/11] Numerical helper self-test"
python3 "$ROOT/tools/ten_out_of_ten/real_model_validation.py" self-test

echo "[9/11] Existing algorithmic verification"
python3 "$ROOT/crates/zymatica-language-u/unified_polyglot_pillars/unified_four_pillars_engine.py"
python3 "$ROOT/crates/zymatica-language-u/31_Epigenetic_Weight_Crystallizer/run_proof.py"
python3 "$ROOT/crates/zymatica-language-u/32_8D_Octonion_Hypercube/run_proof.py"
python3 "$ROOT/crates/zymatica-language-u/33_Z_SPAR_Semantic_Parity/tools/reference_vectors.py" > "$ROOT/.golden.tmp.json"
diff -u "$ROOT/crates/zymatica-language-u/33_Z_SPAR_Semantic_Parity/GOLDEN_VECTORS.json" "$ROOT/.golden.tmp.json"
rm -f "$ROOT/.golden.tmp.json"
python3 "$ROOT/crates/zymatica-language-u/34_Z_WORMHOLE_Latent_Transfer/run_proof.py"
python3 "$ROOT/crates/zymatica-language-u/35_Z_MCTS_Latent_Reasoning/run_proof.py"

echo "[10/11] License consistency"
python3 "$ROOT/tools/ten_out_of_ten/license_audit.py" --root "$ROOT"

echo "[11/11] Optional real-model 10.00 evidence gate"
if [[ -n "$EVIDENCE_DIR" ]]; then
  python3 "$ROOT/tools/ten_out_of_ten/acceptance_gate.py" --evidence-dir "$EVIDENCE_DIR"
  python3 "$ROOT/tools/ten_out_of_ten/evidence_manifest.py" "$EVIDENCE_DIR" --repo "$ROOT" --output "$EVIDENCE_DIR/MANIFEST.json"
else
  echo "INFO: no evidence directory supplied; build/reproducibility gates passed, research 10.00 gate not evaluated"
fi

echo "PASS: Zymatica strict verification completed"
