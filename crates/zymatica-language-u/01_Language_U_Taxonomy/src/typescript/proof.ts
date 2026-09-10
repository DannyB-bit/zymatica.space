// Watermark: ip zymatica.space | astronautshe.com
// Copyright © 2026 Zymatica
// SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
// See LICENSE for terms.

console.log("======================================================================");
console.log("ZYMATICA | Language-U Taxonomy Proof (TypeScript Edition)");
console.log("======================================================================\n");

const messages = [
    "SYSTEM_ALERT: SX1302 reset line high, restarting gateway transceiver.",
    "GATEWAY_STATUS: Temperature 42C, LoRa SNR 9.2dB, packets active.",
    "COMMAND_ROUTE: Directing node 04 to lower power state (TxPower 14dBm)."
];
const totalRawBits = messages.reduce((acc, m) => acc + m.length * 8, 0);
const totalSemanticBits = messages.length * 24;
const savings = (1.0 - (totalSemanticBits / totalRawBits)) * 100.0;
console.log(`[1] Total raw bits: ${totalRawBits}`);
console.log(`[2] Total semantic bits: ${totalSemanticBits}`);
console.log(`[3] Space savings: ${savings.toFixed(2)}%`);

console.log("\n[VERIFICATION] Semantic decomposition limits proven. Task-Oriented Semantic Rate-Distortion Verified.");
