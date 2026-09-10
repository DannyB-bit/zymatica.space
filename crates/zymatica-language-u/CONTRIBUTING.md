# Contributing to Language-U / Zymatica

Thank you for your interest in contributing to the Language-U Semantic Communication Protocol! We welcome contributions from developers, researchers, and edge-computing enthusiasts.

## 🚀 How to Contribute

### 1. Report Issues
- Open a GitHub Issue describing the bug, enhancement, or question.
- Include the component number (e.g., `#07 SVD/DCT Compression`) and relevant error logs.

### 2. Submit Pull Requests
1. Fork this repository.
2. Create a feature branch: `git checkout -b feat/your-feature-name`
3. Make your changes and run the proof scripts: `python run_proof.py --test`
4. Commit with a descriptive message: `git commit -m "feat(07): improve DCT coefficient selection"`
5. Push and open a Pull Request against `main`.

### 3. Add a New Language Implementation
Each invention supports multi-language verification. To add a new language:
1. Navigate to the target invention folder (e.g., `01_Language_U_Taxonomy/src/`)
2. Create a new directory named after the language (e.g., `ruby/`)
3. Implement the proof logic matching the Python reference in `run_proof.py`
4. Ensure your implementation produces the same verification anchors

### 4. Write or Improve Whitepapers
Whitepapers are in Markdown (`WHITEPAPER.md`) with LaTeX math notation. Improvements to clarity, mathematical rigor, or additional adversarial critiques are welcome.

---

## 🏗️ Development Setup

### Prerequisites
- **Python 3.10+** with `numpy`, `torch`, `safetensors` (for model components)
- **Rust** (for native range coder compilation)
- **Node.js 18+** (for TypeScript/Solana components)
- **GCC or Clang** (for compiling `cuneiform_u_v3.h`)

### Running Proof Scripts
```bash
# Run a single component's proof
cd 01_Language_U_Taxonomy
python run_proof.py --test

# Run all proofs (CI style)
python scripts/run_all_proofs.py
```

---

## 📋 Code Standards
- **Python:** PEP 8 formatting, type hints encouraged
- **Rust:** `cargo fmt` and `cargo clippy` clean
- **C/C++:** Header-only where possible, zero heap allocations for edge targets
- **Commit Messages:** Follow [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `docs:`, `test:`)

---

## 🔒 Licensing
All contributions are licensed under the **Zymatica Commercial & Novel-Holder Covenant License 2.0 (zymatica.space)**. By submitting a PR, you agree that your contribution is released under this license.

---

## 📬 Contact
- **GitHub Issues:** Primary communication channel
- **Telegram:** @SmileAlways2026
- **Team:** zymatica.space | astronautshe.com | DevsOne | We Are TheAiCollective.art
