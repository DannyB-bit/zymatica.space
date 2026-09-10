// Gemma-4-31B Sumerian -- Rust-Zig Hybrid GPU CUDA Inference Runner
// Watermark: ip zymatica.space | astronautshe.com
// Copyright © 2026 Zymatica
// SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
// See LICENSE for terms.
// Run: cargo run --release (links to sumerian_cuda_core.lib)

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::time::Instant;
use std::collections::HashMap;
use std::ffi::CString;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tch::{Tensor, Device, Kind};

#[cfg(target_os = "windows")]
extern "system" {
    fn LoadLibraryA(lpLibFileName: *const std::os::raw::c_char) -> *mut std::ffi::c_void;
    fn SetDllDirectoryA(lpPathName: *const std::os::raw::c_char) -> std::os::raw::c_int;
}

#[cfg(target_os = "windows")]
unsafe fn load_cuda_dlls() {
    if let Ok(path_var) = std::env::var("PATH") {
        for path in std::env::split_paths(&path_var) {
            let dll_path = path.join("c10_cuda.dll");
            if dll_path.exists() {
                let path_str = path.to_string_lossy();
                let c_path = std::ffi::CString::new(path_str.as_ref() as &str).unwrap();
                SetDllDirectoryA(c_path.as_ptr());
                println!("[+] Set DLL directory to: {}", path_str);
                break;
            }
        }
    }

    let dlls = ["c10_cuda.dll", "torch_cuda.dll"];
    for dll in &dlls {
        let name = std::ffi::CString::new(*dll).unwrap();
        let handle = LoadLibraryA(name.as_ptr());
        if handle.is_null() {
            println!("[-] Failed to load {}", dll);
        } else {
            println!("[+] Successfully loaded {}", dll);
        }
    }
}

#[cfg(not(target_os = "windows"))]
unsafe fn load_cuda_dlls() {}

// Paths — configurable via environment variables
// Set GEMMA4_MODEL_DIR, GEMMA4_GENESIS, and GEMMA4_CUNEIFORM_BIN to override defaults.
fn get_model_dir() -> String {
    std::env::var("GEMMA4_MODEL_DIR").unwrap_or_else(|_| "model".to_string())
}
fn get_genesis_path() -> String {
    std::env::var("GEMMA4_GENESIS").unwrap_or_else(|_| "gemma4_31b_subzero.genesis".to_string())
}
fn get_cuneiform_bin() -> String {
    std::env::var("GEMMA4_CUNEIFORM_BIN").unwrap_or_else(|_| "gemma4_vocab_cuneiform.bin".to_string())
}
fn get_batch_size() -> i64 {
    std::env::var("GEMMA4_BATCH_SIZE")
        .unwrap_or_else(|_| "1".to_string())
        .parse::<i64>()
        .unwrap_or(1)
}

const MAX_NEW_TOKENS: usize = 128;
const TEMPERATURE: f64 = 0.7;
const TOP_K: i64 = 40;
const TOP_P: f64 = 0.90;

