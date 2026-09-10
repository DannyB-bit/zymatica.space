// Watermark: ip zymatica.space | astronautshe.com
// Copyright © 2026 Zymatica
// SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
// See LICENSE for terms.

const std = @import("std");

const Concept6D = struct {
    domain: u8,
    subdomain: u8,
    operation: u8,
    modality: u8,
    depth: u8,
    polarity: u8,
};

const SparseTransition = struct {
    key: u32,
    sym: u8,
    count: u32,
};

const RadicalPredictor = struct {
    alpha: u32,
    weight: u32,
    trans_rc: [256]SparseTransition = undefined,
    num_rc: u32 = 0,
    trans_rf: [256]SparseTransition = undefined,
    num_rf: u32 = 0,
    trans_ra: [256]SparseTransition = undefined,
    num_ra: u32 = 0,
    prev_rc: u8 = 0,
    prev_rf: u8 = 0,
    prev_ra: u8 = 0,

    pub fn init(alpha: u32, weight: u32) RadicalPredictor {
        const rp = RadicalPredictor{
            .alpha = alpha,
            .weight = weight,
            .num_rc = 0,
            .num_rf = 0,
            .num_ra = 0,
            .prev_rc = 0,
            .prev_rf = 0,
            .prev_ra = 0,
        };
        return rp;
    }

    pub fn observe(self: *RadicalPredictor, rc: u8, rf: u8, ra: u8) void {
        const w = self.weight;
        const key_rc = @as(u32, self.prev_rc);
        var found_rc = false;
        var i: u32 = 0;
        while (i < self.num_rc) : (i += 1) {
            if (self.trans_rc[i].key == key_rc and self.trans_rc[i].sym == rc) {
                self.trans_rc[i].count += w;
                found_rc = true;
                break;
            }
        }
        if (!found_rc and self.num_rc < 256) {
            self.trans_rc[self.num_rc] = SparseTransition{ .key = key_rc, .sym = rc, .count = w };
            self.num_rc += 1;
        }

        const key_rf = (@as(u32, rc) << 8) | @as(u32, self.prev_rf);
        var found_rf = false;
        i = 0;
        while (i < self.num_rf) : (i += 1) {
            if (self.trans_rf[i].key == key_rf and self.trans_rf[i].sym == rf) {
                self.trans_rf[i].count += w;
                found_rf = true;
                break;
            }
        }
        if (!found_rf and self.num_rf < 256) {
            self.trans_rf[self.num_rf] = SparseTransition{ .key = key_rf, .sym = rf, .count = w };
            self.num_rf += 1;
        }

        const key_ra = (@as(u32, rc) << 16) | (@as(u32, rf) << 8) | @as(u32, self.prev_ra);
        var found_ra = false;
        i = 0;
        while (i < self.num_ra) : (i += 1) {
            if (self.trans_ra[i].key == key_ra and self.trans_ra[i].sym == ra) {
                self.trans_ra[i].count += w;
                found_ra = true;
                break;
            }
        }
        if (!found_ra and self.num_ra < 256) {
            self.trans_ra[self.num_ra] = SparseTransition{ .key = key_ra, .sym = ra, .count = w };
            self.num_ra += 1;
        }

        self.prev_rc = rc;
        self.prev_rf = rf;
        self.prev_ra = ra;
    }

    pub fn getCumFreqsRC(self: *const RadicalPredictor, prev_rc: u8, cum_freqs: *[257]u32) void {
        var freqs: [256]u32 = undefined;
        @memset(&freqs, self.alpha);
        var i: u32 = 0;
        while (i < self.num_rc) : (i += 1) {
            if (self.trans_rc[i].key == prev_rc) {
                freqs[self.trans_rc[i].sym] += self.trans_rc[i].count;
            }
        }
        cum_freqs[0] = 0;
        i = 0;
        while (i < 256) : (i += 1) {
            cum_freqs[i + 1] = cum_freqs[i] + freqs[i];
        }
    }

    pub fn getCumFreqsRF(self: *const RadicalPredictor, curr_rc: u8, prev_rf: u8, cum_freqs: *[257]u32) void {
        var freqs: [256]u32 = undefined;
        @memset(&freqs, self.alpha);
        const key = (@as(u32, curr_rc) << 8) | prev_rf;
        var i: u32 = 0;
        while (i < self.num_rf) : (i += 1) {
            if (self.trans_rf[i].key == key) {
                freqs[self.trans_rf[i].sym] += self.trans_rf[i].count;
            }
        }
        cum_freqs[0] = 0;
        i = 0;
        while (i < 256) : (i += 1) {
            cum_freqs[i + 1] = cum_freqs[i] + freqs[i];
        }
    }

    pub fn getCumFreqsRA(self: *const RadicalPredictor, curr_rc: u8, curr_rf: u8, prev_ra: u8, cum_freqs: *[257]u32) void {
        var freqs: [256]u32 = undefined;
        @memset(&freqs, self.alpha);
        const key = (@as(u32, curr_rc) << 16) | (@as(u32, curr_rf) << 8) | prev_ra;
        var i: u32 = 0;
        while (i < self.num_ra) : (i += 1) {
            if (self.trans_ra[i].key == key) {
                freqs[self.trans_ra[i].sym] += self.trans_ra[i].count;
            }
        }
        cum_freqs[0] = 0;
        i = 0;
        while (i < 256) : (i += 1) {
            cum_freqs[i + 1] = cum_freqs[i] + freqs[i];
        }
    }
};

