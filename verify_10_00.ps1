param(
    [string]$EvidenceDir = ""
)
$ErrorActionPreference = "Stop"
$Toolchain = "1.98.0"

foreach ($tool in @("git", "rustup", "cargo", "cmake", "python")) {
    if (-not (Get-Command $tool -ErrorAction SilentlyContinue)) {
        throw "Missing mandatory tool: $tool"
    }
}

rustup toolchain install $Toolchain --profile minimal --component rustfmt --component clippy

Write-Host "[1/11] Toolchain"
rustc "+$Toolchain" --version --verbose
cargo "+$Toolchain" --version

Write-Host "[2/11] Formatting"
cargo "+$Toolchain" fmt --all -- --check

Write-Host "[3/11] Clippy -D warnings"
cargo "+$Toolchain" clippy --workspace --all-targets --locked -- -D warnings

Write-Host "[4/11] Rust workspace tests"
cargo "+$Toolchain" test --workspace --locked --verbose

Write-Host "[5/11] Release build"
cargo "+$Toolchain" build --workspace --release --locked

Write-Host "[6/11] C++20 Z-SPAR"
cmake -S "crates/zymatica-language-u/33_Z_SPAR_Semantic_Parity" -B build_zspar -DCMAKE_BUILD_TYPE=Release
cmake --build build_zspar --parallel
ctest --test-dir build_zspar --output-on-failure

Write-Host "[7/11] Python syntax"
python -m compileall -q tools/ten_out_of_ten crates/zymatica-language-u

Write-Host "[8/11] Numerical helper self-test"
python tools/ten_out_of_ten/real_model_validation.py self-test

Write-Host "[9/11] Existing algorithmic verification"
python crates/zymatica-language-u/unified_polyglot_pillars/unified_four_pillars_engine.py
python crates/zymatica-language-u/31_Epigenetic_Weight_Crystallizer/run_proof.py
python crates/zymatica-language-u/32_8D_Octonion_Hypercube/run_proof.py
python crates/zymatica-language-u/33_Z_SPAR_Semantic_Parity/tools/reference_vectors.py | Out-File -Encoding utf8 .golden.tmp.json
# PowerShell's text encoding can differ from the committed JSON, so use Python for byte comparison.
python -c "from pathlib import Path; a=Path('crates/zymatica-language-u/33_Z_SPAR_Semantic_Parity/GOLDEN_VECTORS.json').read_text(encoding='utf-8-sig').strip(); b=Path('.golden.tmp.json').read_text(encoding='utf-8-sig').strip(); assert a==b, 'golden vectors differ'"
Remove-Item .golden.tmp.json
python crates/zymatica-language-u/34_Z_WORMHOLE_Latent_Transfer/run_proof.py
python crates/zymatica-language-u/35_Z_MCTS_Latent_Reasoning/run_proof.py

Write-Host "[10/11] License consistency"
python tools/ten_out_of_ten/license_audit.py --root .

Write-Host "[11/11] Optional real-model 10.00 evidence gate"
if ($EvidenceDir -ne "") {
    python tools/ten_out_of_ten/acceptance_gate.py --evidence-dir $EvidenceDir
    python tools/ten_out_of_ten/evidence_manifest.py $EvidenceDir --repo . --output "$EvidenceDir/MANIFEST.json"
} else {
    Write-Host "INFO: no evidence directory supplied; research 10.00 gate not evaluated"
}

Write-Host "PASS: Zymatica strict verification completed"
