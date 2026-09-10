import os
import sys
import zlib
import struct
import torch
import numpy as np
from transformers import AutoTokenizer, AutoModelForCausalLM

# Set environments to avoid memory issues
os.environ["PYTORCH_CUDA_ALLOC_CONF"] = "garbage_collection_threshold:0.6,max_split_size_mb:128"

DEVICE = "cuda" if torch.cuda.is_available() else "cpu"
BASE_MODEL = "j:/Language-U/Language-U-V2/qwen-3.5-0.8b-local"
MAP_BIN = "j:/Language-U/qwen_vocab_cuneiform.bin"

# ─── Cuneiform-U Predictor & Range Coder ──────────────────────────────────────────

class PythonRadicalPredictor:
    def __init__(self, alpha=1, weight=128):
        self.alpha = alpha
        self.weight = weight
        self.trans_rc = {}
        self.trans_rf = {}
        self.trans_ra = {}
        self.prev_rc = 0
        self.prev_rf = 0
        self.prev_ra = 0

    def observe(self, rc, rf, ra):
        key_rc = self.prev_rc
        if key_rc not in self.trans_rc:
            self.trans_rc[key_rc] = {}
        self.trans_rc[key_rc][rc] = self.trans_rc[key_rc].get(rc, 0) + self.weight

        key_rf = (rc << 8) | self.prev_rf
        if key_rf not in self.trans_rf:
            self.trans_rf[key_rf] = {}
        self.trans_rf[key_rf][rf] = self.trans_rf[key_rf].get(rf, 0) + self.weight

        key_ra = (rc << 16) | (rf << 8) | self.prev_ra
        if key_ra not in self.trans_ra:
            self.trans_ra[key_ra] = {}
        self.trans_ra[key_ra][ra] = self.trans_ra[key_ra].get(ra, 0) + self.weight

        self.prev_rc = rc
        self.prev_rf = rf
        self.prev_ra = ra

    def get_cum_freqs_rc(self, prev_rc):
        freqs = [self.alpha] * 256
        if prev_rc in self.trans_rc:
            for sym, count in self.trans_rc[prev_rc].items():
                freqs[sym] += count
        cum_freqs = [0] * 257
        for i in range(256):
            cum_freqs[i+1] = cum_freqs[i] + freqs[i]
        return cum_freqs

    def get_cum_freqs_rf(self, curr_rc, prev_rf):
        freqs = [self.alpha] * 256
        key = (curr_rc << 8) | prev_rf
        if key in self.trans_rf:
            for sym, count in self.trans_rf[key].items():
                freqs[sym] += count
        cum_freqs = [0] * 257
        for i in range(256):
            cum_freqs[i+1] = cum_freqs[i] + freqs[i]
        return cum_freqs

    def get_cum_freqs_ra(self, curr_rc, curr_rf, prev_ra):
        freqs = [self.alpha] * 256
        key = (curr_rc << 16) | (curr_rf << 8) | prev_ra
        if key in self.trans_ra:
            for sym, count in self.trans_ra[key].items():
                freqs[sym] += count
        cum_freqs = [0] * 257
        for i in range(256):
            cum_freqs[i+1] = cum_freqs[i] + freqs[i]
        return cum_freqs


class BitWriter:
    def __init__(self):
        self.buffer = []
        self.current_byte = 0
        self.bit_count = 0

    def write_bit(self, bit):
        self.current_byte = (self.current_byte << 1) | (bit & 1)
        self.bit_count += 1
        if self.bit_count % 8 == 0:
            self.buffer.append(self.current_byte)
            self.current_byte = 0

    def write_bit_helper(self, underflow_bits, bit):
        self.write_bit(bit)
        for _ in range(underflow_bits[0]):
            self.write_bit(1 - bit)
        underflow_bits[0] = 0

    def flush(self):
        if self.bit_count % 8 != 0:
            padding_bits = 8 - (self.bit_count % 8)
            self.current_byte <<= padding_bits
            self.buffer.append(self.current_byte)
            self.current_byte = 0
            self.bit_count += padding_bits
        return bytes(self.buffer)


class BitReader:
    def __init__(self, data):
        self.data = data
        self.byte_index = 0
        self.bit_index = 0
        self.total_bits = len(data) * 8

    def read_bit(self):
        if self.byte_index >= len(self.data):
            return 0
        bit = (self.data[self.byte_index] >> (7 - self.bit_index)) & 1
        self.bit_index += 1
        if self.bit_index == 8:
            self.bit_index = 0
            self.byte_index += 1
        return bit


