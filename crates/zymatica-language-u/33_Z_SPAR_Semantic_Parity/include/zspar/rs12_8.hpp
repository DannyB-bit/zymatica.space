#pragma once

#include "gf16.hpp"

#include <algorithm>
#include <array>
#include <cstddef>
#include <cstdint>
#include <optional>
#include <stdexcept>
#include <vector>

namespace zspar {

class Rs12_8 {
public:
    static constexpr std::size_t k_data_symbols = 8;
    static constexpr std::size_t k_parity_symbols = 4;
    static constexpr std::size_t k_code_symbols = 12;
    static constexpr std::size_t k_max_symbol_errors = 2;
    static constexpr std::size_t k_max_erasures = 4;

    using Data = std::array<std::uint8_t, k_data_symbols>;
    using Parity = std::array<std::uint8_t, k_parity_symbols>;
    using Codeword = std::array<std::uint8_t, k_code_symbols>;
    using Syndromes = std::array<std::uint8_t, k_parity_symbols>;

    enum class DecodeStatus : std::uint8_t {
        Clean = 0,
        Corrected = 1,
        Uncorrectable = 2,
        InvalidInput = 3,
    };

    struct DecodeResult {
        DecodeStatus status = DecodeStatus::InvalidInput;
        Codeword codeword{};
        Syndromes syndromes_before{};
        std::vector<std::uint8_t> corrected_positions;
        std::vector<std::uint8_t> erasure_positions;

        [[nodiscard]] bool success() const noexcept {
            return status == DecodeStatus::Clean || status == DecodeStatus::Corrected;
        }
    };

    // Generator polynomial g(x)=∏_{i=1..4}(x-α^i) for α=2 in GF(16).
    // Coefficients are stored from highest degree to constant term.
    static constexpr std::array<std::uint8_t, 5> generator() noexcept {
        return {1, 13, 12, 8, 7};
    }

    static Codeword encode(const Data& data) {
        validate_symbols(data);
        std::array<std::uint8_t, k_code_symbols> work{};
        for (std::size_t i = 0; i < k_data_symbols; ++i) {
            work[i] = data[i];
        }

        const auto g = generator();
        for (std::size_t i = 0; i < k_data_symbols; ++i) {
            const std::uint8_t coef = work[i];
            if (coef == 0) {
                continue;
            }
            for (std::size_t j = 1; j < g.size(); ++j) {
                work[i + j] = GF16::add(work[i + j], GF16::mul(g[j], coef));
            }
        }

        Codeword out{};
        for (std::size_t i = 0; i < k_data_symbols; ++i) {
            out[i] = data[i];
        }
        for (std::size_t i = 0; i < k_parity_symbols; ++i) {
            out[k_data_symbols + i] = work[k_data_symbols + i];
        }
        return out;
    }

    static Parity parity_for(const Data& data) {
        const auto cw = encode(data);
        Parity p{};
        std::copy_n(cw.begin() + static_cast<std::ptrdiff_t>(k_data_symbols), k_parity_symbols, p.begin());
        return p;
    }

    static Syndromes syndromes(const Codeword& codeword) {
        validate_symbols(codeword);
        Syndromes out{};
        for (std::size_t s = 0; s < k_parity_symbols; ++s) {
            const std::uint8_t x = GF16::alpha_pow(static_cast<std::uint32_t>(s + 1));
            std::uint8_t acc = 0;
            for (std::size_t i = 0; i < k_code_symbols; ++i) {
                acc = GF16::add(GF16::mul(acc, x), codeword[i]);
            }
            out[s] = acc;
        }
        return out;
    }

    static bool valid_codeword(const Codeword& codeword) {
        const auto s = syndromes(codeword);
        return std::all_of(s.begin(), s.end(), [](std::uint8_t v) { return v == 0; });
    }

