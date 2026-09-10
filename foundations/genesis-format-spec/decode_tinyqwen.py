# Watermark: ip zymatica.space
__watermark__ = "ip zymatica.space"

import os
os.environ["PYTORCH_CUDA_ALLOC_CONF"] = "garbage_collection_threshold:0.6,max_split_size_mb:24"
import sys
import struct
import json
import time
import numpy as np
import torch
from transformers import AutoModelForCausalLM, AutoTokenizer
from torch.optim import AdamW

sys.stdout.reconfigure(encoding='utf-8', errors='backslashreplace')

DEVICE = "cuda" if torch.cuda.is_available() else "cpu"
PKT_SIZE = 256
MAGIC = bytes([0xA7, 0x07, 0x11])

BASE_MODEL = "j:/Language-U/Language-U-V2/qwen-3.5-0.8b-local"
PKT_PATH = "j:/Language-U/packets_tinyqwen/packet_1paup.bin"
OUTPUT_MODEL = "j:/Language-U/SubZero2.lora"
SFT_DATA_PATH = "j:/Language-U/full_sft_dataset.json"

LAYER_NAMES = [
    "model.layers.3.self_attn.q_proj.weight",
    "model.layers.3.self_attn.k_proj.weight",
    "model.layers.3.self_attn.v_proj.weight",
    "model.layers.3.self_attn.o_proj.weight",
    "model.layers.3.mlp.gate_proj.weight",
    "model.layers.3.mlp.up_proj.weight",
    "model.layers.3.mlp.down_proj.weight",
]

EVAL_TESTS = [
    ("What GPIO pin is the SX1302 reset line on Raspberry Pi 4?",          ["25", "GPIO 25"]),
    ("What is the exact command to reset the LoRa concentrator with gpioset?", ["gpioset", "gpiochip0", "25=0"]),
    ("What script handles the SX1302 hardware reset?",                     ["reset_lgw.sh"]),
    ("On Raspberry Pi 5, which gpiochip and pin is the SX1302 reset mapped to?", ["17", "gpiochip4"]),
    ("What frequency does the Astronaut SHE Handshake Protocol use?",      ["903.0", "903"]),
    ("What Spreading Factor is used for the Astronaut SHE handshake?",     ["SF7", "sf7"]),
    ("What is the transmit power for the Astronaut SHE RAK Miner beacon?", ["14 dBm", "14dBm"]),
    ("What does --pwid 15 represent in test_loragw_hal_tx?",               ["calibration", "14 dBm", "power"]),
    ("What is the full test_loragw_hal_tx command for the Astronaut SHE handshake?",
                                                                            ["-f 903.0", "-s 7", "--pwid 15", "-z 32"]),
    ("What is the payload size for the Astronaut SHE handshake beacon?",   ["32", "32 bytes"]),
    ("How many dimensions does the Cuneiform-U v3.0 semantic hypercube have?", ["6", "six"]),
    ("What are the 6 axes of Cuneiform-U v3.0?",                           ["DOMAIN", "SUBDOMAIN", "MODALITY"]),
    ("What is the Classifier Radical R_C in Cuneiform-U v3.0?",           ["DOMAIN", "SUBDOMAIN", "4 bits"]),
    ("What are the radical coordinates of the ACK glyph (0x807E)?",       ["0x00", "0x7E", "0x0B"]),
    ("What is the Shannon Orthogonality equation in Language U?",          ["H(text)", "H(meaning)", "H(syntax"]),
    ("What does LLD-AC stand for?",                                         ["LLM", "Logits", "Range Cod"]),
    ("What is a collapse signal in LLD-AC range coding?",                  ["probability", "1.0", "bits"]),
    ("What frequency scale does the LLD-AC range coder use?",              ["1,000,000", "1000000", "million"]),
]

BASELINE_SEMANTIC_TESTS = [
    ("A computer program is", ["program", "computer", "code", "software", "instructions"]),
    ("The purpose of a map is", ["map", "place", "location", "direction", "where", "travel"]),
    ("Water is important because", ["water", "important", "drink", "life", "body"]),
    ("A library is a place where", ["library", "place", "book", "read", "find"]),
    ("The moon appears at night", ["moon", "night", "sky", "appears"]),
    ("A keyboard is used to", ["keyboard", "type", "computer", "used"]),
    ("A camera can", ["camera", "photo", "picture", "image"]),
    ("A river flows", ["river", "flow", "water"]),
    ("A doctor helps", ["doctor", "help", "patient", "sick", "health"]),
    ("A calendar shows", ["calendar", "date", "day", "month"]),
    ("A battery stores", ["battery", "energy", "power", "electric"]),
    ("A question mark means", ["question", "mark", "ask"]),
    ("People sleep because", ["sleep", "rest", "tired", "body"]),
    ("Exercise helps", ["exercise", "health", "body", "strong"]),
    ("A triangle has", ["triangle", "three", "3", "sides"]),
]

