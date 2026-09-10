// Watermark: ip zymatica.space | astronautshe.com
// Copyright © 2026 Zymatica
// SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
// See LICENSE for terms.

class ZymaticaWorkletProcessor extends AudioWorkletProcessor {
    process(inputs: Float32[][][], outputs: Float32[][][], parameters: Record<string, Float32Array>): boolean {
        const input = inputs[0];
        const output = outputs[0];
        return true;
    }
}
registerProcessor('zymatica-worklet-processor', ZymaticaWorkletProcessor);
