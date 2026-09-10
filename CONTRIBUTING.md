# Contributing to Zymatica

Thank you for your interest in contributing to the Zymatica sovereign computational architecture and *200 Amsterdam* research ecosystem!

## 1. Contributor License Agreement & Rights Assignment
All contributions submitted to this repository are accepted under the **Zymatica Commercial & Novel-Holder Covenant License (Version 2.0)**.
By opening a pull request or submitting code/documentation, you agree that:
1. You have the legal authority to grant the rights to your contributions.
2. Your contribution does not infringe upon any third-party patents, trade secrets, or copyrights.
3. Your code is contributed under the terms of the Zymatica Covenant License, with copyright assigned or licensed to the Zymatica project authors.

## 2. Development Workflow & Quality Gates
All pull requests must pass the following automated gates:
1. **Formatting**: `cargo fmt --all -- --check`
2. **Linter**: `cargo clippy --workspace --all-targets -- -D warnings`
3. **Rust Test Suite**: `cargo test --workspace --verbose`
4. **Release Build**: `cargo build --workspace --release`
5. **Python Verification Suite**: `python break_the_record_engine.py && python verify_frontier_suite.py`

## 3. Code Standards & Invariants
- **No Unsound Unsafe**: Unsafe Rust is prohibited unless strictly required for FFI/SIMD kernel intrinsics and accompanied by a safety proof.
- **Strict Error Handling**: APIs must return explicit `Result<T, E>` types without silent truncation or unchecked unwraps.
- **Cryptographic Rigor**: Zero-knowledge circuits and consensus engines must include soundness mutation tests.
