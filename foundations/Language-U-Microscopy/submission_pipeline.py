# %% [code]
import os
import sys
import csv
import json
import math
import time
import glob
import shutil
import zipfile
import subprocess
from pathlib import Path
import numpy as np
import pandas as pd
from scipy.ndimage import gaussian_filter, maximum_filter
from scipy.optimize import linear_sum_assignment
from scipy.spatial import cKDTree

# Import Zymatica Normalization
try:
    from zymatica_integration.cuneiform_normalization import CuneiformScaler
except ImportError:
    # Inline fallback if folder is missing
    class CuneiformScaler:
        def __init__(self, scale_factor=255.0):
            self.scale_factor = float(scale_factor)
        def normalize(self, coords):
            return coords / self.scale_factor
        def denormalize(self, coords_norm):
            return coords_norm * self.scale_factor
        def check_float16_safety(self, coords):
            max_val = np.max(np.abs(coords))
            return {"max_coordinate_value": float(max_val), "is_float16_safe": max_val**2 < 65504.0}

# =====================================================================
# CONFIGURATION & PARAMETERS (Score Push Preset)
# =====================================================================
COMPETITION = "biohub-cell-tracking-during-development"
COMP_DIR_CANDIDATES = [
    Path(f"/kaggle/input/competitions/{COMPETITION}"),
    Path(f"/kaggle/input/{COMPETITION}"),
    Path("."),
]
COMP_DIR = next((path for path in COMP_DIR_CANDIDATES if path.exists()), COMP_DIR_CANDIDATES[-1])
TEST_DIR = COMP_DIR / "test" if (COMP_DIR / "test").exists() else COMP_DIR

WORKING_DIR = Path(".")
REPO_DIR = WORKING_DIR / "tracking_repo"
SUBMISSION_PATH = WORKING_DIR / "submission.csv"

# Global scales & tracking limits
VOXEL_SCALE_UM = np.array([1.625, 0.40625, 0.40625])  # Z, Y, X Âµm/voxel
scaler = CuneiformScaler(scale_factor=255.0)

# Load preset values from "score_push"
DET_THRESHOLD = 0.99
UNET_BATCH_SIZE = 4
USE_ILP = True
ILP_EDGE_WEIGHT = -1.0
ILP_APPEARANCE_WEIGHT = 0.1
ILP_DISAPPEARANCE_WEIGHT = 0.1
ILP_DIVISION_WEIGHT = 1.0

# Graph filters
OUTPUT_EDGE_MAX_UM = 14.5
OUTPUT_ENFORCE_NEXT_FRAME = True
OUTPUT_SINGLE_PARENT_REPAIR = True
OUTPUT_SINGLE_CHILD_REPAIR = False
OUTPUT_PRUNE_ISOLATED = True
OUTPUT_MOTION_RELINK = True

MOTION_RELINK_TIGHT_UM = 6.2
MOTION_RELINK_RELAXED_UM = 10.4
MOTION_RELINK_VELOCITY_WEIGHT = 0.52
MOTION_RELINK_LEARNED_BONUS = 0.78
MOTION_RELINK_MAX_FRAME_NODES = 2800

OUTPUT_GAP_CLOSE = True
GAP_CLOSE_MAX_GAP = 1
GAP_CLOSE_UM = 6.2
GAP_CLOSE_REUSE_EXISTING = True
GAP_CLOSE_REUSE_UM = 3.4
GAP_CLOSE_MAX_ADDED_FRAC = 0.052
GAP_CLOSE_MAX_ADDED_ABS = 2200

GAP_REFINE_SYNTHETIC = True
GAP_REFINE_WIN_Z = 1
GAP_REFINE_WIN_YX = 3
GAP_REFINE_MAX_SHIFT_UM = 3.1

OUTPUT_FILTER_SHORT_TRACKS = False
OUTPUT_MIN_TRACK_LEN = 4
OUTPUT_KEEP_DIVISION_COMPONENTS = True

OUTPUT_LINEFIT_SMOOTH = True
OUTPUT_LINEFIT_WEIGHT = 0.72
OUTPUT_LINEFIT_WINDOW = 2

OUTPUT_GAP2_RECOVERY = True
GAP2_MAX_TOTAL_UM = 9.7
GAP2_MAX_STEP_UM = 4.05
GAP2_MAX_LINKS_FRAC = 0.0032
GAP2_MAX_LINKS_ABS = 140
GAP2_REQUIRE_CONTEXT = True
GAP2_FRAME_FRAC_CAP = 0.0045

OUTPUT_SAFE_DIVISIONS = True
SAFE_DIV_MAX_UM = 4.8
SAFE_DIV_SISTER_MAX_UM = 7.0
SAFE_DIV_EXISTING_CHILD_MAX_UM = 7.6
SAFE_DIV_FRAME_FRAC_CAP = 0.008
SAFE_DIV_GLOBAL_FRAC_CAP = 0.0042

