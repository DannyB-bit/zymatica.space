

---

## Tokenizer 7-Level Compression & Restoration Kit

We have uploaded a fully-functional **7-Level Tokenizer Descent** compression kit under `tokenizer_kit/`. This mirrors the compression paradigm of the model:

### 7-Level Tokenizer Descent
| Level | Stage           | Method                              | Payload size (Abs) | Payload size (Ref) |
|-------|-----------------|-------------------------------------|--------------------|--------------------|
| 1     | Baseline Raw    | Raw JSON / TXT files                | ~23 MB             | ~23 MB             |
| 2     | Extract Struct  | Strip verbose schemas               | ~15 MB             | -                  |
| 3     | ID Delta Pack   | Merges mapped to index pairs        | ~7 MB              | -                  |
| 4     | Prefix-Suffix   | Prefix matches + varint coding      | ~4 MB              | -                  |
| 5     | Base Reference  | Zero-delta vs Qwen/Qwen3.5-0.8B     | -                  | 0 bytes            |
| 6     | Hyper-Deflate   | Zlib Level 9 Deflate                | ~2.44 MB           | 28 bytes           |
| 7     | XOR-FEC Chirp   | 28 × 255-byte packetization         | -                  | 7,140 bytes total  |

### Reconstructing the Tokenizer Offline
To reconstruct the tokenizer files, fetch the kit files from HF, then run:

#### Standalone Absolute Reconstruction:
```bash
python decode_tokenizer.py --capsule qwen-3.5-0.8b-28chirps-tokenizer.capsule --out_dir ./restored_tokenizer
```

#### XOR-FEC Reassembly & Base-Oracle Delta:
```bash
python decode_tokenizer.py --packet_dir ./packets_tokenizer --out_dir ./restored_tokenizer
```

The system will verify the reconstructed tokenizers by attempting a tokenization round-trip.


---

## 📜 License & Upstream Developer Attributions

- **Primary IP & Specification License:** Governed by the **[ZYMATICA COMMERCIAL & NOVEL-HOLDER COVENANT LICENSE (Version 2.0)](https://zymatica.space)** (LicenseRef-Zymatica-Covenant-2.0).
- **Upstream Open-Source Acknowledgments:** Base neural model architectures, tokenizers, mathematical libraries, and cryptographic primitives derived from or interoperable with third-party open-source projects (including Alibaba Qwen, Google Gemma, Hugging Face Transformers/Tokenizers, Arkworks zkSNARKs, PyTorch, and ONNX Runtime) remain respectfully attributed to their original creators and are governed by their respective upstream licenses (Apache-2.0, MIT, BSD-3) under Section 3 of the Covenant License.