// -----------------------------------------------------------------------------
// ZIG CORE FFI BINDINGS
// -----------------------------------------------------------------------------
extern "C" {
    fn sumerian_init_cuda() -> std::os::raw::c_int;
    fn sumerian_launch_svd_phase1(
        d_X: u64,
        d_V: u64,
        scale_v: f32,
        d_T: u64,
        B: std::os::raw::c_int,
        n: std::os::raw::c_int,
        r: std::os::raw::c_int,
    ) -> std::os::raw::c_int;
    fn sumerian_launch_svd_phase2(
        d_T: u64,
        d_U_T: u64,
        scale_u: f32,
        d_Y: u64,
        B: std::os::raw::c_int,
        m: std::os::raw::c_int,
        r: std::os::raw::c_int,
        accumulate: std::os::raw::c_int,
    ) -> std::os::raw::c_int;
    fn sumerian_launch_svd_fused(
        d_X: u64,
        d_V: u64,
        scale_v: f32,
        d_U_T: u64,
        scale_u: f32,
        d_Y: u64,
        B: std::os::raw::c_int,
        n: std::os::raw::c_int,
        m: std::os::raw::c_int,
        r: std::os::raw::c_int,
        accumulate: std::os::raw::c_int,
    ) -> std::os::raw::c_int;
    fn sumerian_launch_lm_head(
        d_X: u64,
        d_W_T: u64,
        scale_w: f32,
        d_Y: u64,
        B: std::os::raw::c_int,
        vocab_size: std::os::raw::c_int,
        hidden_dim: std::os::raw::c_int,
    ) -> std::os::raw::c_int;
    fn sumerian_deinit_cuda();
    fn sumerian_cuneiform_init(bin_path: *const std::os::raw::c_char) -> std::os::raw::c_int;
    fn sumerian_cuneiform_coords_to_id(rc: u8, rf: u8, ra: u8) -> std::os::raw::c_int;
    fn sumerian_cuneiform_id_to_coords(id: std::os::raw::c_int, coords_out: *mut u8);
    fn sumerian_cuneiform_free();
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct TensorMeta {
    dtype: String,
    shape: Vec<i64>,
    data_offsets: Vec<u64>,
}

// Custom Safetensors metadata structure (seek-based, memory-safe)
struct SeekSafetensors {
    file: File,
    header: HashMap<String, TensorMeta>,
    data_base: u64,
}

impl SeekSafetensors {
    fn open<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let mut file = File::open(path)?;
        let mut len_buf = [0u8; 8];
        file.read_exact(&mut len_buf)?;
        let hdr_len = u64::from_le_bytes(len_buf);
        
        let mut hdr_buf = vec![0u8; hdr_len as usize];
        file.read_exact(&mut hdr_buf)?;
        let raw_header: HashMap<String, Value> = serde_json::from_slice(&hdr_buf)?;
        let mut header = HashMap::new();
        for (k, v) in raw_header {
            if k != "__metadata__" {
                let meta: TensorMeta = serde_json::from_value(v)?;
                header.insert(k, meta);
            }
        }
        let data_base = 8 + hdr_len;
        
        Ok(Self { file, header, data_base })
    }

    fn get_tensor(&mut self, name: &str) -> Result<Option<Tensor>, Box<dyn std::error::Error>> {
        if !self.header.contains_key(name) {
            return Ok(None);
        }
        let meta = &self.header[name];
        let start = meta.data_offsets[0];
        let end = meta.data_offsets[1];
        let nbytes = end - start;
        
        self.file.seek(SeekFrom::Start(self.data_base + start))?;
        let mut raw_bytes = vec![0u8; nbytes as usize];
        self.file.read_exact(&mut raw_bytes)?;
        
        let t = match meta.dtype.as_str() {
            "BF16" => {
                let i16_data = unsafe {
                    std::slice::from_raw_parts(raw_bytes.as_ptr() as *const i16, raw_bytes.len() / 2)
                }.to_vec();
                Tensor::from_slice(&i16_data).view_dtype(Kind::BFloat16).reshape(&meta.shape)
            }
            "F16" => {
                let i16_data = unsafe {
                    std::slice::from_raw_parts(raw_bytes.as_ptr() as *const i16, raw_bytes.len() / 2)
                }.to_vec();
                Tensor::from_slice(&i16_data).view_dtype(Kind::Half).reshape(&meta.shape)
            }
            "F32" => {
                let f32_data = unsafe {
                    std::slice::from_raw_parts(raw_bytes.as_ptr() as *const f32, raw_bytes.len() / 4)
                }.to_vec();
                Tensor::from_slice(&f32_data).reshape(&meta.shape)
            }
            _ => {
                Tensor::from_slice(&raw_bytes).view_dtype(Kind::Int8).reshape(&meta.shape)
            }
        };
        Ok(Some(t))
    }
}

// FFI CUDA-Driven Procedural Linear Layer
struct ZigProceduralLinear {
    _name: String,
    in_features: i64,
    out_features: i64,
    scale_u: f64,
    scale_v: f64,
    u_q: Tensor,  // [rank, out_features] (Int8) on GPU (transposed for coalesced Phase 2 reads)
    v_q: Tensor,  // [in_features, rank] (Int8) on GPU
    // Factored residual -- stored as low-rank INT8 SVD (NOT dense Float32!)
    res_u_q: Option<Tensor>,  // [res_rank, out_features] (Int8) on GPU
    res_v_q: Option<Tensor>,  // [in_features, res_rank] (Int8) on GPU
    res_scale_u: f64,
    res_scale_v: f64,
}

