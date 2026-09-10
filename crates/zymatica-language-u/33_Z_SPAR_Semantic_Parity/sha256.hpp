#pragma once

#include <algorithm>
#include <array>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <span>
#include <vector>

namespace zspar {

class Sha256 {
public:
    static std::array<std::uint8_t, 32> digest(std::span<const std::uint8_t> data) noexcept {
        std::uint32_t h[8] = {
            0x6a09e667U, 0xbb67ae85U, 0x3c6ef372U, 0xa54ff53aU,
            0x510e527fU, 0x9b05688cU, 0x1f83d9abU, 0x5be0cd19U
        };

        static constexpr std::uint32_t k[64] = {
            0x428a2f98U, 0x71374491U, 0xb5c0fbcfU, 0xe9b5dba5U, 0x3956c25bU, 0x59f111f1U, 0x923f82a4U, 0xab1c5ed5U,
            0xd807aa98U, 0x12835b01U, 0x243185beU, 0x550c7dc3U, 0x72be5d74U, 0x80deb1feU, 0x9bdc06a7U, 0xc19bf174U,
            0xe49b69c1U, 0xefbe4786U, 0x0fc19dc6U, 0x240ca1ccU, 0x2de92c6fU, 0x4a7484aaU, 0x5cb0a9dcU, 0x76f988daU,
            0x983e5152U, 0xa831c66dU, 0xb00327c8U, 0xbf597fc7U, 0xc6e00bf3U, 0xd5a79147U, 0x06ca6351U, 0x14292967U,
            0x27b70a85U, 0x2e1b2138U, 0x4d2c6dfcU, 0x53380d13U, 0x650a7354U, 0x766a0abbU, 0x81c2c92eU, 0x92722c85U,
            0xa2bfe8a1U, 0xa81a664bU, 0xc24b8b70U, 0xc76c51a3U, 0xd192e819U, 0xd6990624U, 0xf40e3585U, 0x106aa070U,
            0x19a4c116U, 0x1e376c08U, 0x2748774cU, 0x34b0bcb5U, 0x391c0cb3U, 0x4ed8aa4aU, 0x5b9cca4fU, 0x682e6ff3U,
            0x748f82eeU, 0x78a5636fU, 0x84c87814U, 0x8cc70208U, 0x90befffaU, 0xa4506cebU, 0xbef9a3f7U, 0xc67178f2U
        };

        auto rotr = [](std::uint32_t x, std::uint32_t n) { return (x >> n) | (x << (32 - n)); };

        const std::size_t total_len = data.size();
        const std::uint64_t bit_len = static_cast<std::uint64_t>(total_len) * 8U;
        const std::size_t padded_len = ((total_len + 8) / 64 + 1) * 64;

        std::vector<std::uint8_t> padded(padded_len, 0);
        if (!data.empty()) {
            std::memcpy(padded.data(), data.data(), total_len);
        }
        padded[total_len] = 0x80;
        for (int i = 0; i < 8; ++i) {
            padded[padded_len - 1 - i] = static_cast<std::uint8_t>((bit_len >> (i * 8)) & 0xFFU);
        }

        for (std::size_t offset = 0; offset < padded_len; offset += 64) {
            std::uint32_t w[64];
            for (std::size_t i = 0; i < 16; ++i) {
                w[i] = (static_cast<std::uint32_t>(padded[offset + i * 4]) << 24) |
                       (static_cast<std::uint32_t>(padded[offset + i * 4 + 1]) << 16) |
                       (static_cast<std::uint32_t>(padded[offset + i * 4 + 2]) << 8) |
                       static_cast<std::uint32_t>(padded[offset + i * 4 + 3]);
            }
            for (std::size_t i = 16; i < 64; ++i) {
                std::uint32_t s0 = rotr(w[i - 15], 7) ^ rotr(w[i - 15], 18) ^ (w[i - 15] >> 3);
                std::uint32_t s1 = rotr(w[i - 2], 17) ^ rotr(w[i - 2], 19) ^ (w[i - 2] >> 10);
                w[i] = w[i - 16] + s0 + w[i - 7] + s1;
            }

            std::uint32_t a = h[0], b = h[1], c = h[2], d = h[3];
            std::uint32_t e = h[4], f = h[5], g = h[6], h_val = h[7];

            for (std::size_t i = 0; i < 64; ++i) {
                std::uint32_t s1 = rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25);
                std::uint32_t ch = (e & f) ^ ((~e) & g);
                std::uint32_t temp1 = h_val + s1 + ch + k[i] + w[i];
                std::uint32_t s0 = rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22);
                std::uint32_t maj = (a & b) ^ (a & c) ^ (b & c);
                std::uint32_t temp2 = s0 + maj;

                h_val = g;
                g = f;
                f = e;
                e = d + temp1;
                d = c;
                c = b;
                b = a;
                a = temp1 + temp2;
            }

            h[0] += a; h[1] += b; h[2] += c; h[3] += d;
            h[4] += e; h[5] += f; h[6] += g; h[7] += h_val;
        }

        std::array<std::uint8_t, 32> out{};
        for (std::size_t i = 0; i < 8; ++i) {
            out[i * 4] = static_cast<std::uint8_t>((h[i] >> 24) & 0xFF);
            out[i * 4 + 1] = static_cast<std::uint8_t>((h[i] >> 16) & 0xFF);
            out[i * 4 + 2] = static_cast<std::uint8_t>((h[i] >> 8) & 0xFF);
            out[i * 4 + 3] = static_cast<std::uint8_t>(h[i] & 0xFF);
        }
        return out;
    }
};

inline std::array<std::uint8_t, 16> sha256_128(std::span<const std::uint8_t> data) noexcept {
    const auto d = Sha256::digest(data);
    std::array<std::uint8_t, 16> out{};
    std::copy_n(d.begin(), 16, out.begin());
    return out;
}

} // namespace zspar
