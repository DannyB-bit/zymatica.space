import os
import struct
import numpy as np
from transformers import AutoTokenizer

TOKENIZER_DIR = "j:/Language-U/Language-U-V2/qwen-3.5-0.8b-local"
OUTPUT_BIN = "j:/Language-U/qwen_vocab_cuneiform.bin"

def classify_token(token_str):
    # Normalize token string (replace special character G/Ġ representing space)
    s = token_str.replace("Ġ", " ").replace(" ", "").lower()
    
    # 1. DOMAIN & SUBDOMAIN
    domain = 0
    subdomain = 0
    
    # Domain 1: Hardware & LoRA Networks
    hw_net_kw = ['gpio', 'lora', 'chirp', 'reset', 'pin', 'spi', 'sx1302', 'pi4', 'pi5', 
                 'concentrator', 'antenna', 'frequency', 'mhz', 'dbm', 'packet', 'transmit', 
                 'tx', 'rx', 'sf7', 'fec', 'parity', 'duty', 'dwell', 'mac', 'gateway', 'beacon']
    # Domain 2: Mathematics, Logic & Information Theory
    math_kw = ['svd', 'dct', 'quant', 'math', 'entropy', 'shannon', 'gradient', 'atom', 
               'ortho', 'eigen', 'projection', 'reconstruct', 'rank', 'matrix', 'vector', 
               'basis', 'decompo', 'smooth', 'laplace', 'probability', 'logits', 'prior', 
               'bypass', 'q_proj', 'v_proj', 'k_proj', 'o_proj']
    # Domain 3: Dialogue, Persona & Authors
    dialogue_kw = ['zymatica', 'collective', 'dialogue', 'persona', 'chat', 'assistant', 
                   'speak', 'talk', 'bot', 'agent', 'she', 'astronaut', 'devsone', 'bouldiez', 
                   'partner', 'art']
    # Domain 4: Software, Systems & Runtimes
    sw_kw = ['rust', 'cpp', 'go', 'python', 'swift', 'java', 'typescript', 'compile', 
             'code', 'exec', 'run', 'lib', 'class', 'struct', 'header', 'import', 'from', 
             'package', 'build', 'cmake', 'cargo']

    if any(k in s for k in hw_net_kw):
        domain = 1
        if 'lora' in s or 'chirp' in s or 'fec' in s:
            subdomain = 1
        elif 'gpio' in s or 'pin' in s or 'reset' in s:
            subdomain = 2
        elif 'packet' in s or 'beacon' in s:
            subdomain = 3
    elif any(k in s for k in math_kw):
        domain = 2
        if 'svd' in s or 'matrix' in s or 'projection' in s:
            subdomain = 1
        elif 'entropy' in s or 'shannon' in s or 'bypass' in s:
            subdomain = 2
        elif 'logits' in s or 'prior' in s or 'smooth' in s:
            subdomain = 3
    elif any(k in s for k in dialogue_kw):
        domain = 3
        if 'zymatica' in s or 'collective' in s:
            subdomain = 1
        elif 'persona' in s or 'dialogue' in s or 'speak' in s:
            subdomain = 2
    elif any(k in s for k in sw_kw):
        domain = 4
        if 'rust' in s or 'go' in s or 'cpp' in s:
            subdomain = 1
        elif 'python' in s or 'typescript' in s or 'java' in s:
            subdomain = 2

    # 2. OPERATION (Actions)
    # Map key verbs to operation IDs (1 to 15)
    operations = [
        'reset', 'clear', 'toggle', 'write', 'read', 'set', 'get', 
        'encode', 'decode', 'compress', 'decompress', 'train', 'heal', 
        'eval', 'test', 'load', 'save', 'grow', 'shrink', 'bypass'
    ]
    operation = 0
    for i, op in enumerate(operations, 1):
        if op in s:
            operation = i % 16
            break

    # 3. MODALITY (Data layouts / formats)
    modalities = [
        'bin', 'zlib', 'json', 'capsule', 'genesis', 'llm', 'file', 
        'packet', 'byte', 'bit', 'char', 'string', 'token', 'wave', 'hal'
    ]
    modality = 0
    for i, mod in enumerate(modalities, 1):
        if mod in s:
            modality = i % 16
            break

    # 4. DEPTH (Complexity scale)
    depth = 0
    if domain == 1:
        if 'seed' in s or 'genesis' in s:
            depth = 8
        elif 'dct' in s:
            depth = 4
        elif 'atom' in s:
            depth = 6
        elif 'qa' in s or 'facts' in s:
            depth = 1
    else:
        depth = len(s) % 16

    # 5. POLARITY (Outcome states)
    polarity = 0
    pos_states = ['ack', 'success', 'ok', 'pass', 'valid', 'correct', 'true']
    neg_states = ['nack', 'fail', 'error', 'wrong', 'miss', 'warn', 'oom', 'crash', 'abort', 'false']
    if any(k in s for k in pos_states):
        polarity = 1
    elif any(k in s for k in neg_states):
        polarity = 2

    return domain, subdomain, operation, modality, depth, polarity

