// Gemma-4-31B Sumerian -- Zig CUDA & Cuneiform Core Library (Dynamic Loading Version)
// Watermark: ip zymatica.space | astronautshe.com
// Copyright © 2026 Zymatica
// SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
// See LICENSE for terms.
// Compile with: zig build-lib sumerian_cuda_core.zig -O ReleaseFast -lc

const std = @import("std");
const fs = std.fs;
const io = std.io;
const mem = std.mem;

// CUDA Driver API Types
pub const CUdevice = i32;
pub const CUcontext = ?*anyopaque;
pub const CUmodule = ?*anyopaque;
pub const CUfunction = ?*anyopaque;
pub const CUdeviceptr = u64;

// Function Pointer Types
const cuInit_t = *const fn (flags: u32) callconv(.C) c_int;
const cuDeviceGet_t = *const fn (device: *CUdevice, ordinal: c_int) callconv(.C) c_int;
const cuCtxCreate_t = *const fn (context: *CUcontext, flags: u32, dev: CUdevice) callconv(.C) c_int;
const cuCtxDestroy_t = *const fn (context: CUcontext) callconv(.C) c_int;
const cuCtxGetCurrent_t = *const fn (pctx: *CUcontext) callconv(.C) c_int;
const cuModuleLoadData_t = *const fn (module: *CUmodule, image: ?*const anyopaque) callconv(.C) c_int;
const cuModuleUnload_t = *const fn (module: CUmodule) callconv(.C) c_int;
const cuModuleGetFunction_t = *const fn (hfunc: *CUfunction, hmod: CUmodule, name: [*:0]const u8) callconv(.C) c_int;
const cuLaunchKernel_t = *const fn (
    f: CUfunction,
    gridDimX: u32, gridDimY: u32, gridDimZ: u32,
    blockDimX: u32, blockDimY: u32, blockDimZ: u32,
    sharedMemBytes: u32,
    hStream: ?*anyopaque,
    kernelParams: ?*const ?*anyopaque,
    extra: ?*const ?*anyopaque,
) callconv(.C) c_int;

const cuMemsetD32Async_t = *const fn (
    dstDevice: CUdeviceptr,
    ui: u32,
    N: usize,
    hStream: ?*anyopaque,
) callconv(.C) c_int;

const nvrtcCreateProgram_t = *const fn (
    prog: *?*anyopaque,
    src: [*:0]const u8,
    name: ?[*:0]const u8,
    numHeaders: c_int,
    headers: ?*const ?[*:0]const u8,
    includeNames: ?*const ?[*:0]const u8,
) callconv(.C) c_int;
const nvrtcCompileProgram_t = *const fn (
    prog: ?*anyopaque,
    numOptions: c_int,
    options: ?*const ?[*:0]const u8,
) callconv(.C) c_int;
const nvrtcGetPTXSize_t = *const fn (prog: ?*anyopaque, ptxSize: *usize) callconv(.C) c_int;
const nvrtcGetPTX_t = *const fn (prog: ?*anyopaque, ptx: [*:0]u8) callconv(.C) c_int;
const nvrtcDestroyProgram_t = *const fn (prog: *?*anyopaque) callconv(.C) c_int;
const nvrtcGetProgramLogSize_t = *const fn (prog: ?*anyopaque, logSize: *usize) callconv(.C) c_int;
const nvrtcGetProgramLog_t = *const fn (prog: ?*anyopaque, log: [*:0]u8) callconv(.C) c_int;

// Function Pointers
var p_cuInit: cuInit_t = undefined;
var p_cuDeviceGet: cuDeviceGet_t = undefined;
var p_cuCtxCreate: cuCtxCreate_t = undefined;
var p_cuCtxDestroy: cuCtxDestroy_t = undefined;
var p_cuCtxGetCurrent: cuCtxGetCurrent_t = undefined;
var p_cuModuleLoadData: cuModuleLoadData_t = undefined;
var p_cuModuleUnload: cuModuleUnload_t = undefined;
var p_cuModuleGetFunction: cuModuleGetFunction_t = undefined;
var p_cuLaunchKernel: cuLaunchKernel_t = undefined;
var p_cuMemsetD32Async: cuMemsetD32Async_t = undefined;