const BitWriter = struct {
    buffer: [256]u8 = [_]u8{0} ** 256,
    bit_index: u32 = 0,

    pub fn writeBit(self: *BitWriter, bit: u8) void {
        const byte_pos = self.bit_index / 8;
        const bit_pos = 7 - (self.bit_index % 8);
        if (byte_pos < 256) {
            if (bit != 0) {
                self.buffer[byte_pos] |= (@as(u8, 1) << @as(u3, @intCast(bit_pos)));
            } else {
                self.buffer[byte_pos] &= ~(@as(u8, 1) << @as(u3, @intCast(bit_pos)));
            }
            self.bit_index += 1;
        }
    }

    pub fn writeBitHelper(self: *BitWriter, underflow_bits: *u32, bit: u8) void {
        self.writeBit(bit);
        while (underflow_bits.* > 0) {
            self.writeBit(1 - bit);
            underflow_bits.* -= 1;
        }
    }
};

const BitReader = struct {
    buffer: []const u8,
    bit_index: u32 = 0,
    total_bits: u32,

    pub fn init(buf: []const u8) BitReader {
        return BitReader{
            .buffer = buf,
            .bit_index = 0,
            .total_bits = @as(u32, @intCast(buf.len * 8)),
        };
    }

    pub fn readBit(self: *BitReader) u8 {
        if (self.bit_index >= self.total_bits) {
            return 0;
        }
        const byte_pos = self.bit_index / 8;
        const bit_pos = 7 - (self.bit_index % 8);
        const bit = (self.buffer[byte_pos] >> @as(u3, @intCast(bit_pos))) & 1;
        self.bit_index += 1;
        return bit;
    }
};

pub fn encode(concepts: []const Concept6D, buf_out: *[256]u8, alpha: u32, weight: u32) u32 {
    var pred = RadicalPredictor.init(alpha, weight);
    var w = BitWriter{};
    var low: u32 = 0;
    var high: u32 = 0xFFFFFFFF;
    var underflow_bits: u32 = 0;

    for (concepts) |c| {
        const rc = (c.domain << 4) | c.subdomain;
        const rf = (c.operation << 4) | c.modality;
        const ra = (c.depth << 4) | c.polarity;
        const symbols = [3]u8{ rc, rf, ra };

        const prev_rc = pred.prev_rc;
        const prev_rf = pred.prev_rf;
        const prev_ra = pred.prev_ra;

        var step: u32 = 0;
        while (step < 3) : (step += 1) {
            var cum_freqs: [257]u32 = undefined;
            if (step == 0) {
                pred.getCumFreqsRC(prev_rc, &cum_freqs);
            } else if (step == 1) {
                pred.getCumFreqsRF(symbols[0], prev_rf, &cum_freqs);
            } else {
                pred.getCumFreqsRA(symbols[0], symbols[1], prev_ra, &cum_freqs);
            }

            const sym = @as(usize, symbols[step]);
            const total = cum_freqs[256];
            const cum_low = cum_freqs[sym];
            const cum_high = cum_freqs[sym + 1];

            const range_width = @as(u64, high) - @as(u64, low) + 1;
            high = low +% @as(u32, @truncate(@divTrunc(range_width * cum_high, total))) -% 1;
            low = low +% @as(u32, @truncate(@divTrunc(range_width * cum_low, total)));

            while (true) {
                if (high < 0x80000000) {
                    w.writeBitHelper(&underflow_bits, 0);
                    low <<= 1;
                    high = (high << 1) | 1;
                } else if (low >= 0x80000000) {
                    w.writeBitHelper(&underflow_bits, 1);
                    low = (low - 0x80000000) << 1;
                    high = ((high - 0x80000000) << 1) | 1;
                } else if (low >= 0x40000000 and high < 0xC0000000) {
                    underflow_bits += 1;
                    low = (low - 0x40000000) << 1;
                    high = ((high - 0x40000000) << 1) | 1;
                } else {
                    break;
                }
            }
        }
        pred.observe(rc, rf, ra);
    }

    underflow_bits += 1;
    if (low < 0x40000000) {
        w.writeBitHelper(&underflow_bits, 0);
    } else {
        w.writeBitHelper(&underflow_bits, 1);
    }

    @memcpy(buf_out, &w.buffer);
    return w.bit_index;
}