def main():
    print(f"Loading tokenizer from: {TOKENIZER_DIR}")
    tokenizer = AutoTokenizer.from_pretrained(TOKENIZER_DIR, trust_remote_code=True)
    vocab = tokenizer.get_vocab()
    vocab_size = len(vocab)
    print(f"Total vocabulary size: {vocab_size}")

    # Build the binary map in token ID order
    # Each record is 3 bytes: R_C, R_F, R_A
    packed_data = bytearray(vocab_size * 3)

    domain_counts = {0: 0, 1: 0, 2: 0, 3: 0, 4: 0}
    op_counts = 0
    polarity_counts = {0: 0, 1: 0, 2: 0}

    for token_str, token_id in vocab.items():
        if token_id >= vocab_size:
            continue
        
        domain, subdomain, operation, modality, depth, polarity = classify_token(token_str)

        # Pack into radicals
        rc = (domain << 4) | (subdomain & 0xF)
        rf = (operation << 4) | (modality & 0xF)
        ra = (depth << 4) | (polarity & 0xF)

        packed_data[token_id * 3]     = rc
        packed_data[token_id * 3 + 1] = rf
        packed_data[token_id * 3 + 2] = ra

        # Metrics collection
        domain_counts[domain] = domain_counts.get(domain, 0) + 1
        if operation > 0:
            op_counts += 1
        polarity_counts[polarity] = polarity_counts.get(polarity, 0) + 1

    # Write to output file
    with open(OUTPUT_BIN, "wb") as f:
        f.write(packed_data)

    print(f"\n[+] Successfully generated binary map at: {OUTPUT_BIN}")
    print(f"    Total tokens written: {vocab_size}")
    print("\n--- Mapping Classification Metrics ---")
    print(f"    Domain 0 (General English): {domain_counts[0]:,} tokens ({domain_counts[0]/vocab_size*100:.2f}%)")
    print(f"    Domain 1 (Hardware & LoRA): {domain_counts[1]:,} tokens ({domain_counts[1]/vocab_size*100:.2f}%)")
    print(f"    Domain 2 (Math & Info Theory): {domain_counts[2]:,} tokens ({domain_counts[2]/vocab_size*100:.2f}%)")
    print(f"    Domain 3 (Dialogue & Persona): {domain_counts[3]:,} tokens ({domain_counts[3]/vocab_size*100:.2f}%)")
    print(f"    Domain 4 (Software & Systems): {domain_counts[4]:,} tokens ({domain_counts[4]/vocab_size*100:.2f}%)")
    print(f"    Active Operations Detected: {op_counts:,} tokens")
    print(f"    Neutral Polarity (0):       {polarity_counts[0]:,} tokens")
    print(f"    Positive Polarity (1 - ACK): {polarity_counts[1]:,} tokens")
    print(f"    Negative Polarity (2 - ERR): {polarity_counts[2]:,} tokens")

if __name__ == "__main__":
    main()