var p_nvrtcCreateProgram: nvrtcCreateProgram_t = undefined;
var p_nvrtcCompileProgram: nvrtcCompileProgram_t = undefined;
var p_nvrtcGetPTXSize: nvrtcGetPTXSize_t = undefined;
var p_nvrtcGetPTX: nvrtcGetPTX_t = undefined;
var p_nvrtcDestroyProgram: nvrtcDestroyProgram_t = undefined;
var p_nvrtcGetProgramLogSize: nvrtcGetProgramLogSize_t = undefined;
var p_nvrtcGetProgramLog: nvrtcGetProgramLog_t = undefined;

// Libraries
var cuda_lib: std.DynLib = undefined;
var nvrtc_lib: std.DynLib = undefined;

// Global State
var cu_context: CUcontext = null;
var cu_context_is_owned: bool = false;
var cu_module: CUmodule = null;
var phase1_function: CUfunction = null;
var phase2_function: CUfunction = null;
var lm_head_function: CUfunction = null;
var fused_function: CUfunction = null;

var gpa = std.heap.GeneralPurposeAllocator(.{}){};
var gpa_allocator = gpa.allocator();

// Cuneiform coordinate map memory
var cuneiform_coords: []u8 = &[_]u8{};
const CoordKey = struct { rc: u8, rf: u8, ra: u8 };
var cuneiform_id_map: ?std.AutoHashMap(CoordKey, c_int) = null;