    // Bounded-distance error+erasure decoder. Supports every pattern satisfying 2e+s <= 4,
    // where e is the number of unknown symbol errors and s is the number of known erasures.
    // The short n=12 profile uses a direct parity-check solve, which is deterministic and avoids
    // large generic decoder machinery on constrained edge nodes.
    static DecodeResult decode(Codeword received, const std::vector<std::uint8_t>& erasures = {}) {
        DecodeResult result{};
        result.codeword = received;

        if (!symbols_valid(received) || !erasures_valid(erasures)) {
            result.status = DecodeStatus::InvalidInput;
            return result;
        }

        result.erasure_positions = erasures;
        std::array<bool, k_code_symbols> erased{};
        for (const auto pos : erasures) {
            erased[pos] = true;
            received[pos] = 0; // unknown symbol becomes a zero placeholder; solver adds the missing delta/value.
        }

        result.syndromes_before = syndromes(received);
        if (erasures.empty() && all_zero(result.syndromes_before)) {
            result.status = DecodeStatus::Clean;
            result.codeword = received;
            return result;
        }

        const std::size_t max_errors = (k_parity_symbols - erasures.size()) / 2;

        // Try erasures-only first, then one error, then two errors as permitted by 2e+s<=4.
        for (std::size_t error_count = 0; error_count <= max_errors; ++error_count) {
            if (error_count == 0) {
                if (const auto candidate = try_unknown_positions(received, erasures, {}, erased)) {
                    result.status = DecodeStatus::Corrected;
                    result.codeword = candidate->first;
                    result.corrected_positions = candidate->second;
                    return result;
                }
            } else if (error_count == 1) {
                for (std::size_t p = 0; p < k_code_symbols; ++p) {
                    if (erased[p]) {
                        continue;
                    }
                    const std::vector<std::uint8_t> errors = {static_cast<std::uint8_t>(p)};
                    if (const auto candidate = try_unknown_positions(received, erasures, errors, erased)) {
                        result.status = DecodeStatus::Corrected;
                        result.codeword = candidate->first;
                        result.corrected_positions = candidate->second;
                        return result;
                    }
                }
            } else if (error_count == 2) {
                for (std::size_t p1 = 0; p1 < k_code_symbols; ++p1) {
                    if (erased[p1]) {
                        continue;
                    }
                    for (std::size_t p2 = p1 + 1; p2 < k_code_symbols; ++p2) {
                        if (erased[p2]) {
                            continue;
                        }
                        const std::vector<std::uint8_t> errors = {
                            static_cast<std::uint8_t>(p1), static_cast<std::uint8_t>(p2)};
                        if (const auto candidate = try_unknown_positions(received, erasures, errors, erased)) {
                            result.status = DecodeStatus::Corrected;
                            result.codeword = candidate->first;
                            result.corrected_positions = candidate->second;
                            return result;
                        }
                    }
                }
            }
        }

        result.status = DecodeStatus::Uncorrectable;
        result.codeword = received;
        return result;
    }

    static std::array<std::uint8_t, 6> pack_codeword(const Codeword& codeword) {
        validate_symbols(codeword);
        std::array<std::uint8_t, 6> out{};
        for (std::size_t i = 0; i < out.size(); ++i) {
            out[i] = static_cast<std::uint8_t>((codeword[i * 2] << 4U) | codeword[i * 2 + 1]);
        }
        return out;
    }

    static Codeword unpack_codeword(const std::array<std::uint8_t, 6>& bytes) noexcept {
        Codeword out{};
        for (std::size_t i = 0; i < bytes.size(); ++i) {
            out[i * 2] = static_cast<std::uint8_t>((bytes[i] >> 4U) & 0x0F);
            out[i * 2 + 1] = static_cast<std::uint8_t>(bytes[i] & 0x0F);
        }
        return out;
    }

    static std::array<std::uint8_t, 2> pack_parity(const Parity& parity) {
        validate_symbols(parity);
        return {
            static_cast<std::uint8_t>((parity[0] << 4U) | parity[1]),
            static_cast<std::uint8_t>((parity[2] << 4U) | parity[3]),
        };
    }

    static Parity unpack_parity(const std::array<std::uint8_t, 2>& bytes) noexcept {
        return {
            static_cast<std::uint8_t>((bytes[0] >> 4U) & 0x0F),
            static_cast<std::uint8_t>(bytes[0] & 0x0F),
            static_cast<std::uint8_t>((bytes[1] >> 4U) & 0x0F),
            static_cast<std::uint8_t>(bytes[1] & 0x0F),
        };
    }

private:
    template <typename Container>
    static bool symbols_valid(const Container& c) noexcept {
        return std::all_of(c.begin(), c.end(), [](std::uint8_t v) { return v < 16; });
    }

    template <typename Container>
    static void validate_symbols(const Container& c) {
        if (!symbols_valid(c)) {
            throw std::invalid_argument("RS(12,8) symbols must be GF(16) nibbles in [0,15]");
        }
    }

    static bool erasures_valid(const std::vector<std::uint8_t>& erasures) noexcept {
        if (erasures.size() > k_max_erasures) {
            return false;
        }
        std::array<bool, k_code_symbols> seen{};
        for (const auto p : erasures) {
            if (p >= k_code_symbols || seen[p]) {
                return false;
            }
            seen[p] = true;
        }
        return true;
    }

    static bool all_zero(const Syndromes& s) noexcept {
        return std::all_of(s.begin(), s.end(), [](std::uint8_t v) { return v == 0; });
    }

    // H[row][position] is the coefficient of codeword[position] in c(alpha^(row+1)).
    static std::uint8_t parity_check_coefficient(std::size_t row, std::size_t position) noexcept {
        const std::uint32_t root_power = static_cast<std::uint32_t>(row + 1);
        const std::uint32_t polynomial_power = static_cast<std::uint32_t>(k_code_symbols - 1 - position);
        return GF16::alpha_pow(root_power * polynomial_power);
    }