// Pre-computed raw GPU pointer dispatch table -- zero tensor ops in hot loop
#[derive(Clone)]
struct LayerDispatch {
    d_v: u64,
    d_u_t: u64,
    scale_v: f32,
    scale_u: f32,
    in_features: i32,
    out_features: i32,
    rank: i32,
    // Factored residual dispatch
    has_res: bool,
    d_res_v: u64,
    d_res_u_t: u64,
    res_scale_v: f32,
    res_scale_u: f32,
    res_rank: i32,
}

// High-performance logit sampling -- operates on top-K subset only (40 elements, not 262K)
fn sample_next_token(logits: &Tensor, temperature: f64, top_k: i64, top_p: f64) -> i64 {
    if temperature <= 0.0 {
        return logits.argmax(0, false).int64_value(&[]);
    }
    let scaled_logits = logits / temperature;

    // Extract top-K candidates (returned pre-sorted descending by topk)
    let (top_values, top_indices) = scaled_logits.topk(top_k, 0, true, true);

    // Softmax over the tiny 40-element top-K set (NOT 262K!)
    let mut probs = top_values.softmax(0, Kind::Float);

    // Top-P (nucleus) filtering on the 40-element subset
    if top_p < 1.0 {
        let cum_probs = probs.cumsum(0, Kind::Float);
        // Build shifted mask: keep at least the top-1 token
        let shifted_cum = Tensor::cat(&[
            &Tensor::from_slice(&[0.0f32]).to_device(probs.device()),
            &cum_probs.slice(0, 0, top_k - 1, 1),
        ], 0);
        let mask = shifted_cum.ge(top_p);
        probs = probs.masked_fill(&mask, 0.0);
        // Renormalize
        let sum = probs.sum(Kind::Float);
        let sum_val = sum.double_value(&[]);
        // Safety: if all probs were masked to 0 or NaN, fallback to top-1 token
        if sum_val <= 0.0 || sum_val.is_nan() {
            return top_indices.int64_value(&[0]);
        }
        probs = probs / sum;
    }

    // Sample from the tiny candidate set and map back to vocab index
    let sampled_local = probs.multinomial(1, true).int64_value(&[]);
    top_indices.int64_value(&[sampled_local])
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    unsafe {
        load_cuda_dlls();
    }
    let device = if tch::Cuda::is_available() { Device::Cuda(0) } else { Device::Cpu };
    
    let model_dir = get_model_dir();
    let genesis_path = get_genesis_path();
    let cuneiform_bin = get_cuneiform_bin();
    let b_size = get_batch_size();

    println!("=========================================================");
    println!("  GEMMA-4-31B SUMERIAN -- RUST-ZIG HYBRID SVD KERNEL BENCHMARK");
    println!("  NOTE: This runner executes SVD factored linear layers.");
    println!("  It omits softmax attention, normalization, and residuals.");
    println!("  For full Gemma inference, use run_gemma4_unified.py.");
    println!("  Target Device: {:?}", device);
    println!("=========================================================");
    println!("  WARNING: Cuneiform coordinate steering is many-to-one");
    println!("  and is NOT lossless. Steered tokens may differ from sampled.");
    println!("=========================================================");

    // Force PyTorch CUDA context initialization by allocating a tiny dummy tensor on CUDA
    if let Device::Cuda(_) = device {
        let _dummy = Tensor::zeros(&[1], (Kind::Float, device));
    }

    // Step 1: Initialize Zig CUDA Core via FFI
    println!("\n[1] Initializing Zig CUDA JIT PTX Engine...");
    unsafe {
        let status = sumerian_init_cuda();
        if status != 0 {
            eprintln!("[-] Failed to initialize Zig CUDA Core. Code: {}", status);
            std::process::exit(1);
        }
    }
    println!("    [+] Zig Core CUDA driver initialization & NVRTC compiler JIT: OK.");

    // Step 2: Initialize Cuneiform vocabulary coordinates map in Zig
    println!("\n[2] Loading Cuneiform-U vocabulary coordinate index in Zig...");
    let c_path = CString::new(cuneiform_bin.as_str())?;
    unsafe {
        let status = sumerian_cuneiform_init(c_path.as_ptr());
        if status != 0 {
            eprintln!("[-] Failed to load Cuneiform coordinate map. Code: {}", status);
            std::process::exit(1);
        }
    }
    println!("    [+] Indexed 6D hypercube coordinate mappings successfully.");

    // Step 3: Load non-SVD weights via seek safetensors
    println!("\n[3] Seek-loading non-SVD parameters onto GPU/CPU...");
    let index_file = File::open(format!("{}/model.safetensors.index.json", model_dir))?;
    let index_data: Value = serde_json::from_reader(index_file)?;
    let weight_map = index_data["weight_map"].as_object().ok_or("Invalid index json format")?;

    let mut non_svd_map: HashMap<String, Vec<String>> = HashMap::new();
    let svd_keys = ["q_proj", "k_proj", "v_proj", "o_proj", "gate_proj", "up_proj", "down_proj"];

    for (param_name, shard_name) in weight_map {
        let is_svd = svd_keys.iter().any(|&k| param_name.contains(k));
        if !is_svd {
            let shard_str = shard_name.as_str().unwrap().to_string();
            non_svd_map.entry(shard_str).or_insert_with(Vec::new).push(param_name.clone());
        }
    }

    let mut model_params: HashMap<String, Tensor> = HashMap::new();
    for (shard, param_names) in non_svd_map {
        let shard_path = format!("{}/{}", model_dir, shard);
        println!("    Loading {} parameters from {}...", param_names.len(), shard);
        let mut reader = SeekSafetensors::open(shard_path)?;
        for name in param_names {
            if let Some(mut tensor) = reader.get_tensor(&name)? {
                // Keep massive embeddings on CPU, move lightweight layernorms/scales to CUDA
                if !name.contains("embed_tokens") && !name.contains("lm_head") {
                    tensor = tensor.to_device(device);
                }
                model_params.insert(name, tensor);
            }
        }
    }

    // Quantize embedding weights to INT8 and move transposed to GPU
    println!("\n[3b] Quantizing shared embedding to INT8 for GPU-resident LM Head...");
    let embed_tensor = model_params.get("model.language_model.embed_tokens.weight").ok_or("Missing embedding weight")?;
    let max_val = embed_tensor.abs().max().double_value(&[]);
    let scale_w = (max_val / 127.0) as f32;
    let w_q = (embed_tensor / scale_w as f64).round().to_kind(Kind::Int8);
    let w_q_T = w_q.to_device(device).tr().contiguous();
    let embed_vram_mb = (262144i64 * 5376 * 1) as f64 / (1024.0 * 1024.0);
    println!("    [+] Quantized shared parameter matrix to INT8 in GPU VRAM (~{:.0} MB).", embed_vram_mb);

    // Pre-allocate GPU scratchpad tensors to avoid dynamic allocation overhead
    println!("    [+] Pre-allocating GPU scratchpads for batch size {}...", b_size);
    let t_scratch = Tensor::zeros(&[b_size, 128], (Kind::Float, device)); // Max rank = 128
    let y_scratch_1 = Tensor::zeros(&[b_size, 21504], (Kind::BFloat16, device)); // Max out_features = 21504
    let y_scratch_2 = Tensor::zeros(&[b_size, 21504], (Kind::BFloat16, device)); // Max out_features = 21504
    let logits = Tensor::zeros(&[b_size, 262144], (Kind::Float, device)); // Pre-allocated logit space

    // Step 4: Stream-patch SVD layers from Genesis
    println!("\n[4] Stream-patching SVD layers from SubZero Genesis...");
    let mut genesis_file = File::open(&genesis_path)?;
    
    let mut magic_buf = [0u8; 4];
    genesis_file.read_exact(&mut magic_buf)?;
    
    let mut ver_buf = [0u8; 2];
    genesis_file.read_exact(&mut ver_buf)?;
    let version = u16::from_be_bytes(ver_buf);
    
    let mut watermark = [0u8; 32];
    genesis_file.read_exact(&mut watermark)?;
    let watermark_str = String::from_utf8_lossy(&watermark).trim().to_string();
    println!("    Genesis Version: {} | Watermark: {}", version, watermark_str);
    
    let mut perf_magic_buf = [0u8; 4];
    genesis_file.read_exact(&mut perf_magic_buf)?;
    
    let mut meta_buf = [0u8; 24];
    genesis_file.read_exact(&mut meta_buf)?; // skip shape metadata (hidden, head counts, etc.)

    let mut energy_buf = [0u8; 16];
    genesis_file.read_exact(&mut energy_buf)?; // skip energy threshold
    
    let mut layers_buf = [0u8; 4];
    genesis_file.read_exact(&mut layers_buf)?;
    let num_layers = u32::from_be_bytes(layers_buf);
    
    let mut procedural_layers: Vec<ZigProceduralLinear> = Vec::with_capacity(num_layers as usize);

    for _ in 0..num_layers {
        let mut name_len_buf = [0u8; 2];
        genesis_file.read_exact(&mut name_len_buf)?;
        let name_len = u16::from_be_bytes(name_len_buf) as usize;
        
        let mut name_buf = vec![0u8; name_len];
        genesis_file.read_exact(&mut name_buf)?;
        let name = String::from_utf8(name_buf)?;
        
        let mut dims_buf = [0u8; 12];
        genesis_file.read_exact(&mut dims_buf)?;
        let m = u32::from_be_bytes([dims_buf[0], dims_buf[1], dims_buf[2], dims_buf[3]]) as i64;
        let n = u32::from_be_bytes([dims_buf[4], dims_buf[5], dims_buf[6], dims_buf[7]]) as i64;
        let rank = u32::from_be_bytes([dims_buf[8], dims_buf[9], dims_buf[10], dims_buf[11]]) as i64;
        
        let mut scale_buf = [0u8; 8];
        genesis_file.read_exact(&mut scale_buf)?;
        let scale_u = f32::from_be_bytes([scale_buf[0], scale_buf[1], scale_buf[2], scale_buf[3]]) as f64;
        let scale_v = f32::from_be_bytes([scale_buf[4], scale_buf[5], scale_buf[6], scale_buf[7]]) as f64;
        
        // Read Quantized Matrices (Int8) and move immediately to CUDA (transpose U to [rank, m])
        let mut u_q_buf = vec![0u8; (m * rank) as usize];
        genesis_file.read_exact(&mut u_q_buf)?;
        let u_q = Tensor::from_slice(&u_q_buf).view_dtype(Kind::Int8).reshape(&[m, rank]).to_device(device).tr().contiguous();
        
        let mut v_q_buf = vec![0u8; (n * rank) as usize];
        genesis_file.read_exact(&mut v_q_buf)?;
        let v_q = Tensor::from_slice(&v_q_buf).view_dtype(Kind::Int8).reshape(&[n, rank]).to_device(device);
        
        let mut has_res = [0u8; 1];
        genesis_file.read_exact(&mut has_res)?;
        let mut res_u_q = None;
        let mut res_v_q = None;
        let mut res_scale_u = 0.0f64;
        let mut res_scale_v = 0.0f64;
        
        if has_res[0] == 1 {
            let mut res_rank_buf = [0u8; 4];
            genesis_file.read_exact(&mut res_rank_buf)?;
            let res_rank = u32::from_be_bytes(res_rank_buf) as i64;
            
            let mut res_scale_buf = [0u8; 8];
            genesis_file.read_exact(&mut res_scale_buf)?;
            res_scale_u = f32::from_be_bytes([res_scale_buf[0], res_scale_buf[1], res_scale_buf[2], res_scale_buf[3]]) as f64;
            res_scale_v = f32::from_be_bytes([res_scale_buf[4], res_scale_buf[5], res_scale_buf[6], res_scale_buf[7]]) as f64;
            
            let mut u_res_buf = vec![0u8; (m * res_rank) as usize];
            genesis_file.read_exact(&mut u_res_buf)?;
            // Keep as INT8 and transpose to [res_rank, m] -- reuses same JIT kernel as main SVD
            res_u_q = Some(Tensor::from_slice(&u_res_buf).view_dtype(Kind::Int8).reshape(&[m, res_rank]).to_device(device).tr().contiguous());
            
            let mut v_res_buf = vec![0u8; (n * res_rank) as usize];
            genesis_file.read_exact(&mut v_res_buf)?;
            // Keep as INT8 [n, res_rank] -- same layout as main v_q
            res_v_q = Some(Tensor::from_slice(&v_res_buf).view_dtype(Kind::Int8).reshape(&[n, res_rank]).to_device(device));
        }

        let p_linear = ZigProceduralLinear {
            _name: name.clone(),
            in_features: n, out_features: m,
            scale_u, scale_v,
            u_q, v_q,
            res_u_q, res_v_q, res_scale_u, res_scale_v,
        };
        procedural_layers.push(p_linear);
    }
    println!("    [+] Loaded {} procedural linear layers into GPU memory.", procedural_layers.len());

    let prompt_tokens = vec![100i64, 200, 300];
    let mut curr_ids = prompt_tokens.clone();
    let last_token_id = *curr_ids.last().unwrap();

    // Build pre-computed dispatch table -- caches ALL raw GPU pointers
    println!("\n[5] Building zero-overhead dispatch table...");
    let mut dispatch_table: Vec<LayerDispatch> = Vec::with_capacity(procedural_layers.len());
    for layer in &procedural_layers {
        let rank = layer.u_q.size()[0];
        let (has_res, d_res_v, d_res_u_t, rs_v, rs_u, rr) = match (&layer.res_v_q, &layer.res_u_q) {
            (Some(rv), Some(ru)) => (
                true,
                rv.data_ptr() as u64,
                ru.data_ptr() as u64,
                layer.res_scale_v as f32,
                layer.res_scale_u as f32,
                ru.size()[0] as i32,
            ),
            _ => (false, 0u64, 0u64, 0.0f32, 0.0f32, 0i32),
        };
        dispatch_table.push(LayerDispatch {
            d_v: layer.v_q.data_ptr() as u64,
            d_u_t: layer.u_q.data_ptr() as u64,
            scale_v: layer.scale_v as f32,
            scale_u: layer.scale_u as f32,
            in_features: layer.in_features as i32,
            out_features: layer.out_features as i32,
            rank: rank as i32,
            has_res, d_res_v, d_res_u_t,
            res_scale_v: rs_v, res_scale_u: rs_u, res_rank: rr,
        });
    }
    let num_with_res = dispatch_table.iter().filter(|d| d.has_res).count();
    println!("    [+] Dispatch table: {} layers ({} with factored residuals)", dispatch_table.len(), num_with_res);

    // Initialize hidden state from embedding lookup (not random)
    // NOTE: This still only runs SVD projections sequentially without attention/norm/residuals.
    // It is a kernel throughput benchmark, not full Gemma transformer inference.
    let first_layer_in_features = dispatch_table[0].in_features as i64;
    let base_hidden = embed_tensor.get(last_token_id).unsqueeze(0).repeat(&[b_size, 1]).to_kind(Kind::BFloat16).to_device(device);
    let hidden_input = if first_layer_in_features > 5376 {
        Tensor::cat(&[
            &base_hidden,
            &Tensor::zeros(&[b_size, first_layer_in_features - 5376], (Kind::BFloat16, device))
        ], 1)
    } else {
        base_hidden
    };

    // Pre-cache all fixed GPU pointers for the generation loop
    let d_hidden_input = hidden_input.data_ptr() as u64;
    let d_t_scratch = t_scratch.data_ptr() as u64;
    let d_y1 = y_scratch_1.data_ptr() as u64;
    let d_y2 = y_scratch_2.data_ptr() as u64;
    let d_lm_w = w_q_T.data_ptr() as u64;
    let d_logits = logits.data_ptr() as u64;

    println!("\n[6] Starting autoregressive generation loop (zero-alloc dispatch)...");
    let t_start = Instant::now();
    let mut generated = Vec::new();

    for _step in 0..MAX_NEW_TOKENS {
        let t_dispatch = Instant::now();

        // === HOT LOOP: ZERO tensor operations — raw FFI dispatch only ===
        let mut d_input = d_hidden_input;
        let mut use_scratch_1 = true;

        for layer in &dispatch_table {
            let d_y_out = if use_scratch_1 { d_y1 } else { d_y2 };

            unsafe {
                // Phase 1: T = X × V_q × scale_v
                sumerian_launch_svd_phase1(
                    d_input, layer.d_v, layer.scale_v, d_t_scratch,
                    b_size as i32, layer.in_features, layer.rank,
                );
                // Phase 2: Y = T × U_q_T × scale_u
                sumerian_launch_svd_phase2(
                    d_t_scratch, layer.d_u_t, layer.scale_u, d_y_out,
                    b_size as i32, layer.out_features, layer.rank, 0,
                );

                // Residual SVD: Y += T_res × U_res_T × scale_u_res
                if layer.has_res {
                    sumerian_launch_svd_phase1(
                        d_input, layer.d_res_v, layer.res_scale_v, d_t_scratch,
                        b_size as i32, layer.in_features, layer.res_rank,
                    );
                    sumerian_launch_svd_phase2(
                        d_t_scratch, layer.d_res_u_t, layer.res_scale_u, d_y_out,
                        b_size as i32, layer.out_features, layer.res_rank, 1, // accumulate!
                    );
                }
            }

            d_input = d_y_out;
            use_scratch_1 = !use_scratch_1;
        }

        // LM Head: logits = hidden × W_q_T × scale_w
        unsafe {
            let status = sumerian_launch_lm_head(
                d_input, d_lm_w, scale_w, d_logits,
                b_size as i32, 262144, 5376,
            );
            if status != 0 {
                panic!("[-] Zig LM Head kernel launch failed with status: {}", status);
            }
        }

        let dispatch_ms = t_dispatch.elapsed().as_secs_f64() * 1000.0;

        // GPU sync
        let t_sync = Instant::now();
        tch::Cuda::synchronize(0);
        let sync_ms = t_sync.elapsed().as_secs_f64() * 1000.0;

        // Sample next tokens for each sequence in batch
        let t_sample = Instant::now();
        let mut steered_tokens = Vec::with_capacity(b_size as usize);
        for b in 0..b_size {
            let row_logits = logits.get(b);
            let next_token = sample_next_token(&row_logits, TEMPERATURE, TOP_K, TOP_P);

            // Map token ID through Cuneiform 6D coordinate steering
            let mut coords = [0u8; 3];
            unsafe {
                sumerian_cuneiform_id_to_coords(next_token as i32, coords.as_mut_ptr());
            }
            let steered_token = unsafe {
                sumerian_cuneiform_coords_to_id(coords[0], coords[1], coords[2]) as i64
            };
            steered_tokens.push(steered_token);

            // Update row of hidden_input for next step in-place
            let token_embed = embed_tensor.get(steered_token).to_device(device).to_kind(Kind::BFloat16);
            let _ = hidden_input.get(b).slice(0, 0, 5376, 1).copy_(&token_embed);
        }
        let sample_ms = t_sample.elapsed().as_secs_f64() * 1000.0;

        // Print diagnostics for first 3 tokens
        if _step < 3 {
            println!("    [token {}] dispatch={:.2}ms  sync={:.2}ms  sample={:.2}ms  total={:.2}ms",
                _step, dispatch_ms, sync_ms, sample_ms, dispatch_ms + sync_ms + sample_ms);
        }

        let steered_token = steered_tokens[0]; // Log the first batch sequence to console
        curr_ids.push(steered_token);
        generated.push(steered_token);
    }

    let duration = t_start.elapsed();
    let total_tokens = MAX_NEW_TOKENS * b_size as usize;
    let tok_s = total_tokens as f64 / duration.as_secs_f64();

    println!("\n---------------------------------------------------------");
    println!("  SVD KERNEL THROUGHPUT BENCHMARK (NOT full inference)");
    println!("  Batch Size       : {}", b_size);
    println!("  Tokens Generated : {}", total_tokens);
    println!("  Time Elapsed     : {:.2}s", duration.as_secs_f64());
    println!("  Kernel Speed     : {:.2} tok/s", tok_s);
    println!("  NOTE: This measures SVD projection kernel throughput.");
    println!("  It does NOT include attention, normalization, or residuals.");
    println!("  For full Gemma inference, use run_gemma4_unified.py.");
    println!("=========================================================");

    // Cleanup
    unsafe {
        sumerian_cuneiform_free();
        sumerian_deinit_cuda();
    }

    Ok(())
}
