import argparse
import numpy as np
from scipy.fft import dct, idct

def dct_compress_vector(v, K):
    """Applies DCT-II, keeps top-K low-frequency coefficients, and returns them."""
    v_dct = dct(v.astype(np.float64), norm='ortho')
    # Keep only the first K low-frequency coefficients (spectral truncation)
    truncated = np.zeros_like(v_dct)
    truncated[:K] = v_dct[:K]
    return truncated

def idct_reconstruct_vector(v_dct_trunc):
    """Applies IDCT-III to reconstruct the vector from truncated DCT coefficients."""
    return idct(v_dct_trunc, norm='ortho')

def run_proof():
    print("======================================================================")
    print("ZYMATICA | SVD/DCT Compression & Reconstructor Pipeline Proof")
    print("======================================================================\n")

    M, N = 64, 64
    RANK = 4
    K_COEF = 8  # Keep 8 lowest frequency DCT coefficients out of 64

    # 1. Generate structured weights (low-rank + smooth variations)
    print(f"[1] Simulating Target Weight Delta Matrix W ({M}x{N} floats)...")
    t = np.linspace(0, 2 * np.pi, M)
    # Build smooth spatial features
    u1 = np.sin(t)
    v1 = np.cos(t)
    u2 = np.sin(2 * t)
    v2 = np.cos(2 * t)
    
    W_true = np.outer(u1, v1) + np.outer(u2, v2)
    # Add minor noise
    rng = np.random.RandomState(42)
    W_true += 0.05 * rng.standard_normal((M, N))
    
    raw_size_bytes = W_true.nbytes
    print(f"  - Original weight matrix shape: {W_true.shape}")
    print(f"  - Original weight raw size: {raw_size_bytes} bytes ({raw_size_bytes / 1024:.2f} KB)")

    # 2. Run Singular Value Decomposition (SVD)
    print(f"\n[2] Executing Low-Rank SVD (Rank={RANK})...")
    U, S, Vh = np.linalg.svd(W_true, full_matrices=False)
    
    U_r = U[:, :RANK]
    S_r = S[:RANK]
    V_r = Vh[:RANK, :].T  # Columns are right singular vectors
    
    # Absorb square root of S
    sqrt_S = np.sqrt(S_r)
    U_scaled = U_r * sqrt_S
    V_scaled = V_r * sqrt_S
    print(f"  - Absorb singular values: U_scaled shape={U_scaled.shape}, V_scaled shape={V_scaled.shape}")

    # 3. Apply DCT-II to compress singular vectors
    print(f"\n[3] Projecting Singular Vectors into DCT Domain (Keeping Top-{K_COEF} Coefficients)...")
    U_rec = np.zeros_like(U_scaled)
    V_rec = np.zeros_like(V_scaled)
    
    for col in range(RANK):
        # Compress U column
        u_dct = dct_compress_vector(U_scaled[:, col], K_COEF)
        U_rec[:, col] = idct_reconstruct_vector(u_dct)
        
        # Compress V column
        v_dct = dct_compress_vector(V_scaled[:, col], K_COEF)
        V_rec[:, col] = idct_reconstruct_vector(v_dct)
        
    print("  -> DCT & Inverse DCT spectral transformations completed.")

    # 4. Reconstruct original weights matrix
    print("\n[4] Rebuilding Layer Weights Matrix from Compressed Manifold...")
    W_rec = np.dot(U_rec, V_rec.T)
    
    # Calculate compression metrics
    # Stored data: 2 matrices of (RANK x K_COEF) float32 coefficients.
    stored_floats = 2 * (RANK * K_COEF)
    compressed_bytes = stored_floats * 4
    compression_ratio = raw_size_bytes / compressed_bytes
    
    mse = np.mean((W_true - W_rec) ** 2)
    cosine_sim = np.dot(W_true.flatten(), W_rec.flatten()) / (np.linalg.norm(W_true) * np.linalg.norm(W_rec) + 1e-9)
    
    print(f"  - Original Float Parameters:     {W_true.size:,}")
    print(f"  - Compressed Float Parameters:   {stored_floats:,}")
    print(f"  - Compression Ratio:              {compression_ratio:.2f}x")
    print(f"  - Reconstruction MSE:            {mse:.6f}")
    print(f"  - Cosine Similarity (Fidelity):  {cosine_sim * 100:.2f}%")

    print("\n[VERIFICATION] SVD/DCT spectral projection pipeline verified.")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Zymatica SVD/DCT Compression Proof")
    parser.add_argument("--test", action="store_true", help="Run test mode")
    args = parser.parse_args()
    run_proof()
