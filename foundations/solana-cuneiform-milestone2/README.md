# Zymatica: Solana Cuneiform Anchor — Milestone 2 ✅

> **Solana Devnet Program ID:** `BJKrKzXX4YfEYMZaVT2dbuaNuq7aqN3Xmib27JLALs3M`  
> **Treasury Wallet:** `7kZ3XwggVosBMag5mAJt6JVM2uP86YLoBaY9rQXccKS`  
> **Status: COMPLETE** | **License: Conditional Open-Source Grant License**

![Logo](./Logo_Zymatica_Voice.png)

This repository contains the standalone codebase and deliverables for **Milestone 2** of the Solana Foundation USA Grant.

---

## 🎯 Milestone 2 Deliverables
*   **Solana Pay Mesh Adapter:** BIP21/Solana Pay payment URL generator (`solana_pay_mock.ts`) to reward edge LoRa relay gateways for routing intent coordinate packets.
*   **On-Chain Transaction Verification:** Gateway verification scanner that monitors the Solana ledger to confirm receipt of references-tagged packets.
*   **Live Integration Tests:** End-to-end test suite (`test_solana_pay_live.ts`) verifying live devnet transfers.

---

## 📂 Repository File Structure

*   `programs/solana-cuneiform-anchor/` — Rust smart contract source code.
*   `app/src/cuneiform_client.ts` — TypeScript SDK.
*   `app/src/solana_pay_mock.ts` — Solana Pay payment request and verification adapter.
*   `app/src/test_solana_pay_live.ts` — Live Solana Pay validation tests.
*   `app/src/test_devnet.ts` — Milestone 1 integration tests.
*   `app/src/deploy_devnet.ts` — Contract state initialization script.
*   `milestones/milestone-1-devnet-deployment/` — Compiled program `.so` and documentation.
*   `Logo_Zymatica_Voice.png` / `Logo.jpg` — Brand assets.

---

## 🚀 How to Verify (Evaluator Instructions)

### 1. Run Milestone 2 Tests (Solana Pay Live Transfer & Scan)
```bash
# Install dependencies
npm install

# Run the Solana Pay integration test
npx tsx app/src/test_solana_pay_live.ts
```

### 2. Run Milestone 1 Tests (On-Chain Coordinates & Fees)
```bash
# Setup network and run integration test suite
solana config set --url devnet
npx tsx app/src/test_devnet.ts
```

---
---

## 📜 License

**Conditional Open-Source Grant License**

This project is currently under a proprietary license during the grant evaluation period. It will automatically transition to the open-source **Apache License 2.0** upon the approval and funding of the Superteam USA / Solana Foundation grant.

Please see the [LICENSE](./LICENSE) file for the full terms and conditions.
