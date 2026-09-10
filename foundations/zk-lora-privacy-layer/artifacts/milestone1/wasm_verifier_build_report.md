# WASM Verifier Build Report

Date: 2026-06-29

## Build command

```powershell
cargo build --release --target wasm32-unknown-unknown --manifest-path Multi_Language_Proofs\typescript\wasm_verifier\Cargo.toml
```

## Result

- Exit code: 0
- Rust: `rustc 1.90.0 (1159e78c4 2025-09-14)`
- Cargo: `cargo 1.90.0 (840b83a10 2025-07-30)`
- Artifact: `zk_lora_wasm_verifier.wasm`
- Size: 451 bytes
- SHA-256: `6B0DB858EF4E6FFCA35AD48DCC55DF7DBBE73CE937B91F0EC544F552D9DDA8C6`

The WASM verifier exports `verify_pairing` from `Multi_Language_Proofs/typescript/wasm_verifier/src/lib.rs`.
