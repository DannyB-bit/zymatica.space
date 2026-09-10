import numpy as np
from scipy.fft import dct, idct

class TrajectoryCompressor:
    """
    SVD/DCT Trajectory Compressor (Zymatica Invention 07 Adaptation).
    Compresses cell trajectories (3D position sequences over time, shape T x 3)
    using Singular Value Decomposition (SVD) and Discrete Cosine Transform (DCT)
    to represent the spatial movement patterns compactly.
    """
    def __init__(self, rank=2, k_coef=8):
        self.rank = rank
        self.k_coef = k_coef

    def compress(self, trajectory):
        """
        Compresses a T x 3 trajectory.
        Returns:
            compressed_dict: Dictionary containing compressed DCT coefficients and scale factors.
        """
        T, D = trajectory.shape
        assert D == 3, "Trajectory must be 3-dimensional (Z, Y, X)"
        
        # Center the trajectory
        mean_vector = np.mean(trajectory, axis=0)
        centered_traj = trajectory - mean_vector
        
        # Perform SVD
        # U: T x 3, S: 3, Vh: 3 x 3
        U, S, Vh = np.linalg.svd(centered_traj, full_matrices=False)
        
        # Truncate to Rank
        r = min(self.rank, D)
        U_r = U[:, :r]
        S_r = S[:r]
        V_r = Vh[:r, :].T  # columns are right singular vectors (D x r)
        
        # Scale U and V by singular values
        sqrt_S = np.sqrt(S_r)
        U_scaled = U_r * sqrt_S
        V_scaled = V_r * sqrt_S
        
        # Compress U_scaled using DCT (since temporal trajectories are smooth)
        U_dct_coefs = np.zeros((self.k_coef, r))
        for col in range(r):
            # Compute DCT
            col_dct = dct(U_scaled[:, col], norm='ortho')
            # Keep first k_coef low frequency coefficients
            k_eff = min(self.k_coef, T)
            U_dct_coefs[:k_eff, col] = col_dct[:k_eff]
            
        return {
            "mean": mean_vector,
            "U_dct_coefs": U_dct_coefs,
            "V_scaled": V_scaled,
            "original_shape": (T, D)
        }

    def decompress(self, compressed_dict):
        """
        Decompresses trajectory coefficients back to T x 3 spatial coordinates.
        """
        mean_vector = compressed_dict["mean"]
        U_dct_coefs = compressed_dict["U_dct_coefs"]
        V_scaled = compressed_dict["V_scaled"]
        T, D = compressed_dict["original_shape"]
        
        r = U_dct_coefs.shape[1]
        
        # Reconstruct U_scaled using IDCT
        U_recon = np.zeros((T, r))
        for col in range(r):
            # Pad truncated coefficients with zeros
            full_dct = np.zeros(T)
            k_eff = min(self.k_coef, T)
            full_dct[:k_eff] = U_dct_coefs[:k_eff, col]
            U_recon[:, col] = idct(full_dct, norm='ortho')
            
        # Reconstruct centered trajectory: U_recon * V_scaled.T
        centered_recon = np.dot(U_recon, V_scaled.T)
        
        # Restore mean offset
        return centered_recon + mean_vector

def test_compression():
    print("Testing SVD/DCT Trajectory Compressor...")
    # Simulate a smooth spiral cell trajectory (T = 50 time steps)
    T = 50
    t = np.linspace(0, 4 * np.pi, T)
    z = t * 1.5 + 5.0
    y = np.sin(t) * 10.0 + 100.0
    x = np.cos(t) * 10.0 + 100.0
    trajectory = np.stack([z, y, x], axis=1) # T x 3
    
    # Initialize compressor with rank 3 (retaining all 3 principal axes of motion) and k_coef 12
    compressor = TrajectoryCompressor(rank=3, k_coef=12)
    
    # Compress
    compressed = compressor.compress(trajectory)
    
    # Decompress
    recon = compressor.decompress(compressed)
    
    # Compute stats
    original_size = trajectory.nbytes
    # Stored floats: mean (3) + U_dct_coefs (k_coef * rank) + V_scaled (3 * rank)
    stored_floats = 3 + (compressor.k_coef * compressor.rank) + (3 * compressor.rank)
    compressed_size = stored_floats * 8 # float64 size
    
    mse = np.mean((trajectory - recon) ** 2)
    cosine_sim = np.dot(trajectory.flatten(), recon.flatten()) / (np.linalg.norm(trajectory) * np.linalg.norm(recon) + 1e-9)
    
    print(f"  - Original size: {original_size} bytes")
    print(f"  - Compressed parameters: {stored_floats} floats ({compressed_size} bytes)")
    print(f"  - Compression Ratio: {original_size / compressed_size:.2f}x")
    print(f"  - Reconstruction MSE: {mse:.4f}")
    print(f"  - Cosine Fidelity: {cosine_sim * 100:.2f}%")
    
    assert mse < 1.0, "MSE reconstruction error is too high!"
    assert cosine_sim > 0.999, "Fidelity is too low!"
    print("  - Trajectory SVD/DCT spectral projection: PASSED")

if __name__ == "__main__":
    test_compression()
