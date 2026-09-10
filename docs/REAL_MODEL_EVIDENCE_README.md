# Real-model evidence commands

Create prompt corpora that contain ordinary factual, procedural, multilingual and reasoning prompts. Do not hand-pick only examples that make one algorithm look good. Keep a held-out set that is not used to fit Class 34.

## Class 34 — learn a bridge from real source/target activations

```powershell
python tools/ten_out_of_ten/real_model_validation.py wormhole-train `
  --source-model <REAL_SOURCE_MODEL> `
  --target-model <REAL_TARGET_MODEL> `
  --prompts evidence/prompts.txt `
  --harmonics 64 `
  --output evidence/10_00/latest/wormhole.json
```

The JSON output is directly loadable by the replacement Rust `ZWormholeBridge::load_learned_json` because it contains `source_dim`, `target_dim`, `intermediate_dim`, `proj_down` and `proj_up`.

## Class 29 — actual KV cache compression and reinjection

```powershell
python tools/ten_out_of_ten/real_model_validation.py hyper-kv `
  --model <REAL_CAUSAL_LM> `
  --prompts evidence/prompts.txt `
  --rank 8 `
  --output evidence/10_00/latest/hyper_kv.json
```

A scientifically useful run requires `injection_supported: true`; reconstruction MSE alone is not sufficient.

## Class 31 — real projected model update

```powershell
python tools/ten_out_of_ten/real_model_validation.py epigenetic `
  --model <REAL_CAUSAL_LM> `
  --base-prompts evidence/base_prompts.txt `
  --adapt-prompts evidence/adapt_prompts.txt `
  --steps 20 `
  --output evidence/10_00/latest/epigenetic.json
```

This measures the exact constrained-layer activation delta and also checks whether the adaptation objective improves without unacceptable base-loss degradation.

## Class 35 — real LM-head objective

Run this only after the Class 34 bridge has been trained:

```powershell
python tools/ten_out_of_ten/real_model_validation.py mcts-lmhead `
  --source-model <SAME_SOURCE_MODEL> `
  --target-model <SAME_TARGET_MODEL> `
  --bridge evidence/10_00/latest/wormhole.json `
  --prompts evidence/heldout_prompts.txt `
  --output evidence/10_00/latest/mcts.json
```

## Final gate

```powershell
python tools/ten_out_of_ten/acceptance_gate.py --evidence-dir evidence/10_00/latest
python tools/ten_out_of_ten/evidence_manifest.py evidence/10_00/latest --repo . --output evidence/10_00/latest/MANIFEST.json
```