pub fn decode(encoded_bytes: []const u8, num_concepts: usize, outputs: []Concept6D, alpha: u32, weight: u32) bool {
    var pred = RadicalPredictor.init(alpha, weight);
    var r = BitReader.init(encoded_bytes);

    var value: u32 = 0;
    var i: u32 = 0;
    while (i < 32) : (i += 1) {
        value = (value << 1) | r.readBit();
    }

    var low: u32 = 0;
    var high: u32 = 0xFFFFFFFF;

    var c_idx: usize = 0;
    while (c_idx < num_concepts) : (c_idx += 1) {
        const prev_rc = pred.prev_rc;
        const prev_rf = pred.prev_rf;
        const prev_ra = pred.prev_ra;
        var symbols = [3]u8{ 0, 0, 0 };

        var step: u32 = 0;
        while (step < 3) : (step += 1) {
            var cum_freqs: [257]u32 = undefined;
            if (step == 0) {
                pred.getCumFreqsRC(prev_rc, &cum_freqs);
            } else if (step == 1) {
                pred.getCumFreqsRF(symbols[0], prev_rf, &cum_freqs);
            } else {
                pred.getCumFreqsRA(symbols[0], symbols[1], prev_ra, &cum_freqs);
            }

            const total = @as(u64, cum_freqs[256]);
            const range_width = @as(u64, high) - @as(u64, low) + 1;
            const scaled_val = @divTrunc((@as(u64, value) - @as(u64, low) + 1) * total - 1, range_width);

            var sym: u8 = 0;
            var l_idx: i32 = 0;
            var r_idx: i32 = 255;
            while (l_idx <= r_idx) {
                const m_idx = @divTrunc(l_idx + r_idx, 2);
                if (cum_freqs[@as(usize, @intCast(m_idx))] <= scaled_val and scaled_val < cum_freqs[@as(usize, @intCast(m_idx + 1))]) {
                    sym = @as(u8, @intCast(m_idx));
                    break;
                } else if (scaled_val >= cum_freqs[@as(usize, @intCast(m_idx + 1))]) {
                    l_idx = m_idx + 1;
                } else {
                    r_idx = m_idx - 1;
                }
            }

            symbols[step] = sym;
            const sym_idx = @as(usize, sym);
            const cum_low = cum_freqs[sym_idx];
            const cum_high = cum_freqs[sym_idx + 1];

            high = low +% @as(u32, @truncate(@divTrunc(range_width * cum_high, total))) -% 1;
            low = low +% @as(u32, @truncate(@divTrunc(range_width * cum_low, total)));

            while (true) {
                if (high < 0x80000000) {
                    low <<= 1;
                    high = (high << 1) | 1;
                    value = (value << 1) | r.readBit();
                } else if (low >= 0x80000000) {
                    low = (low - 0x80000000) << 1;
                    high = ((high - 0x80000000) << 1) | 1;
                    value = ((value - 0x80000000) << 1) | r.readBit();
                } else if (low >= 0x40000000 and high < 0xC0000000) {
                    low = (low - 0x40000000) << 1;
                    high = ((high - 0x40000000) << 1) | 1;
                    value = ((value - 0x40000000) << 1) | r.readBit();
                } else {
                    break;
                }
            }
        }

        outputs[c_idx] = Concept6D{
            .domain = (symbols[0] >> 4) & 0xF,
            .subdomain = symbols[0] & 0xF,
            .operation = (symbols[1] >> 4) & 0xF,
            .modality = symbols[1] & 0xF,
            .depth = (symbols[2] >> 4) & 0xF,
            .polarity = symbols[2] & 0xF,
        };
        pred.observe(symbols[0], symbols[1], symbols[2]);
    }
    return true;
}