OFF_TOPIC_LANGUAGE_U = [
    "sx1302", "astronaut she", "gpio", "cuneiform", "lora", "lld-ac", "spreading factor", "903.0", "14 dbm"
]

def gradient_atom_decompress(data: bytes, pos: int) -> tuple:
    R     = data[pos]; pos += 1
    scale = struct.unpack('>e', data[pos:pos+2])[0]; pos += 2
    n_bytes = (R + 1) // 2
    packed = data[pos:pos+n_bytes]; pos += n_bytes

    nibbles = []
    for b in packed:
        nibbles.append(b & 0x0F)
        nibbles.append((b >> 4) & 0x0F)

    MAG_TABLE = [0.125, 0.375, 0.625, 0.875]
    delta_s = np.zeros(R, dtype=np.float64)
    for i in range(R):
        if i >= len(nibbles): break
        nib  = nibbles[i]
        sign = +1 if (nib >> 2) & 1 else -1
        mag  = MAG_TABLE[nib & 0x3]
        delta_s[i] = sign * mag * scale
    return delta_s, pos

def eigenspace_decompress(data: bytes, pos: int) -> tuple:
    R     = data[pos]; pos += 1
    scale = struct.unpack('>e', data[pos:pos+2])[0]; pos += 2
    q_vals = []
    for _ in range(R):
        val = data[pos]
        if val > 127: val = val - 256
        q_vals.append(val)
        pos += 1
    delta_s = np.array(q_vals, dtype=np.float64) * scale
    return delta_s, pos

def decode_layer_delta(data: bytes, pos: int, W_base: np.ndarray, level: int) -> tuple:
    if level == 5:
        delta_s, pos = eigenspace_decompress(data, pos)
    elif level == 6:
        delta_s, pos = gradient_atom_decompress(data, pos)
    else:
        raise ValueError(f"Unknown encoding level: 0x{level:02X}")
        
    U_b, _, Vh_b = np.linalg.svd(W_base.astype(np.float64), full_matrices=False)
    R = len(delta_s)
    W_delta = sum(delta_s[i] * np.outer(U_b[:, i], Vh_b[i, :]) for i in range(R))
    return W_delta.astype(np.float32), pos

def evaluate_fidelity(model, tokenizer) -> float:
    model.eval()
    passed = 0
    print("\n  Fidelity test results:")
    for i, (q, kws) in enumerate(EVAL_TESTS):
        prompt = f"Q: {q}\nA:"
        inputs = tokenizer(prompt, return_tensors='pt').to(DEVICE)
        with torch.no_grad():
            out = model.generate(**inputs, max_new_tokens=48,
                                 do_sample=False, pad_token_id=tokenizer.eos_token_id)
        answer = tokenizer.decode(out[0][inputs['input_ids'].shape[1]:],
                                  skip_special_tokens=True).lower()
        ok = any(kw.lower() in answer for kw in kws)
        passed += ok
        mark = "✓" if ok else "✗"
        if i < 5:
            print(f"    [{mark}] Q{i+1:>2}: {q[:55]}")
            sys.stdout.flush()
    fidelity = passed / len(EVAL_TESTS) * 100
    print(f"    ... evaluated {len(EVAL_TESTS)} fidelity tests.")
    print(f"  FIDELITY: {passed}/{len(EVAL_TESTS)} = {fidelity:.1f}%")
    sys.stdout.flush()
    return fidelity

def evaluate_semantic(model, tokenizer) -> float:
    model.eval()
    passed = 0
    for prompt, kws in BASELINE_SEMANTIC_TESTS:
        inputs = tokenizer(prompt, return_tensors='pt').to(DEVICE)
        with torch.no_grad():
            out = model.generate(**inputs, max_new_tokens=32,
                                 do_sample=False, pad_token_id=tokenizer.eos_token_id)
        answer = tokenizer.decode(out[0][inputs['input_ids'].shape[1]:],
                                  skip_special_tokens=True).lower()
        matched = any(kw.lower() in answer for kw in kws)
        off_topic = any(ot in answer for ot in OFF_TOPIC_LANGUAGE_U)
        ok = matched and not off_topic
        passed += ok
    return passed

