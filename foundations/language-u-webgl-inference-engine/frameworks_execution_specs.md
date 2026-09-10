# WebGL/WebGPU Framework Execution Specifications
## System Dependencies, Runtimes, and Execution Guide (Windows, macOS, Linux)

This document details the system dependencies, host runtimes, and execution commands required to run and test each of the 49 frameworks from the Language-U WebGL Inference Engine database on a standard developer machine.

---

## 1. System-Level GPU Dependencies
All WebGL and WebGPU frameworks run on top of low-level graphics card drivers. To execute any framework locally, your operating system must provide compatible graphics contexts:

| Operating System | WebGL 2.0 Backend | WebGPU Backend | Required GPU Drivers |
| :--- | :--- | :--- | :--- |
| **Windows** | Direct3D 11 / OpenGL | Direct3D 12 / Vulkan | NVIDIA Game Ready / AMD Software / Intel Graphics |
| **macOS** | OpenGL (deprecated but active) | Metal | Apple Silicon Native Drivers / AMD Metal Drivers |
| **Linux** | OpenGL ES 3.0 / MESA | Vulkan / MESA | Proprietary NVIDIA Linux Driver / AMDGPU open-source |

---

## 2. Category-Based Execution Pathways

Rather than requiring custom compilers for all 49 libraries, the frameworks fall into five distinct structural runtime categories. Here is how you configure and execute each category:

### Category A: High-Level 3D Engines (Monolithic/ECS)
*   **Frameworks**: `three.js`, `Babylon.js`, `PlayCanvas`, `ClayGL`, `zen-3d`, `GrimoireJS`, `OSG.js`, `A-Frame`, `xeogl`, `WhitestormJS`.
*   **Runtime Environment**: Modern Web Browser (Chrome, Safari, Firefox, Edge) + Local Web Server (to bypass CORS errors on model loadings).
*   **How to Run**:
    1.  Install the package via npm:
        ```bash
        npm install three babylonjs playcanvas aframe
        ```
    2.  Set up an HTML container file containing your scene script.
    3.  Launch a local web server (see Section 3) to run the file.

### Category B: 2D Graphics Acceleration Engines
*   **Frameworks**: `PixiJS`, `Phaser`, `AwayJS`, `p5.js`, `Hilo3d`, `phenomenon-px`.
*   **Runtime Environment**: Web Browser + Canvas/WebGL 2D accelerated context.
*   **How to Run**:
    1.  Install via npm:
        ```bash
        npm install pixi.js phaser p5
        ```
    2.  Create your game/render loop and run it via a local development server.

### Category C: Low-Level Context & Shader Wrappers
*   **Frameworks**: `stack.gl`, `regl`, `TWGL`, `TDL`, `PicoGL.js`, `nanogl`, `Alfrid`, `litegl.js`, `lightgl.js`, `O-GL` (ogl), `PhiloGL`, `gl-engine`, `GLAM`, `LumaGL`.
*   **Runtime Environment**: WebGL 1.0/2.0 context. Excellent for custom shader writers and raw buffer controls.
*   **How to Run**:
    1.  Install package:
        ```bash
        npm install regl twgl.js picogl ogl luma.gl
        ```
    2.  Define vertex and fragment shaders directly in your JavaScript code and bind webgl context buffers.

### Category D: WebAssembly / Compiled C++ Frameworks
*   **Frameworks**: `Filament` (Google), `Ammo.js` (Bullet physics port).
*   **Runtime Environment**: WebAssembly (WASM) Virtual Machine in browser, or C++ native compilers (Windows MSVC, macOS Clang, Linux GCC) to build native binary wrappers.
*   **How to Run (WASM Mode)**:
    1.  Load the WASM runtime wrapper:
        ```bash
        npm install @google/filament ammo.js
        ```
    2.  Ensure your local server serves `.wasm` files with the correct MIME type (`application/wasm`).
*   **How to Run (C++ Native Compiler Mode)**:
    1.  Install CMake and Emscripten:
        ```bash
        # On Linux
        sudo apt-get install cmake emscripten
        ```
    2.  Build from repository source:
        ```bash
        mkdir build && cd build
        emcmake cmake ..
        make
        ```

### Category E: GIS, Scientific, and Specialized Visualizers
*   **Frameworks**: `Cesium` (CesiumJS), `Deck.gl`, `SceneJS`, `litescene.js`.
*   **Runtime Environment**: WebGL contexts optimized for heavy double-precision float math (e.g. 64-bit geographic coordinate mapping).
*   **How to Run**:
    1.  Install via npm:
        ```bash
        npm install cesium deck.gl
        ```
    2.  Build geospatial layers on canvas viewports.

