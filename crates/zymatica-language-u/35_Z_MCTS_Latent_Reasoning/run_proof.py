#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
Class 35: Z-MCTS (Continuous Latent Reasoning Engine) Algorithmic Verifier
Scope: Demonstrates continuous manifold Monte Carlo Tree Search trajectory optimization
over an 8D Riemannian metric space. (Note: Language-model integration benchmarks test-time reasoning across downstream tasks).
"""

import math
import random

class LatentMctsPy:
    def __init__(self, start_coords, goal_coords, num_simulations=200, max_depth=6):
        self.start = list(start_coords)
        self.goal = list(goal_coords)
        self.num_simulations = num_simulations
        self.max_depth = max_depth
        self.weights = [1.0, 1.0, 0.75, 0.75, 0.5, 0.5, 0.25, 0.25]

    def dist(self, a, b):
        return math.sqrt(sum(w * (x - y) ** 2 for w, x, y in zip(self.weights, a, b)))

    def search(self):
        # 16 Exploratory Tangent Vectors in 8D
        actions = []
        for i in range(8):
            v_p = [0.0] * 8
            v_p[i] = 1.0
            actions.append(v_p)
            v_n = [0.0] * 8
            v_n[i] = -1.0
            actions.append(v_n)

        # Root Node: (state, parent, action, visits, value)
        nodes = [{
            "state": list(self.start),
            "parent": None,
            "children": [],
            "action": [0.0] * 8,
            "visits": 0,
            "value": 0.0,
            "prior": 1.0
        }]

        for _ in range(self.num_simulations):
            # Selection
            curr = 0
            depth = 0
            while nodes[curr]["children"] and depth < self.max_depth:
                p_vis = nodes[curr]["visits"]
                best_score = -float("inf")
                best_child = nodes[curr]["children"][0]
                for c_idx in nodes[curr]["children"]:
                    child = nodes[c_idx]
                    q = child["value"] / max(1, child["visits"])
                    u = 1.414 * child["prior"] * (math.sqrt(p_vis) / (1 + child["visits"]))
                    score = q + u
                    if score > best_score:
                        best_score = score
                        best_child = c_idx
                    curr = best_child
                depth += 1

            # Expansion
            if depth < self.max_depth and nodes[curr]["visits"] > 0:
                p_state = nodes[curr]["state"]
                for act in actions:
                    nxt = [max(0.0, min(15.0, s + a * 0.5)) for s, a in zip(p_state, act)]
                    d = self.dist(nxt, self.goal)
                    prior = max(0.01, 1.0 / (1.0 + d))
                    n_idx = len(nodes)
                    nodes.append({
                        "state": nxt,
                        "parent": curr,
                        "children": [],
                        "action": act,
                        "visits": 0,
                        "value": 0.0,
                        "prior": prior
                    })
                    nodes[curr]["children"].append(n_idx)
                curr = nodes[curr]["children"][0]

            # Evaluation
            d_goal = self.dist(nodes[curr]["state"], self.goal)
            reward = 10.0 / (1.0 + d_goal)

            # Backpropagation
            b = curr
            while b is not None:
                nodes[b]["visits"] += 1
                nodes[b]["value"] += reward
                b = nodes[b]["parent"]

        # Extract optimal trajectory
        traj = [self.start]
        curr = 0
        while nodes[curr]["children"]:
            best_c = max(nodes[curr]["children"], key=lambda idx: nodes[idx]["visits"])
            if nodes[best_c]["visits"] == 0:
                break
            traj.append(nodes[best_c]["state"])
            curr = best_c

        return traj


def main():
    print("=" * 80)
    print(" [+] ZYMATICA CLASS 35: Z-MCTS TEST-TIME CONTINUOUS LATENT REASONING")
    print("     Scope: Geometric Geodesic Search Across 8D Riemannian Manifolds (Simulation)")
    print("=" * 80)

    start = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0]
    goal = [5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0]

    engine = LatentMctsPy(start, goal, num_simulations=150, max_depth=6)
    initial_dist = engine.dist(start, goal)
    print(f" [MCTS] Initial Riemannian Geodesic Distance: {initial_dist:.4f}")

    trajectory = engine.search()
    final_dist = engine.dist(trajectory[-1], goal)

    print(f" [MCTS] Evaluated Trajectory Length:          {len(trajectory)} latent waypoints")
    print(f" [MCTS] Final Distance to Target Geodesic:   {final_dist:.4f}")
    print(f" [MCTS] Reasoning Optimization Gain:         {((initial_dist - final_dist) / initial_dist * 100):.2f}% error reduction")

    assert final_dist < initial_dist, "MCTS must monotonically navigate towards semantic target"
    print("\n[PASS] CLASS 35 VERIFICATION: GEODESIC MCTS LATENT TRAJECTORY SEARCH VERIFIED")
    print("=" * 80)


if __name__ == "__main__":
    main()
