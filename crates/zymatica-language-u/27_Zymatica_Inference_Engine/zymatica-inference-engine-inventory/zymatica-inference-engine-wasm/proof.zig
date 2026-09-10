// Watermark: ip zymatica.space | astronautshe.com
// Copyright © 2026 Zymatica
// SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
// See LICENSE for terms.

const std = @import("std");

const SparseTransition = struct {
    key: u32,
    sym: u8,
    count: u32,
};

const RadicalPredictor = struct {
    alpha: u32,
    weight: u32,
    trans_rc: [256]SparseTransition,
    trans_rc_len: usize,
    trans_rf: [256]SparseTransition,
    trans_rf_len: usize,
    trans_ra: [256]SparseTransition,
    trans_ra_len: usize,
    prev_rc: u8,
    prev_rf: u8,
    prev_ra: u8,

    pub fn init(alpha: u32, weight: u32) RadicalPredictor {
        return .{
            .alpha = alpha,
            .weight = weight,
            .trans_rc = undefined,
            .trans_rc_len = 0,
            .trans_rf = undefined,
            .trans_rf_len = 0,
            .trans_ra = undefined,
            .trans_ra_len = 0,
            .prev_rc = 0,
            .prev_rf = 0,
            .prev_ra = 0,
        };
    }

    pub fn observe(self: *RadicalPredictor, rc: u8, rf: u8, ra: u8) void {
        const w = self.weight;
        
        // RC Transitions
        const key_rc = @as(u32, self.prev_rc);
        var found_rc = false;
        for (self.trans_rc[0..self.trans_rc_len]) |*entry| {
            if (entry.key == key_rc and entry.sym == rc) {
                entry.count += w;
                found_rc = true;
                break;
            }
        }
        if (!found_rc and self.trans_rc_len < 256) {
            self.trans_rc[self.trans_rc_len] = .{ .key = key_rc, .sym = rc, .count = w };
            self.trans_rc_len += 1;
        }

        // RF Transitions
        const key_rf = (@as(u32, rc) << 8) | @as(u32, self.prev_rf);
        var found_rf = false;
        for (self.trans_rf[0..self.trans_rf_len]) |*entry| {
            if (entry.key == key_rf and entry.sym == rf) {
                entry.count += w;
                found_rf = true;
                break;
            }
        }
        if (!found_rf and self.trans_rf_len < 256) {
            self.trans_rf[self.trans_rf_len] = .{ .key = key_rf, .sym = rf, .count = w };
            self.trans_rf_len += 1;
        }

        // RA Transitions
        const key_ra = (@as(u32, rc) << 16) | (@as(u32, rf) << 8) | @as(u32, self.prev_ra);
        var found_ra = false;
        for (self.trans_ra[0..self.trans_ra_len]) |*entry| {
            if (entry.key == key_ra and entry.sym == ra) {
                entry.count += w;
                found_ra = true;
                break;
            }
        }
        if (!found_ra and self.trans_ra_len < 256) {
            self.trans_ra[self.trans_ra_len] = .{ .key = key_ra, .sym = ra, .count = w };
            self.trans_ra_len += 1;
        }

        self.prev_rc = rc;
        self.prev_rf = rf;
        self.prev_ra = ra;
    }

    pub fn getCumFreqsRC(self: *const RadicalPredictor, prev_rc: u8, cum_freqs: *[257]u32) void {
        var freqs = [_]u32{self.alpha} ** 256;
        for (self.trans_rc[0..self.trans_rc_len]) |entry| {
            if (entry.key == @as(u32, prev_rc)) {
                freqs[entry.sym] += entry.count;
            }
        }
        cum_freqs[0] = 0;
        var i: usize = 0;
        while (i < 256) : (i += 1) {
            cum_freqs[i + 1] = cum_freqs[i] + freqs[i];
        }
    }

    pub fn getCumFreqsRF(self: *const RadicalPredictor, curr_rc: u8, prev_rf: u8, cum_freqs: *[257]u32) void {
        var freqs = [_]u32{self.alpha} ** 256;
        const key = (@as(u32, curr_rc) << 8) | @as(u32, prev_rf);
        for (self.trans_rf[0..self.trans_rf_len]) |entry| {
            if (entry.key == key) {
                freqs[entry.sym] += entry.count;
            }
        }
        cum_freqs[0] = 0;
        var i: usize = 0;
        while (i < 256) : (i += 1) {
            cum_freqs[i + 1] = cum_freqs[i] + freqs[i];
        }
    }

    pub fn getCumFreqsRA(self: *const RadicalPredictor, curr_rc: u8, curr_rf: u8, prev_ra: u8, cum_freqs: *[257]u32) void {
        var freqs = [_]u32{self.alpha} ** 256;
        const key = (@as(u32, curr_rc) << 16) | (@as(u32, curr_rf) << 8) | @as(u32, prev_ra);
        for (self.trans_ra[0..self.trans_ra_len]) |entry| {
            if (entry.key == key) {
                freqs[entry.sym] += entry.count;
            }
        }
        cum_freqs[0] = 0;
        var i: usize = 0;
        while (i < 256) : (i += 1) {
            cum_freqs[i + 1] = cum_freqs[i] + freqs[i];
        }
    }
};