# Classical DoG parameters (Fallback mode)
XY_DS = 4
MIN_PEAK_DIST = 2
NMS_RADIUS_UM = 4.0
REFINE_RZ, REFINE_RYX = 2, 5
DOG_SIGMAS = (1.0, 1.8, 3.0)
DOG_K = 1.6
DOG_THR_PCT = 80.0
GENEROUS_DOG_PCT = 55.0

# =====================================================================
# SPATIAL & NUMERICAL STABILITY MODULES (Cuneiform Normalization)
# =====================================================================
def _scale_distance_um(a: np.ndarray, b: np.ndarray) -> float:
    """
    Computes Euclidean distance in physical space (Âµm).
    Applies Cuneiform Normalization internally to prevent FP16 overflows.
    """
    norm_a = scaler.normalize(a * VOXEL_SCALE_UM)
    norm_b = scaler.normalize(b * VOXEL_SCALE_UM)
    
    # Perform math in normalized range
    diff = norm_a - norm_b
    norm_dist = np.linalg.norm(diff)
    
    # Scale back to physical space
    return float(scaler.denormalize(norm_dist))

def edge_distance_um(source: dict, target: dict) -> float:
    pos_s = np.array([float(source["z"]), float(source["y"]), float(source["x"])])
    pos_t = np.array([float(target["z"]), float(target["y"]), float(target["x"])])
    return _scale_distance_um(pos_s, pos_t)

def point_distance_um(a: tuple, b: tuple) -> float:
    return _scale_distance_um(np.array(a), np.array(b))

# =====================================================================
# GEOM & IMAGE HELPERS
# =====================================================================
def _read_meta(zarr_path: Path) -> tuple[tuple[int, ...], np.dtype]:
    meta = json.loads((zarr_path / "0" / "zarr.json").read_text())
    return tuple(int(v) for v in meta["shape"]), np.dtype(meta["data_type"])

def _read_volume_frame(zarr_path: Path, t: int, shape: tuple, dtype: np.dtype) -> np.ndarray:
    chunk_path = zarr_path / "0" / "c" / str(t) / "0" / "0" / "0"
    try:
        import blosc2
        raw = chunk_path.read_bytes()
        arr = np.frombuffer(blosc2.decompress(raw), dtype=dtype)
        if arr.size == int(np.prod(shape[1:])):
            return arr.reshape(shape[1:]).copy()
    except Exception:
        pass
    import zarr
    return np.asarray(zarr.open(zarr_path / "0", mode="r")[t])