def range_encode_radicals(radicals, alpha=1, weight=128):
    pred = PythonRadicalPredictor(alpha, weight)
    w = BitWriter()

    low = 0
    high = 0xFFFFFFFF
    underflow_bits = [0]

    for rc, rf, ra in radicals:
        symbols = [rc, rf, ra]
        prev_rc = pred.prev_rc
        prev_rf = pred.prev_rf
        prev_ra = pred.prev_ra

        for step in range(3):
            if step == 0:
                cum_freqs = pred.get_cum_freqs_rc(prev_rc)
            elif step == 1:
                cum_freqs = pred.get_cum_freqs_rf(symbols[0], prev_rf)
            else:
                cum_freqs = pred.get_cum_freqs_ra(symbols[0], symbols[1], prev_ra)

            sym = symbols[step]
            total = cum_freqs[256]
            cum_low = cum_freqs[sym]
            cum_high = cum_freqs[sym + 1]

            range_width = high - low + 1
            high = low + (range_width * cum_high) // total - 1
            low = low + (range_width * cum_low) // total

            # Renormalize
            while True:
                if high < 0x80000000:
                    w.write_bit_helper(underflow_bits, 0)
                    low = (low << 1) & 0xFFFFFFFF
                    high = ((high << 1) | 1) & 0xFFFFFFFF
                elif low >= 0x80000000:
                    w.write_bit_helper(underflow_bits, 1)
                    low = ((low - 0x80000000) << 1) & 0xFFFFFFFF
                    high = (((high - 0x80000000) << 1) | 1) & 0xFFFFFFFF
                elif low >= 0x40000000 and high < 0xC0000000:
                    underflow_bits[0] += 1
                    low = ((low - 0x40000000) << 1) & 0xFFFFFFFF
                    high = (((high - 0x40000000) << 1) | 1) & 0xFFFFFFFF
                else:
                    break

        pred.observe(rc, rf, ra)

    underflow_bits[0] += 1
    if low < 0x40000000:
        w.write_bit_helper(underflow_bits, 0)
    else:
        w.write_bit_helper(underflow_bits, 1)

    return w.flush()


def range_decode_radicals(encoded_bytes, num_concepts, alpha=1, weight=128):
    pred = PythonRadicalPredictor(alpha, weight)
    r = BitReader(encoded_bytes)

    value = 0
    for _ in range(32):
        value = (value << 1) | r.read_bit()

    low = 0
    high = 0xFFFFFFFF
    decoded_radicals = []

    for c in range(num_concepts):
        prev_rc = pred.prev_rc
        prev_rf = pred.prev_rf
        prev_ra = pred.prev_ra

        symbols = [0, 0, 0]

        for step in range(3):
            if step == 0:
                cum_freqs = pred.get_cum_freqs_rc(prev_rc)
            elif step == 1:
                cum_freqs = pred.get_cum_freqs_rf(symbols[0], prev_rf)
            else:
                cum_freqs = pred.get_cum_freqs_ra(symbols[0], symbols[1], prev_ra)

            total = cum_freqs[256]
            range_width = high - low + 1

            scaled_val = (((value - low) + 1) * total - 1) // range_width

            # Binary search
            sym = 0
            l = 0
            rr = 255
            while l <= rr:
                mid = (l + rr) // 2
                if cum_freqs[mid] <= scaled_val < cum_freqs[mid + 1]:
                    sym = mid
                    break
                elif scaled_val >= cum_freqs[mid + 1]:
                    l = mid + 1
                else:
                    rr = mid - 1

            symbols[step] = sym

            cum_low = cum_freqs[sym]
            cum_high = cum_freqs[sym + 1]

            high = low + (range_width * cum_high) // total - 1
            low = low + (range_width * cum_low) // total

            # Renormalize
            while True:
                if high < 0x80000000:
                    low = (low << 1) & 0xFFFFFFFF
                    high = ((high << 1) | 1) & 0xFFFFFFFF
                    value = ((value << 1) | r.read_bit()) & 0xFFFFFFFF
                elif low >= 0x80000000:
                    low = ((low - 0x80000000) << 1) & 0xFFFFFFFF
                    high = (((high - 0x80000000) << 1) | 1) & 0xFFFFFFFF
                    value = (((value - 0x80000000) << 1) | r.read_bit()) & 0xFFFFFFFF
                elif low >= 0x40000000 and high < 0xC0000000:
                    low = ((low - 0x40000000) << 1) & 0xFFFFFFFF
                    high = (((high - 0x40000000) << 1) | 1) & 0xFFFFFFFF
                    value = (((value - 0x40000000) << 1) | r.read_bit()) & 0xFFFFFFFF
                else:
                    break

        decoded_radicals.append((symbols[0], symbols[1], symbols[2]))
        pred.observe(symbols[0], symbols[1], symbols[2])

    return decoded_radicals

# ─── Sumerian Execution Harness ──────────────────────────────────────────────────

def load_vocab_map(path):
    with open(path, "rb") as f:
        data = f.read()
    vocab_size = len(data) // 3
    vocab_map = {}
    for i in range(vocab_size):
        vocab_map[i] = (data[i*3], data[i*3+1], data[i*3+2])
    return vocab_map