const BitWriter = struct {
    buffer: [1024]u8,
    bit_index: usize,

    pub fn init() BitWriter {
        return .{
            .buffer = [_]u8{0} ** 1024,
            .bit_index = 0,
        };
    }

    pub fn writeBit(self: *BitWriter, bit: u8) void {
        const byte_pos = self.bit_index / 8;
        const bit_pos = @as(u3, @intCast(7 - (self.bit_index % 8)));
        if (byte_pos < 1024) {
            if (bit != 0) {
                self.buffer[byte_pos] |= (@as(u8, 1) << bit_pos);
            } else {
                self.buffer[byte_pos] &= ~(@as(u8, 1) << bit_pos);
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
    bit_index: usize,
    total_bits: usize,

    pub fn init(buffer: []const u8) BitReader {
        return .{
            .buffer = buffer,
            .bit_index = 0,
            .total_bits = buffer.len * 8,
        };
    }

    pub fn readBit(self: *BitReader) u8 {
        if (self.bit_index >= self.total_bits) {
            return 0;
        }
        const byte_pos = self.bit_index / 8;
        const bit_pos = @as(u3, @intCast(7 - (self.bit_index % 8)));
        const bit = (self.buffer[byte_pos] >> bit_pos) & 1;
        self.bit_index += 1;
        return bit;
    }
};

const Concept6D = struct {
    domain: u8,
    subdomain: u8,
    operation: u8,
    modality: u8,
    depth: u8,
    polarity: u8,
};

fn encode(concepts: []const Concept6D, writer: *BitWriter) void {
    var pred = RadicalPredictor.init(1, 128);
    var low: u32 = 0;
    var high: u32 = 0xFFFFFFFF;
    var underflow_bits: u32 = 0;
    var cum_freqs: [257]u32 = undefined;

    for (concepts) |c| {
        const rc = (c.domain << 4) | c.subdomain;
        const rf = (c.operation << 4) | c.modality;
        const ra = (c.depth << 4) | c.polarity;
        const symbols = [_]u8{ rc, rf, ra };

        const prev_rc = pred.prev_rc;
        const prev_rf = pred.prev_rf;
        const prev_ra = pred.prev_ra;

        var step: usize = 0;
        while (step < 3) : (step += 1) {
            switch (step) {
                0 => pred.getCumFreqsRC(prev_rc, &cum_freqs),
                1 => pred.getCumFreqsRF(symbols[0], prev_rf, &cum_freqs),
                else => pred.getCumFreqsRA(symbols[0], symbols[1], prev_ra, &cum_freqs),
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
                    writer.writeBitHelper(&underflow_bits, 0);
                    low <<= 1;
                    high = (high << 1) | 1;
                } else if (low >= 0x80000000) {
                    writer.writeBitHelper(&underflow_bits, 1);
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
        writer.writeBitHelper(&underflow_bits, 0);
    } else {
        writer.writeBitHelper(&underflow_bits, 1);
    }
}

fn decode(encoded_bytes: []const u8, num_concepts: usize, decoded: []Concept6D) void {
    var pred = RadicalPredictor.init(1, 128);
    var r = BitReader.init(encoded_bytes);

    var value: u32 = 0;
    var i: usize = 0;
    while (i < 32) : (i += 1) {
        value = (value << 1) | @as(u32, r.readBit());
    }

    var low: u32 = 0;
    var high: u32 = 0xFFFFFFFF;
    var cum_freqs: [257]u32 = undefined;

    var c_idx: usize = 0;
    while (c_idx < num_concepts) : (c_idx += 1) {
        const prev_rc = pred.prev_rc;
        const prev_rf = pred.prev_rf;
        const prev_ra = pred.prev_ra;
        var symbols = [_]u8{ 0, 0, 0 };

        var step: usize = 0;
        while (step < 3) : (step += 1) {
            switch (step) {
                0 => pred.getCumFreqsRC(prev_rc, &cum_freqs),
                1 => pred.getCumFreqsRF(symbols[0], prev_rf, &cum_freqs),
                else => pred.getCumFreqsRA(symbols[0], symbols[1], prev_ra, &cum_freqs),
            }

            const total = @as(u64, cum_freqs[256]);
            const range_width = @as(u64, high) - @as(u64, low) + 1;
            const scaled_val = @divTrunc(((@as(u64, value) - @as(u64, low)) + 1) * total - 1, range_width);

            var sym: u8 = 0;
            var l: i32 = 0;
            var rr: i32 = 255;
            while (l <= rr) {
                const mid = @divTrunc(l + rr, 2);
                if (@as(u64, cum_freqs[@intCast(mid)]) <= scaled_val and scaled_val < @as(u64, cum_freqs[@intCast(mid + 1)])) {
                    sym = @intCast(mid);
                    break;
                } else if (scaled_val >= @as(u64, cum_freqs[@intCast(mid + 1)])) {
                    l = mid + 1;
                } else {
                    rr = mid - 1;
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
                    value = (value << 1) | @as(u32, r.readBit());
                } else if (low >= 0x80000000) {
                    low = (low - 0x80000000) << 1;
                    high = ((high - 0x80000000) << 1) | 1;
                    value = ((value - 0x80000000) << 1) | @as(u32, r.readBit());
                } else if (low >= 0x40000000 and high < 0xC0000000) {
                    low = (low - 0x40000000) << 1;
                    high = ((high - 0x40000000) << 1) | 1;
                    value = ((value - 0x40000000) << 1) | @as(u32, r.readBit());
                } else {
                    break;
                }
            }
        }

        decoded[c_idx] = .{
            .domain = symbols[0] >> 4,
            .subdomain = symbols[0] & 0x0F,
            .operation = symbols[1] >> 4,
            .modality = symbols[1] & 0x0F,
            .depth = symbols[2] >> 4,
            .polarity = symbols[2] & 0x0F,
        };
        pred.observe(symbols[0], symbols[1], symbols[2]);
    }
}

export fn run_verification() i32 {
    const inputs = [_]Concept6D{
        .{ .domain = 1, .subdomain = 2, .operation = 3, .modality = 4, .depth = 5, .polarity = 6 },
        .{ .domain = 8, .subdomain = 0, .operation = 15, .modality = 1, .depth = 0, .polarity = 15 },
        .{ .domain = 0, .subdomain = 0, .operation = 0, .modality = 0, .depth = 0, .polarity = 0 },
        .{ .domain = 15, .subdomain = 15, .operation = 15, .modality = 15, .depth = 15, .polarity = 15 },
        .{ .domain = 4, .subdomain = 5, .operation = 6, .modality = 7, .depth = 8, .polarity = 9 },
    };

    var writer = BitWriter.init();
    encode(&inputs, &writer);

    const written_bytes = (writer.bit_index + 7) / 8;
    var decoded_buf: [5]Concept6D = undefined;
    decode(writer.buffer[0..written_bytes], 5, &decoded_buf);

    var i: usize = 0;
    while (i < 5) : (i += 1) {
        const orig = inputs[i];
        const dec = decoded_buf[i];
        if (orig.domain != dec.domain or
            orig.subdomain != dec.subdomain or
            orig.operation != dec.operation or
            orig.modality != dec.modality or
            orig.depth != dec.depth or
            orig.polarity != dec.polarity) {
            return 0;
        }
    }
    return 1;
}

pub fn main() void {}
