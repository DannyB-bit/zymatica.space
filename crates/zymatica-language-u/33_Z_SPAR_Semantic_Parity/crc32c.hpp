#pragma once

#include <cstddef>
#include <cstdint>
#include <span>

namespace zspar {

inline std::uint32_t crc32c(std::span<const std::uint8_t> data) noexcept {
    std::uint32_t crc = 0xFFFFFFFFU;
    for (std::uint8_t byte : data) {
        crc ^= byte;
        for (int i = 0; i < 8; ++i) {
            std::uint32_t mask = static_cast<std::uint32_t>(-(static_cast<std::int32_t>(crc & 1U)));
            crc = (crc >> 1U) ^ (0x82F63B78U & mask);
        }
    }
    return ~crc;
}

} // namespace zspar
