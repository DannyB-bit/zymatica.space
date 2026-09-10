import numpy as np
import torch

class CuneiformScaler:
    """
    Cuneiform-U Normalization Scalar for Numerical Stability.
    Scales 3D coordinates (Z, Y, X) to the range [0.0, 1.0] by dividing by 
    a normalization scalar (default 255.0) or specific spatial bounds.
    
    This ensures that distance calculations, coordinate losses, and motion
    gradients remain stable in half-precision (float16) environments.
    """
    def __init__(self, scale_factor=255.0):
        self.scale_factor = float(scale_factor)
        
    def normalize(self, coords):
        """
        Normalize coordinates by dividing by the scale factor.
        Supports numpy arrays and torch tensors.
        """
        if isinstance(coords, np.ndarray):
            return coords / self.scale_factor
        elif isinstance(coords, torch.Tensor):
            return coords / self.scale_factor
        else:
            raise TypeError("Unsupported coordinate type. Must be numpy.ndarray or torch.Tensor.")
            
    def denormalize(self, coords_norm):
        """
        Restore normalized coordinates to their original scale.
        """
        if isinstance(coords_norm, np.ndarray):
            return coords_norm * self.scale_factor
        elif isinstance(coords_norm, torch.Tensor):
            return coords_norm * self.scale_factor
        else:
            raise TypeError("Unsupported coordinate type. Must be numpy.ndarray or torch.Tensor.")

    def check_float16_safety(self, coords):
        """
        Verifies if squared coordinate distances could overflow standard IEEE 754 Float16 limits (65504).
        """
        max_val = np.max(np.abs(coords)) if isinstance(coords, np.ndarray) else torch.max(torch.abs(coords)).item()
        squared_limit = max_val ** 2
        is_safe = squared_limit < 65504.0
        return {
            "max_coordinate_value": float(max_val),
            "max_squared_value": float(squared_limit),
            "is_float16_safe": bool(is_safe)
        }

def test_normalization():
    print("Testing Cuneiform Normalization Scaler...")
    scaler = CuneiformScaler()
    
    # 1. Test scaling correctness
    coords = np.array([[100.5, 200.2, 50.8], [0.0, 255.0, 128.0]])
    coords_norm = scaler.normalize(coords)
    assert np.allclose(coords_norm, coords / 255.0)
    
    coords_recon = scaler.denormalize(coords_norm)
    assert np.allclose(coords_recon, coords)
    print("  - Scaling correctness: PASSED")
    
    # 2. Test Float16 safety check
    unstable_coords = np.array([300.0, 400.0, 500.0]) # 500^2 = 250000 -> overflows float16 sum if elements are squared and added
    safety = scaler.check_float16_safety(unstable_coords)
    print(f"  - Unstable coordinates max value: {safety['max_coordinate_value']}")
    print(f"  - Float16 Safe: {safety['is_float16_safe']} (Max squared value = {safety['max_squared_value']})")
    
    stable_coords = scaler.normalize(unstable_coords)
    safety_stable = scaler.check_float16_safety(stable_coords)
    print(f"  - Normalized coordinates max value: {safety_stable['max_coordinate_value']:.4f}")
    print(f"  - Float16 Safe: {safety_stable['is_float16_safe']} (Max squared value = {safety_stable['max_squared_value']:.4f})")
    
    assert safety_stable['is_float16_safe']
    print("  - Float16 range stability verification: PASSED")

if __name__ == "__main__":
    test_normalization()