def run_sumerian_inference(model, tokenizer, prompt, vocab_map, max_new_tokens=40):
    print("\n" + "="*80)
    print("  RUNNING QWEN-3.5-0.8B-SUMERIAN TOKEN STREAMING INFERENCE")
    print("="*80)
    print(f"Prompt: \"{prompt}\"")
    
    # Encode prompt tokens
    input_ids = tokenizer.encode(prompt, return_tensors="pt").to(DEVICE)
    prompt_tokens = input_ids[0].tolist()
    
    # 1. Map & compress input prompt (transmitting side)
    prompt_radicals = [vocab_map[tid] for tid in prompt_tokens]
    compressed_prompt = range_encode_radicals(prompt_radicals, alpha=1, weight=128)
    
    # 2. Decompress prompt (receiver side)
    decoded_prompt_radicals = range_decode_radicals(compressed_prompt, len(prompt_tokens), alpha=1, weight=128)
    assert decoded_prompt_radicals == prompt_radicals, "Prompt transmission failed: lossless constraint broken!"
    print(f"\n[OK] Input prompt successfully encoded, transmitted, and decoded losslessly.")
    print(f"     Prompt tokens: {len(prompt_tokens)}")
    print(f"     Transmission payload: {len(compressed_prompt)} bytes (ASCII: {len(prompt.encode('utf-8'))} bytes)")
    
    # 3. Dynamic output streaming loop
    generated_tokens = []
    generated_radicals = []
    curr_input_ids = input_ids.clone()
    
    print("\nSimulated Receiver Output (Decoded Token Stream):")
    print("----------------------------------------------------------------")
    
    for step in range(max_new_tokens):
        with torch.no_grad():
            outputs = model(curr_input_ids)
            next_token_logits = outputs.logits[0, -1, :]
            # greedy selection
            next_token_id = torch.argmax(next_token_logits).item()
            
        if next_token_id == tokenizer.eos_token_id:
            break
            
        generated_tokens.append(next_token_id)
        
        # Look up coordinate radicals for transmission
        rc, rf, ra = vocab_map[next_token_id]
        generated_radicals.append((rc, rf, ra))
        
        # Dynamic streaming decode to simulate receiving end parsing text
        token_str = tokenizer.decode([next_token_id])
        print(token_str, end="", flush=True)
        
        # Append to context
        curr_input_ids = torch.cat([curr_input_ids, torch.tensor([[next_token_id]], device=DEVICE)], dim=-1)
        
    print("\n----------------------------------------------------------------")
    
    # 4. Final output compression & decompression verification
    if generated_radicals:
        compressed_gen = range_encode_radicals(generated_radicals, alpha=1, weight=128)
        decoded_gen_radicals = range_decode_radicals(compressed_gen, len(generated_tokens), alpha=1, weight=128)
        assert decoded_gen_radicals == generated_radicals, "Output stream transmission failed: lossless constraint broken!"
        
        gen_text = tokenizer.decode(generated_tokens)
        gen_ascii_len = len(gen_text.encode('utf-8'))
        gen_compressed_len = len(compressed_gen)
        
        # Calculate compression metrics
        vs_ascii = gen_ascii_len / gen_compressed_len
        
        # Zlib token IDs comparison
        token_bytes_flat = bytearray()
        for tid in generated_tokens:
            token_bytes_flat.extend(struct.pack(">I", tid))
        zlib_compressed = zlib.compress(bytes(token_bytes_flat), level=9)
        zlib_len = len(zlib_compressed)
        vs_zlib = zlib_len / gen_compressed_len
        
        print("\nSumerian Stream Transmission Summary:")
        print(f"  - Total output tokens generated: {len(generated_tokens)}")
        print(f"  - Raw generated text size:       {gen_ascii_len} bytes")
        print(f"  - **Sumerian Range-Coded size**:  {gen_compressed_len} bytes")
        print(f"  - Token IDs + Zlib size:         {zlib_len} bytes")
        print(f"  - Compression gain vs ASCII:     {vs_ascii:.2f}x")
        print(f"  - Compression gain vs Zlib:      {vs_zlib:.2f}x")
        print("[OK] Token-level Sumerian transmission verified successfully.")

def main():
    if not os.path.exists(MAP_BIN):
        print(f"Error: Vocab map {MAP_BIN} does not exist.")
        return
        
    print("Loading Sumerian coordinate map...")
    vocab_map = load_vocab_map(MAP_BIN)
    
    print(f"Loading Qwen base model from: {BASE_MODEL}")
    tokenizer = AutoTokenizer.from_pretrained(BASE_MODEL, trust_remote_code=True)
    model = AutoModelForCausalLM.from_pretrained(
        BASE_MODEL, 
        torch_dtype=torch.float16, 
        device_map="auto" if torch.cuda.is_available() else "cpu"
    )
    model.eval()
    
    # Run test prompt
    prompt = "Configure Raspberry Pi 4 GPIO pins and reset the SX1302 concentrator lines."
    run_sumerian_inference(model, tokenizer, prompt, vocab_map)

if __name__ == "__main__":
    main()
