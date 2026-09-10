# Component License & Provenance Matrix

Complete this table from actual ownership/upstream records. Do **not** mark a component Covenant-covered merely because it lives in this repository.

| Component/path | Origin / upstream | Copyright holder | License governing that component | Local modifications | Evidence/reference |
|---|---|---|---|---|---|
| `crates/zymatica-engine/` | Zymatica original / verify imported agent portions |  | Covenant 2.0 where owned |  |  |
| `crates/zymatica-zspar/` |  |  |  |  |  |
| `crates/zymatica-zk-mesh/groth16/` | Zymatica code + Arkworks dependencies | Zymatica for original circuit; upstream holders for dependencies | Covenant for original code; MIT/Apache for dependencies |  |  |
| `crates/zymatica-language-u/` | mixed research modules; review file provenance |  |  |  |  |
| `crates/zymatica-agent-harness/` | provenance audit required |  | preserve upstream licenses where applicable |  |  |

After this matrix is complete, place only confirmed Zymatica-owned source globs in `LICENSE_SCOPE.txt`. Put genuine upstream paths that intentionally remain under those globs in `THIRD_PARTY_LICENSE_ALLOWLIST.txt`.
