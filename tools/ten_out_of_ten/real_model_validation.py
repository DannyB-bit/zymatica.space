#!/usr/bin/env python3
# Copyright © 2026 Zymatica
# SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
# See LICENSE for terms.
"""Real-model validation battery for Zymatica Classes 29, 31, 34 and 35.

This script deliberately refuses to call synthetic vector tests "model validation".  Every research
subcommand loads one or more real Hugging Face causal language models and writes machine-readable
JSON evidence.  The lightweight ``self-test`` command only validates the numerical helper routines
and is not accepted as research evidence.

Suggested environment:
    python -m pip install "torch>=2.6" "transformers>=4.56" numpy

Examples:
    python tools/ten_out_of_ten/real_model_validation.py wormhole-train \
      --source-model Qwen/Qwen3.5-0.8B --target-model google/gemma-2-2b-it \
      --prompts evidence/prompts.txt --harmonics 64 --output evidence/10_00/wormhole.json

    python tools/ten_out_of_ten/real_model_validation.py hyper-kv \
      --model HuggingFaceTB/SmolLM2-135M-Instruct --prompts evidence/prompts.txt \
      --rank 8 --output evidence/10_00/hyper_kv.json

    python tools/ten_out_of_ten/real_model_validation.py epigenetic \
      --model HuggingFaceTB/SmolLM2-135M-Instruct --base-prompts evidence/base.txt \
      --adapt-prompts evidence/adapt.txt --steps 20 --output evidence/10_00/epigenetic.json
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import platform
import random
import re
import sys
import time
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any, Iterable, Sequence

import numpy as np


EVIDENCE_SCHEMA = "zymatica.real-model-validation.v1"


def _sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def _write_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def _load_prompts(path: Path, limit: int | None = None) -> list[str]:
    if not path.exists():
        raise FileNotFoundError(path)
    text = path.read_text(encoding="utf-8")
    prompts: list[str] = []
    if path.suffix.lower() == ".jsonl":
        for line in text.splitlines():
            if not line.strip():
                continue
            item = json.loads(line)
            if isinstance(item, str):
                prompt = item
            elif isinstance(item, dict) and isinstance(item.get("prompt"), str):
                prompt = item["prompt"]
            else:
                raise ValueError("JSONL lines must be strings or objects with a string 'prompt'")
            if prompt.strip():
                prompts.append(prompt.strip())
    else:
        # Blank lines separate prompts; if there are no blank lines, each non-empty line is a prompt.
        blocks = [block.strip() for block in re.split(r"\n\s*\n", text) if block.strip()]
        if len(blocks) == 1 and "\n" in blocks[0]:
            prompts = [line.strip() for line in blocks[0].splitlines() if line.strip()]
        else:
            prompts = blocks
    if limit is not None:
        prompts = prompts[:limit]
    if not prompts:
        raise ValueError(f"no prompts found in {path}")
    return prompts


def _require_ml_stack():
    try:
        import torch  # type: ignore
        import transformers  # type: ignore
        from transformers import AutoModelForCausalLM, AutoTokenizer  # type: ignore
    except Exception as exc:  # pragma: no cover - dependency/runtime specific
        raise RuntimeError(
            "real-model commands require torch and transformers; install them before running"
        ) from exc
    return torch, transformers, AutoModelForCausalLM, AutoTokenizer


def _resolve_device(torch: Any, requested: str) -> str:
    if requested != "auto":
        return requested
    if torch.cuda.is_available():
        return "cuda"
    if getattr(torch.backends, "mps", None) is not None and torch.backends.mps.is_available():
        return "mps"
    return "cpu"


def _resolve_dtype(torch: Any, requested: str, device: str):
    if requested == "float32":
        return torch.float32
    if requested == "float16":
        return torch.float16
    if requested == "bfloat16":
        return torch.bfloat16
    if device == "cuda":
        return torch.bfloat16 if torch.cuda.is_bf16_supported() else torch.float16
    if device == "mps":
        return torch.float16
    return torch.float32


def _load_model_and_tokenizer(model_id: str, device: str, dtype_name: str, trust_remote_code: bool):
    torch, transformers, AutoModelForCausalLM, AutoTokenizer = _require_ml_stack()
    resolved_device = _resolve_device(torch, device)
    dtype = _resolve_dtype(torch, dtype_name, resolved_device)
    tokenizer = AutoTokenizer.from_pretrained(model_id, trust_remote_code=trust_remote_code)
    if tokenizer.pad_token_id is None and tokenizer.eos_token_id is not None:
        tokenizer.pad_token = tokenizer.eos_token
    model = AutoModelForCausalLM.from_pretrained(
        model_id,
        torch_dtype=dtype,
        trust_remote_code=trust_remote_code,
    )
    model.to(resolved_device)
    model.eval()
    return torch, transformers, model, tokenizer, resolved_device, dtype


def _last_token_hidden(
    model: Any,
    tokenizer: Any,
    prompts: Sequence[str],
    device: str,
    max_length: int,
    layer: int,
) -> np.ndarray:
    torch, *_ = _require_ml_stack()
    rows: list[np.ndarray] = []
    with torch.no_grad():
        for prompt in prompts:
            inputs = tokenizer(
                prompt,
                return_tensors="pt",
                truncation=True,
                max_length=max_length,
            )
            inputs = {key: value.to(device) for key, value in inputs.items()}
            outputs = model(**inputs, output_hidden_states=True, use_cache=False, return_dict=True)
            hidden_states = outputs.hidden_states
            if hidden_states is None:
                raise RuntimeError("model did not return hidden states")
            hidden = hidden_states[layer]
            if "attention_mask" in inputs:
                position = int(inputs["attention_mask"][0].sum().item()) - 1
            else:
                position = hidden.shape[1] - 1
            row = hidden[0, position].detach().float().cpu().numpy()
            rows.append(np.asarray(row, dtype=np.float32))
    return np.stack(rows, axis=0)


def _cosine_rows(a: np.ndarray, b: np.ndarray) -> np.ndarray:
    numerator = np.sum(a * b, axis=1)
    denominator = np.linalg.norm(a, axis=1) * np.linalg.norm(b, axis=1)
    return numerator / np.maximum(denominator, 1.0e-12)


def _fit_reduced_rank_ridge(
    x: np.ndarray,
    y: np.ndarray,
    rank: int,
    ridge: float,
    seed: int,
) -> tuple[np.ndarray, np.ndarray]:
    """Fit X->Y ridge regression and factor it into X @ down @ up.

    Uses the primal or dual ridge form depending on sample count, then randomized SVD of the
    resulting linear map.  This is deterministic for a fixed seed and input corpus.
    """

    if x.ndim != 2 or y.ndim != 2 or x.shape[0] != y.shape[0]:
        raise ValueError("X and Y must be 2-D matrices with equal sample counts")
    max_rank = min(x.shape[1], y.shape[1])
    if rank < 1 or rank > max_rank:
        raise ValueError(f"rank must be in 1..={max_rank}, got {rank}")
    if ridge <= 0.0:
        raise ValueError("ridge must be positive")

    x64 = np.asarray(x, dtype=np.float64)
    y64 = np.asarray(y, dtype=np.float64)
    n_samples, source_dim = x64.shape

    if n_samples <= source_dim:
        gram = x64 @ x64.T
        gram.flat[:: n_samples + 1] += ridge
        alpha = np.linalg.solve(gram, y64)
        full_map = x64.T @ alpha
    else:
        gram = x64.T @ x64
        gram.flat[:: source_dim + 1] += ridge
        full_map = np.linalg.solve(gram, x64.T @ y64)

    oversample = min(12, max_rank - rank)
    probe_width = rank + oversample
    rng = np.random.default_rng(seed)
    omega = rng.standard_normal((full_map.shape[1], probe_width))
    q, _ = np.linalg.qr(full_map @ omega, mode="reduced")
    small = q.T @ full_map
    u_small, singular, vt = np.linalg.svd(small, full_matrices=False)
    u = q @ u_small[:, :rank]
    singular = singular[:rank]
    vt = vt[:rank]
    root = np.sqrt(np.maximum(singular, 0.0))
    down = u * root[None, :]
    up = root[:, None] * vt

    # The Rust transport maps the first eight factors through sigmoid/logit.  Rescale those
    # columns so 99% of calibration intermediate values remain in roughly [-4, 4], reducing
    # sigmoid saturation while preserving the factorized product exactly.
    z = x64 @ down
    for column in range(min(8, rank)):
        p99 = float(np.percentile(np.abs(z[:, column]), 99.0))
        scale = max(1.0, p99 / 4.0)
        down[:, column] /= scale
        up[column, :] *= scale

    return down.astype(np.float32), up.astype(np.float32)


def _bridge_forward(x: np.ndarray, down: np.ndarray, up: np.ndarray) -> np.ndarray:
    return (x @ down) @ up


def _lm_head_top1_agreement(model: Any, predicted: np.ndarray, reference: np.ndarray, device: str) -> float:
    torch, *_ = _require_ml_stack()
    lm_head = model.get_output_embeddings()
    if lm_head is None:
        raise RuntimeError("target model does not expose output embeddings / lm_head")
    matches = 0
    with torch.no_grad():
        for pred_row, ref_row in zip(predicted, reference, strict=True):
            pred_tensor = torch.from_numpy(pred_row).to(device=device, dtype=lm_head.weight.dtype)
            ref_tensor = torch.from_numpy(ref_row).to(device=device, dtype=lm_head.weight.dtype)
            pred_logits = lm_head(pred_tensor)
            ref_logits = lm_head(ref_tensor)
            matches += int(pred_logits.argmax().item() == ref_logits.argmax().item())
    return matches / max(1, len(predicted))


def command_wormhole_train(args: argparse.Namespace) -> int:
    prompts = _load_prompts(Path(args.prompts), args.limit)
    if len(prompts) < 20:
        raise ValueError("wormhole training requires at least 20 prompts; 100+ is strongly recommended")

    split_index = max(1, min(len(prompts) - 1, int(len(prompts) * args.train_fraction)))
    train_prompts = prompts[:split_index]
    eval_prompts = prompts[split_index:]

    torch, transformers, source_model, source_tok, source_device, _ = _load_model_and_tokenizer(
        args.source_model, args.device, args.dtype, args.trust_remote_code
    )
    _, _, target_model, target_tok, target_device, _ = _load_model_and_tokenizer(
        args.target_model, args.device, args.dtype, args.trust_remote_code
    )
    if source_device != target_device:
        raise RuntimeError("source and target models resolved to different devices")

    started = time.time()
    x_train = _last_token_hidden(
        source_model, source_tok, train_prompts, source_device, args.max_length, args.source_layer
    )
    y_train = _last_token_hidden(
        target_model, target_tok, train_prompts, target_device, args.max_length, args.target_layer
    )
    x_eval = _last_token_hidden(
        source_model, source_tok, eval_prompts, source_device, args.max_length, args.source_layer
    )
    y_eval = _last_token_hidden(
        target_model, target_tok, eval_prompts, target_device, args.max_length, args.target_layer
    )

    total_rank = 8 + args.harmonics
    down, up = _fit_reduced_rank_ridge(x_train, y_train, total_rank, args.ridge, args.seed)
    predicted = _bridge_forward(x_eval, down, up)
    cosine = _cosine_rows(predicted, y_eval)
    mse = float(np.mean((predicted - y_eval) ** 2))
    top1 = _lm_head_top1_agreement(target_model, predicted, y_eval, target_device)

    prompt_path = Path(args.prompts)
    output = {
        "schema": EVIDENCE_SCHEMA,
        "class": 34,
        "kind": "learned_cross_model_bridge",
        "format_version": 1,
        "source_model": args.source_model,
        "target_model": args.target_model,
        "source_dim": int(x_train.shape[1]),
        "target_dim": int(y_train.shape[1]),
        "intermediate_dim": int(args.harmonics),
        "total_intermediate_rank": int(total_rank),
        "source_layer": args.source_layer,
        "target_layer": args.target_layer,
        "training_samples": len(train_prompts),
        "heldout_samples": len(eval_prompts),
        "ridge": args.ridge,
        "seed": args.seed,
        "training_fingerprint_sha256": _sha256_file(prompt_path),
        "heldout_cosine_mean": float(np.mean(cosine)),
        "heldout_cosine_median": float(np.median(cosine)),
        "heldout_cosine_min": float(np.min(cosine)),
        "heldout_mse": mse,
        "heldout_top1_agreement": float(top1),
        "proj_down": down.reshape(-1).tolist(),
        "proj_up": up.reshape(-1).tolist(),
        "elapsed_seconds": time.time() - started,
        "environment": {
            "python": sys.version,
            "platform": platform.platform(),
            "numpy": np.__version__,
            "torch": torch.__version__,
            "transformers": transformers.__version__,
            "device": source_device,
        },
    }
    _write_json(Path(args.output), output)
    print(json.dumps({k: v for k, v in output.items() if k not in {"proj_down", "proj_up"}}, indent=2))
    return 0


def _pivoted_low_rank_f16(matrix: np.ndarray, rank: int) -> tuple[np.ndarray, float, float, float]:
    matrix = np.asarray(matrix, dtype=np.float32)
    if matrix.ndim != 2 or matrix.shape[0] == 0 or matrix.shape[1] == 0:
        raise ValueError("matrix must be non-empty and 2-D")
    if rank < 1 or rank > min(matrix.shape):
        raise ValueError("rank exceeds matrix dimensions")

    basis: list[np.ndarray] = []
    for _ in range(rank):
        best_energy = 0.0
        best_residual: np.ndarray | None = None
        for row in matrix:
            residual = row.astype(np.float64, copy=True)
            for b in basis:
                residual -= float(np.dot(row, b)) * b
            energy = float(np.dot(residual, residual))
            if energy > best_energy:
                best_energy = energy
                best_residual = residual
        if best_residual is None or best_energy <= 1.0e-12:
            break
        vector = best_residual / math.sqrt(best_energy)
        for b in basis:
            vector -= float(np.dot(vector, b)) * b
        norm = float(np.linalg.norm(vector))
        if norm <= 1.0e-12:
            break
        basis.append((vector / norm).astype(np.float32))
    if not basis:
        raise ValueError("matrix is degenerate")

    bmat = np.stack(basis, axis=0)
    coeff = matrix @ bmat.T
    # Match the Rust storage format: quantize both factors to f16 before reconstruction.
    b16 = bmat.astype(np.float16)
    c16 = coeff.astype(np.float16)
    recon = c16.astype(np.float32) @ b16.astype(np.float32)
    diff = matrix - recon
    mse = float(np.mean(diff * diff))
    max_abs = float(np.max(np.abs(diff)))
    original_bytes = matrix.size * 4
    compressed_bytes = b16.nbytes + c16.nbytes + 32
    ratio = original_bytes / compressed_bytes
    return recon, mse, max_abs, float(ratio)


def _legacy_cache(cache: Any) -> tuple[Any, ...]:
    if hasattr(cache, "to_legacy_cache"):
        return tuple(cache.to_legacy_cache())
    if isinstance(cache, (tuple, list)):
        return tuple(cache)
    try:
        return tuple(cache)
    except TypeError as exc:
        raise TypeError(f"unsupported cache type: {type(cache)!r}") from exc


def _rebuild_cache(original_cache: Any, legacy: tuple[Any, ...]) -> Any:
    if isinstance(original_cache, (tuple, list)):
        return legacy
    cls = type(original_cache)
    constructor = getattr(cls, "from_legacy_cache", None)
    if callable(constructor):
        return constructor(legacy)
    try:
        from transformers.cache_utils import DynamicCache  # type: ignore

        if hasattr(DynamicCache, "from_legacy_cache"):
            return DynamicCache.from_legacy_cache(legacy)
    except Exception:
        pass
    return legacy


def command_hyper_kv(args: argparse.Namespace) -> int:
    prompts = _load_prompts(Path(args.prompts), args.limit)
    torch, transformers, model, tokenizer, device, _ = _load_model_and_tokenizer(
        args.model, args.device, args.dtype, args.trust_remote_code
    )

    all_mse: list[float] = []
    all_max_abs: list[float] = []
    ratios: list[float] = []
    top1_matches: list[int] = []
    kl_values: list[float] = []
    injection_errors: list[str] = []
    started = time.time()

    with torch.no_grad():
        for prompt in prompts:
            inputs = tokenizer(prompt, return_tensors="pt", truncation=True, max_length=args.max_length)
            inputs = {key: value.to(device) for key, value in inputs.items()}
            outputs = model(**inputs, use_cache=True, return_dict=True)
            cache = outputs.past_key_values
            if cache is None:
                raise RuntimeError("model did not return a KV cache")
            legacy = _legacy_cache(cache)
            reconstructed_layers = []

            for key_tensor, value_tensor in legacy:
                if key_tensor.ndim != 4 or value_tensor.ndim != 4:
                    raise RuntimeError(
                        f"expected [batch, heads, seq, head_dim] cache tensors, got "
                        f"{tuple(key_tensor.shape)} and {tuple(value_tensor.shape)}"
                    )
                key_out = key_tensor.detach().float().clone()
                value_out = value_tensor.detach().float().clone()
                for batch in range(key_tensor.shape[0]):
                    for head in range(key_tensor.shape[1]):
                        k_np = key_tensor[batch, head].detach().float().cpu().numpy()
                        v_np = value_tensor[batch, head].detach().float().cpu().numpy()
                        effective_rank = min(args.rank, min(k_np.shape), min(v_np.shape))
                        if effective_rank < 1:
                            continue
                        k_recon, k_mse, k_max, k_ratio = _pivoted_low_rank_f16(k_np, effective_rank)
                        v_recon, v_mse, v_max, v_ratio = _pivoted_low_rank_f16(v_np, effective_rank)
                        key_out[batch, head] = torch.from_numpy(k_recon).to(key_out.device)
                        value_out[batch, head] = torch.from_numpy(v_recon).to(value_out.device)
                        all_mse.extend([k_mse, v_mse])
                        all_max_abs.extend([k_max, v_max])
                        ratios.extend([k_ratio, v_ratio])
                reconstructed_layers.append(
                    (
                        key_out.to(device=device, dtype=key_tensor.dtype),
                        value_out.to(device=device, dtype=value_tensor.dtype),
                    )
                )

            try:
                reconstructed_cache = _rebuild_cache(cache, tuple(reconstructed_layers))
                candidate = outputs.logits[:, -1, :].argmax(dim=-1, keepdim=True)
                original_next = model(
                    input_ids=candidate,
                    past_key_values=cache,
                    use_cache=False,
                    return_dict=True,
                ).logits[:, -1, :].float()
                compressed_next = model(
                    input_ids=candidate,
                    past_key_values=reconstructed_cache,
                    use_cache=False,
                    return_dict=True,
                ).logits[:, -1, :].float()
                top1_matches.append(
                    int(original_next.argmax(dim=-1).item() == compressed_next.argmax(dim=-1).item())
                )
                p = torch.log_softmax(original_next, dim=-1)
                q = torch.log_softmax(compressed_next, dim=-1)
                kl = torch.sum(torch.exp(p) * (p - q), dim=-1).mean().item()
                kl_values.append(float(kl))
            except Exception as exc:  # model/cache implementation specific
                injection_errors.append(f"{type(exc).__name__}: {exc}")

    payload = {
        "schema": EVIDENCE_SCHEMA,
        "class": 29,
        "kind": "real_model_kv_cache_compression",
        "model": args.model,
        "prompt_count": len(prompts),
        "rank": args.rank,
        "mean_reconstruction_mse": float(np.mean(all_mse)) if all_mse else None,
        "p95_reconstruction_mse": float(np.percentile(all_mse, 95.0)) if all_mse else None,
        "max_abs_error": max(all_max_abs) if all_max_abs else None,
        "mean_compression_ratio": float(np.mean(ratios)) if ratios else None,
        "injection_supported": bool(top1_matches) and not injection_errors,
        "next_step_top1_parity": float(np.mean(top1_matches)) if top1_matches else None,
        "next_step_kl_mean": float(np.mean(kl_values)) if kl_values else None,
        "injection_errors": injection_errors,
        "elapsed_seconds": time.time() - started,
        "prompt_sha256": _sha256_file(Path(args.prompts)),
        "environment": {
            "python": sys.version,
            "platform": platform.platform(),
            "numpy": np.__version__,
            "torch": torch.__version__,
            "transformers": transformers.__version__,
            "device": device,
        },
    }
    _write_json(Path(args.output), payload)
    print(json.dumps(payload, indent=2))
    return 0


def _mean_causal_lm_loss(model: Any, tokenizer: Any, prompts: Sequence[str], device: str, max_length: int) -> float:
    torch, *_ = _require_ml_stack()
    values: list[float] = []
    with torch.no_grad():
        for prompt in prompts:
            encoded = tokenizer(prompt, return_tensors="pt", truncation=True, max_length=max_length)
            encoded = {key: value.to(device) for key, value in encoded.items()}
            outputs = model(**encoded, labels=encoded["input_ids"], use_cache=False, return_dict=True)
            values.append(float(outputs.loss.item()))
    return float(np.mean(values))


def _choose_linear_layer(torch: Any, model: Any, pattern: str | None):
    candidates = [(name, module) for name, module in model.named_modules() if isinstance(module, torch.nn.Linear)]
    if pattern:
        regex = re.compile(pattern)
        candidates = [(name, module) for name, module in candidates if regex.search(name)]
    if not candidates:
        raise ValueError("no torch.nn.Linear layer matched --layer-regex")
    # Prefer a reasonably wide internal projection; deterministic tie-breaking by name.
    candidates.sort(key=lambda item: (-item[1].in_features * item[1].out_features, item[0]))
    return candidates[0]


def command_epigenetic(args: argparse.Namespace) -> int:
    base_prompts = _load_prompts(Path(args.base_prompts), args.base_limit)
    adapt_prompts = _load_prompts(Path(args.adapt_prompts), args.adapt_limit)
    torch, transformers, model, tokenizer, device, _ = _load_model_and_tokenizer(
        args.model, args.device, args.dtype, args.trust_remote_code
    )
    layer_name, layer = _choose_linear_layer(torch, model, args.layer_regex)

    for parameter in model.parameters():
        parameter.requires_grad_(False)
    layer.weight.requires_grad_(True)

    captured: list[Any] = []

    def capture_input(_module: Any, module_inputs: tuple[Any, ...]) -> None:
        hidden = module_inputs[0].detach().reshape(-1, layer.in_features).float().cpu()
        captured.append(hidden)

    hook = layer.register_forward_pre_hook(capture_input)
    try:
        with torch.no_grad():
            for prompt in base_prompts:
                encoded = tokenizer(prompt, return_tensors="pt", truncation=True, max_length=args.max_length)
                encoded = {key: value.to(device) for key, value in encoded.items()}
                model(**encoded, use_cache=False)
    finally:
        hook.remove()

    activations = torch.cat(captured, dim=0)
    if activations.shape[0] > args.max_activation_rows:
        generator = torch.Generator().manual_seed(args.seed)
        indices = torch.randperm(activations.shape[0], generator=generator)[: args.max_activation_rows]
        activations = activations[indices]

    # Vh rows span the observed input-activation row space.  Q columns therefore form the basis
    # used by ΔW <- ΔW - (ΔW Q) Q^T.
    _, singular, vh = torch.linalg.svd(activations, full_matrices=False)
    if singular.numel() == 0:
        raise RuntimeError("activation matrix is empty")
    relative = singular / singular[0].clamp_min(1.0e-12)
    numerical_rank = int((relative > args.svd_tolerance).sum().item())
    basis_rank = min(max(1, numerical_rank), args.basis_rank, vh.shape[0])
    q = vh[:basis_rank].T.contiguous().to(device=device, dtype=layer.weight.dtype)

    weight_before = layer.weight.detach().float().cpu().clone()
    base_loss_before = _mean_causal_lm_loss(model, tokenizer, base_prompts, device, args.max_length)
    adapt_loss_before = _mean_causal_lm_loss(model, tokenizer, adapt_prompts, device, args.max_length)

    def project_gradient(gradient: Any):
        # Rows of the weight gradient are projected into the nullspace of old activation inputs.
        return gradient - (gradient @ q) @ q.T

    grad_hook = layer.weight.register_hook(project_gradient)
    optimizer = torch.optim.SGD([layer.weight], lr=args.learning_rate)
    model.train()
    randomizer = random.Random(args.seed)
    try:
        for _ in range(args.steps):
            prompt = randomizer.choice(adapt_prompts)
            encoded = tokenizer(prompt, return_tensors="pt", truncation=True, max_length=args.max_length)
            encoded = {key: value.to(device) for key, value in encoded.items()}
            optimizer.zero_grad(set_to_none=True)
            outputs = model(**encoded, labels=encoded["input_ids"], use_cache=False, return_dict=True)
            outputs.loss.backward()
            optimizer.step()
    finally:
        grad_hook.remove()
        model.eval()

    weight_after = layer.weight.detach().float().cpu().clone()
    delta_weight = weight_after - weight_before
    calibration_delta = activations @ delta_weight.T
    max_abs_activation_delta = float(calibration_delta.abs().max().item())
    rms_activation_delta = float(calibration_delta.square().mean().sqrt().item())
    base_loss_after = _mean_causal_lm_loss(model, tokenizer, base_prompts, device, args.max_length)
    adapt_loss_after = _mean_causal_lm_loss(model, tokenizer, adapt_prompts, device, args.max_length)

    payload = {
        "schema": EVIDENCE_SCHEMA,
        "class": 31,
        "kind": "real_model_nullspace_projected_update",
        "model": args.model,
        "layer": layer_name,
        "in_features": layer.in_features,
        "out_features": layer.out_features,
        "calibration_activation_rows": int(activations.shape[0]),
        "basis_rank": basis_rank,
        "steps": args.steps,
        "learning_rate": args.learning_rate,
        "max_abs_calibration_activation_delta": max_abs_activation_delta,
        "rms_calibration_activation_delta": rms_activation_delta,
        "base_loss_before": base_loss_before,
        "base_loss_after": base_loss_after,
        "base_loss_relative_change": (base_loss_after - base_loss_before) / max(base_loss_before, 1.0e-12),
        "adapt_loss_before": adapt_loss_before,
        "adapt_loss_after": adapt_loss_after,
        "adapt_loss_relative_change": (adapt_loss_after - adapt_loss_before) / max(adapt_loss_before, 1.0e-12),
        "base_prompt_sha256": _sha256_file(Path(args.base_prompts)),
        "adapt_prompt_sha256": _sha256_file(Path(args.adapt_prompts)),
        "environment": {
            "python": sys.version,
            "platform": platform.platform(),
            "torch": torch.__version__,
            "transformers": transformers.__version__,
            "device": device,
        },
    }
    _write_json(Path(args.output), payload)
    print(json.dumps(payload, indent=2))
    return 0


def _sigmoid_np(x: np.ndarray) -> np.ndarray:
    positive = x >= 0
    out = np.empty_like(x, dtype=np.float32)
    out[positive] = 1.0 / (1.0 + np.exp(-x[positive]))
    exp = np.exp(x[~positive])
    out[~positive] = exp / (1.0 + exp)
    return out


def _logit_np(p: np.ndarray) -> np.ndarray:
    clipped = np.clip(p, 1.0e-5, 1.0 - 1.0e-5)
    return np.log(clipped / (1.0 - clipped))


def _mcts_axes(
    start_axes: np.ndarray,
    reward_fn,
    simulations: int,
    max_depth: int,
    step: float,
    c_puct: float,
) -> tuple[np.ndarray, float, float]:
    """Small deterministic PUCT search over the eight bounded axes."""

    class Node:
        __slots__ = ("axes", "parent", "children", "visits", "value", "prior", "depth")

        def __init__(self, axes, parent, prior, depth):
            self.axes = axes
            self.parent = parent
            self.children: list[int] = []
            self.visits = 0
            self.value = 0.0
            self.prior = prior
            self.depth = depth

    actions = []
    for axis in range(8):
        for sign in (-1.0, 1.0):
            delta = np.zeros(8, dtype=np.float32)
            delta[axis] = sign * step
            actions.append(delta)

    nodes = [Node(np.asarray(start_axes, dtype=np.float32).copy(), None, 1.0, 0)]
    start_reward = float(reward_fn(nodes[0].axes))
    for _ in range(simulations):
        current = 0
        while nodes[current].children and nodes[current].depth < max_depth:
            parent_visits = max(1, nodes[current].visits)
            best_score = -float("inf")
            best_child = current
            for child_idx in nodes[current].children:
                child = nodes[child_idx]
                q = child.value / child.visits if child.visits else 0.0
                u = c_puct * child.prior * math.sqrt(parent_visits) / (1 + child.visits)
                if q + u > best_score:
                    best_score = q + u
                    best_child = child_idx
            current = best_child
        if nodes[current].depth < max_depth and nodes[current].visits > 0:
            parent_axes = nodes[current].axes
            parent_reward = float(reward_fn(parent_axes))
            for action in actions:
                child_axes = np.clip(parent_axes + action, 0.0, 15.0)
                child_reward = float(reward_fn(child_axes))
                # A positive local improvement receives a stronger prior but never a zero prior.
                prior = max(1.0e-4, math.exp(max(-10.0, min(10.0, child_reward - parent_reward))))
                child_idx = len(nodes)
                nodes.append(Node(child_axes, current, prior, nodes[current].depth + 1))
                nodes[current].children.append(child_idx)
            current = max(nodes[current].children, key=lambda idx: nodes[idx].prior)
        value = float(reward_fn(nodes[current].axes))
        cursor: int | None = current
        while cursor is not None:
            nodes[cursor].visits += 1
            nodes[cursor].value += value
            cursor = nodes[cursor].parent

    current = 0
    best_axes = nodes[0].axes
    while nodes[current].children:
        candidates = [idx for idx in nodes[current].children if nodes[idx].visits > 0]
        if not candidates:
            break
        current = max(candidates, key=lambda idx: nodes[idx].visits)
        best_axes = nodes[current].axes
    final_reward = float(reward_fn(best_axes))
    return best_axes, start_reward, final_reward


def command_mcts_lmhead(args: argparse.Namespace) -> int:
    prompts = _load_prompts(Path(args.prompts), args.limit)
    bridge = json.loads(Path(args.bridge).read_text(encoding="utf-8"))
    down = np.asarray(bridge["proj_down"], dtype=np.float32).reshape(
        int(bridge["source_dim"]), 8 + int(bridge["intermediate_dim"])
    )
    up = np.asarray(bridge["proj_up"], dtype=np.float32).reshape(
        8 + int(bridge["intermediate_dim"]), int(bridge["target_dim"])
    )

    torch, transformers, source_model, source_tok, device, _ = _load_model_and_tokenizer(
        args.source_model, args.device, args.dtype, args.trust_remote_code
    )
    _, _, target_model, target_tok, target_device, _ = _load_model_and_tokenizer(
        args.target_model, args.device, args.dtype, args.trust_remote_code
    )
    if device != target_device:
        raise RuntimeError("source and target models resolved to different devices")
    lm_head = target_model.get_output_embeddings()
    if lm_head is None:
        raise RuntimeError("target model has no output embedding head")

    improvements: list[float] = []
    successes = 0
    details = []
    for prompt in prompts:
        source_hidden = _last_token_hidden(
            source_model, source_tok, [prompt], device, args.max_length, args.source_layer
        )[0]
        z = source_hidden @ down
        start_axes = _sigmoid_np(z[:8]) * 15.0
        harmonics = z[8:].copy()

        with torch.no_grad():
            target_inputs = target_tok(prompt, return_tensors="pt", truncation=True, max_length=args.max_length)
            target_inputs = {key: value.to(device) for key, value in target_inputs.items()}
            target_outputs = target_model(**target_inputs, use_cache=False, return_dict=True)
            reference_token = int(target_outputs.logits[:, -1, :].argmax(dim=-1).item())

        def reward(axes: np.ndarray) -> float:
            inter = np.concatenate((_logit_np(axes / 15.0), harmonics), axis=0)
            target_hidden = inter @ up
            with torch.no_grad():
                hidden_tensor = torch.from_numpy(target_hidden).to(device=device, dtype=lm_head.weight.dtype)
                logits = lm_head(hidden_tensor).float()
                log_probs = torch.log_softmax(logits, dim=-1)
                return float(log_probs[reference_token].item())

        best_axes, start_reward, final_reward = _mcts_axes(
            start_axes,
            reward,
            args.simulations,
            args.max_depth,
            args.step,
            args.c_puct,
        )
        improvement = final_reward - start_reward
        improvements.append(improvement)
        successes += int(improvement > 0.0)
        details.append(
            {
                "reference_token": reference_token,
                "start_logprob": start_reward,
                "final_logprob": final_reward,
                "improvement": improvement,
                "start_axes": start_axes.tolist(),
                "final_axes": best_axes.tolist(),
            }
        )

    payload = {
        "schema": EVIDENCE_SCHEMA,
        "class": 35,
        "kind": "real_lm_head_latent_mcts",
        "source_model": args.source_model,
        "target_model": args.target_model,
        "bridge_sha256": _sha256_file(Path(args.bridge)),
        "prompt_sha256": _sha256_file(Path(args.prompts)),
        "prompt_count": len(prompts),
        "simulations": args.simulations,
        "max_depth": args.max_depth,
        "mean_reference_logprob_improvement": float(np.mean(improvements)),
        "median_reference_logprob_improvement": float(np.median(improvements)),
        "success_fraction": successes / len(prompts),
        "details": details,
        "environment": {
            "python": sys.version,
            "platform": platform.platform(),
            "torch": torch.__version__,
            "transformers": transformers.__version__,
            "device": device,
        },
    }
    _write_json(Path(args.output), payload)
    summary = dict(payload)
    summary.pop("details")
    print(json.dumps(summary, indent=2))
    return 0


def command_self_test(_args: argparse.Namespace) -> int:
    rng = np.random.default_rng(7)
    x = rng.standard_normal((64, 12)).astype(np.float32)
    true_down = rng.standard_normal((12, 5)).astype(np.float32)
    true_up = rng.standard_normal((5, 9)).astype(np.float32)
    y = x @ true_down @ true_up
    down, up = _fit_reduced_rank_ridge(x, y, 5, 1.0e-6, 7)
    predicted = _bridge_forward(x, down, up)
    cosine = float(np.mean(_cosine_rows(predicted, y)))
    if cosine < 0.999:
        raise AssertionError(f"reduced-rank ridge self-test failed: cosine={cosine}")

    low_rank = rng.standard_normal((32, 4)).astype(np.float32) @ rng.standard_normal((4, 64)).astype(np.float32)
    _, mse, _, ratio = _pivoted_low_rank_f16(low_rank, 4)
    if mse > 1.0e-3 or ratio <= 2.0:
        raise AssertionError(f"Hyper-KV numerical self-test failed: mse={mse}, ratio={ratio}")

    target = np.array([8.0] + [3.0] * 7, dtype=np.float32)
    start = np.array([1.0] * 8, dtype=np.float32)
    reward = lambda axes: -float(np.sum((axes - target) ** 2))
    _, start_reward, final_reward = _mcts_axes(start, reward, 80, 5, 0.5, math.sqrt(2.0))
    if final_reward <= start_reward:
        raise AssertionError("MCTS helper failed to improve deterministic objective")

    print(json.dumps({"self_test": "PASS", "ridge_cosine": cosine, "hyper_kv_mse": mse, "hyper_kv_ratio": ratio}, indent=2))
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)

    worm = sub.add_parser("wormhole-train", help="learn and evaluate a real cross-model latent bridge")
    worm.add_argument("--source-model", required=True)
    worm.add_argument("--target-model", required=True)
    worm.add_argument("--prompts", required=True)
    worm.add_argument("--output", required=True)
    worm.add_argument("--harmonics", type=int, default=64)
    worm.add_argument("--ridge", type=float, default=1.0e-3)
    worm.add_argument("--train-fraction", type=float, default=0.8)
    worm.add_argument("--source-layer", type=int, default=-1)
    worm.add_argument("--target-layer", type=int, default=-1)
    worm.add_argument("--max-length", type=int, default=256)
    worm.add_argument("--limit", type=int)
    worm.add_argument("--seed", type=int, default=7)
    worm.add_argument("--device", default="auto")
    worm.add_argument("--dtype", choices=["auto", "float32", "float16", "bfloat16"], default="auto")
    worm.add_argument("--trust-remote-code", action="store_true")
    worm.set_defaults(func=command_wormhole_train)

    kv = sub.add_parser("hyper-kv", help="compress/reinject actual Hugging Face past_key_values")
    kv.add_argument("--model", required=True)
    kv.add_argument("--prompts", required=True)
    kv.add_argument("--output", required=True)
    kv.add_argument("--rank", type=int, default=8)
    kv.add_argument("--max-length", type=int, default=256)
    kv.add_argument("--limit", type=int, default=32)
    kv.add_argument("--device", default="auto")
    kv.add_argument("--dtype", choices=["auto", "float32", "float16", "bfloat16"], default="auto")
    kv.add_argument("--trust-remote-code", action="store_true")
    kv.set_defaults(func=command_hyper_kv)

    epi = sub.add_parser("epigenetic", help="run a real nullspace-projected weight update")
    epi.add_argument("--model", required=True)
    epi.add_argument("--base-prompts", required=True)
    epi.add_argument("--adapt-prompts", required=True)
    epi.add_argument("--output", required=True)
    epi.add_argument("--layer-regex")
    epi.add_argument("--basis-rank", type=int, default=64)
    epi.add_argument("--svd-tolerance", type=float, default=1.0e-5)
    epi.add_argument("--max-activation-rows", type=int, default=512)
    epi.add_argument("--steps", type=int, default=20)
    epi.add_argument("--learning-rate", type=float, default=1.0e-4)
    epi.add_argument("--base-limit", type=int, default=16)
    epi.add_argument("--adapt-limit", type=int, default=16)
    epi.add_argument("--max-length", type=int, default=128)
    epi.add_argument("--seed", type=int, default=7)
    epi.add_argument("--device", default="auto")
    epi.add_argument("--dtype", choices=["auto", "float32", "float16", "bfloat16"], default="auto")
    epi.add_argument("--trust-remote-code", action="store_true")
    epi.set_defaults(func=command_epigenetic)

    mcts = sub.add_parser("mcts-lmhead", help="search 8D axes using a real target LM-head objective")
    mcts.add_argument("--source-model", required=True)
    mcts.add_argument("--target-model", required=True)
    mcts.add_argument("--bridge", required=True)
    mcts.add_argument("--prompts", required=True)
    mcts.add_argument("--output", required=True)
    mcts.add_argument("--source-layer", type=int, default=-1)
    mcts.add_argument("--simulations", type=int, default=64)
    mcts.add_argument("--max-depth", type=int, default=5)
    mcts.add_argument("--step", type=float, default=0.35)
    mcts.add_argument("--c-puct", type=float, default=math.sqrt(2.0))
    mcts.add_argument("--max-length", type=int, default=256)
    mcts.add_argument("--limit", type=int, default=16)
    mcts.add_argument("--device", default="auto")
    mcts.add_argument("--dtype", choices=["auto", "float32", "float16", "bfloat16"], default="auto")
    mcts.add_argument("--trust-remote-code", action="store_true")
    mcts.set_defaults(func=command_mcts_lmhead)

    self_test = sub.add_parser("self-test", help="validate helper math only; not research evidence")
    self_test.set_defaults(func=command_self_test)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    try:
        return int(args.func(args))
    except KeyboardInterrupt:
        return 130
    except Exception as exc:
        print(f"ERROR: {type(exc).__name__}: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