    // Solve A*x=b over GF(16). A has exactly 4 equations and m<=4 unknowns.
    static std::optional<std::vector<std::uint8_t>> solve_unknowns(
        const std::vector<std::uint8_t>& unknown_positions,
        const Syndromes& target) {

        const std::size_t m = unknown_positions.size();
        if (m == 0) {
            if (all_zero(target)) {
                return std::vector<std::uint8_t>{};
            }
            return std::nullopt;
        }
        if (m > k_parity_symbols) {
            return std::nullopt;
        }

        // 4 rows x (m coefficients + RHS), fixed maximum 5 columns.
        std::array<std::array<std::uint8_t, 5>, 4> mat{};
        for (std::size_t row = 0; row < 4; ++row) {
            for (std::size_t col = 0; col < m; ++col) {
                mat[row][col] = parity_check_coefficient(row, unknown_positions[col]);
            }
            mat[row][m] = target[row];
        }

        std::array<int, 4> pivot_row_for_col{};
        pivot_row_for_col.fill(-1);
        std::size_t pivot_row = 0;

        for (std::size_t col = 0; col < m && pivot_row < 4; ++col) {
            std::size_t found = pivot_row;
            while (found < 4 && mat[found][col] == 0) {
                ++found;
            }
            if (found == 4) {
                return std::nullopt; // rank deficient -> no unique correction for this position set
            }
            if (found != pivot_row) {
                std::swap(mat[found], mat[pivot_row]);
            }

            const std::uint8_t inv = GF16::inverse(mat[pivot_row][col]);
            for (std::size_t j = col; j <= m; ++j) {
                mat[pivot_row][j] = GF16::mul(mat[pivot_row][j], inv);
            }

            for (std::size_t row = 0; row < 4; ++row) {
                if (row == pivot_row || mat[row][col] == 0) {
                    continue;
                }
                const std::uint8_t factor = mat[row][col];
                for (std::size_t j = col; j <= m; ++j) {
                    mat[row][j] = GF16::sub(mat[row][j], GF16::mul(factor, mat[pivot_row][j]));
                }
            }

            pivot_row_for_col[col] = static_cast<int>(pivot_row);
            ++pivot_row;
        }

        if (pivot_row < m) {
            return std::nullopt;
        }

        // Any all-zero coefficient row with a nonzero RHS indicates inconsistency.
        for (std::size_t row = 0; row < 4; ++row) {
            bool any_coeff = false;
            for (std::size_t col = 0; col < m; ++col) {
                any_coeff = any_coeff || (mat[row][col] != 0);
            }
            if (!any_coeff && mat[row][m] != 0) {
                return std::nullopt;
            }
        }

        std::vector<std::uint8_t> x(m, 0);
        for (std::size_t col = 0; col < m; ++col) {
            const int row = pivot_row_for_col[col];
            if (row < 0) {
                return std::nullopt;
            }
            x[col] = mat[static_cast<std::size_t>(row)][m];
        }

        // Validate all four original equations, including overdetermined rows.
        for (std::size_t row = 0; row < 4; ++row) {
            std::uint8_t lhs = 0;
            for (std::size_t col = 0; col < m; ++col) {
                lhs = GF16::add(lhs, GF16::mul(
                    parity_check_coefficient(row, unknown_positions[col]), x[col]));
            }
            if (lhs != target[row]) {
                return std::nullopt;
            }
        }

        return x;
    }

    static std::optional<std::pair<Codeword, std::vector<std::uint8_t>>> try_unknown_positions(
        const Codeword& base,
        const std::vector<std::uint8_t>& erasures,
        const std::vector<std::uint8_t>& error_positions,
        const std::array<bool, k_code_symbols>& erased) {

        std::vector<std::uint8_t> unknowns = erasures;
        unknowns.insert(unknowns.end(), error_positions.begin(), error_positions.end());
        if (unknowns.empty()) {
            if (valid_codeword(base)) {
                return std::make_pair(base, std::vector<std::uint8_t>{});
            }
            return std::nullopt;
        }
        if (unknowns.size() > 4) {
            return std::nullopt;
        }

        const auto s = syndromes(base);
        const auto solution = solve_unknowns(unknowns, s);
        if (!solution) {
            return std::nullopt;
        }

        Codeword corrected = base;
        std::vector<std::uint8_t> changed;
        for (std::size_t i = 0; i < unknowns.size(); ++i) {
            const auto pos = unknowns[i];
            const auto delta = (*solution)[i];
            const bool is_erasure = erased[pos];
            if (!is_erasure && delta == 0) {
                return std::nullopt; // hypothesized error position must actually differ
            }
            corrected[pos] = GF16::add(corrected[pos], delta);
            if (delta != 0 || is_erasure) {
                changed.push_back(pos);
            }
        }

        if (!valid_codeword(corrected)) {
            return std::nullopt;
        }
        return std::make_pair(corrected, changed);
    }
};

} // namespace zspar
