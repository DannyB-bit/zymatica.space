# Kaggle SFT Kernel Packager for qwen-3.5-0.8b-DNA-GROW
# Watermark: ip zymatica.space | astronautshe.com

import os
import sys
import json
import base64
import zlib

def b64_file(path):
    with open(path, "rb") as f:
        return base64.b64encode(f.read()).decode('utf-8')

def b64_zlib_file(path):
    with open(path, "rb") as f:
        compressed = zlib.compress(f.read(), level=9)
        return base64.b64encode(compressed).decode('utf-8')

def main():
    target_dir = "j:/Language-U/kaggle_dna_grow"
    os.makedirs(target_dir, exist_ok=True)
    
    print("=" * 80)
    print("  PACKAGING DNA-GROW KAGGLE TRAINING KERNEL")
    print("=" * 80)
    
    # Payload Paths
    seed_path = "j:/Language-U/DnaGrowSeed.LLM"
    sft_path  = "j:/Language-U/brain_sft_dataset.json"
    vocab_path = "j:/Language-U/qwen_vocab_cuneiform.bin"
    
    if not os.path.exists(seed_path):
        print(f"Error: {seed_path} not found. Please compile the seed first using build_dnagrow_seed.py")
        sys.exit(1)
        
    if not os.path.exists(sft_path):
        print(f"Warning: {sft_path} not found. Falling back to full_sft_dataset.json...")
        sft_path = "j:/Language-U/full_sft_dataset.json"
        if not os.path.exists(sft_path):
            print("Error: No SFT dataset found!")
            sys.exit(1)
            
    if not os.path.exists(vocab_path):
        print(f"Error: {vocab_path} not found.")
        sys.exit(1)
        
    print("Reading and encoding payload files...")
    seed_b64 = b64_file(seed_path)
    sft_b64  = b64_file(sft_path)
    vocab_b64 = b64_zlib_file(vocab_path)
    
    # Kernel Code Template
    template = """# Qwen-3.5-0.8B-DNA-GROW: SFT Healing (RCRA Loss + EHSS Validation)
# Watermark: ip zymatica.space | astronautshe.com

import os, sys, json, time, warnings, base64, zlib, struct, shutil
warnings.filterwarnings("ignore")

print("Installing transformers and PEFT dependencies...")
os.system("pip install -q git+https://github.com/huggingface/transformers.git peft scipy safetensors huggingface_hub accelerate")

import torch
import torch.nn as nn
from pathlib import Path
from torch.utils.data import Dataset, DataLoader
from transformers import AutoTokenizer, AutoModelForCausalLM, get_scheduler, LogitsProcessor, LogitsProcessorList
from torch.optim import AdamW
from peft import get_peft_model, LoraConfig, TaskType
from safetensors.torch import save_file
from huggingface_hub import login, create_repo, upload_folder
import numpy as np

# SciPy DCT Fallback if not installed
try:
    from scipy.fft import idct
except ImportError:
    import math
    def idct(v, norm='ortho'):
        N = len(v)
        n = np.arange(N); k = n.reshape((N,1))
        M = np.cos(np.pi * k * (2*n+1) / (2*N))
        out = np.zeros(N, dtype=np.float64)
        for i in range(N):
            term = 0.0
            for j in range(N):
                c = 1.0 / math.sqrt(2) if j == 0 else 1.0
                term += c * v[j] * M[j, i]
            out[i] = term
        if norm == 'ortho':
            out[0] *= math.sqrt(2)
            out *= math.sqrt(2.0 / N) / 2.0
            out *= 2.0
        return out

HF_TOKEN       = "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"
REPO_ID        = "TheAiCollectiveART/qwen-3.5-0.8b-DNA-GROW"
BASE_MODEL_DIR = Path("/kaggle/working/reconstructed_base")
OUTPUT_DIR     = Path("/kaggle/working/qwen-3.5-0.8b-dnagrow-trained")

# Training Parameters
MAX_SEQ_LEN    = 192
BATCH_SIZE     = 2
GRAD_ACCUM     = 8
EPOCHS         = 3
LR             = 2e-4
LORA_R         = 32
LORA_ALPHA     = 64
COORD_ALPHA    = 0.5   # RCRA loss scale coefficient

# Extract embedded files
print("Extracting payloads...")
os.makedirs("/kaggle/working", exist_ok=True)

with open("/kaggle/working/DnaGrowSeed.LLM", "wb") as f:
    f.write(base64.b64decode("{seed_b64}"))

with open("/kaggle/working/brain_sft_dataset.json", "wb") as f:
    f.write(base64.b64decode("{sft_b64}"))

with open("/kaggle/working/qwen_vocab_cuneiform.bin", "wb") as f:
    f.write(zlib.decompress(base64.b64decode("{vocab_b64}")))

print("Payloads extracted.")

# ── Standalone Reconstruction ──────────────────────────────────────────────────
print("Decompressing SVD seed format...")
with open("/kaggle/working/DnaGrowSeed.LLM", "rb") as f_in:
    raw_genesis_data = zlib.decompress(f_in.read())

def get_dictionary(dim, dictionary_size, seed):
    rng = np.random.RandomState(seed)
    dict_mat = rng.standard_normal((dim, dictionary_size)).astype(np.float32)
    norms = np.linalg.norm(dict_mat, axis=0, keepdims=True) + 1e-9
    return dict_mat / norms

def unpack_2bit(packed_bytes, K):
    syms = []
    for b in packed_bytes:
        syms += [b & 3, (b>>2)&3, (b>>4)&3, (b>>6)&3]
    return syms[:K]

def decompress_grad_vec(col_data, n):
    off = 0
    orig_n = struct.unpack('>H', col_data[off:off+2])[0]; off += 2
    K      = col_data[off]; off += 1
    scale  = struct.unpack('>e', col_data[off:off+2])[0]; off += 2
    idx_mode = col_data[off]; K_check = col_data[off+1]; off += 2

    idx_size = K_check*2 if idx_mode == 1 else K_check
    idx_bytes = col_data[off:off+idx_size]; off += idx_size
    indices, running = [], 0
    if idx_mode == 0:
        for b in idx_bytes:
            running += b; indices.append(running)
    else:
        for i in range(0, idx_size, 2):
            running += struct.unpack('>H', idx_bytes[i:i+2])[0]; indices.append(running)

    packed_len = (K_check + 3) // 4
    symbols = unpack_2bit(col_data[off:off+packed_len], K_check); off += packed_len

    q_vals = []
    for s in symbols:
        sign = (s>>1)&1; mag = s&1
        q_vals.append((5.5 if mag else 1.5) * (1 if sign else -1))

    v_dct = np.zeros(orig_n, dtype=np.float64)
    v_dct[indices] = np.array(q_vals) * scale
    return idct(v_dct, norm='ortho').astype(np.float32), off

def decompress_matrix(data, rank, dim):
    cols, pos = [], 0
    for _ in range(rank):
        col, consumed = decompress_grad_vec(data[pos:], dim)
        cols.append(col); pos += consumed
    return np.stack(cols, axis=1)

print("Fetching base shapes / metadata structure from Qwen/Qwen3.5-0.8B...")
meta_model = AutoModelForCausalLM.from_pretrained("Qwen/Qwen3.5-0.8B", device_map="cpu", trust_remote_code=True)
tensor_meta = {k: (v.shape, v.dtype) for k, v in meta_model.state_dict().items()}
meta_state = meta_model.state_dict()

layer_map = {}
pos = 0

# Parse seed header
GENESIS_MAGIC = 0x47454E45
PERFECT_MAGIC = 0x50455246
magic = struct.unpack_from('>I', raw_genesis_data, pos)[0]; pos += 4
assert magic == GENESIS_MAGIC
version = struct.unpack_from('>H', raw_genesis_data, pos)[0]; pos += 2
assert version == 15, f"Expected v15, got {version}"
watermark = raw_genesis_data[pos : pos + 32].decode('utf-8', errors='ignore').strip(); pos += 32
perf_magic = struct.unpack_from('>I', raw_genesis_data, pos)[0]; pos += 4
assert perf_magic == PERFECT_MAGIC

hidden, heads, kv_heads, ffn_dim, blocks, vocab = struct.unpack_from('>IIIIII', raw_genesis_data, pos); pos += 24
mode_byte = raw_genesis_data[pos]; pos += 4
pos += 12 # skip reserved
num_layers = struct.unpack_from('>I', raw_genesis_data, pos)[0]; pos += 4

print(f"Reconstructing weights using Mode: {mode_byte}")
for idx in range(num_layers):
    name_len = struct.unpack_from('>H', raw_genesis_data, pos)[0]; pos += 2
    name = raw_genesis_data[pos : pos + name_len].decode('utf-8'); pos += name_len
    m, n, r = struct.unpack_from('>III', raw_genesis_data, pos); pos += 12
    
    dtype = tensor_meta.get(name, (None, torch.float16))[1]
    
    if mode_byte == 0x08:
        # Dictionary mode
        seed_u = MASTER_SEED + idx * 1000
        seed_v = MASTER_SEED + idx * 1000 + 500
        U_dict = get_dictionary(m, DICT_SIZE, seed_u)
        V_dict = get_dictionary(n, DICT_SIZE, seed_v)
        
        W_rec = np.zeros((m, n), dtype=np.float32)
        for rank in range(r):
            idx_u = raw_genesis_data[pos]; pos += 1
            idx_v = raw_genesis_data[pos]; pos += 1
            c = struct.unpack_from('>e', raw_genesis_data, pos)[0]; pos += 2
            W_rec += c * np.outer(U_dict[:, idx_u], V_dict[:, idx_v])
            
        layer_map[name] = torch.from_numpy(W_rec).to(dtype)
    else:
        # Spectral mode
        scale_u, scale_v = struct.unpack_from('>ff', raw_genesis_data, pos); pos += 8
        u_len, v_len = struct.unpack_from('>II', raw_genesis_data, pos); pos += 8
        u_data = raw_genesis_data[pos : pos + u_len]; pos += u_len
        v_data = raw_genesis_data[pos : pos + v_len]; pos += v_len
        U_rec = decompress_matrix(u_data, r, m)
        V_rec = decompress_matrix(v_data, r, n)
        W_rec = (U_rec * scale_u) @ (V_rec * scale_v).T
        pos += 1 # has_residual bool
        layer_map[name] = torch.from_numpy(W_rec).to(dtype)

# Assemble safetensors structure
reconstructed_tensors = {}
for k, (shape, dtype) in tensor_meta.items():
    if k in layer_map:
        t = layer_map[k]
        if t.shape == shape:
            reconstructed_tensors[k] = t.clone()
        elif t.T.shape == shape:
            reconstructed_tensors[k] = t.T.clone()
        else:
            reconstructed_tensors[k] = meta_state[k].to(dtype).clone()
    else:
        reconstructed_tensors[k] = meta_state[k].to(dtype).clone()

del meta_state, meta_model
import gc
gc.collect()

BASE_MODEL_DIR.mkdir(parents=True, exist_ok=True)
save_file(reconstructed_tensors, str(BASE_MODEL_DIR / "model.safetensors"))

# Copy configuration and tokenizers from base Qwen/Qwen3.5-0.8B
tok_base = AutoTokenizer.from_pretrained("Qwen/Qwen3.5-0.8B", trust_remote_code=True)
tok_base.save_pretrained(str(BASE_MODEL_DIR))

base_config = AutoModelForCausalLM.from_pretrained("Qwen/Qwen3.5-0.8B", device_map="meta").config
base_config.save_pretrained(str(BASE_MODEL_DIR))

print("[Success] Reconstructed base model safetensors built.")

# ─── Training Dataset ─────────────────────────────────────────────────────────
class SFTDataset(Dataset):
    def __init__(self, tokenizer, max_len, sft_path):
        self.tokenizer = tokenizer
        self.max_len   = max_len
        self.samples   = []
        
        with open(sft_path, encoding="utf-8") as f:
            data = json.load(f)
        for item in data:
            self.samples.append(item.get("prompt", "") + item.get("completion", ""))
            
        print(f"Loaded {len(self.samples)} SFT training samples.")

    def __len__(self):
        return len(self.samples)

    def __getitem__(self, idx):
        enc = self.tokenizer(
            self.samples[idx],
            max_length=self.max_len,
            truncation=True,
            padding="max_length",
            return_tensors="pt",
        )
        ids   = enc["input_ids"].squeeze(0)
        mask  = enc["attention_mask"].squeeze(0)
        lbl   = ids.clone()
        lbl[mask == 0] = -100
        return {"input_ids": ids, "attention_mask": mask, "labels": lbl}

# ─── Cognitive Fidelity Test & EHSS Implementation ───────────────────────────
EVAL_TESTS = [
    ("What is a Geometric Seed?", ["67 KB", "SVD", "geometry"]),
    ("What does the Curator do?", ["monitors", "reviews", "consolidate"]),
    ("Explain Reflexion-style learning.", ["correct", "failure", "critique"]),
]

def evaluate_ehss(model, tokenizer, device, enable_ehss=True):
    model.eval()
    passed = 0
    hook_handles = []
    logits_processor = None
    
    if enable_ehss:
        try:
            vocab_size = model.config.vocab_size
            evg_mask = torch.ones(vocab_size, dtype=torch.bool)
            for token_str, token_id in tokenizer.get_vocab().items():
                if token_id >= vocab_size: continue
                clean_str = token_str.replace("Ġ", "").replace(" ", "")
                try:
                    clean_str.encode('ascii')
                    is_english = True
                except UnicodeEncodeError:
                    is_english = False
                if not is_english:
                    evg_mask[token_id] = False
                    
            for tid in [tokenizer.eos_token_id, tokenizer.bos_token_id, tokenizer.pad_token_id]:
                if tid is not None:
                    evg_mask[tid] = True
            
            # Compute real English embedding centroid
            embed_weight = model.get_input_embeddings().weight.detach()
            english_indices = torch.nonzero(evg_mask).squeeze(-1).to(embed_weight.device)
            centroid = embed_weight[english_indices].mean(dim=0).to(device)
            centroid = centroid / (centroid.norm() + 1e-9)
            
            class EVGLogitsProcessor(LogitsProcessor):
                def __init__(self, mask):
                    self.mask = mask
                def __call__(self, input_ids: torch.LongTensor, logits: torch.FloatTensor) -> torch.FloatTensor:
                    mask_dev = self.mask.to(logits.device)
                    logits[:, ~mask_dev[:logits.shape[-1]]] = -float('inf')
                    return logits
                    
            logits_processor = LogitsProcessorList([EVGLogitsProcessor(evg_mask)])
            
            # Install HSDC hooks on last 25% of layer blocks
            layers = model.model.layers
            total_layers = len(layers)
            
            def make_hook(layer_idx):
                alpha = 0.005
                def hook_fn(module, input, output):
                    if layer_idx < (total_layers * 0.75):
                        return output
                    is_tuple = isinstance(output, tuple)
                    h = output[0] if is_tuple else output
                    h_norm = h.norm(dim=-1, keepdim=True)
                    h_normalized = h / (h_norm + 1e-9)
                    centroid_dev = centroid.to(device=h.device, dtype=h.dtype)
                    correction = alpha * (centroid_dev.view(1, 1, -1) - h_normalized) * h_norm
                    h_new = h + correction
                    return (h_new,) + output[1:] if is_tuple else h_new
                return hook_fn
                
            for idx, layer in enumerate(layers):
                hook_handles.append(layer.register_forward_hook(make_hook(idx)))
                
        except Exception as e:
            print(f"Error configuring EHSS: {e}")
            hook_handles = []
            logits_processor = None
            
    print("\n--- Running Cognitive Fidelity Assessments ---")
    with torch.no_grad():
        for q, kws in EVAL_TESTS:
            inp = tokenizer(f"Q: {q}\\nA:", return_tensors="pt").to(device)
            out = model.generate(
                **inp, max_new_tokens=40, do_sample=False,
                pad_token_id=tokenizer.eos_token_id,
                logits_processor=logits_processor
            )
            ans = tokenizer.decode(out[0][inp["input_ids"].shape[1]:], skip_special_tokens=True).strip()
            ok = any(kw.lower() in ans.lower() for kw in kws)
            passed += int(ok)
            print(f"  [{'PASS' if ok else 'FAIL'}] Q: {q} | A: {ans}")
            
    # Cleanup hooks
    for h in hook_handles:
        h.remove()
    model.train()
    return passed / len(EVAL_TESTS) * 100

# ─── SFT Optimization Loop ────────────────────────────────────────────────────
device = "cuda" if torch.cuda.is_available() else "cpu"
print(f"SFT Device: {device}")

tok = AutoTokenizer.from_pretrained(str(BASE_MODEL_DIR))
if tok.pad_token is None: tok.pad_token = tok.eos_token

model = AutoModelForCausalLM.from_pretrained(
    str(BASE_MODEL_DIR),
    torch_dtype=torch.float32,
    device_map="auto" if device == "cuda" else "cpu",
    trust_remote_code=True
)
model.config.use_cache = False

# Setup PEFT LoRA
all_mod = [n for n, _ in model.named_modules()]
targets = [t for t in ["q_proj","k_proj","v_proj","o_proj","gate_proj","up_proj","down_proj"] if any(n.endswith(t) for n in all_mod)]

peft_cfg = LoraConfig(
    task_type=TaskType.CAUSAL_LM,
    r=LORA_R, lora_alpha=LORA_ALPHA, lora_dropout=0.05,
    bias="none", target_modules=targets
)
model = get_peft_model(model, peft_cfg)
model.print_trainable_parameters()

# Load radicals
with open("/kaggle/working/qwen_vocab_cuneiform.bin", "rb") as f:
    bin_data = f.read()
coords = np.frombuffer(bin_data, dtype=np.uint8).reshape(len(bin_data) // 3, 3).astype(np.float32) / 255.0
coords_tensor = torch.tensor(coords, dtype=torch.float32).to(device)

# Align vocab sizes
vocab_dim = model.config.vocab_size
if coords_tensor.shape[0] < vocab_dim:
    padding = torch.zeros((vocab_dim - coords_tensor.shape[0], 3), dtype=torch.float32, device=device)
    coords_tensor = torch.cat([coords_tensor, padding], dim=0)
else:
    coords_tensor = coords_tensor[:vocab_dim]

dataset = SFTDataset(tok, MAX_SEQ_LEN, "/kaggle/working/brain_sft_dataset.json")
loader = DataLoader(dataset, batch_size=BATCH_SIZE, shuffle=True)
optimizer = AdamW(filter(lambda p: p.requires_grad, model.parameters()), lr=LR)
scheduler = get_scheduler("cosine", optimizer=optimizer, num_warmup_steps=10, num_training_steps=len(loader)*EPOCHS//GRAD_ACCUM)

print("\nEvaluating Pre-SFT Baseline...")
evaluate_ehss(model, tok, device, enable_ehss=True)

print(f"\nStarting SFT cognitive healing loop for {EPOCHS} epochs...")
model.train()
t_start = time.time()

for epoch in range(1, EPOCHS + 1):
    epoch_loss = 0.0
    optimizer.zero_grad()
    for step, batch in enumerate(loader, 1):
        input_ids = batch["input_ids"].to(device)
        attention_mask = batch["attention_mask"].to(device)
        labels = batch["labels"].to(device)
        
        out = model(input_ids=input_ids, attention_mask=attention_mask)
        logits = out.logits
        
        # Shift targets
        shift_logits = logits[..., :-1, :].contiguous()
        shift_labels = labels[..., 1:].contiguous()
        flat_logits = shift_logits.view(-1, shift_logits.size(-1))
        flat_labels = shift_labels.view(-1)
        
        active = (flat_labels != -100)
        if active.any():
            loss_ce = nn.CrossEntropyLoss()(flat_logits[active], flat_labels[active])
            
            # RCRA Coordinate Loss
            topk_logits, topk_indices = torch.topk(flat_logits[active].float(), k=256, dim=-1)
            probs = torch.softmax(topk_logits, dim=-1)
            topk_coords = coords_tensor[topk_indices]
            pred_coords = torch.bmm(probs.unsqueeze(1), topk_coords).squeeze(1)
            target_coords = coords_tensor[flat_labels[active]]
            loss_coord = torch.mean((pred_coords - target_coords) ** 2)
            
            loss_val = loss_ce + COORD_ALPHA * loss_coord
        else:
            loss_val = torch.tensor(0.0, device=device, requires_grad=True)
            
        loss = loss_val / GRAD_ACCUM
        loss.backward()
        epoch_loss += loss_val.item()
        
        if step % GRAD_ACCUM == 0 or step == len(loader):
            torch.nn.utils.clip_grad_norm_(model.parameters(), 1.0)
            optimizer.step()
            scheduler.step()
            optimizer.zero_grad()
            
    avg_loss = epoch_loss / len(loader)
    print(f"  Epoch {epoch}/{EPOCHS} complete | Loss: {avg_loss:.4f}")
    evaluate_ehss(model, tok, device, enable_ehss=True)

# Merge LoRA weights
print("\nSFT Complete. Merging LoRA adapters...")
model.eval()
merged = model.merge_and_unload()
OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
merged.save_pretrained(str(OUTPUT_DIR), safe_serialization=True)
tok.save_pretrained(str(OUTPUT_DIR))

# Upload directory to HF Hub
print(f"Uploading trained model weights to Hugging Face repository {REPO_ID}...")
login(token=HF_TOKEN, add_to_git_credential=False)
create_repo(repo_id=REPO_ID, token=HF_TOKEN, private=True, exist_ok=True)
upload_folder(
    folder_path=str(OUTPUT_DIR),
    repo_id=REPO_ID,
    repo_type="model",
    token=HF_TOKEN,
    commit_message="Add healed weights (DNA-GROW release)"
)
print("SUCCESS!")
"""
    
    final_code = template
    final_code = final_code.replace("{seed_b64}", seed_b64)
    final_code = final_code.replace("{sft_b64}", sft_b64)
    final_code = final_code.replace("{vocab_b64}", vocab_b64)
    
    # Pack to Notebook
    notebook_content = {
        "cells": [
            {
                "cell_type": "code",
                "execution_count": None,
                "metadata": {},
                "outputs": [],
                "source": [line + "\n" for line in final_code.splitlines()]
            }
        ],
        "metadata": {
            "kernelspec": {
                "display_name": "Python 3",
                "language": "python",
                "name": "python3"
            },
            "language_info": {
                "name": "python"
            }
        },
        "nbformat": 4,
        "nbformat_minor": 2
    }
    
    nb_path = os.path.join(target_dir, "dna_grow_train_kernel.ipynb")
    with open(nb_path, "w", encoding="utf-8") as f:
        json.dump(notebook_content, f, indent=1)
        
    # Kaggle metadata
    meta = {
      "id": "devs01/qwen-3-5-0-8b-dna-grow",
      "title": "qwen-3-5-0-8b-dna-grow",
      "code_file": "dna_grow_train_kernel.ipynb",
      "language": "python",
      "kernel_type": "notebook",
      "is_private": True,
      "enable_gpu": True,
      "machine_shape": "NvidiaTeslaT4",
      "enable_internet": True,
      "dataset_sources": [],
      "competition_sources": [],
      "kernel_sources": []
    }
    
    meta_path = os.path.join(target_dir, "kernel-metadata.json")
    with open(meta_path, "w") as f:
        json.dump(meta, f, indent=2)
        
    print(f"Kaggle package generated successfully inside {target_dir}")
    print("Files created:")
    print(f"  - dna_grow_train_kernel.ipynb")
    print(f"  - kernel-metadata.json")
    print("=" * 80)

if __name__ == "__main__":
    main()
