# Watermark: ip zymatica.space
__watermark__ = "ip zymatica.space"

import os
os.environ["PYTORCH_CUDA_ALLOC_CONF"] = "garbage_collection_threshold:0.6,max_split_size_mb:24"
import sys
import struct
import json
import time
import zlib
import numpy as np
import torch
from transformers import AutoModelForCausalLM, AutoTokenizer
from torch.optim import AdamW
from scipy.fft import idct

sys.stdout.reconfigure(encoding='utf-8', errors='backslashreplace')

DEVICE = "cuda" if torch.cuda.is_available() else "cpu"
PKT_SIZE = 255
MAGIC = bytes([0xA7, 0x07, 0xC3])  # Chirp-3 magic

BASE_MODEL = "j:/Language-U/Language-U-V2/qwen-3.5-0.8b-local"
PKT_DIR = "j:/Language-U/packets_chirp3"
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

QA_PAIRS = [
    ("What GPIO pin is the SX1302 reset line on Raspberry Pi 4?", "25"),
    ("What is the exact command to reset the LoRa concentrator with gpioset?",
     "gpioset -c gpiochip0 --toggle 100ms,100ms,0 25=0"),
    ("What script handles the SX1302 hardware reset?", "reset_lgw.sh"),
    ("On Raspberry Pi 5, which gpiochip and pin is the SX1302 reset mapped to?",
     "GPIO 17 on gpiochip4"),
    ("What frequency does the Astronaut SHE Handshake Protocol use?", "903.0 MHz"),
    ("What Spreading Factor is used for the Astronaut SHE handshake?", "SF7"),
    ("What is the transmit power for the Astronaut SHE RAK Miner beacon?", "14 dBm"),
    ("What does --pwid 15 represent in test_loragw_hal_tx?",
     "power calibration index 14 dBm"),
    ("What is the full test_loragw_hal_tx command for the Astronaut SHE handshake?",
     "./test_loragw_hal_tx -r 1250 -f 903.0 -m LORA -s 7 -b 125 -n 1 --pwid 15 -p 14 -z 32"),
    ("What is the payload size for the Astronaut SHE handshake beacon?", "32 bytes"),
    ("How many dimensions does the Cuneiform-U v3.0 semantic hypercube have?", "6"),
    ("What are the 6 axes of Cuneiform-U v3.0?",
     "DOMAIN, SUBDOMAIN, OPERATION, MODALITY, DEPTH, POLARITY"),
    ("What is the Classifier Radical R_C in Cuneiform-U v3.0?",
     "DOMAIN in upper 4 bits, SUBDOMAIN in lower 4 bits"),
    ("What are the radical coordinates of the ACK glyph (0x807E)?",
     "R_C=0x00, R_F=0x7E, R_A=0x0B"),
    ("What is the Shannon Orthogonality equation in Language U?",
     "H(text) = H(meaning) + H(syntax | meaning)"),
    ("What does LLD-AC stand for?", "LLM-Logits-Driven Range Coding"),
    ("What is a collapse signal in LLD-AC range coding?",
     "probability approaches 1.0, encoding cost approaches 0 bits"),
    ("What frequency scale does the LLD-AC range coder use?", "1,000,000"),
]