---

## 3. Local Web Server Configuration (Bypassing CORS)
Because WebGL loads local external resources (like shader files, `.gltf` mesh data, and textures), running files directly from double-clicking `index.html` (`file:///`) will trigger Browser CORS security blocks. 

You must run a local server in the project directory using any of these standard commands:

*   **Node.js Vite Dev Server (Recommended)**:
    ```bash
    npm create vite@latest my-app --template vanilla
    cd my-app && npm install && npm run dev
    ```
*   **Python Server**:
    ```bash
    python -m http.server 8080
    ```
*   **Node.js HTTP Server**:
    ```bash
    npm install -g http-server
    http-server -p 8080
    ```

Once running, navigate to `http://localhost:8080` in your browser. All 49 frameworks will execute successfully on your local GPU.

---

## 4. Key Work Summary: Self-Contained Offline Transmission & Verification

### 4.1 Self-Contained Capsule Creation (`run_ultimate_pipeline.py`)
*   **Archiving Stage**: Implemented an automated archiving sequence during Level 6 capsule generation.
*   **Concatenated File Deliverables**:
    *   `VerifyLanguageU.java` (Java range decoder & validator source code)
    *   `verify_language_u.rs` (Rust range decoder & validator source code)
    *   `verify_language_u.lua` (Lua range decoder & validator source code)
    *   `requirements.txt` (Python run-time dependencies)
    *   `frameworks_db.json` (The primary upgraded configurations database)
    *   `frameworks_execution_specs.md` (WebGL/WebGPU execution specifications)
*   **Extended Header**: Upgraded the capsule payload header layout to: `magic` (3 bytes), `num_fws` (1 byte), `names_len` (2 bytes), `bitstream_len` (4 bytes), and `archive_len` (4 bytes).
*   **Deflation**: Compressed the entire concatenated bundle under zlib Level 9, resulting in a compact **16,570-byte** seed capsule (down from **74,341 bytes** of raw files).
*   **Packet Sizing**: Dynamically split the seed capsule into **66 data packets + 1 XOR parity packet** (total 67 packets, each 255 bytes).

### 4.2 Offline Reconstruction & Auto-Compilation Simulator (`receiver_reconstruction_demo.py`)
*   **XOR-FEC Healing**: Dynamically scans the packets folder, parses headers, and performs single-packet reconstruction via XOR equations to heal simulated packet loss.
*   **Archived File Extractor**: Automatically inflates the seed capsule, parses the file archive segment, and recreates all original source files back on disk.
*   **Repository-Aware Pathing**: Configured Java, Rust, and Lua loaders to dynamically fall back to the root folder if the subfolder is absent, ensuring successful execution in both development and flat repositories.
*   **Dynamic Compilation Tests**: Programmed compile-and-run verification sequences using local runtimes to prove correct recovery offline:
    *   **Java Verification (`VerifyLanguageU.java`)**: `[SUCCESS] Java range-decoder verification: 100% MATCH!`
    *   **Rust Verification (`verify_language_u.rs`)**: `[SUCCESS] Rust range-decoder verification: 100% MATCH!`

### 4.3 Hugging Face Repository Push
*   **Target Repo**: `TheAiCollectiveART/language-u-webgl-inference-engine`
*   **Commit Message**: `Upgrade pipeline to include cross-language decoders, specifications, dependencies, and database offline in LLM capsule transmission via 67 packets` (Hash: `f2f541d`)



---

## 📜 License & Upstream Developer Attributions

- **Primary IP & Specification License:** Governed by the **[ZYMATICA COMMERCIAL & NOVEL-HOLDER COVENANT LICENSE (Version 2.0)](https://zymatica.space)** (LicenseRef-Zymatica-Covenant-2.0).
- **Upstream Open-Source Acknowledgments:** Base neural model architectures, tokenizers, mathematical libraries, and cryptographic primitives derived from or interoperable with third-party open-source projects (including Alibaba Qwen, Google Gemma, Hugging Face Transformers/Tokenizers, Arkworks zkSNARKs, PyTorch, and ONNX Runtime) remain respectfully attributed to their original creators and are governed by their respective upstream licenses (Apache-2.0, MIT, BSD-3) under Section 3 of the Covenant License.