// High-Performance SVD CUDA C++ Kernel source code
const SVD_KERNEL_SRC =
    \\__device__ float bf16_to_float(unsigned short val) {
    \\    unsigned int temp = ((unsigned int)val) << 16;
    \\    return __int_as_float(temp);
    \\}
    \\
    \\__device__ unsigned short float_to_bf16(float val) {
    \\    unsigned int temp = __float_as_uint(val);
    \\    return (unsigned short)(temp >> 16);
    \\}
    \\
    \\extern "C" __global__ void procedural_linear_phase1(
    \\    const unsigned short* X,
    \\    const char* V_q,
    \\    float scale_v,
    \\    float* T,
    \\    int B,
    \\    int n,
    \\    int r
    \\) {
    \\    int b = blockIdx.y;
    \\    int tile_idx = blockIdx.x;
    \\    int tx = threadIdx.x;
    \\
    \\    __shared__ float shared_X[128];
    \\    alignas(16) __shared__ char shared_V[128 * 128];
    \\
    \\    int global_row = tile_idx * 128 + tx;
    \\    shared_X[tx] = bf16_to_float(X[b * n + global_row]);
    \\
    \\    int num_int4s = (128 * r) / 16;
    \\    const int4* V_q_int4 = (const int4*)&V_q[tile_idx * 128 * r];
    \\    int4* shared_V_int4 = (int4*)shared_V;
    \\
    \\    for (int i = tx; i < num_int4s; i += 128) {
    \\        shared_V_int4[i] = V_q_int4[i];
    \\    }
    \\    __syncthreads();
    \\
    \\    float sum = 0.0f;
    \\    if (tx < r) {
    \\        #pragma unroll 8
    \\        for (int j = 0; j < 128; j++) {
    \\            sum += shared_X[j] * (float)shared_V[j * r + tx];
    \\        }
    \\    }
    \\
    \\    if (tx < r) {
    \\        atomicAdd(&T[b * r + tx], sum * scale_v);
    \\    }
    \\}
    \\
    \\extern "C" __global__ void procedural_linear_phase2(
    \\    const float* T,
    \\    const char* U_q_T, // Transposed: [r, m]
    \\    float scale_u,
    \\    unsigned short* Y,
    \\    int B,
    \\    int m,
    \\    int r,
    \\    int accumulate
    \\) {
    \\    int b = blockIdx.y;
    \\    int block_i = blockIdx.x;
    \\    int tx = threadIdx.x;
    \\    int i = block_i * 128 + tx;
    \\
    \\    __shared__ float shared_T[128];
    \\    alignas(16) __shared__ char shared_U[128 * 128];
    \\
    \\    if (tx < r) {
    \\        shared_T[tx] = T[b * r + tx];
    \\    }
    \\
    \\    #pragma unroll 4
    \\    for (int k = 0; k < r; k++) {
    \\        if (block_i * 128 + tx < m) {
    \\            shared_U[k * 128 + tx] = U_q_T[k * m + block_i * 128 + tx];
    \\        } else {
    \\            shared_U[k * 128 + tx] = 0;
    \\        }
    \\    }
    \\    __syncthreads();
    \\
    \\    if (i < m) {
    \\        float sum = 0.0f;
    \\        #pragma unroll 8
    \\        for (int k = 0; k < r; k++) {
    \\            sum += shared_T[k] * (float)shared_U[k * 128 + tx];
    \\        }
    \\        sum *= scale_u;
    \\
    \\        if (accumulate != 0) {
    \\            sum += bf16_to_float(Y[b * m + i]);
    \\        }
    \\        Y[b * m + i] = float_to_bf16(sum);
    \\    }
    \\}
    \\
    \\extern "C" __global__ void quantized_lm_head_kernel(
    \\    const unsigned short* X,
    \\    const char* W_q_T, // Transposed: [hidden_dim, vocab_size]
    \\    float scale_w,
    \\    float* Y,
    \\    int B,
    \\    int vocab_size,
    \\    int hidden_dim
    \\) {
    \\    int b = blockIdx.x / ((vocab_size + 127) / 128);
    \\    int block_i = blockIdx.x % ((vocab_size + 127) / 128);
    \\    int tx = threadIdx.x;
    \\    int i = block_i * 128 + tx;
    \\
    \\    __shared__ float shared_X[128];
    \\    alignas(16) __shared__ char shared_W[128 * 128];
    \\
    \\    float sum = 0.0f;
    \\    for (int j_block = 0; j_block < hidden_dim; j_block += 128) {
    \\        shared_X[tx] = bf16_to_float(X[b * hidden_dim + j_block + tx]);
    \\
    \\        #pragma unroll 4
    \\        for (int row = 0; row < 128; row++) {
    \\            if (block_i * 128 + tx < vocab_size) {
    \\                shared_W[row * 128 + tx] = W_q_T[(j_block + row) * vocab_size + block_i * 128 + tx];
    \\            } else {
    \\                shared_W[row * 128 + tx] = 0;
    \\            }
    \\        }
    \\        __syncthreads();
    \\
    \\        if (i < vocab_size) {
    \\            #pragma unroll 8
    \\            for (int j = 0; j < 128; j++) {
    \\                sum += shared_X[j] * (float)shared_W[j * 128 + tx];
    \\            }
    \\        }
    \\        __syncthreads();
    \\    }
    \\    if (i < vocab_size) {
    \\        Y[b * vocab_size + i] = sum * scale_w;
    \\    }
    \\}
    \\
    \\extern "C" __global__ void procedural_linear_fused(
    \\    const unsigned short* X,
    \\    const char* V_q,
    \\    float scale_v,
    \\    const char* U_q_T, // Transposed: [r, m]
    \\    float scale_u,
    \\    unsigned short* Y,
    \\    int B,
    \\    int n,
    \\    int m,
    \\    int r,
    \\    int accumulate
    \\) {
    \\    int b = blockIdx.y;
    \\    int block_i = blockIdx.x;
    \\    int tx = threadIdx.x;
    \\    int i = block_i * 128 + tx;
    \\
    \\    __shared__ float shared_T[128];
    \\    __shared__ float shared_X[128];
    \\    alignas(16) __shared__ char shared_V[128 * 128];
    \\    alignas(16) __shared__ char shared_U[128 * 128];
    \\
    \\    if (tx < r) {
    \\        shared_T[tx] = 0.0f;
    \\    }
    \\    __syncthreads();
    \\
    \\    for (int j_block = 0; j_block < n; j_block += 128) {
    \\        shared_X[tx] = bf16_to_float(X[b * n + j_block + tx]);
    \\
    \\        int num_int4s = (128 * r) / 16;
    \\        const int4* V_q_int4 = (const int4*)&V_q[j_block * r];
    \\        int4* shared_V_int4 = (int4*)shared_V;
    \\
    \\        for (int idx = tx; idx < num_int4s; idx += 128) {
    \\            shared_V_int4[idx] = V_q_int4[idx];
    \\        }
    \\        __syncthreads();
    \\
    \\        if (tx < r) {
    \\            float sum = 0.0f;
    \\            #pragma unroll 8
    \\            for (int j = 0; j < 128; j++) {
    \\                sum += shared_X[j] * (float)shared_V[j * r + tx];
    \\            }
    \\            shared_T[tx] += sum;
    \\        }
    \\        __syncthreads();
    \\    }
    \\
    \\    if (tx < r) {
    \\        shared_T[tx] *= scale_v;
    \\    }
    \\
    \\    #pragma unroll 4
    \\    for (int k = 0; k < r; k++) {
    \\        if (block_i * 128 + tx < m) {
    \\            shared_U[k * 128 + tx] = U_q_T[k * m + block_i * 128 + tx];
    \\        } else {
    \\            shared_U[k * 128 + tx] = 0;
    \\        }
    \\    }
    \\    __syncthreads();
    \\
    \\    if (i < m) {
    \\        float sum = 0.0f;
    \\        #pragma unroll 8
    \\        for (int k = 0; k < r; k++) {
    \\            sum += shared_T[k] * (float)shared_U[k * 128 + tx];
    \\        }
    \\        sum *= scale_u;
    \\
    \\        if (accumulate != 0) {
    \\            sum += bf16_to_float(Y[b * m + i]);
    \\        }
    \\        Y[b * m + i] = float_to_bf16(sum);
    \\    }
    \\}
