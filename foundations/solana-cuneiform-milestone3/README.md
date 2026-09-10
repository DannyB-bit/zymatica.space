# Zymatica: Solana Cuneiform Anchor — Milestone 3 🗺️

> **Solana Program ID:** `BJKrKzXX4YfEYMZaVT2dbuaNuq7aqN3Xmib27JLALs3M`  
> **Treasury Wallet:** `7kZ3XwggVosBMag5mAJt6JVM2uP86YLoBaY9rQXccKS`  
> **Status: PENDING IMPLEMENTATION** | **License: Conditional Open-Source Grant License**

![Logo](./Logo_Zymatica_Voice.png)

This repository contains the standalone codebase, mainnet rollout scripts, and gateway configurations for **Milestone 3** of the Solana Foundation USA Grant.

---

## 🎯 Milestone 3 Deliverables & Preparation
*   **Mainnet Rollout Script:** `deploy_mainnet.ts` configured to initialize the program state PDA on Solana Mainnet Beta.
*   **5-Node Physical Gateway Configs:** Concrete JSON configuration files mapping LoRa parameters, solar power constraints, locations (Mountainous State Park, Urban Downtown, Lake Ontario over-water boundary), and Solana Pay recipient addresses.
*   **Verification Workflow:** Automated GitHub Actions verification workflow (`verify.yml`) checking contract compilation and SDK compilation.

---

## 📂 Repository File Structure

*   `programs/solana-cuneiform-anchor/` — Rust smart contract source code.
*   `app/src/cuneiform_client.ts` — TypeScript client SDK.
*   `app/src/deploy_mainnet.ts` — Solana Mainnet Beta deployment and initialization script.
*   `app/src/solana_pay_mock.ts` — Solana Pay payment request and verification adapter.
*   `app/src/test_solana_pay_live.ts` — Live Solana Pay validation tests (Devnet).
*   `app/src/test_devnet.ts` — Devnet attestation registry tests.
*   `gateway_configs/` — JSON configurations for the 5-node physical gateway deployment.
*   `Logo_Zymatica_Voice.png` / `Logo.jpg` — Brand assets.

---

## 🚀 How to Verify & Deploy (Evaluator Instructions)

### 1. Execute Mainnet Rollout Dry-Run
```bash
# Verify mainnet deployer balance and PDA derivation
npx tsx app/src/deploy_mainnet.ts
```

### 2. Inspect Node Configurations
The configurations for the 5-node physical mesh testnet are located in `/gateway_configs/`:
*   `node_1_mainhub.json` (Computer lab mainhub gateway)
*   `node_2_statepark.json` (Solar-powered mountainous terrain relay)
*   `node_3_urban.json` (High-obstruction urban concrete relay)
*   `node_4_ontario.json` (Solar-powered over-water path relay on Lake Ontario)
*   `node_5_backup.json` (Roaming backup relay)

---
---

## 📜 License

**Conditional Open-Source Grant License**

This project is currently under a proprietary license during the grant evaluation period. It will automatically transition to the open-source **Apache License 2.0** upon the approval and funding of the Superteam USA / Solana Foundation grant.

Please see the [LICENSE](./LICENSE) file for the full terms and conditions.