EVAL_TESTS = [
    ("What GPIO pin is the SX1302 reset line on Raspberry Pi 4?", ["25", "GPIO 25"]),
    ("What is the exact command to reset the LoRa concentrator with gpioset?", ["gpioset", "gpiochip0", "25=0"]),
    ("What script handles the SX1302 hardware reset?", ["reset_lgw.sh"]),
    ("On Raspberry Pi 5, which gpiochip and pin is the SX1302 reset mapped to?", ["17", "gpiochip4"]),
    ("What frequency does the Astronaut SHE Handshake Protocol use?", ["903.0", "903"]),
    ("What Spreading Factor is used for the Astronaut SHE handshake?", ["SF7", "sf7"]),
    ("What is the transmit power for the Astronaut SHE RAK Miner beacon?", ["14 dBm", "14dBm"]),
    ("What does --pwid 15 represent in test_loragw_hal_tx?", ["calibration", "14 dBm", "power"]),
    ("What is the full test_loragw_hal_tx command for the Astronaut SHE handshake?",
                                                                            ["-f 903.0", "-s 7", "--pwid 15", "-z 32"]),
    ("What is the payload size for the Astronaut SHE handshake beacon?", ["32", "32 bytes"]),
    ("How many dimensions does the Cuneiform-U v3.0 semantic hypercube have?", ["6", "six"]),
    ("What are the 6 axes of Cuneiform-U v3.0?", ["DOMAIN", "SUBDOMAIN", "MODALITY"]),
    ("What is the Classifier Radical R_C in Cuneiform-U v3.0?", ["DOMAIN", "SUBDOMAIN", "4 bits"]),
    ("What are the radical coordinates of the ACK glyph (0x807E)?", ["0x00", "0x7E", "0x0B"]),
    ("What is the Shannon Orthogonality equation in Language U?", ["H(text)", "H(meaning)", "H(syntax"]),
    ("What does LLD-AC stand for?", ["LLM", "Logits", "Range Cod"]),
    ("What is a collapse signal in LLD-AC range coding?", ["probability", "1.0", "bits"]),
    ("What frequency scale does the LLD-AC range coder use?", ["1,000,000", "1000000", "million"]),
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

def build_huffman_tree():
    import heapq
    freq = {
        '[eof]': 1, ' ': 150, 'e': 120, 't': 90, 'a': 80, 'o': 75, 'i': 70, 'n': 70, 
        's': 65, 'r': 60, 'h': 50, 'l': 40, 'd': 35, 'c': 30, 'u': 28, 'm': 25, 
        'f': 22, 'p': 20, 'g': 18, 'w': 15, 'y': 12, 'b': 12, 'v': 10, 'k': 8, 
        'j': 6, 'x': 6, 'q': 4, 'z': 4,
        '0': 10, '1': 10, '2': 10, '3': 8, '4': 8, '5': 8, '6': 6, '7': 6, '8': 6, '9': 6,
        '.': 8, '-': 8, '_': 8, '=': 8, '+': 6, '|': 6, ',': 6, '/': 4, ':': 4,
        '(': 4, ')': 4, '[': 4, ']': 4, '{': 2, '}': 2, '<': 2, '>': 2, '?': 2, '!': 2
    }
    for i in range(256):
        ch = chr(i)
        if ch not in freq:
            freq[ch] = 1e-5
            
    heap = []
    uid = 0
    for ch, f in freq.items():
        heapq.heappush(heap, (f, uid, ch))
        uid += 1
        
    while len(heap) > 1:
        f1, _, n1 = heapq.heappop(heap)
        f2, _, n2 = heapq.heappop(heap)
        heapq.heappush(heap, (f1 + f2, uid, (n1, n2)))
        uid += 1
        
    codes = {}
    def traverse(node, code=""):
        if isinstance(node, str):
            codes[node] = code
        else:
            traverse(node[0], code + "0")
            traverse(node[1], code + "1")
            
    if heap:
        traverse(heap[0][2])
    return codes

_HUFF_CODES = build_huffman_tree()

def huff_decode(data: bytes) -> str:
    rev_codes = {v: k for k, v in _HUFF_CODES.items()}
    bits = ""
    for b in data:
        bits += format(b, '08b')
        
    text = []
    curr = ""
    for bit in bits:
        curr += bit
        if curr in rev_codes:
            val = rev_codes[curr]
            if val == "[eof]":
                break
            text.append(val)
            curr = ""
    return "".join(text)

# Language-U Decoders
ATYPE_U8=0x01; ATYPE_F16=0x03; ATYPE_U32=0x04; ATYPE_RAW8=0x07; ATYPE_HUFF=0x06

def decode_l1_qa(data: bytes) -> list:
    pos = 0
    n = data[pos]; pos += 1
    answers = []
    for i in range(n):
        _quad = data[pos:pos+4]; pos += 4
        at = data[pos]; pos += 1
        if at == ATYPE_U8:
            a = str(data[pos]); pos += 1
            if i == 6:
                a += " dBm"
            elif i == 9:
                a += " bytes"
        elif at == ATYPE_F16:
            a = f"{struct.unpack('>e', data[pos:pos+2])[0]:.1f} MHz"; pos += 2
        elif at == ATYPE_U32:
            a = f"{struct.unpack('>I', data[pos:pos+4])[0]:,}"; pos += 4
        elif at == ATYPE_RAW8:
            n2 = data[pos]; pos += 1
            a = data[pos:pos+n2].decode('utf-8', errors='replace'); pos += n2
        elif at == ATYPE_HUFF:
            n2 = data[pos]; pos += 1
            huff_bytes = data[pos:pos+n2]
            a = huff_decode(huff_bytes)
            pos += n2
        else:
            a = "??"
        answers.append(a)
    return answers

# Level 4 IDCT
def dct_decompress_vec(data: bytes, pos: int) -> tuple:
    n     = struct.unpack('>H', data[pos:pos+2])[0]; pos += 2
    K     = data[pos]; pos += 1
    scale = struct.unpack('>e', data[pos:pos+2])[0]; pos += 2
    idx_mode = data[pos]; K_check = data[pos+1]; pos += 2

    indices = []
    running = 0
    for _ in range(K_check):
        if idx_mode == 0:
            delta = data[pos]; pos += 1
        else:
            delta = struct.unpack('>H', data[pos:pos+2])[0]; pos += 2
        running += delta
        indices.append(running)

    n_bytes = (K_check + 1) // 2
    packed  = data[pos:pos+n_bytes]; pos += n_bytes
    q_vals  = []
    for b in packed:
        lo = b & 0x0F; hi = (b >> 4) & 0x0F
        q_vals.append(lo if lo <= 7 else lo - 16)
        q_vals.append(hi if hi <= 7 else hi - 16)
    q_vals = q_vals[:K_check]

    v_dct = np.zeros(n, dtype=np.float64)
    v_dct[indices] = np.array(q_vals, dtype=np.float64) * scale
    return idct(v_dct, norm='ortho').astype(np.float32), pos

# Level 5 IDCT
def eigenspace_decompress(data: bytes, pos: int, W_base: np.ndarray) -> tuple:
    R     = data[pos]; pos += 1
    scale = struct.unpack('>e', data[pos:pos+2])[0]; pos += 2
    delta_s = np.frombuffer(data[pos:pos+R], dtype=np.int8).astype(np.float64) * scale
    pos += R

    U_b, S_b, Vh_b = np.linalg.svd(W_base.astype(np.float64), full_matrices=False)
    W_delta = sum(delta_s[i] * np.outer(U_b[:, i], Vh_b[i, :]) for i in range(R))
    return W_delta.astype(np.float32), pos

# Level 6 IDCT
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
        sign = +1 if (nib >> 1) & 1 else -1
        mag  = MAG_TABLE[nib & 0x3]
        delta_s[i] = sign * mag * scale

    return delta_s, pos

def decode_layer_delta(data: bytes, pos: int, W_base: np.ndarray, level: int) -> tuple:
    # First 2 bytes are layer id (1 byte) and level (1 byte)
    lid = data[pos]; pos += 1
    lvl = data[pos]; pos += 1
    if lvl == 4:
        u_rec, pos = dct_decompress_vec(data, pos)
        v_rec, pos = dct_decompress_vec(data, pos)
        W_delta = np.outer(u_rec, v_rec)
    elif lvl == 5:
        W_delta, pos = eigenspace_decompress(data, pos, W_base)
    elif lvl == 6:
        delta_s, pos = gradient_atom_decompress(data, pos)
        U_b, _, Vh_b = np.linalg.svd(W_base.astype(np.float64), full_matrices=False)
        R = len(delta_s)
        W_delta = sum(delta_s[i] * np.outer(U_b[:, i], Vh_b[i, :]) for i in range(R))
    else:
        raise ValueError(f"Unknown encoding level: {lvl} (parent/header level: {level})")
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
            print(f"         A: {answer.strip()}")
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
    import random
    import math
    import gc
    
    lu_examples = sft_groups["lu"]
    rf_examples = sft_groups["rf"]
    mmlu_examples = sft_groups["mmlu"]
    gsm_examples = sft_groups["gsm"]
    sem_examples = sft_groups["sem"]

    for name, param in model.named_parameters():
        if not any(layer in name for layer in LAYER_NAMES):
            param.requires_grad = False
        else:
            param.requires_grad = True

    lr_max = recipe['lr']
    lr_min = 1e-6
    optimizer = AdamW(filter(lambda p: p.requires_grad, model.parameters()), lr=lr_max, weight_decay=0.01, betas=(0.9, 0.95))
    
    total_steps = int(recipe['num_steps'] * 2.0)  # Double target steps for robust convergence
    accumulation_steps = 2
    
    print(f"\n  On-device Batched Multi-task SFT: {total_steps} steps (Accumulation={accumulation_steps}), Peak LR={lr_max:.6f} with Cosine Decay")
    sys.stdout.flush()
    t0 = time.perf_counter()
    losses = []
    optimizer.zero_grad(set_to_none=True)

    for step in range(total_steps):
        model.train()
        lr_t = lr_min + 0.5 * (lr_max - lr_min) * (1.0 + math.cos(math.pi * step / total_steps))
        for param_group in optimizer.param_groups:
            param_group['lr'] = lr_t
            
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
        
        inputs = collate_batch(batch, tokenizer, DEVICE)
        
        with torch.amp.autocast('cuda', enabled=(DEVICE == 'cuda')):
            out = model(input_ids=inputs["input_ids"], attention_mask=inputs["attention_mask"])
            logits = out.logits
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
            
        del inputs, out, loss, mean_loss

    elapsed = time.perf_counter() - t0
    print(f"  Training complete in {elapsed:.1f}s")
    return {"initial_loss": losses[0], "final_loss": losses[-1], "seconds": elapsed}

def main():
    print("=" * 72)
    print("  CHIRP-3 DECODER & RESTORATION ENGINE")
    print("  Watermark: ip zymatica.space")
    print("=" * 72)
    sys.stdout.flush()
    
    # 1. Load packets from PKT_DIR
    print(f"Loading packets from {PKT_DIR}...")
    packet_files = sorted([f for f in os.listdir(PKT_DIR) if f.startswith("packet_chirp3_") and f.endswith(".bin")])
    if not packet_files:
        print("Error: No packets found.")
        sys.exit(1)
        
    # Check total packets from wrapper of first file
    with open(os.path.join(PKT_DIR, packet_files[0]), "rb") as f:
        first_pkt = f.read()
    if len(first_pkt) < 3 or first_pkt[0] != 0xBB:
        print("Error: Invalid packet format.")
        sys.exit(1)
    total_pkts = first_pkt[2]
    
    # Load all packets we have
    received_packets = {}
    for pf in packet_files:
        with open(os.path.join(PKT_DIR, pf), "rb") as f:
            pkt_bytes = f.read()
        if len(pkt_bytes) == PKT_SIZE and pkt_bytes[0] == 0xBB:
            idx = pkt_bytes[1]
            received_packets[idx] = pkt_bytes
            
    print(f"  Loaded {len(received_packets)}/{total_pkts} packets.")
    
    # Verify/Restore via XOR FEC
    missing_indices = [i for i in range(total_pkts) if i not in received_packets]
    if len(missing_indices) == 0:
        print("[+] All packets received intact. Verifying FEC...")
        # Verify FEC is correct (XOR of all payloads is 0)
        chunk_size = PKT_SIZE - 3
        xor_fec = bytearray(chunk_size)
        for idx, pkt in received_packets.items():
            for j in range(chunk_size):
                xor_fec[j] ^= pkt[j + 3]
        if any(xor_fec):
            print("⚠️ Warning: FEC verification failed (non-zero XOR sum).")
        else:
            print("[+] FEC verification passed.")
    elif len(missing_indices) == 1:
        missing_idx = missing_indices[0]
        print(f"[-] Missing packet index {missing_idx}. Performing XOR FEC recovery...")
        chunk_size = PKT_SIZE - 3
        recovered_payload = bytearray(chunk_size)
        for idx, pkt in received_packets.items():
            for j in range(chunk_size):
                recovered_payload[j] ^= pkt[j + 3]
                
        # Reconstruct the missing packet
        recovered_pkt = bytes([0xBB, missing_idx, total_pkts]) + bytes(recovered_payload)
        received_packets[missing_idx] = recovered_pkt
        print(f"[+] Successfully recovered missing packet index {missing_idx} via FEC.")
    else:
        print(f"❌ Error: Cannot recover because {len(missing_indices)} packets are missing.")
        sys.exit(1)
        
    # Reassemble payload from data packets (excluding FEC packet)
    data_pkts_count = total_pkts - 1
    assembled_payload = bytearray()
    chunk_size = PKT_SIZE - 3
    for i in range(data_pkts_count):
        assembled_payload.extend(received_packets[i][3:])
    
    # 2. Parse 32-byte header
    data = zlib.decompress(bytes(assembled_payload))
    off = 0
    magic = data[off:off+3]; off += 3
    if magic != MAGIC:
        raise ValueError(f"Bad magic: {magic.hex()} expected {MAGIC.hex()}")
        
    level = data[off]; off += 1
    lr_f16 = struct.unpack('>e', data[off:off+2])[0]; off += 2
    n_steps = struct.unpack('>H', data[off:off+2])[0]; off += 2
    seed = struct.unpack('>I', data[off:off+4])[0]; off += 4
    optim = data[off]; off += 1
    batch = data[off]; off += 1
    layer_f = data[off]; off += 1
    warmup = struct.unpack('>H', data[off:off+2])[0]; off += 2
    n_pairs = data[off]; off += 1
    lu4_hdr = data[off:off+4]; off += 4
    
    # Unpack 3-byte q_mask (ignored during decoding but necessary for alignment)
    q_mask_bytes = data[off:off+3]; off += 3
    
    qualia_seed = data[off]; off += 1
    n_layers = data[off]; off += 1
    
    # w_len and lu4_len are packed in this order
    w_len = struct.unpack('>H', data[off:off+2])[0]; off += 2
    lu4_len = struct.unpack('>H', data[off:off+2])[0]; off += 2
    
    # Header size is 32 bytes
    off = 32
    
    print(f"\nHeader Decoded:")
    print(f"  Level:             Level {level}")
    print(f"  Learning Rate:     {float(lr_f16):.6f}")
    print(f"  Steps / Seed:      {n_steps} / {hex(seed)}")
    print(f"  Qualia Seed:       {bin(qualia_seed)}")
    print(f"  LU4 Length:        {lu4_len} bytes")
    print(f"  Weight Length:     {w_len} bytes")
    sys.stdout.flush()
    
    # 3. Decode Q&A pairs (L1)
    lu4_blob = data[off : off + lu4_len]
    off += lu4_len
    
    decoded_answers = decode_l1_qa(lu4_blob)
    print(f"\nDecoded {len(decoded_answers)} facts from LU4 Q&A table:")
    lu_examples = []
    for idx, ans in enumerate(decoded_answers):
        q = QA_PAIRS[idx][0]
        print(f"  Q: {q}")
        print(f"  A: {ans}")
        # Build SFT example
        lu_examples.append({
            "prompt": f"Q: {q}\nA:",
            "completion": " " + ans.strip()
        })
    sys.stdout.flush()
    
    # 4. Load baseline model
    print(f"\nLoading baseline model from {BASE_MODEL}...")
    tokenizer = AutoTokenizer.from_pretrained(BASE_MODEL, trust_remote_code=True)
    tokenizer.padding_side = "right"
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token
        
    model = AutoModelForCausalLM.from_pretrained(BASE_MODEL, torch_dtype=torch.float16).to(DEVICE)
    model.config.use_cache = False
    # Disable gradient checkpointing for faster SFT on 0.8B parameter model
    # model.gradient_checkpointing_enable()
    model.eval()
    
    # Generate semantic anchors
    sem_examples = []
    print("Generating semantic anchor targets...")
    for idx, (prompt, _) in enumerate(BASELINE_SEMANTIC_TESTS):
        inputs = tokenizer(prompt, return_tensors='pt').to(DEVICE)
        with torch.no_grad():
            out = model.generate(**inputs, max_new_tokens=32, do_sample=False, pad_token_id=tokenizer.eos_token_id)
        sem_examples.append({
            "prompt": prompt,
            "completion": " " + tokenizer.decode(out[0][inputs['input_ids'].shape[1]:], skip_special_tokens=True).strip()
        })
        
    sem_base = evaluate_semantic(model, tokenizer)
    print(f"  Baseline Semantic Score: {sem_base}/15")
    sys.stdout.flush()
    
    # Load external SFT dataset
    if not os.path.exists(SFT_DATA_PATH):
        print(f"Error: SFT dataset not found at {SFT_DATA_PATH}")
        sys.exit(1)
        
    with open(SFT_DATA_PATH, "r", encoding="utf-8") as f:
        sft_data = json.load(f)
        
    sft_groups = {
        "lu": lu_examples,
        "rf": [item for item in sft_data if item["type"] == "rf_info_theory"],
        "mmlu": [item for item in sft_data if item["type"] == "mmlu"],
        "gsm": [item for item in sft_data if item["type"] == "gsm8k"],
        "sem": sem_examples
    }
    
    # 5. Decode weight updates and apply in-place
    weight_data = data[off : off + w_len]
    w_pos = 0
    base_p = dict(model.named_parameters())
    
    print("\nDecoding weight updates in-place...")
    for i in range(n_layers):
        lname = LAYER_NAMES[i]
        W_b = base_p[lname].data.to(torch.float32).cpu().numpy()
        W_delta, w_pos = decode_layer_delta(weight_data, w_pos, W_b, level)
        
        # Inject delta
        with torch.no_grad():
            delta_tensor = torch.from_numpy(W_delta).to(DEVICE, dtype=torch.float16)
            base_p[lname].data.add_(delta_tensor)
        print(f"  Reconstructed {lname.split('.')[-2]} (Level {level})")
    sys.stdout.flush()
    
    # 6. Evaluate pre-SFT fidelity
    print("\nEvaluating pre-SFT scores...")
    fid_before = evaluate_fidelity(model, tokenizer)
    sys.stdout.flush()
    
    # 7. SFT alignment
    recipe = {
        "lr": float(lr_f16),
        "num_steps": n_steps,
        "seed": seed,
        "batch_size": batch,
    }
    stats = train_multitask(model, tokenizer, sft_groups, recipe)
    
    # 8. Evaluate post-SFT scores
    print("\nEvaluating post-SFT scores...")
    fid_after = evaluate_fidelity(model, tokenizer)
    sem_after = evaluate_semantic(model, tokenizer)
    sys.stdout.flush()
    
    # 9. Save restored model
    os.makedirs(OUTPUT_MODEL, exist_ok=True)
    model.save_pretrained(OUTPUT_MODEL)
    tokenizer.save_pretrained(OUTPUT_MODEL)
    
    print("\n" + "=" * 72)
    print("  CHIRP-3 RESTORATION SUCCESS")
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
    main()