;

// Helper function to compile CUDA C++ code to PTX at runtime using NVRTC
fn compileCudaSource(src: [*:0]const u8) ![]u8 {
    var prog: ?*anyopaque = null;
    if (p_nvrtcCreateProgram(&prog, src, "procedural_linear.cu", 0, null, null) != 0) {
        return error.NvrtcCreateProgramFailed;
    }
    defer _ = p_nvrtcDestroyProgram(&prog);

    // Compile options targeting Compute Capability 7.5 (standard modern GPUs like GTX 1660 Ti)
    const opts = [_]?[*:0]const u8{
        "-arch=compute_75",
    };

    const compile_status = p_nvrtcCompileProgram(prog, opts.len, &opts[0]);
    if (compile_status != 0) {
        var log_size: usize = 0;
        _ = p_nvrtcGetProgramLogSize(prog, &log_size);
        const log = try gpa_allocator.alloc(u8, log_size);
        defer gpa_allocator.free(log);
        _ = p_nvrtcGetProgramLog(prog, @ptrCast(log.ptr));
        std.debug.print("[-] CUDA Compilation failed log:\n{s}\n", .{log});
        return error.CudaCompilationFailed;
    }

    var ptx_size: usize = 0;
    if (p_nvrtcGetPTXSize(prog, &ptx_size) != 0) {
        return error.NvrtcGetPTXSizeFailed;
    }

    const ptx = try gpa_allocator.alloc(u8, ptx_size);
    if (p_nvrtcGetPTX(prog, @ptrCast(ptx.ptr)) != 0) {
        gpa_allocator.free(ptx);
        return error.NvrtcGetPTXFailed;
    }

    return ptx;
}

// -----------------------------------------------------------------------------
// EXPORTED C FFI INTERFACE
// -----------------------------------------------------------------------------

