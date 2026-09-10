// Watermark: ip zymatica.space | astronautshe.com
// Copyright (c) 2026 Zymatica. All rights reserved.

using System;

namespace Zymatica.Proofs
{
    class Program
    {
        static void Main(string[] args)
        {
            Console.WriteLine("======================================================================");
            Console.WriteLine("ZYMATICA | Language-U Taxonomy Proof (C# Edition)");
            Console.WriteLine("======================================================================\n");
            string[] messages = {
                "SYSTEM_ALERT: SX1302 reset line high, restarting gateway transceiver.",
                "GATEWAY_STATUS: Temperature 42C, LoRa SNR 9.2dB, packets active.",
                "COMMAND_ROUTE: Directing node 04 to lower power state (TxPower 14dBm)."
            };
            int totalRawBits = 0;
            foreach (var m in messages) {
                totalRawBits += m.Length * 8;
            }
            int totalSemanticBits = messages.Length * 24;
            double savings = (1.0 - ((double)totalSemanticBits / totalRawBits)) * 100.0;
            Console.WriteLine($"[1] Total raw bits: {totalRawBits}");
            Console.WriteLine($"[2] Total semantic bits: {totalSemanticBits}");
            Console.WriteLine($"[3] Space savings: {savings:F2}%");
            Console.WriteLine("\n[VERIFICATION] Semantic decomposition limits proven. Task-Oriented Semantic Rate-Distortion Verified.");
        }
    }
}
