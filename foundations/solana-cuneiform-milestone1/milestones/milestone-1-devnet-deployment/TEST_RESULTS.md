# Test Results — Milestone 1 Integration Tests

**Date:** June 27, 2026  
**Network:** Solana Devnet  
**Program ID:** `BJKrKzXX4YfEYMZaVT2dbuaNuq7aqN3Xmib27JLALs3M`  
**Result: 11/11 PASSED ✅**

---

## Full Test Output

```
======================================================================
ZYMATICA | Solana Cuneiform — Devnet Integration Tests
======================================================================

🔑  Wallet: 7kZ3XwggVosBMag5mAJt6JVM2uP86YLoBaY9rQXccKS
💰  Balance: 3.3608362 SOL

── Test 1: Fetch Program State ──
  ✅ PASS: Admin matches deployer
  ✅ PASS: Treasury matches cold wallet
  ✅ PASS: Protocol fee = 150,000 lamports

── Test 2: Register Cuneiform-U Coordinates ──
📋  Session ID: mqw9znzz00000000
📊  Coords: [42, 7, 3, 128, 200, 15]
🔐  Merkle Root: 1277f5faef1c66f4e1409a7df186c15e...
  ✅ PASS: Coordinate registration transaction succeeded
📝  TX: 2WbKq8A9BfAofU46QXQaYny2RRz1wx8aDirkR6N7HxmcmsFxHC6UAdooYzd6BLnwztaV49qq3Tdw74MAwi7V1aMT
🔗  https://explorer.solana.com/tx/2WbKq8A9BfAofU46QXQaYny2RRz1wx8aDirkR6N7HxmcmsFxHC6UAdooYzd6BLnwztaV49qq3Tdw74MAwi7V1aMT?cluster=devnet
  ✅ PASS: Protocol fee collected: 150000 lamports sent to treasury

── Test 3: Fetch and Verify On-Chain Record ──
  ✅ PASS: Authority matches
  ✅ PASS: Coordinates match: [42, 7, 3, 128, 200, 15]
  ✅ PASS: Timestamp recorded: 2026-06-27T11:27:29.000Z
  ✅ PASS: Merkle root matches

── Test 4: Update Coordinates ──
  ✅ PASS: Update transaction succeeded
📝  TX: 4NEHNuGmqrkoqaf7upkyBKRV8rmSNhULr6ybFiZbeyan6qFXNJTDUEa6Ekz42p131uBUqKSfHcUNSLZPmMnYxLfT
  ✅ PASS: Coords updated to: [99, 14, 6, 64, 255, 30]

======================================================================
RESULTS: 11 passed, 0 failed, 11 total
======================================================================

🎉  ALL TESTS PASSED — Protocol is fully operational on devnet!
🚀  Grant Milestone 1: VERIFIED
```

---

## On-Chain Transaction Proofs

| Test | Transaction Signature | Explorer Link |
|---|---|---|
| Program Init | `vRcx2h5N1zzvUG3xUuaDbxkUnFNhB3GSPhtJK7ByvHWkk88Je8Fr9tBKFiFENx7h2hv2ZnBeFzziLmxPjp6pHAx` | [View](https://explorer.solana.com/tx/vRcx2h5N1zzvUG3xUuaDbxkUnFNhB3GSPhtJK7ByvHWkk88Je8Fr9tBKFiFENx7h2hv2ZnBeFzziLmxPjp6pHAx?cluster=devnet) |
| Coordinate Registration | `2WbKq8A9BfAofU46QXQaYny2RRz1wx8aDirkR6N7HxmcmsFxHC6UAdooYzd6BLnwztaV49qq3Tdw74MAwi7V1aMT` | [View](https://explorer.solana.com/tx/2WbKq8A9BfAofU46QXQaYny2RRz1wx8aDirkR6N7HxmcmsFxHC6UAdooYzd6BLnwztaV49qq3Tdw74MAwi7V1aMT?cluster=devnet) |
| Coordinate Update | `4NEHNuGmqrkoqaf7upkyBKRV8rmSNhULr6ybFiZbeyan6qFXNJTDUEa6Ekz42p131uBUqKSfHcUNSLZPmMnYxLfT` | [View](https://explorer.solana.com/tx/4NEHNuGmqrkoqaf7upkyBKRV8rmSNhULr6ybFiZbeyan6qFXNJTDUEa6Ekz42p131uBUqKSfHcUNSLZPmMnYxLfT?cluster=devnet) |

---

## Program Logs (On-Chain)

### Registration Transaction Logs
```
Program log: Instruction: RegisterCoordinates
Program 11111111111111111111111111111111 invoke [2]
Program 11111111111111111111111111111111 success
Program 11111111111111111111111111111111 invoke [2]
Program 11111111111111111111111111111111 success
Program log: Collected protocol fee: 150000 lamports
Program log: Language-U coordinates registered successfully!
Program log: Coordinates: Domain=42, Subdomain=7, Modality=3, Polarity=128, Strength=200, Depth=15
Program BJKrKzXX4YfEYMZaVT2dbuaNuq7aqN3Xmib27JLALs3M consumed 14964 of 200000 compute units
Program BJKrKzXX4YfEYMZaVT2dbuaNuq7aqN3Xmib27JLALs3M success
```

### Key Metrics
- **Compute Units:** 14,964 / 200,000 (7.5% — extremely efficient)
- **Protocol Fee:** 150,000 lamports (~$0.002 at $200/SOL)
- **Account Size:** 103 bytes per coordinate record