export fn sumerian_init_cuda() callconv(.C) c_int {
    // Load nvcuda.dll dynamically
    cuda_lib = std.DynLib.open("nvcuda.dll") catch return -10;
    p_cuInit = cuda_lib.lookup(cuInit_t, "cuInit") orelse return -11;
    p_cuDeviceGet = cuda_lib.lookup(cuDeviceGet_t, "cuDeviceGet") orelse return -12;
    p_cuCtxCreate = cuda_lib.lookup(cuCtxCreate_t, "cuCtxCreate") orelse return -13;
    p_cuCtxDestroy = cuda_lib.lookup(cuCtxDestroy_t, "cuCtxDestroy") orelse return -14;
    p_cuModuleLoadData = cuda_lib.lookup(cuModuleLoadData_t, "cuModuleLoadData") orelse return -15;
    p_cuModuleUnload = cuda_lib.lookup(cuModuleUnload_t, "cuModuleUnload") orelse return -16;
    p_cuModuleGetFunction = cuda_lib.lookup(cuModuleGetFunction_t, "cuModuleGetFunction") orelse return -17;
    p_cuLaunchKernel = cuda_lib.lookup(cuLaunchKernel_t, "cuLaunchKernel") orelse return -18;
    p_cuMemsetD32Async = cuda_lib.lookup(cuMemsetD32Async_t, "cuMemsetD32Async") orelse return -28;
    p_cuCtxGetCurrent = cuda_lib.lookup(cuCtxGetCurrent_t, "cuCtxGetCurrent") orelse return -19;

    // Load nvrtc.dll dynamically
    nvrtc_lib = std.DynLib.open("nvrtc64_120_0.dll") catch std.DynLib.open("nvrtc.dll") catch return -20;
    p_nvrtcCreateProgram = nvrtc_lib.lookup(nvrtcCreateProgram_t, "nvrtcCreateProgram") orelse return -21;
    p_nvrtcCompileProgram = nvrtc_lib.lookup(nvrtcCompileProgram_t, "nvrtcCompileProgram") orelse return -22;
    p_nvrtcGetPTXSize = nvrtc_lib.lookup(nvrtcGetPTXSize_t, "nvrtcGetPTXSize") orelse return -23;
    p_nvrtcGetPTX = nvrtc_lib.lookup(nvrtcGetPTX_t, "nvrtcGetPTX") orelse return -24;
    p_nvrtcDestroyProgram = nvrtc_lib.lookup(nvrtcDestroyProgram_t, "nvrtcDestroyProgram") orelse return -25;
    p_nvrtcGetProgramLogSize = nvrtc_lib.lookup(nvrtcGetProgramLogSize_t, "nvrtcGetProgramLogSize") orelse return -26;
    p_nvrtcGetProgramLog = nvrtc_lib.lookup(nvrtcGetProgramLog_t, "nvrtcGetProgramLog") orelse return -27;

    if (p_cuInit(0) != 0) return -1;

    var dev: CUdevice = 0;
    if (p_cuDeviceGet(&dev, 0) != 0) return -2;

    var current_ctx: CUcontext = null;
    if (p_cuCtxGetCurrent(&current_ctx) == 0 and current_ctx != null) {
        cu_context = current_ctx;
        cu_context_is_owned = false;
    } else {
        if (p_cuCtxCreate(&cu_context, 0, dev) != 0) return -3;
        cu_context_is_owned = true;
    }

    // Compile high-performance JIT CUDA kernel at runtime
    const ptx = compileCudaSource(SVD_KERNEL_SRC) catch |err| {
        std.debug.print("[-] NVRTC CUDA compilation failed: {}\n", .{err});
        return -4;
    };
    defer gpa_allocator.free(ptx);

    // Null-terminate PTX string safely for loading
    const ptx_null_terminated = gpa_allocator.alloc(u8, ptx.len + 1) catch return -5;
    defer gpa_allocator.free(ptx_null_terminated);
    @memcpy(ptx_null_terminated[0..ptx.len], ptx);
    ptx_null_terminated[ptx.len] = 0;

    if (p_cuModuleLoadData(&cu_module, @ptrCast(ptx_null_terminated.ptr)) != 0) return -6;

    if (p_cuModuleGetFunction(&phase1_function, cu_module, "procedural_linear_phase1") != 0) return -7;
    if (p_cuModuleGetFunction(&phase2_function, cu_module, "procedural_linear_phase2") != 0) return -8;
    if (p_cuModuleGetFunction(&lm_head_function, cu_module, "quantized_lm_head_kernel") != 0) return -9;
    if (p_cuModuleGetFunction(&fused_function, cu_module, "procedural_linear_fused") != 0) return -29;

    return 0; // Success
}