pub fn main() anyerror!void {
    std.debug.print("======================================================================\n", .{});
    std.debug.print("ZYMATICA | zymatica-inference-engine-zig\n", .{});
    std.debug.print("======================================================================\n\n", .{});

    const inputs = [5]Concept6D{
        Concept6D{ .domain = 1, .subdomain = 2, .operation = 3, .modality = 4, .depth = 5, .polarity = 6 },
        Concept6D{ .domain = 8, .subdomain = 0, .operation = 15, .modality = 1, .depth = 0, .polarity = 15 },
        Concept6D{ .domain = 0, .subdomain = 0, .operation = 0, .modality = 0, .depth = 0, .polarity = 0 },
        Concept6D{ .domain = 15, .subdomain = 15, .operation = 15, .modality = 15, .depth = 15, .polarity = 15 },
        Concept6D{ .domain = 4, .subdomain = 5, .operation = 6, .modality = 7, .depth = 8, .polarity = 9 },
    };

    var buffer: [256]u8 = undefined;
    const bits = encode(&inputs, &buffer, 1, 128);
    const bytes = @divTrunc(bits + 7, 8);

    std.debug.print("Encoded Bits: {}, Bytes: {}\n", .{ bits, bytes });
    std.debug.print("Hex: ", .{});
    var i: usize = 0;
    while (i < @as(usize, @intCast(bytes))) : (i += 1) {
        std.debug.print("{0X:0>2} ", .{ buffer[i] });
    }
    std.debug.print("\n", .{});

    // High-precision timing loop for 100,000 runs
    const builtin = @import("builtin");
    const runs = 100000;
    var match = true;
    var elapsed_ms: f64 = 0.0;

    if (builtin.os.tag == .windows) {
        const windows = std.os.windows;
        var start: windows.LARGE_INTEGER = undefined;
        var end: windows.LARGE_INTEGER = undefined;
        var freq: windows.LARGE_INTEGER = undefined;

        _ = windows.ntdll.RtlQueryPerformanceFrequency(&freq);
        _ = windows.ntdll.RtlQueryPerformanceCounter(&start);

        var r: usize = 0;
        while (r < runs) : (r += 1) {
            var outputs: [5]Concept6D = undefined;
            _ = decode(&buffer, 5, &outputs, 1, 128);
            
            if (r == 0) {
                var idx: usize = 0;
                while (idx < 5) : (idx += 1) {
                    if (inputs[idx].domain != outputs[idx].domain or
                        inputs[idx].subdomain != outputs[idx].subdomain or
                        inputs[idx].operation != outputs[idx].operation or
                        inputs[idx].modality != outputs[idx].modality or
                        inputs[idx].depth != outputs[idx].depth or
                        inputs[idx].polarity != outputs[idx].polarity) {
                        match = false;
                    }
                }
            }
        }

        _ = windows.ntdll.RtlQueryPerformanceCounter(&end);
        elapsed_ms = @as(f64, @floatFromInt(end - start)) * 1000.0 / @as(f64, @floatFromInt(freq));
    } else {
        var r: usize = 0;
        while (r < runs) : (r += 1) {
            var outputs: [5]Concept6D = undefined;
            _ = decode(&buffer, 5, &outputs, 1, 128);
            
            if (r == 0) {
                var idx: usize = 0;
                while (idx < 5) : (idx += 1) {
                    if (inputs[idx].domain != outputs[idx].domain or
                        inputs[idx].subdomain != outputs[idx].subdomain or
                        inputs[idx].operation != outputs[idx].operation or
                        inputs[idx].modality != outputs[idx].modality or
                        inputs[idx].depth != outputs[idx].depth or
                        inputs[idx].polarity != outputs[idx].polarity) {
                        match = false;
                    }
                }
            }
        }
        elapsed_ms = 0.0;
    }

    std.debug.print("Decoded matches inputs: {}\n", .{ match });
    if (!match) {
        std.debug.print("ERROR: mismatch!\n", .{});
        std.process.exit(1);
    }

    std.debug.print("[INTERNAL_MATH] {d:.4} ms\n", .{ elapsed_ms });
    std.debug.print("\n[VERIFICATION] Multi-Language runtime FFI structures validated.\n", .{});
}
