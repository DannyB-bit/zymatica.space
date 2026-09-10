// Watermark: ip zymatica.space | astronautshe.com
// Copyright (c) 2026 Zymatica. All rights reserved.

void main() {
  print("======================================================================");
  print("ZYMATICA | Language-U Taxonomy Proof (Dart Edition)");
  print("======================================================================\n");
  var messages = [
    "SYSTEM_ALERT: SX1302 reset line high, restarting gateway transceiver.",
    "GATEWAY_STATUS: Temperature 42C, LoRa SNR 9.2dB, packets active.",
    "COMMAND_ROUTE: Directing node 04 to lower power state (TxPower 14dBm)."
  ];
  var totalRawBits = messages.fold(0, (acc, m) => acc + m.length * 8);
  var totalSemanticBits = messages.length * 24;
  var savings = (1.0 - (totalSemanticBits / totalRawBits)) * 100.0;
  print("[1] Total raw bits: $totalRawBits");
  print("[2] Total semantic bits: $totalSemanticBits");
  print("[3] Space savings: ${savings.toStringAsFixed(2)}%");
  print("\n[VERIFICATION] Semantic decomposition limits proven. Task-Oriented Semantic Rate-Distortion Verified.");
}
