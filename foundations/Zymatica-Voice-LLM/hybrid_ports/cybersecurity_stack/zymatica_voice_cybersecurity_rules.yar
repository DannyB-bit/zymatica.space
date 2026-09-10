/*
  Watermark: ip zymatica.space | astronautshe.com
  Copyright (c) 2026 Zymatica. All rights reserved.
*/
rule ZymaticaAudioStreamAudit {
    meta:
        description = "Detects specific signature telemetry loops in Zymatica audio buffers"
    strings:
        $anchor = "Zymatica Voice LLM Cybersecurity Stack verified."
    condition:
        $anchor
}
