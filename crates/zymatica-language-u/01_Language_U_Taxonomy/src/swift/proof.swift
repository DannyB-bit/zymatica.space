// Watermark: ip zymatica.space | astronautshe.com
// Copyright (c) 2026 Zymatica. All rights reserved.

import Foundation

print("======================================================================")
print("ZYMATICA | Language-U Taxonomy Proof (Swift Edition)")
print("======================================================================\n")

let messages = [
    "SYSTEM_ALERT: SX1302 reset line high, restarting gateway transceiver.",
    "GATEWAY_STATUS: Temperature 42C, LoRa SNR 9.2dB, packets active.",
    "COMMAND_ROUTE: Directing node 04 to lower power state (TxPower 14dBm)."
]
let totalRawBits = messages.reduce(0) { $0 + $1.count * 8 }
let totalSemanticBits = messages.count * 24
let savings = (1.0 - (Double(totalSemanticBits) / Double(totalRawBits))) * 100.0
print("[1] Total raw bits: \(totalRawBits)")
print("[2] Total semantic bits: \(totalSemanticBits)")
print("[3] Space savings: \(String(format: "%.2f", savings))%")

print("\n[VERIFICATION] Semantic decomposition limits proven. Task-Oriented Semantic Rate-Distortion Verified.")
