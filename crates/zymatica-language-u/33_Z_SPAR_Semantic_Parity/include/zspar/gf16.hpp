#pragma once

#include <array>
#include <cstddef>
#include <cstdint>

namespace zspar {

struct GF16 {
    // GF(16) representation with primitive polynomial x^4 + x + 1 (0x13) and generator alpha = 2.
    static constexpr std::array<std::uint8_t, 32> EXP = {
        1, 2, 4, 8, 3, 6, 12, 11, 5, 10, 7, 14, 15, 13, 9, 1,
        2, 4, 8, 3, 6, 12, 11, 5, 10, 7, 14, 15, 13, 9, 1, 2
    };

    static constexpr std::array<std::uint8_t, 16> LOG = {
        0, 0, 1, 4, 2, 8, 5, 10, 3, 14, 9, 7, 6, 13, 11, 12
    };

    static constexpr std::uint8_t add(std::uint8_t a, std::uint8_t b) noexcept {
        return (a ^ b) & 0x0F;
    }

    static constexpr std::uint8_t sub(std::uint8_t a, std::uint8_t b) noexcept {
        return (a ^ b) & 0x0F;
    }

    static constexpr std::uint8_t mul(std::uint8_t a, std::uint8_t b) noexcept {
        a &= 0x0F;
        b &= 0x0F;
        if (a == 0 || b == 0) return 0;
        return EXP[(LOG[a] + LOG[b]) % 15];
    }

    static constexpr std::uint8_t div(std::uint8_t a, std::uint8_t b) noexcept {
        a &= 0x0F;
        b &= 0x0F;
        if (b == 0 || a == 0) return 0;
        return EXP[(LOG[a] + 15 - LOG[b]) % 15];
    }

    static constexpr std::uint8_t inverse(std::uint8_t a) noexcept {
        a &= 0x0F;
        if (a == 0) return 0;
        return EXP[(15 - LOG[a]) % 15];
    }

    static constexpr std::uint8_t alpha_pow(std::uint32_t power) noexcept {
        return EXP[power % 15];
    }
};

} // namespace zspar