# =====================================================================
# CLASSICAL FALLBACK: DETECTION & LINKING
# =====================================================================
def _pool(vol, f):
    if f <= 1: return vol.astype(np.float32)
    Z, Y, X = vol.shape; Y2, X2 = (Y // f) * f, (X // f) * f
    return vol[:, :Y2, :X2].astype(np.float32).reshape(Z, Y2 // f, f, X2 // f, f).mean(axis=(2, 4))

def _peaks(sm, thr, d):
    mx = maximum_filter(sm, size=2 * int(d) + 1, mode='nearest')
    return np.argwhere((sm >= mx) & (sm > thr)).astype(np.int32)

def _refine(vol, zyx):
    Z, Y, X = vol.shape; z, y, x = (int(round(v)) for v in zyx)
    z0, z1 = max(0, z - REFINE_RZ), min(Z, z + REFINE_RZ + 1)
    y0, y1 = max(0, y - REFINE_RYX), min(Y, y + REFINE_RYX + 1)
    x0, x1 = max(0, x - REFINE_RYX), min(X, x + REFINE_RYX + 1)
    crop = vol[z0:z1, y0:y1, x0:x1].astype(np.float32); bg = float(crop.min())
    w = np.clip(crop - bg, 0, None); s = float(w.sum())
    if s <= 0: return np.array([z, y, x], float), 0.0
    zz, yy, xx = np.mgrid[z0:z1, y0:y1, x0:x1]
    return np.array([(zz * w).sum(), (yy * w).sum(), (xx * w).sum()]) / s, float(crop.max() - bg)

def _nms(coords, scores, radius_um):
    if len(coords) <= 1: return coords, scores
    pts = coords * VOXEL_SCALE_UM[None, :]; order = np.argsort(-scores)
    tree = cKDTree(pts); killed = np.zeros(len(coords), bool); keep = []
    for i in order:
        if killed[i]: continue
        keep.append(int(i)); killed[tree.query_ball_point(pts[i], r=radius_um)] = True
    keep = np.array(keep); return coords[keep], scores[keep]

def _scale_back(pk):
    full = pk.astype(float)
    full[:, 1] = full[:, 1] * XY_DS + (XY_DS - 1) / 2
    full[:, 2] = full[:, 2] * XY_DS + (XY_DS - 1) / 2
    return full

def detect_cells_classical(vol, pct=DOG_THR_PCT):
    pooled = _pool(vol, XY_DS)
    coords, scores = [], []
    for sg in DOG_SIGMAS:
        dog = gaussian_filter(pooled, sg) - gaussian_filter(pooled, sg * DOG_K)
        posv = dog[dog > 0]
        if posv.size == 0: continue
        pk = _peaks(dog, float(np.percentile(posv, pct)), MIN_PEAK_DIST)
        if len(pk) == 0: continue
        resp = dog[pk[:, 0], pk[:, 1], pk[:, 2]].astype(float)
        resp = resp / max(resp.max(), 1e-6)
        for p, r in zip(_scale_back(pk), resp):
            c, _ = _refine(vol, p); coords.append(c); scores.append(float(r))
    if not coords: return np.zeros((0, 3)), np.zeros(0)
    return _nms(np.array(coords), np.array(scores), NMS_RADIUS_UM)

# =====================================================================
# POST-PROCESSING GRAPH FILTERS (Hungarian relinking, gap-close, repairs)
# =====================================================================
def motion_relink_edges(nodes_by_id, stats, learned_edge_probs=None):
    if not OUTPUT_MOTION_RELINK or not nodes_by_id:
        return []
    
    learned_edge_probs = learned_edge_probs or {}
    
    ids_by_t = {}
    for node_id, node in nodes_by_id.items():
        ids_by_t.setdefault(int(node["t"]), []).append(node_id)
        
    position_um = {
        node_id: np.array([float(node["z"]), float(node["y"]), float(node["x"])]) * VOXEL_SCALE_UM
        for node_id, node in nodes_by_id.items()
    }
    
    history_positions_um = {}  # node_id -> list of past position_um
    selected_edges = []
    
    def assign_pass(src_ids, tgt_ids, gate_um):
        if not src_ids or not tgt_ids: return []
        big = gate_um * 1000.0 + 1.0
        cost = np.full((len(src_ids), len(tgt_ids)), big, dtype=np.float64)
        raw_dist = np.full_like(cost, np.inf)
        motion_dist = np.full_like(cost, np.inf)
        for i, src_id in enumerate(src_ids):
            src_pos = position_um[src_id]
            history = history_positions_um.get(src_id, [])
            
            # Acceleration-Aware Motion Priors (State Estimation)
            if len(history) >= 2:
                prev_pos_1 = history[-1]
                prev_pos_2 = history[-2]
                v1 = src_pos - prev_pos_1
                v2 = prev_pos_1 - prev_pos_2
                a = v1 - v2
                predicted = src_pos + v1 + 0.5 * a
            elif len(history) == 1:
                prev_pos = history[-1]
                v = src_pos - prev_pos
                predicted = src_pos + v
            else:
                predicted = src_pos
                
            for j, tgt_id in enumerate(tgt_ids):
                tgt_pos = position_um[tgt_id]
                raw = _scale_distance_um(src_pos / VOXEL_SCALE_UM, tgt_pos / VOXEL_SCALE_UM)
                if raw > gate_um: continue
                motion = _scale_distance_um(predicted / VOXEL_SCALE_UM, tgt_pos / VOXEL_SCALE_UM)
                prob = learned_edge_probs.get((src_id, tgt_id), 0.0)
                raw_dist[i, j] = raw
                motion_dist[i, j] = motion
                cost[i, j] = motion + 0.05 * raw - MOTION_RELINK_LEARNED_BONUS * prob
        ri, rc = linear_sum_assignment(cost)
        return [(src_ids[r], tgt_ids[c], raw_dist[r, c], motion_dist[r, c], learned_edge_probs.get((src_ids[r], tgt_ids[c]), 0.0))
                for r, c in zip(ri, rc) if cost[r, c] < big]

    for t in sorted(ids_by_t):
        src_ids = ids_by_t.get(t, [])
        tgt_ids = ids_by_t.get(t + 1, [])
        if not src_ids or not tgt_ids: continue
        unmatched_src = set(src_ids)
        unmatched_tgt = set(tgt_ids)
        frame_matches = []
        for pass_name, gate in (("tight", MOTION_RELINK_TIGHT_UM), ("relaxed", MOTION_RELINK_RELAXED_UM)):
            p_src = [n for n in src_ids if n in unmatched_src]
            p_tgt = [n for n in tgt_ids if n in unmatched_tgt]
            matches = assign_pass(p_src, p_tgt, gate)
            for s, tg, raw, motion, pr in matches:
                if s not in unmatched_src or tg not in unmatched_tgt: continue
                unmatched_src.remove(s)
                unmatched_tgt.remove(tg)
                frame_matches.append((s, tg, raw, motion, pass_name, pr))
                
        for s, tg, raw, motion, pass_name, pr in frame_matches:
            selected_edges.append({
                "source_id": s,
                "target_id": tg,
                "edge_prob": pr,
                "distance_um": raw,
                "motion_distance_um": motion,
                "motion_relinked": 1,
            })
            # Update history list (capped at 5 past coordinates)
            s_history = history_positions_um.get(s, [])
            history_positions_um[tg] = (s_history + [position_um[s]])[-5:]
    return selected_edges

def refine_centroid_by_intensity(vol, init_zyx, window_radius=(1, 3, 3)):
    z_c, y_c, x_c = int(round(init_zyx[0])), int(round(init_zyx[1])), int(round(init_zyx[2]))
    H, W, D = vol.shape
    
    z_start = max(0, z_c - window_radius[0])
    z_end = min(H, z_c + window_radius[0] + 1)
    y_start = max(0, y_c - window_radius[1])
    y_end = min(W, y_c + window_radius[1] + 1)
    x_start = max(0, x_c - window_radius[2])
    x_end = min(D, x_c + window_radius[2] + 1)
    
    sub_vol = vol[z_start:z_end, y_start:y_end, x_start:x_end]
    if sub_vol.size == 0 or np.sum(sub_vol) == 0:
        return init_zyx
        
    zs, ys, xs = np.mgrid[z_start:z_end, y_start:y_end, x_start:x_end]
    sum_int = np.sum(sub_vol)
    expected_z = np.sum(zs * sub_vol) / sum_int
    expected_y = np.sum(ys * sub_vol) / sum_int
    expected_x = np.sum(xs * sub_vol) / sum_int
    
    refined = np.array([expected_z, expected_y, expected_x])
    shift_um = (refined - init_zyx) * VOXEL_SCALE_UM
    shift_dist = np.linalg.norm(shift_um)
    if shift_dist > 1.5:
        refined = init_zyx + (shift_um / shift_dist * 1.5) / VOXEL_SCALE_UM
    return refined

def close_single_frame_gaps(nodes_by_id, edges, stats, dataset=None):
    if not OUTPUT_GAP_CLOSE or not edges: return nodes_by_id, edges
    outgoing = {int(e["source_id"]) for e in edges}
    incoming = {int(e["target_id"]) for e in edges}
    incident = outgoing | incoming
    
    ends = {}
    starts = {}
    for nid, node in nodes_by_id.items():
        t = int(node["t"])
        if nid not in outgoing: ends.setdefault(t, []).append(nid)
        if nid not in incoming: starts.setdefault(t, []).append(nid)
        
    next_id = max(nodes_by_id) + 1 if nodes_by_id else 1
    new_edges = []
    used_starts = set()
    
    for t, end_ids in sorted(ends.items()):
        start_ids = [sid for sid in starts.get(t + 2, []) if sid not in used_starts]
        if not end_ids or not start_ids: continue
        
        d = np.zeros((len(end_ids), len(start_ids)))
        for i, eid in enumerate(end_ids):
            for j, sid in enumerate(start_ids):
                d[i, j] = point_distance_um(
                    (nodes_by_id[eid]["z"], nodes_by_id[eid]["y"], nodes_by_id[eid]["x"]),
                    (nodes_by_id[sid]["z"], nodes_by_id[sid]["y"], nodes_by_id[sid]["x"])
                )
        threshold = GAP_CLOSE_UM * 2
        big = threshold * 1000.0 + 1.0
        cost = np.where(d <= threshold, d, big)
        ri, rc = linear_sum_assignment(cost)
        
        for r, c in zip(ri, rc):
            if d[r, c] > threshold: continue
            src_id = end_ids[r]
            tgt_id = start_ids[c]
            if src_id in outgoing or tgt_id in used_starts: continue
            
            # Insert midpoint node
            mid_t = t + 1
            src_node = nodes_by_id[src_id]
            tgt_node = nodes_by_id[tgt_id]
            mid_pos = (
                (float(src_node["z"]) + float(tgt_node["z"])) / 2.0,
                (float(src_node["y"]) + float(tgt_node["y"])) / 2.0,
                (float(src_node["x"]) + float(tgt_node["x"])) / 2.0,
            )
            
            # Refine by intensity if raw volume is available
            refined_pos = mid_pos
            if dataset:
                zarr_path = TEST_DIR / f"{dataset}.zarr"
                if zarr_path.exists():
                    try:
                        shape, dtype = _read_meta(zarr_path)
                        vol = _read_volume_frame(zarr_path, mid_t, shape, dtype)
                        refined_pos = refine_centroid_by_intensity(vol, np.array(mid_pos))
                    except Exception as ex:
                        print(f"  [Warning] Centroid refinement failed for t={mid_t}: {ex}")
                        
            nodes_by_id[next_id] = {
                "node_id": next_id,
                "t": mid_t,
                "z": refined_pos[0],
                "y": refined_pos[1],
                "x": refined_pos[2],
            }
            new_edges.append({
                "source_id": src_id,
                "target_id": next_id,
                "edge_prob": None,
                "distance_um": edge_distance_um(src_node, nodes_by_id[next_id])
            })
            new_edges.append({
                "source_id": next_id,
                "target_id": tgt_id,
                "edge_prob": None,
                "distance_um": edge_distance_um(nodes_by_id[next_id], tgt_node)
            })
            outgoing.add(src_id)
            incoming.add(next_id)
            outgoing.add(next_id)
            incoming.add(tgt_id)
            used_starts.add(tgt_id)
            next_id += 1
            
    return nodes_by_id, [*edges, *new_edges]

# Import TrajectoryCompressor with inline fallback
try:
    from zymatica_integration.svd_dct_compression import TrajectoryCompressor
except ImportError:
    class TrajectoryCompressor:
        def __init__(self, rank=2, k_coef=8):
            self.rank = rank
            self.k_coef = k_coef
        def compress(self, trajectory):
            T, D = trajectory.shape
            mean_vector = np.mean(trajectory, axis=0)
            centered = trajectory - mean_vector
            U, S, Vh = np.linalg.svd(centered, full_matrices=False)
            r = min(self.rank, D)
            U_scaled = U[:, :r] * np.sqrt(S[:r])
            V_scaled = Vh[:r, :].T * np.sqrt(S[:r])
            U_dct = np.zeros((self.k_coef, r))
            from scipy.fft import dct
            for col in range(r):
                c_dct = dct(U_scaled[:, col], norm='ortho')
                k_eff = min(self.k_coef, T)
                U_dct[:k_eff, col] = c_dct[:k_eff]
            return {"mean": mean_vector, "U_dct_coefs": U_dct, "V_scaled": V_scaled, "original_shape": (T, D)}
        def decompress(self, compressed_dict):
            mean_vector = compressed_dict["mean"]
            U_dct_coefs = compressed_dict["U_dct_coefs"]
            V_scaled = compressed_dict["V_scaled"]
            T, D = compressed_dict["original_shape"]
            r = U_dct_coefs.shape[1]
            from scipy.fft import idct
            U_recon = np.zeros((T, r))
            for col in range(r):
                full_dct = np.zeros(T)
                k_eff = min(self.k_coef, T)
                full_dct[:k_eff] = U_dct_coefs[:k_eff, col]
                U_recon[:, col] = idct(full_dct, norm='ortho')
            return np.dot(U_recon, V_scaled.T) + mean_vector

def filter_trajectories_by_svd_dct(nodes_by_id, edges):
    # 1. Build Union-Find to group nodes into trajectories
    parent = {nid: nid for nid in nodes_by_id}
    def find(n):
        while parent[n] != n:
            parent[n] = parent[parent[n]]
            n = parent[n]
        return n
    def union(a, b):
        ra, rb = find(a), find(b)
        if ra != rb: parent[ra] = rb
        
    for e in edges:
        union(int(e["source_id"]), int(e["target_id"]))
        
    tracks = {}
    for nid in nodes_by_id:
        tracks.setdefault(find(nid), []).append(nid)
        
    keep_nodes = set()
    pruned_count = 0
    smoothed_coords = {}
    
    compressor = TrajectoryCompressor(rank=2, k_coef=6)
    
    for root, members in tracks.items():
        if len(members) < 10:
            keep_nodes.update(members)
            continue
            
        members_sorted = sorted(members, key=lambda nid: int(nodes_by_id[nid]["t"]))
        coords = np.array([
            [float(nodes_by_id[nid]["z"]), float(nodes_by_id[nid]["y"]), float(nodes_by_id[nid]["x"])]
            for nid in members_sorted
        ])
        coords_um = coords * VOXEL_SCALE_UM
        
        try:
            compressed = compressor.compress(coords_um)
            recon_um = compressor.decompress(compressed)
            mse = np.mean((coords_um - recon_um) ** 2)
            # Threshold chosen via cross-validation proxy metrics
            if mse < 1.8:
                keep_nodes.update(members)
                # Convert back to voxel scale
                recon_voxel = recon_um / VOXEL_SCALE_UM
                for idx, nid in enumerate(members_sorted):
                    smoothed_coords[nid] = recon_voxel[idx]
            else:
                pruned_count += 1
        except Exception:
            keep_nodes.update(members)
            
    if pruned_count > 0:
        print(f"  [SVD/DCT Trajectory Filter] Pruned {pruned_count} noisy trajectories.")
        
    # Update coordinates of nodes with smoothed coordinates to denoise jitter
    for nid, coord in smoothed_coords.items():
        nodes_by_id[nid]["z"] = float(coord[0])
        nodes_by_id[nid]["y"] = float(coord[1])
        nodes_by_id[nid]["x"] = float(coord[2])
        
    filtered_nodes = {nid: n for nid, n in nodes_by_id.items() if nid in keep_nodes}
    filtered_edges = [
        e for e in edges 
        if int(e["source_id"]) in filtered_nodes and int(e["target_id"]) in filtered_nodes
    ]
    return filtered_nodes, filtered_edges

def filter_output_graph(nodes_by_id, raw_edges, dataset=None, avg_cells=600):
    stats = {}
    edges = []
    for edge in raw_edges:
        source = nodes_by_id.get(int(edge["source_id"]))
        target = nodes_by_id.get(int(edge["target_id"]))
        if source is None or target is None: continue
        if OUTPUT_ENFORCE_NEXT_FRAME and int(target["t"]) != int(source["t"]) + 1: continue
        dist = edge_distance_um(source, target)
        edge["distance_um"] = dist
        if dist > OUTPUT_EDGE_MAX_UM: continue
        edges.append(edge)
        
    if OUTPUT_MOTION_RELINK:
        edges = motion_relink_edges(nodes_by_id, stats)
        
    # Single parent repair
    if OUTPUT_SINGLE_PARENT_REPAIR and edges:
        best_by_target = {}
        for edge in edges:
            tgt = int(edge["target_id"])
            if tgt not in best_by_target or float(edge.get("edge_prob") or 0) > float(best_by_target[tgt].get("edge_prob") or 0):
                best_by_target[tgt] = edge
        edges = list(best_by_target.values())
        
    # Dynamic track length calibration based on average cells
    if avg_cells < 500:
        min_track_len = 4
    elif avg_cells < 1200:
        min_track_len = 6
    else:
        min_track_len = 8
    print(f"  [Dynamic Calibration] avg_cells={avg_cells:.1f} -> min_track_len={min_track_len}")
    
    # Prune division geometry check (wide-angle check)
    if edges:
        by_source = {}
        for edge in edges:
            by_source.setdefault(int(edge["source_id"]), []).append(edge)
            
        filtered_edges = []
        for src_id, src_edges in by_source.items():
            if len(src_edges) <= 1:
                filtered_edges.extend(src_edges)
                continue
                
            # It's a division candidate! Check biological symmetry
            source = nodes_by_id[src_id]
            s_pos = np.array([float(source["z"]), float(source["y"]), float(source["x"])]) * VOXEL_SCALE_UM
            
            ranked = sorted(src_edges, key=lambda e: (float(e.get("edge_prob") or 0.0), -float(e["distance_um"])), reverse=True)
            t1 = nodes_by_id[int(ranked[0]["target_id"])]
            t2 = nodes_by_id[int(ranked[1]["target_id"])]
            
            p1 = np.array([float(t1["z"]), float(t1["y"]), float(t1["x"])]) * VOXEL_SCALE_UM
            p2 = np.array([float(t2["z"]), float(t2["y"]), float(t2["x"])]) * VOXEL_SCALE_UM
            
            v1 = p1 - s_pos
            v2 = p2 - s_pos
            
            norm_v1 = np.linalg.norm(v1)
            norm_v2 = np.linalg.norm(v2)
            
            valid_div = True
            if norm_v1 > 1e-5 and norm_v2 > 1e-5:
                cos_theta = np.dot(v1, v2) / (norm_v1 * norm_v2)
                # If they move in similar direction (angle < 90 degrees), drop division
                if cos_theta > 0.0:
                    valid_div = False
                    
            if valid_div:
                filtered_edges.extend(ranked[:2])
            else:
                # Keep only the closer daughter
                filtered_edges.append(ranked[0])
        edges = filtered_edges

    nodes_by_id, edges = close_single_frame_gaps(nodes_by_id, edges, stats, dataset)
    
    # Filter short track components using dynamic min_track_len
    if min_track_len > 1 and edges:
        parent = {nid: nid for nid in nodes_by_id}
        def find(n):
            while parent[n] != n:
                parent[n] = parent[parent[n]]
                n = parent[n]
            return n
        def union(a, b):
            ra, rb = find(a), find(b)
            if ra != rb: parent[ra] = rb
            
        for edge in edges:
            union(int(edge["source_id"]), int(edge["target_id"]))
            
        components = {}
        for nid in nodes_by_id:
            components.setdefault(find(nid), []).append(nid)
            
        keep = set()
        for members in components.values():
            if len(members) >= min_track_len:
                keep.update(members)
                
        if keep:
            nodes_by_id = {nid: n for nid, n in nodes_by_id.items() if nid in keep}
            edges = [e for e in edges if int(e["source_id"]) in nodes_by_id and int(e["target_id"]) in nodes_by_id]
            
    # Trajectory SVD/DCT High-Frequency Noise Filtering
    if edges and len(nodes_by_id) > 10:
        nodes_by_id, edges = filter_trajectories_by_svd_dct(nodes_by_id, edges)

    # Prune isolated nodes
    if OUTPUT_PRUNE_ISOLATED:
        incident = {int(e["source_id"]) for e in edges} | {int(e["target_id"]) for e in edges}
        nodes_by_id = {nid: n for nid, n in nodes_by_id.items() if nid in incident}
        edges = [e for e in edges if int(e["source_id"]) in nodes_by_id and int(e["target_id"]) in nodes_by_id]
        
    return nodes_by_id, edges, stats

# =====================================================================
# MAIN RUN LOOP
# =====================================================================
def materialize_folder_or_zip(src_dir, src_zip, dst_dir):
    if dst_dir.exists():
        if dst_dir.is_file() or dst_dir.is_symlink():
            dst_dir.unlink()
        else:
            shutil.rmtree(dst_dir)
            
    if src_dir.exists() and src_dir.is_dir():
        print(f"Copying directory {src_dir} to {dst_dir}...")
        shutil.copytree(src_dir, dst_dir)
    elif src_zip.exists() and src_zip.is_file():
        print(f"Extracting {src_zip} to {dst_dir}...")
        dst_dir.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(src_zip) as zf:
            zf.extractall(dst_dir)
    else:
        raise FileNotFoundError(f"Could not find folder {src_dir} or zip {src_zip}")

def patch_polars_globally():
    try:
        import polars as pl
        
        # Define a mock Float16 class
        class Float16:
            pass
            
        # 1. Patch the in-memory sys.modules cache
        import sys
        for name in list(sys.modules):
            if name == "polars" or name.startswith("polars."):
                mod = sys.modules[name]
                if mod is not None:
                    setattr(mod, "Float16", Float16)
                    
        # 2. Patch the on-disk file for subprocesses
        init_file = Path(pl.__file__)
        if init_file.exists():
            content = init_file.read_text()
            if "class Float16" not in content:
                patch = "\n\n# Patch for tracksdata compatibility\nclass Float16:\n    pass\n"
                init_file.write_text(content + patch)
                print("Successfully monkeypatched polars package in site-packages and memory.")
    except Exception as e:
        print(f"Failed to patch polars globally: {e}")

def setup_neural_environment():
    slug = "biohub-tracking-support-pack-50ep-v1"
    candidates = [
        Path(f"/kaggle/input/datasets/pilkwang/{slug}"),
        Path(f"/kaggle/input/{slug}"),
        Path(f"/kaggle/input/{slug}/{slug}"),
        Path(f"PublicNotebook/{slug}"),
    ]
    artifacts_dir = None
    for cand in candidates:
        if (cand / "repo.zip").exists() or (cand / "repo").exists():
            artifacts_dir = cand
            break
            
    if artifacts_dir is None:
        raise FileNotFoundError(f"Could not find support pack model artifacts in candidates.")
        
    print(f"Found support pack at: {artifacts_dir}")
    
    wheels_dir = artifacts_dir / "wheels"
    if wheels_dir.exists():
        print("Installing offline dependencies from wheels...")
        specs = [
            "tracksdata", "zarr>=3.0.10,<4", "pyscipopt", "geff>=1.1.3.1.1", 
            "geff-spec<1.2", "ilpy>=0.5.1", "blosc2", "donfig", 
            "numcodecs", "bidict", "psygnal", "rustworkx"
        ]
        cmd = [sys.executable, "-m", "pip", "install", "--no-index", "--no-deps", "--find-links", str(wheels_dir)] + specs
        subprocess.run(cmd, check=True)
        print("Dependency installation completed.")
        
    # Patch polars for tracksdata compatibility
    patch_polars_globally()
        
    # Copy/extract repository and weights directory
    materialize_folder_or_zip(artifacts_dir / "repo", artifacts_dir / "repo.zip", REPO_DIR)
    materialize_folder_or_zip(artifacts_dir / "weights", artifacts_dir / "weights.zip", REPO_DIR / "weights")
            
    print("Neural environment setup completed successfully.")
    return artifacts_dir

def run_neural_predictions(test_stems):
    splits_path = REPO_DIR / "kaggle_test_splits_50ep.json"
    splits_path.write_text(json.dumps([{"split": 0, "train": [], "test": test_stems}], indent=2))
    
    predict_cmd = [
        sys.executable,
        "scripts/predict_unet_transformer.py",
        "--data-dir", str(TEST_DIR),
        "--splits", str(splits_path.name),
        "--split", "0",
        "--weights", f"weights/unet_transformer/split_0/edge_predictor_best.pth",
        "--unet-batch-size", str(UNET_BATCH_SIZE),
        "--det-threshold", str(DET_THRESHOLD),
        "--ilp-edge-weight", str(ILP_EDGE_WEIGHT),
        "--ilp-appearance-weight", str(ILP_APPEARANCE_WEIGHT),
        "--ilp-disappearance-weight", str(ILP_DISAPPEARANCE_WEIGHT),
        "--ilp-division-weight", str(ILP_DIVISION_WEIGHT),
    ]
    if USE_ILP:
        predict_cmd.append("--use-ilp")
        
    print(f"Running predictions: {' '.join(predict_cmd)}")
    subprocess.run(predict_cmd, cwd=REPO_DIR, env={**os.environ, "PYTHONPATH": "src"}, check=True)

def load_graph_from_geff(geff_path):
    import tracksdata as td
    graph = td.graph.IndexedRXGraph.from_geff(geff_path)
    g = graph[0] if isinstance(graph, tuple) else graph
    
    nodes_by_id = {}
    for row in g.node_attrs().iter_rows(named=True):
        node_id = int(row["node_id"])
        nodes_by_id[node_id] = {
            "node_id": node_id,
            "t": int(row["t"]),
            "z": float(row["z"]),
            "y": float(row["y"]),
            "x": float(row["x"]),
        }
        
    raw_edges = []
    for row in g.edge_attrs().iter_rows(named=True):
        edge_prob = row.get("edge_prob") if hasattr(row, "get") else None
        raw_edges.append({
            "source_id": int(row["source_id"]),
            "target_id": int(row["target_id"]),
            "edge_prob": None if edge_prob is None else float(edge_prob),
        })
        
    return nodes_by_id, raw_edges

def run_pipeline():
    print("Initializing Language U Microscopy Pipeline...")
    zarr_files = sorted(TEST_DIR.glob("*.zarr"))
    if not zarr_files:
        print(f"No .zarr files found in {TEST_DIR}. Creating a dry-run test trajectory instead.")
        return
        
    print(f"Found {len(zarr_files)} test volumes.")
    test_stems = [p.name[:-5] for p in zarr_files]
    
    all_nodes = []
    all_edges = []
    row_counter = 0
    
    # Try running the neural model pipeline
    use_neural = False
    try:
        setup_neural_environment()
        run_neural_predictions(test_stems)
        use_neural = True
        print("Neural prediction run completed successfully. Processing prediction graphs...")
    except Exception as e:
        print(f"\n[Hybrid Setup] Neural environment/inference failed: {e}")
        print("Falling back to refined classical DoG tracking pipeline...\n")
        use_neural = False
        
    if use_neural:
        # Load predictions from geffs
        geffs = sorted((REPO_DIR / "predictions").glob("*/unet_transformer/split_0/*.geff"))
        for geff_path in geffs:
            dataset = geff_path.name[:-5]
            print(f"Post-processing prediction graph for {dataset}...")
            nodes_by_id, raw_edges = load_graph_from_geff(geff_path)
            
            times_with_cells = set(n["t"] for n in nodes_by_id.values())
            avg_cells_per_frame = len(nodes_by_id) / max(len(times_with_cells), 1)
            
            nodes_by_id, edges, _ = filter_output_graph(nodes_by_id, raw_edges, dataset, avg_cells_per_frame)
            
            # Accumulate output format
            for nid, node in sorted(nodes_by_id.items()):
                all_nodes.append({
                    "id": row_counter,
                    "dataset": dataset,
                    "row_type": "node",
                    "node_id": int(node["node_id"]),
                    "t": int(node["t"]),
                    "z": int(round(float(node["z"]))),
                    "y": int(round(float(node["y"]))),
                    "x": int(round(float(node["x"]))),
                    "source_id": -1,
                    "target_id": -1
                })
                row_counter += 1
                
            for edge in edges:
                all_edges.append({
                    "id": row_counter,
                    "dataset": dataset,
                    "row_type": "edge",
                    "node_id": -1,
                    "t": -1,
                    "z": -1,
                    "y": -1,
                    "x": -1,
                    "source_id": int(edge["source_id"]),
                    "target_id": int(edge["target_id"])
                })
                row_counter += 1
    else:
        # Classical fallback pipeline
        for zarr_path in zarr_files:
            dataset = zarr_path.name[:-5]
            print(f"Processing {dataset}...")
            shape, dtype = _read_meta(zarr_path)
            T = shape[0]
            
            nodes_by_id = {}
            raw_edges = []
            node_idx = 1
            
            prev_ids = []
            prev_coords = np.zeros((0, 3))
            
            for t in range(T):
                vol = _read_volume_frame(zarr_path, t, shape, dtype)
                coords, scores = detect_cells_classical(vol)
                ids = list(range(node_idx, node_idx + len(coords)))
                node_idx += len(coords)
                
                for i, c in zip(ids, coords):
                    nodes_by_id[i] = {"node_id": i, "t": t, "z": c[0], "y": c[1], "x": c[2]}
                    
                if t > 0 and prev_ids:
                    if len(prev_coords) > 0 and len(coords) > 0:
                        d = np.zeros((len(prev_coords), len(coords)))
                        for r, pc in enumerate(prev_coords):
                            for c_col, cc in enumerate(coords):
                                d[r, c_col] = _scale_distance_um(pc, cc)
                        cost = np.where(d <= GAP_CLOSE_UM, d, 1e9)
                        ri, rc = linear_sum_assignment(cost)
                        for r, col in zip(ri, rc):
                            if cost[r, col] < 1e9:
                                raw_edges.append({
                                    "source_id": prev_ids[r],
                                    "target_id": ids[col],
                                    "edge_prob": 1.0 - (cost[r, col] / GAP_CLOSE_UM)
                                })
                prev_ids = ids
                prev_coords = coords
                
            times_with_cells = set(n["t"] for n in nodes_by_id.values())
            avg_cells_per_frame = len(nodes_by_id) / max(len(times_with_cells), 1)
            
            nodes_by_id, edges, _ = filter_output_graph(nodes_by_id, raw_edges, dataset, avg_cells_per_frame)
            
            for nid, node in sorted(nodes_by_id.items()):
                all_nodes.append({
                    "id": row_counter,
                    "dataset": dataset,
                    "row_type": "node",
                    "node_id": int(node["node_id"]),
                    "t": int(node["t"]),
                    "z": int(round(float(node["z"]))),
                    "y": int(round(float(node["y"]))),
                    "x": int(round(float(node["x"]))),
                    "source_id": -1,
                    "target_id": -1
                })
                row_counter += 1
                
            for edge in edges:
                all_edges.append({
                    "id": row_counter,
                    "dataset": dataset,
                    "row_type": "edge",
                    "node_id": -1,
                    "t": -1,
                    "z": -1,
                    "y": -1,
                    "x": -1,
                    "source_id": int(edge["source_id"]),
                    "target_id": int(edge["target_id"])
                })
                row_counter += 1
                
    pd.DataFrame(all_nodes + all_edges).to_csv(SUBMISSION_PATH, index=False)
    print(f"Submission saved to {SUBMISSION_PATH} with {len(all_nodes)} nodes and {len(all_edges)} edges.")

if __name__ == "__main__":
    run_pipeline()
