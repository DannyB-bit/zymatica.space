# Contributing to ZK-LoRa Privacy Layer 🛠️

Thank you for your interest in contributing to the ZK-LoRa Privacy Layer! To maintain the highest standard of security, code readability, and history preservation, we adhere to the style guides and development workflows used by the Zcash core library (`librustzcash`).

---

## 📋 Code Style & Formatting
*   **Rust Code:** All Rust code must be formatted using the standard toolchain:
    ```bash
    cargo fmt
    cargo clippy
    ```
*   **No Warnings:** Code must compile cleanly without warnings or errors.
*   **Documentation:** All public functions, structs, and modules must be fully documented using standard docstrings (`///`).

---

## 📝 Commit Guidelines
We maintain a clean and linear git commit history. All commits must follow these guidelines:

1.  **Format:** Commit messages should have a concise title (preferably under 120 characters) and a detailed body explaining the motivation for the change.
2.  **Imperative Mood:** Use the imperative mood in commit titles (e.g., `Add Groth16 verifier` instead of `Added Groth16 verifier`).
3.  **Motivation:** The commit body should explain *why* the change is necessary, unless the change is trivial.
4.  **Co-Authors:** If a commit has multiple contributors, add the metadata field:
    ```text
    Co-Authored-By: Name <email@example.com>
    ```
5.  **Partial Work:** Commits representing incomplete work must clearly indicate this in the commit message, detailing what remains to be completed and if the code is expected to compile.

---

## 🔄 Merge Workflow & Pull Requests
*   **Rebasing:** We prefer a clean history. Always rebase your feature branch on the target branch before merging.
*   **Pull Request Review:** All code changes must go through a formal PR review process. Squashing review changes into the original commit is expected during revision loops.
*   **Changelog Updates:** Any change that modifies public APIs, CLI flags, or cryptographic behavior must document the change in `CHANGELOG.md`.
