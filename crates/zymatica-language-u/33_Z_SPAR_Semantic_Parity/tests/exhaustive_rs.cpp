#include <zspar/zspar.hpp>

#include <cstdint>
#include <cstdlib>
#include <iostream>
#include <vector>

using namespace zspar;

#define CHECK(cond) do { if (!(cond)) { std::cerr << "CHECK FAILED: " #cond << " at " << __FILE__ << ":" << __LINE__ << "\n"; std::exit(1); } } while (0)

int main() {
    const Rs12_8::Data data = {1, 4, 12, 1, 0, 15, 2, 13};
    const auto original = Rs12_8::encode(data);
    std::uint64_t cases = 0;

    // Exhaust every possible pair of symbol locations and every non-zero pair of GF(16) deltas.
    for (std::size_t p1 = 0; p1 < 12; ++p1) {
        for (std::size_t p2 = p1 + 1; p2 < 12; ++p2) {
            for (std::uint8_t d1 = 1; d1 < 16; ++d1) {
                for (std::uint8_t d2 = 1; d2 < 16; ++d2) {
                    auto damaged = original;
                    damaged[p1] ^= d1;
                    damaged[p2] ^= d2;
                    const auto decoded = Rs12_8::decode(damaged);
                    CHECK(decoded.success());
                    CHECK(decoded.codeword == original);
                    ++cases;
                }
            }
        }
    }

    // Exhaust every four-erasure location set. Erasure values can be arbitrary; use deterministic junk.
    for (std::size_t a = 0; a < 9; ++a) {
        for (std::size_t b = a + 1; b < 10; ++b) {
            for (std::size_t c = b + 1; c < 11; ++c) {
                for (std::size_t d = c + 1; d < 12; ++d) {
                    auto damaged = original;
                    const std::vector<std::uint8_t> erasures = {
                        static_cast<std::uint8_t>(a), static_cast<std::uint8_t>(b),
                        static_cast<std::uint8_t>(c), static_cast<std::uint8_t>(d)};
                    for (const auto p : erasures) damaged[p] = static_cast<std::uint8_t>((p * 7 + 3) & 0x0F);
                    const auto decoded = Rs12_8::decode(damaged, erasures);
                    CHECK(decoded.success());
                    CHECK(decoded.codeword == original);
                    ++cases;
                }
            }
        }
    }

    // Exhaust 1-error + 2-erasure positional patterns, with all 15 non-zero error magnitudes.
    for (std::size_t e1 = 0; e1 < 11; ++e1) {
        for (std::size_t e2 = e1 + 1; e2 < 12; ++e2) {
            const std::vector<std::uint8_t> erasures = {
                static_cast<std::uint8_t>(e1), static_cast<std::uint8_t>(e2)};
            for (std::size_t ep = 0; ep < 12; ++ep) {
                if (ep == e1 || ep == e2) continue;
                for (std::uint8_t delta = 1; delta < 16; ++delta) {
                    auto damaged = original;
                    damaged[e1] = static_cast<std::uint8_t>((e1 * 5 + 1) & 0x0F);
                    damaged[e2] = static_cast<std::uint8_t>((e2 * 5 + 1) & 0x0F);
                    damaged[ep] ^= delta;
                    const auto decoded = Rs12_8::decode(damaged, erasures);
                    CHECK(decoded.success());
                    CHECK(decoded.codeword == original);
                    ++cases;
                }
            }
        }
    }

    std::cout << "EXHAUSTIVE RS(12,8) BOUNDED-DISTANCE CASES PASSED: " << cases << "\n";
    return 0;
}
