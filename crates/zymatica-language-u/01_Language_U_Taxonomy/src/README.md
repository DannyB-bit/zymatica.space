# Language-U Taxonomy (Decomposition) - Multi-Language Proof Executables

This directory contains functional, logically equivalent implementations of the **Language-U Taxonomy (Decomposition)** proof across 23 programming languages. These implementations verify the mathematical logic, data structures, and semantic transformations supporting the Sumerian: Language-U Semantic Communication Protocol.

Each implementation executes the verification proof sequence and asserts the designated validation anchor upon successful execution.

---

## 🛠️ System Prerequisites

Ensure you have the appropriate toolchains installed for the languages you wish to build or run:

| Language | Runtime/Compiler | Minimum Version | Package Manager / Notes |
|:---|:---|:---|:---|
| **Python** | Python 3 interpreter | `>= 3.8` | standard library only |
| **Go** | Go compiler | `>= 1.16` | standard library only |
| **Rust** | Rustc / Cargo compiler | `>= 1.56` | standard library only |
| **Java** | JDK (Java Development Kit) | `>= 11` | standard library only |
| **TypeScript**| Node.js & TypeScript Compiler | Node `>= 14`, TS `>= 4.0`| Runs via `node` (JS output) |
| **C++** | C++ compiler (g++, clang++, MSVC)| C++17 support | standard library only |
| **Swift** | Swift compiler / runtime | `>= 5.0` | standard library only |
| **Pure C** | C compiler (gcc, clang, MSVC) | C99 / C11 | standard library only |
| **Lua** | Lua interpreter (lua, luajit) | `>= 5.1` | standard library only |
| **Zig** | Zig compiler | `>= 0.11` | standard library only |
| **C#** | .NET SDK / csc compiler | .NET `>= 6.0` | standard library only |
| **Kotlin** | Kotlin compiler / JVM runtime | `>= 1.5` | standard library only |
| **Bash** | Bash Shell interpreter | Bash `>= 4.0` | standard system core utilities |
| **Julia** | Julia runtime | `>= 1.6` | standard library only |
| **Dart** | Dart SDK | `>= 2.12` | standard library only |
| **Elixir** | Elixir/Erlang OTP | Elixir `>= 1.12`, OTP `>= 24` | standard library only |
| **Haskell** | GHC / GHCi | `>= 8.8` | standard library only |
| **PowerShell** | PowerShell Core / Desktop | `>= 5.1` | Windows or Cross-platform |
| **MATLAB** | MATLAB / GNU Octave runtime | Octave `>= 6.0` | standard library only |
| **GLSL** | glslang / Vulkan SDK | Vulkan `>= 1.1` | GPU shader validator |
| **Faust** | Faust compiler | `>= 2.0` | sound DSP compiler |
| **Assembly** | NASM Assembler / Linker | NASM `>= 2.15` | x86-64 NASM assembler |
| **WAT** | wabt (wat2wasm) / Wasmtime | Wasmtime `>= 1.0` | WebAssembly Text Compiler |

---

## 🚀 Build and Run Instructions

### 1. Python (Interpreted)
```bash
cd python
python proof.py
```

### 2. Go (Compiled/Interpreted)
```bash
cd go
go run proof.go
```

### 3. Rust (Compiled)
```bash
cd rust
cargo run --quiet
```

### 4. Java (Compiled JVM)
```bash
cd java
javac Proof.java
java Proof
```

### 5. TypeScript (Compiled JS)
```bash
cd typescript
tsc proof.ts && node proof.js
```

### 6. C++ (Compiled Native)
```bash
cd cpp
g++ -std=c++17 proof.cpp -o proof && ./proof
```

### 7. Swift (Compiled/Interpreted)
```bash
cd swift
swift proof.swift
```

### 8. Pure C (Compiled Native)
```bash
cd c
gcc -std=c11 proof.c -o proof && ./proof
```

### 9. Lua (Interpreted)
```bash
cd lua
lua proof.lua
```

### 10. Zig (Compiled Native)
```bash
cd zig
zig run proof.zig
```

### 11. C# (Compiled Native/JVM)
```bash
cd csharp
csc proof.cs && ./proof.exe
# Or using dotnet:
# dotnet run proof.cs
```

### 12. Kotlin (Compiled JVM)
```bash
cd kotlin
kotlinc proof.kt -include-runtime -d proof.jar
java -jar proof.jar
```

### 13. Bash (Interpreted Script)
```bash
cd bash
bash proof.sh
```

### 14. Julia (Interpreted)
```bash
cd julia
julia proof.jl
```

### 15. Dart (Interpreted/Compiled)
```bash
cd dart
dart run proof.dart
```

### 16. Elixir (Interpreted Script)
```bash
cd elixir
elixir proof.exs
```

### 17. Haskell (Compiled/Interpreted)
```bash
cd haskell
runhaskell proof.hs
```

### 18. PowerShell (Interpreted Script)
```bash
cd powershell
powershell -ExecutionPolicy Bypass -File proof.ps1
```

### 19. MATLAB/Octave (Interpreted)
```bash
cd matlab
octave proof.m
```

### 20. GLSL (Shader validation)
```bash
cd glsl
glslangValidator proof.glsl
```

### 21. Faust (Compiled/Simulated DSP)
```bash
cd faust
faust -vec proof.dsp
```

### 22. Assembly (Compiled Native)
```bash
cd assembly
nasm -f win64 proof.asm -o proof.obj
# Link on Windows or Linux:
# link /subsystem:console /entry:_start proof.obj
```

### 23. WAT (Compiled WebAssembly)
```bash
cd wat
wat2wasm proof.wat -o proof.wasm
wasmtime proof.wasm
```

---

## ✅ Verification and Anchors

Upon successful execution, each language implementation is guaranteed to print a unique verification anchor indicating system integrity.

### Expected Output Signature
Each implementation will output standard diagnostic logs followed by the following verification signature:

```text
[VERIFICATION] Semantic decomposition limits proven. Task-Oriented Semantic Rate-Distortion Verified.
```

If this signature is printed and the program exits with code `0`, the logic has been successfully validated.

---

## 🧹 Housekeeping & Pruning

To maintain a clean master repository, temporary build outputs (like `.class` files, transpiled `.js` files, `.zig-cache/` folders, `.jar` files, and compiled C/C++/Go/Swift/C# binaries) should be cleaned after local test runs. You can delete them manually or use the automated clean targets.