def collate_batch(batch, tokenizer, device):
    prompts = [item["prompt"] for item in batch]
    completions = [item["completion"] for item in batch]
    
    full_texts = [p + c for p, c in zip(prompts, completions)]
    inputs = tokenizer(full_texts, padding=True, truncation=True, max_length=192, return_tensors="pt").to(device)
    labels = inputs["input_ids"].clone()
    
    for i, p in enumerate(prompts):
        p_len = tokenizer(p, truncation=True, max_length=192, return_tensors="pt")["input_ids"].shape[1]
        labels[i, :p_len] = -100
        pad_mask = (inputs["attention_mask"][i] == 0)
        labels[i, pad_mask] = -100
        
    inputs["labels"] = labels
    return inputs

def train_multitask(model, tokenizer, sft_groups: dict, recipe: dict) -> dict:
    import gc
    import random
    import math
    lu_examples = sft_groups["lu"]
    rf_examples = sft_groups["rf"]
    mmlu_examples = sft_groups["mmlu"]
    gsm_examples = sft_groups["gsm"]
    sem_examples = sft_groups["sem"]

    # Freeze other parameters, train only Layer 3 projections
    for name, param in model.named_parameters():
        if not any(layer in name for layer in LAYER_NAMES):
            param.requires_grad = False
        else:
            param.requires_grad = True

    # Use a cosine learning rate scheduler starting at 2.0e-4 and decaying to 1e-6
    lr_max = recipe['lr'] * 1.0
    lr_min = 1e-6
    
    optimizer = AdamW(filter(lambda p: p.requires_grad, model.parameters()), lr=lr_max, weight_decay=0.01, betas=(0.9, 0.95))
    total_steps = int(recipe['num_steps'] * 2.0) # 300 steps
    accumulation_steps = 2
    
    print(f"\n  On-device Batched Multi-task SFT: {total_steps} steps (Accumulation={accumulation_steps}), Peak LR={lr_max:.6f} with Cosine Decay")
    sys.stdout.flush()
    t0 = time.perf_counter()
    losses = []
    optimizer.zero_grad(set_to_none=True)

    for step in range(total_steps):
        model.train()
        
        # Apply Cosine Annealing Learning Rate
        lr_t = lr_min + 0.5 * (lr_max - lr_min) * (1.0 + math.cos(math.pi * step / total_steps))
        for param_group in optimizer.param_groups:
            param_group['lr'] = lr_t
            
        # Build balanced batch of size 4 (always include 'lu', sample 3 others)
        sampled_tasks = ["lu"] + random.sample(["rf", "mmlu", "gsm", "sem"], 3)
        batch = []
        for task in sampled_tasks:
            if task == "lu":
                batch.extend(random.sample(lu_examples, 1))
            elif task == "rf":
                batch.extend(random.sample(rf_examples, 1))
            elif task == "mmlu":
                batch.extend(random.sample(mmlu_examples, 1))
            elif task == "gsm":
                batch.extend(random.sample(gsm_examples, 1))
            elif task == "sem":
                batch.extend(random.sample(sem_examples, 1))
        
        # Collate & Push to device
        inputs = collate_batch(batch, tokenizer, DEVICE)
        
        with torch.amp.autocast('cuda', enabled=(DEVICE == 'cuda')):
            out = model(input_ids=inputs["input_ids"], attention_mask=inputs["attention_mask"])
            logits = out.logits
            
            # Custom balanced cross-entropy loss (vectorized)
            shift_logits = logits[..., :-1, :].contiguous()
            shift_labels = inputs["labels"][..., 1:].contiguous()
            
            loss_fct = torch.nn.CrossEntropyLoss(reduction="none")
            token_losses = loss_fct(shift_logits.view(-1, shift_logits.size(-1)), shift_labels.view(-1))
            token_losses = token_losses.view(shift_labels.size())
            
            mask = (shift_labels != -100).float()
            masked_losses = token_losses * mask
            example_loss_sums = masked_losses.sum(dim=-1)
            example_token_counts = torch.clamp(mask.sum(dim=-1), min=1.0)
            example_losses = example_loss_sums / example_token_counts
            
            # Apply weights dynamically
            weight_map = {"lu": 12.0, "rf": 1.0, "mmlu": 1.0, "gsm": 1.0, "sem": 1.0}
            step_weights = torch.tensor([weight_map[task] for task in sampled_tasks], device=DEVICE)
            mean_loss = (example_losses * step_weights).sum() / step_weights.sum()
            loss = mean_loss / accumulation_steps
            
        loss.backward()
        
        if (step + 1) % accumulation_steps == 0:
            torch.nn.utils.clip_grad_norm_(model.parameters(), 1.0)
            optimizer.step()
            optimizer.zero_grad(set_to_none=True)
            
        losses.append(mean_loss.item())

        if step == 0 or (step + 1) % 10 == 0 or step == total_steps - 1:
            elapsed = time.perf_counter() - t0
            print(f"    Step {step+1:>4}/{total_steps} | Batch Loss={mean_loss.item():.4f} | LR={lr_t:.2e} | Time: {elapsed:.1f}s")
            sys.stdout.flush()
            
        # Clean up memory
        del inputs, out, loss, mean_loss

    elapsed = time.perf_counter() - t0
    print(f"  Training complete in {elapsed:.1f}s")
    return {"initial_loss": losses[0], "final_loss": losses[-1], "seconds": elapsed}

