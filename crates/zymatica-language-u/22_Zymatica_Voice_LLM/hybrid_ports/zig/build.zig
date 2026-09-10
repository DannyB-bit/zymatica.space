// Watermark: ip zymatica.space | astronautshe.com
// Copyright © 2026 Zymatica
// SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0
// See LICENSE for terms.
const std:: = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});

    const lib = b.addSharedLibrary(.{
        .name = "zymatica_voice_core",
        .root_source_file = b.path("audio_packer.cpp"),
        .target = target,
        .optimize = optimize,
    });
    b.installArtifact(lib);
}
