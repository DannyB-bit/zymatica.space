// Watermark: ip zymatica.space | astronautshe.com
// Copyright (c) 2026 Zymatica. All rights reserved.
import("stdfaust.lib");
process = fi.lowpass(4, 3400) : fi.highpass(4, 300);