def main():
    print("=" * 72)
    print("  TINYQWEN 1-PAUP DECODER & RESTORATION ENGINE (BATCHED)")
    print("  Watermark: ip zymatica.space")
    print("=" * 72)
    sys.stdout.flush()

    # 1. Load the single packet
    # Check if a zlib version exists first or fallback to raw bin
    zlib_path = PKT_PATH + ".zlib"
    if os.path.exists(zlib_path):
        print(f"[+] Found zlib compressed packet version at {zlib_path}")
        PKT_PATH = zlib_path

    if not os.path.exists(PKT_PATH):
        print(f"Error: 1-PAUP packet not found at {PKT_PATH}")
        sys.exit(1)
        
    with open(PKT_PATH, "rb") as f:
        packet = f.read()

    # Decompress using zlib if compressed
    import zlib
    try:
        decompressed = zlib.decompress(packet)
        print(f"[+] Successfully decompressed packet via zlib ({len(packet)} bytes -> {len(decompressed)} bytes)")
        packet = decompressed
    except Exception:
        print("[.] Packet is not zlib-compressed (or decompression failed), using raw bytes.")

    if len(packet) < 3:
        raise ValueError(f"Packet too short: {len(packet)} bytes")

    sync, pkt_idx, pkt_total = packet[0], packet[1], packet[2]
    if sync != 0xBB or pkt_idx != 0 or pkt_total != 1:
        raise ValueError(f"Bad packet wrapper headers: sync=0x{sync:02X} idx={pkt_idx} total={pkt_total}")

    data = packet[3:]

    # 2. Parse 32-byte header
    off = 0
    magic   = data[off:off+3]; off += 3
    level   = data[off]; off += 1
    lr_f16  = struct.unpack('>e', data[off:off+2])[0]; off += 2
    n_steps = struct.unpack('>H', data[off:off+2])[0]; off += 2
    seed    = struct.unpack('>I', data[off:off+4])[0]; off += 4
    optim   = data[off]; off += 1
    batch   = data[off]; off += 1
    layer_f = data[off]; off += 1
    warmup  = struct.unpack('>H', data[off:off+2])[0]; off += 2
    n_pairs = data[off]; off += 1
    lu4_hdr = data[off:off+4]; off += 4
    q_mask_bytes = data[off:off+3]; off += 3
    n_layers= data[off]; off += 1
    w_len   = struct.unpack('>H', data[off:off+2])[0]; off += 2
    off = 32

    if magic != MAGIC:
        raise ValueError(f"Bad magic: {magic.hex()} expected {MAGIC.hex()}")

    print(f"\n1-PAUP Header:")
    print(f"  Level (Mode):      Level {level}")
    print(f"  Learning Rate:     {float(lr_f16):.6f}")
    print(f"  Steps / Seed:      {n_steps} / {hex(seed)}")
    print(f"  Layers to update:  {n_layers}")
    print(f"  Weight length:     {w_len} bytes")
    sys.stdout.flush()

    # 3. Load baseline model and prepare targets & base evaluation
    print(f"\nLoading baseline model from {BASE_MODEL}...")
    tokenizer = AutoTokenizer.from_pretrained(BASE_MODEL, trust_remote_code=True)
    tokenizer.padding_side = "right"
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token
    
    model = AutoModelForCausalLM.from_pretrained(BASE_MODEL, torch_dtype=torch.float16).to(DEVICE)
    model.config.use_cache = False
    model.gradient_checkpointing_enable()
    model.eval()

    # Generate semantic anchors
    sem_examples = []
    print("Generating semantic anchor targets for alignment:")
    for idx, (prompt, _) in enumerate(BASELINE_SEMANTIC_TESTS):
        inputs = tokenizer(prompt, return_tensors='pt').to(DEVICE)
        with torch.no_grad():
            out = model.generate(**inputs, max_new_tokens=32, do_sample=False, pad_token_id=tokenizer.eos_token_id)
        sem_examples.append({
            "prompt": prompt,
            "completion": " " + tokenizer.decode(out[0][inputs['input_ids'].shape[1]:], skip_special_tokens=True).strip()
        })

    # Evaluate clean model baseline semantic score before modification
    sem_base = evaluate_semantic(model, tokenizer)
    print(f"  Baseline Semantic Score: {sem_base}/15")
    sys.stdout.flush()

    # Load SFT dataset
    if not os.path.exists(SFT_DATA_PATH):
        print(f"Error: SFT data not found at {SFT_DATA_PATH}.")
        sys.exit(1)
    with open(SFT_DATA_PATH, "r", encoding="utf-8") as f:
        sft_data = json.load(f)

    sft_groups = {
        "lu": [item for item in sft_data if item["type"] == "language_u"],
        "rf": [item for item in sft_data if item["type"] == "rf_info_theory"],
        "mmlu": [item for item in sft_data if item["type"] == "mmlu"],
        "gsm": [item for item in sft_data if item["type"] == "gsm8k"],
        "sem": sem_examples
    }

    base_p = dict(model.named_parameters())

    # 4. Decode and apply SVD weight deltas in-place
    weight_data = data[off:off+w_len]
    w_pos = 0
    print("\nDecoding SVD weight deltas in-place...")
    for i in range(n_layers):
        lname = LAYER_NAMES[i]
        W_b = base_p[lname].data.to(torch.float32).cpu().numpy()
        W_delta, w_pos = decode_layer_delta(weight_data, w_pos, W_b, level)
        
        # Inject weights in float16
        with torch.no_grad():
            delta_tensor = torch.from_numpy(W_delta).to(DEVICE, dtype=torch.float16)
            base_p[lname].data.add_(delta_tensor)
        print(f"  Reconstructed {lname.split('.')[-2]} via L{level} SVD Eigenspace")
    sys.stdout.flush()

    # 5. Evaluate pre-training fidelity
    print("\nEvaluating pre-training scores...")
    fid_before = evaluate_fidelity(model, tokenizer)
    sys.stdout.flush()

    # 6. Run on-device multi-task training
    recipe = {
        "lr": float(lr_f16),
        "num_steps": n_steps,
        "seed": seed,
        "batch_size": batch,
    }
    stats = train_multitask(model, tokenizer, sft_groups, recipe)

    # 7. Evaluate post-training scores
    print("\nEvaluating post-training scores...")
    fid_after = evaluate_fidelity(model, tokenizer)
    sem_after = evaluate_semantic(model, tokenizer)
    sys.stdout.flush()

    # Save output
    os.makedirs(OUTPUT_MODEL, exist_ok=True)
    model.save_pretrained(OUTPUT_MODEL)
    tokenizer.save_pretrained(OUTPUT_MODEL)

    print("\n" + "=" * 72)
    print("  TINYQWEN ALIGNED RESTORATION SUCCESS")
    print("=" * 72)
    print(f"  Fidelity Before:   {fid_before:.1f}%")
    print(f"  Fidelity After:    {fid_after:.1f}%")
    print(f"  Semantic Before:   {sem_base}/15")
    print(f"  Semantic After:    {sem_after}/15")
    print(f"  Loss Initial/Final:{stats['initial_loss']:.4f} / {stats['final_loss']:.4f}")
    print(f"  Output Model:      {OUTPUT_MODEL}")
    print("=" * 72)
    sys.stdout.flush()

if __name__ == '__main__':
    import random
    main()
