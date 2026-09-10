# LORA-OPERATOR Operations & Integration Instructions

This document provides setup, compilation, and execution instructions for the LORA-OPERATOR repository.
The instructions are split into two target audiences: **Human Operators** and **AI Agents/Subagents**.

---

## 🧑‍💻 1. Instructions for Human Operators

### A. Prerequisites
1. **Python 3.8+** must be installed.
2. Install dependencies using:
   ```bash
   pip install -r requirements.txt
   ```
3. To run in **Serial mode**, ensure you have connected a compatible LoRa concentrator module (e.g. RAK Wireless) via USB/UART and know its COM port (on Windows, e.g. `COM3`) or device path (on Linux, e.g. `/dev/ttyUSB0`).

### B. Local Compilation (If modifying the C core)
The repository contains a precompiled Windows dynamic library (`cuneiform_u_v3.dll`). If you make changes to the C library or compile on other operating systems:
* **Windows (MinGW/gcc):**
  ```bash
  gcc -O3 -shared -fPIC -o cuneiform_u_v3.dll cuneiform_u_v3_wrapper.c
  ```
* **Linux (gcc):**
  ```bash
  gcc -O3 -shared -fPIC -o libcuneiform_u_v3.so cuneiform_u_v3_wrapper.c
  ```
* **macOS (clang):**
  ```bash
  clang -O3 -shared -fPIC -o libcuneiform_u_v3.dylib cuneiform_u_v3_wrapper.c
  ```

### C. Execution Methods (UDP Simulation)
To test the transmitter and receiver locally over UDP without hardware:
1. **Start the Receiver Node:**
   ```bash
   python RakMiner-B2.py --mode udp --ip 0.0.0.0 --port 50001
   ```
2. **Start the Transmitter Node:** (In another terminal window)
   ```bash
   python RakMiner-A1.py --mode udp --ip 127.0.0.1 --port 50001
   ```
If successful, both terminals will display:
`[+] Cuneiform-U v3 native acceleration loaded successfully! (Yang Mode)`
And the receiver will output:
`[+] Payload checksum verified successfully! Lossless path confirmed. [OK]`
`[VERIFICATION] Multi-Language runtime FFI structures validated.`

### D. Execution Methods (LoRa Serial Hardware)
For real LoRa wireless communication using serial AT commands:
1. **Start the Receiver AT Listener:**
   ```bash
   python RakMiner-B2.py --mode serial --device /dev/ttyUSB0 --baud 115200
   ```
2. **Start the Transmitter AT Command broadcaster:**
   ```bash
   python RakMiner-A1.py --mode serial --device /dev/ttyUSB1 --baud 115200
   ```

---

## 🤖 2. Instructions for AI Agents and Code Generators

When parsing, refactoring, or integrating this repository, adhere strictly to the following mathematical, structural, and interface boundaries.

### A. Shared Library Export Interfaces (ctypes)
The compiled binary exposes two dynamic library function interfaces. The definitions mapped to python must match exactly:

#### Concept6D C-Struct Definition:
```c
typedef struct {
    uint8_t domain;       /* 0-15 */
    uint8_t subdomain;    /* 0-15 */
    uint8_t operation;    /* 0-15 */
    uint8_t modality;     /* 0-15 */
    uint8_t depth;        /* 0-15 */
    uint8_t polarity;     /* 0-15 */
} Concept6D;
```
Mapped in Python ctypes as:
```python
class Concept6D(ctypes.Structure):
    _fields_ = [
        ("domain", ctypes.c_uint8),
        ("subdomain", ctypes.c_uint8),
        ("operation", ctypes.c_uint8),
        ("modality", ctypes.c_uint8),
        ("depth", ctypes.c_uint8),
        ("polarity", ctypes.c_uint8),
    ]
```

#### Function Bindings:
* **Encode API**:
  ```python
  lib.cuneiform_u_v3_encode_dll.argtypes = [
      ctypes.POINTER(Concept6D), ctypes.c_uint32,
      ctypes.POINTER(ctypes.c_uint8), ctypes.c_uint32,
      ctypes.c_uint32, ctypes.c_uint32
  ]
  lib.cuneiform_u_v3_encode_dll.restype = ctypes.c_int
  ```
* **Decode API**:
  ```python
  lib.cuneiform_u_v3_decode_dll.argtypes = [
      ctypes.POINTER(ctypes.c_uint8), ctypes.c_uint32,
      ctypes.POINTER(Concept6D), ctypes.c_uint32,
      ctypes.c_uint32, ctypes.c_uint32
  ]
  lib.cuneiform_u_v3_decode_dll.restype = ctypes.c_int
  ```

### B. Protocol Packaging Specifications
* **XOR-FEC Frames**: Payload packets are padded to `252` data bytes + `3` byte header (`SYNC_MARKER (0xBB)`, `frame_index`, `total_frames`) to create `255` byte physical packets matching LoRa hardware channel constraints.
* **Metadata Signature Format**: 
  `META:num_concepts:payload_hash:compressed_len`
  * AI agents parsing the stream must extract `compressed_len` to trim the padded packet back to the exact compressed bitstream size before verifying the SHA-256 `payload_hash` and calling `cuneiform_u_v3_decode_dll`.

### C. State Constraints (Radical Predictor)
* The transition count tables (`trans_rc`, `trans_rf`, `trans_ra`) are statically capped at `MAX_TRANSITIONS = 256` transition elements. The model state predictor must maintain zero heap reallocation to be memory-safe on low-RAM microcontrollers.

### D. Automated Verification Target Anchors
Scripts verifying execution output must check for the following exact stdout string sequences:
1. `[+] Cuneiform-U v3 native acceleration loaded successfully! (Yang Mode)` — confirms ctypes loaded the DLL.
2. `[+] Payload checksum verified successfully! Lossless path confirmed. [OK]` — confirms hash verification and trimming.
3. `[VERIFICATION] Multi-Language runtime FFI structures validated.` — confirms mathematical parity has been preserved.


---

## 📜 License & Upstream Developer Attributions

- **Primary IP & Specification License:** Governed by the **[ZYMATICA COMMERCIAL & NOVEL-HOLDER COVENANT LICENSE (Version 2.0)](https://zymatica.space)** (LicenseRef-Zymatica-Covenant-2.0).
- **Upstream Open-Source Acknowledgments:** Base neural model architectures, tokenizers, mathematical libraries, and cryptographic primitives derived from or interoperable with third-party open-source projects (including Alibaba Qwen, Google Gemma, Hugging Face Transformers/Tokenizers, Arkworks zkSNARKs, PyTorch, and ONNX Runtime) remain respectfully attributed to their original creators and are governed by their respective upstream licenses (Apache-2.0, MIT, BSD-3) under Section 3 of the Covenant License.
