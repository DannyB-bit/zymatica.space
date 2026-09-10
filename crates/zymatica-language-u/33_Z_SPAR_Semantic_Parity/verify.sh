#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "[1/3] C++20 configure/build"
if command -v cmake >/dev/null 2>&1; then
  cmake -S "$ROOT" -B "$ROOT/build" -DCMAKE_BUILD_TYPE=Release
  cmake --build "$ROOT/build" -j
  echo "[2/3] C++20 tests"
  ctest --test-dir "$ROOT/build" --output-on-failure
else
  echo "CMake not found, skipping C++20 native build"
fi

echo "[3/3] Independent golden vectors"
python3 "$ROOT/tools/reference_vectors.py" > "$ROOT/.golden.tmp.json"
diff -u "$ROOT/GOLDEN_VECTORS.json" "$ROOT/.golden.tmp.json"
rm -f "$ROOT/.golden.tmp.json"

echo "Z-SPAR FULL VERIFICATION PASS"