export fn sumerian_launch_svd_phase1(
    d_X: u64,
    d_V: u64,
    scale_v: f32,
    d_T: u64,
    B: c_int,
    n: c_int,
    r: c_int,
) callconv(.C) c_int {
    if (phase1_function == null) return -1;

    // Clear d_T output buffer asynchronously
    const memset_status = p_cuMemsetD32Async(d_T, 0, @intCast(B * r), null);
    if (memset_status != 0) return memset_status;

    var d_X_val = d_X;
    var d_V_val = d_V;
    var scale_v_val = scale_v;
    var d_T_val = d_T;
    var B_val = B;
    var n_val = n;
    var r_val = r;

    const args = [_]?*anyopaque{
        &d_X_val,
        &d_V_val,
        &scale_v_val,
        &d_T_val,
        &B_val,
        &n_val,
        &r_val,
    };

    const grid_x = @as(u32, @intCast(n)) / 128;

    const status = p_cuLaunchKernel(
        phase1_function,
        grid_x, @intCast(B), 1,
        128, 1, 1,
        0,
        null,
        &args[0],
        null,
    );

    return status;
}

export fn sumerian_launch_svd_phase2(
    d_T: u64,
    d_U_T: u64,
    scale_u: f32,
    d_Y: u64,
    B: c_int,
    m: c_int,
    r: c_int,
    accumulate: c_int,
) callconv(.C) c_int {
    if (phase2_function == null) return -1;

    var d_T_val = d_T;
    var d_U_T_val = d_U_T;
    var scale_u_val = scale_u;
    var d_Y_val = d_Y;
    var B_val = B;
    var m_val = m;
    var r_val = r;
    var accumulate_val = accumulate;

    const args = [_]?*anyopaque{
        &d_T_val,
        &d_U_T_val,
        &scale_u_val,
        &d_Y_val,
        &B_val,
        &m_val,
        &r_val,
        &accumulate_val,
    };

    const shared_mem_bytes: u32 = @intCast(@as(usize, @intCast(r)) * @sizeOf(f32));
    const grid_x: u32 = (@as(u32, @intCast(m)) + 127) / 128;

    const status = p_cuLaunchKernel(
        phase2_function,
        grid_x, @intCast(B), 1,
        128, 1, 1,
        shared_mem_bytes,
        null,
        &args[0],
        null,
    );

    return status;
}

export fn sumerian_launch_lm_head(
    d_X: u64,
    d_W_T: u64,
    scale_w: f32,
    d_Y: u64,
    B: c_int,
    vocab_size: c_int,
    hidden_dim: c_int,
) callconv(.C) c_int {
    if (lm_head_function == null) return -1;

    var d_X_val = d_X;
    var d_W_T_val = d_W_T;
    var scale_w_val = scale_w;
    var d_Y_val = d_Y;
    var B_val = B;
    var vocab_size_val = vocab_size;
    var hidden_dim_val = hidden_dim;

    const args = [_]?*anyopaque{
        &d_X_val,
        &d_W_T_val,
        &scale_w_val,
        &d_Y_val,
        &B_val,
        &vocab_size_val,
        &hidden_dim_val,
    };

    const grid_x: u32 = ((@as(u32, @intCast(vocab_size)) + 127) / 128) * @as(u32, @intCast(B));

    const status = p_cuLaunchKernel(
        lm_head_function,
        grid_x, 1, 1,
        128, 1, 1,
        0,
        null,
        &args[0],
        null,
    );

    return status;
}

export fn sumerian_deinit_cuda() callconv(.C) void {
    if (cu_module) |mod| {
        _ = p_cuModuleUnload(mod);
        cu_module = null;
    }
    if (cu_context) |ctx| {
        if (cu_context_is_owned) {
            _ = p_cuCtxDestroy(ctx);
        }
        cu_context = null;
    }
    cuda_lib.close();
    nvrtc_lib.close();
    _ = gpa.deinit();
}

export fn sumerian_cuneiform_init(bin_path: [*c]const u8) callconv(.C) c_int {
    const path = std.mem.span(bin_path);
    const file = fs.openFileAbsolute(path, .{}) catch |err| {
        std.debug.print("[-] Failed to open cuneiform binary file: {s} (err={})\n", .{path, err});
        return -1;
    };
    defer file.close();

    const size = file.getEndPos() catch return -2;
    const buffer = gpa_allocator.alloc(u8, size) catch return -3;

    const read_bytes = file.readAll(buffer) catch {
        gpa_allocator.free(buffer);
        return -4;
    };

    cuneiform_coords = buffer[0..read_bytes];
    const vocab_size = read_bytes / 3;

    cuneiform_id_map = std.AutoHashMap(CoordKey, c_int).init(gpa_allocator);
    var i: usize = 0;
    while (i < vocab_size) : (i += 1) {
        const offset = i * 3;
        const key = CoordKey{
            .rc = cuneiform_coords[offset],
            .rf = cuneiform_coords[offset + 1],
            .ra = cuneiform_coords[offset + 2],
        };
        cuneiform_id_map.?.put(key, @intCast(i)) catch return -5;
    }

    return 0; // Success
}

export fn sumerian_cuneiform_coords_to_id(rc: u8, rf: u8, ra: u8) callconv(.C) c_int {
    if (cuneiform_id_map) |*map| {
        const key = CoordKey{ .rc = rc, .rf = rf, .ra = ra };
        if (map.get(key)) |id| {
            return id;
        }
    }
    return 0; // Fallback to token ID 0
}

export fn sumerian_cuneiform_id_to_coords(id: c_int, coords_out: [*c]u8) callconv(.C) void {
    const uid: usize = @intCast(id);
    const offset = uid * 3;
    if (offset + 2 < cuneiform_coords.len) {
        coords_out[0] = cuneiform_coords[offset];
        coords_out[1] = cuneiform_coords[offset + 1];
        coords_out[2] = cuneiform_coords[offset + 2];
    } else {
        coords_out[0] = 0;
        coords_out[1] = 0;
        coords_out[2] = 0;
    }
}

export fn sumerian_cuneiform_free() callconv(.C) void {
    if (cuneiform_id_map) |*map| {
        map.deinit();
        cuneiform_id_map = null;
    }
    if (cuneiform_coords.len > 0) {
        gpa_allocator.free(cuneiform_coords);
        cuneiform_coords = &[_]u8{};
    }
}

export fn sumerian_launch_svd_fused(
    d_X: u64,
    d_V: u64,
    scale_v: f32,
    d_U_T: u64,
    scale_u: f32,
    d_Y: u64,
    B: c_int,
    n: c_int,
    m: c_int,
    r: c_int,
    accumulate: c_int,
) callconv(.C) c_int {
    if (fused_function == null) return -1;

    var d_X_val = d_X;
    var d_V_val = d_V;
    var scale_v_val = scale_v;
    var d_U_T_val = d_U_T;
    var scale_u_val = scale_u;
    var d_Y_val = d_Y;
    var B_val = B;
    var n_val = n;
    var m_val = m;
    var r_val = r;
    var accumulate_val = accumulate;

    const args = [_]?*anyopaque{
        &d_X_val,
        &d_V_val,
        &scale_v_val,
        &d_U_T_val,
        &scale_u_val,
        &d_Y_val,
        &B_val,
        &n_val,
        &m_val,
        &r_val,
        &accumulate_val,
    };

    const grid_x: u32 = (@as(u32, @intCast(m)) + 127) / 128;

    const status = p_cuLaunchKernel(
        fused_function,
        grid_x, @intCast(B), 1,
        128, 1, 1,
        0,
        null,
        &args[0],
        null,
    );

    return status;
}
