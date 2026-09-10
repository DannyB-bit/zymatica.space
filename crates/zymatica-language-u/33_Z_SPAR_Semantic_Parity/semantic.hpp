#pragma once

#include "crc32c.hpp"
#include "rs12_8.hpp"
#include "sha256.hpp"

#include <algorithm>
#include <array>
#include <cstddef>
#include <cstdint>
#include <cstring>
#include <optional>
#include <span>
#include <stdexcept>
#include <string>
#include <string_view>
#include <utility>
#include <vector>

namespace zspar {

struct Concept8D {
    std::uint8_t domain = 0;
    std::uint8_t subdomain = 0;
    std::uint8_t operation = 0;
    std::uint8_t modality = 0;
    std::uint8_t strength = 0;
    std::uint8_t polarity = 0;
    std::uint8_t temporal_horizon = 0;
    std::uint8_t epistemic_certainty = 0;

    constexpr Concept8D() = default;

    constexpr Concept8D(
        std::uint8_t d, std::uint8_t sd, std::uint8_t op, std::uint8_t mod,
        std::uint8_t st, std::uint8_t pol, std::uint8_t temp, std::uint8_t cert)
        : domain(d & 0x0F), subdomain(sd & 0x0F), operation(op & 0x0F),
          modality(mod & 0x0F), strength(st & 0x0F), polarity(pol & 0x0F),
          temporal_horizon(temp & 0x0F), epistemic_certainty(cert & 0x0F) {}

    [[nodiscard]] constexpr Rs12_8::Data symbols() const noexcept {
        return {domain, subdomain, operation, modality, strength, polarity,
                temporal_horizon, epistemic_certainty};
    }

    [[nodiscard]] static constexpr Concept8D from_symbols(const Rs12_8::Data& s) noexcept {
        return Concept8D{s[0], s[1], s[2], s[3], s[4], s[5], s[6], s[7]};
    }

    [[nodiscard]] constexpr std::uint32_t to_dword() const noexcept {
        return (static_cast<std::uint32_t>(domain) << 28U) |
               (static_cast<std::uint32_t>(subdomain) << 24U) |
               (static_cast<std::uint32_t>(operation) << 20U) |
               (static_cast<std::uint32_t>(modality) << 16U) |
               (static_cast<std::uint32_t>(strength) << 12U) |
               (static_cast<std::uint32_t>(polarity) << 8U) |
               (static_cast<std::uint32_t>(temporal_horizon) << 4U) |
               static_cast<std::uint32_t>(epistemic_certainty);
    }

    [[nodiscard]] static constexpr Concept8D from_dword(std::uint32_t dword) noexcept {
        return Concept8D{
            static_cast<std::uint8_t>((dword >> 28U) & 0x0F),
            static_cast<std::uint8_t>((dword >> 24U) & 0x0F),
            static_cast<std::uint8_t>((dword >> 20U) & 0x0F),
            static_cast<std::uint8_t>((dword >> 16U) & 0x0F),
            static_cast<std::uint8_t>((dword >> 12U) & 0x0F),
            static_cast<std::uint8_t>((dword >> 8U) & 0x0F),
            static_cast<std::uint8_t>((dword >> 4U) & 0x0F),
            static_cast<std::uint8_t>(dword & 0x0F),
        };
    }

    friend constexpr bool operator==(const Concept8D&, const Concept8D&) = default;
};

enum class InvariantKind : std::uint8_t {
    Entity = 1,
    Quantity = 2,
    Unit = 3,
    Negation = 4,
    Tool = 5,
    Argument = 6,
    Policy = 7,
    Custom = 255,
};

struct InvariantRecord {
    InvariantKind kind = InvariantKind::Custom;
    std::uint16_t key = 0;
    std::int64_t value = 0;

    friend bool operator==(const InvariantRecord&, const InvariantRecord&) = default;
};

class InvariantSet {
public:
    static constexpr std::size_t k_max_records = 32;

    InvariantSet() = default;
    explicit InvariantSet(std::vector<InvariantRecord> records) : records_(std::move(records)) {
        if (records_.size() > k_max_records) {
            throw std::invalid_argument("too many semantic invariant records");
        }
        canonicalize();
    }

    void add(InvariantRecord record) {
        if (records_.size() >= k_max_records) {
            throw std::length_error("semantic invariant record limit exceeded");
        }
        records_.push_back(record);
        canonicalize();
    }

    [[nodiscard]] const std::vector<InvariantRecord>& records() const noexcept { return records_; }
    [[nodiscard]] bool empty() const noexcept { return records_.empty(); }

    [[nodiscard]] std::vector<std::uint8_t> canonical_bytes() const {
        std::vector<std::uint8_t> out;
        out.reserve(1 + records_.size() * 11);
        out.push_back(static_cast<std::uint8_t>(records_.size()));
        for (const auto& r : records_) {
            out.push_back(static_cast<std::uint8_t>(r.kind));
            append_u16(out, r.key);
            append_i64(out, r.value);
        }
        return out;
    }

    [[nodiscard]] static std::optional<InvariantSet> parse_canonical(std::span<const std::uint8_t> bytes) {
        if (bytes.empty()) {
            return std::nullopt;
        }
        const std::size_t count = bytes[0];
        if (count > k_max_records || bytes.size() != 1 + count * 11) {
            return std::nullopt;
        }
        std::vector<InvariantRecord> records;
        records.reserve(count);
        std::size_t idx = 1;
        for (std::size_t i = 0; i < count; ++i) {
            const auto kind = static_cast<InvariantKind>(bytes[idx++]);
            const std::uint16_t key = read_u16(bytes, idx);
            idx += 2;
            const std::int64_t value = read_i64(bytes, idx);
            idx += 8;
            records.push_back({kind, key, value});
        }
        return InvariantSet(std::move(records));
    }

    friend bool operator==(const InvariantSet& a, const InvariantSet& b) {
        return a.records_ == b.records_;
    }

private:
    static void append_u16(std::vector<std::uint8_t>& out, std::uint16_t v) {
        out.push_back(static_cast<std::uint8_t>((v >> 8U) & 0xFF));
        out.push_back(static_cast<std::uint8_t>(v & 0xFF));
    }

    static void append_i64(std::vector<std::uint8_t>& out, std::int64_t v) {
        const auto u = static_cast<std::uint64_t>(v);
        for (int i = 7; i >= 0; --i) {
            out.push_back(static_cast<std::uint8_t>((u >> (i * 8)) & 0xFFU));
        }
    }

    static std::uint16_t read_u16(std::span<const std::uint8_t> bytes, std::size_t idx) {
        return static_cast<std::uint16_t>((static_cast<std::uint16_t>(bytes[idx]) << 8U) | bytes[idx + 1]);
    }

    static std::int64_t read_i64(std::span<const std::uint8_t> bytes, std::size_t idx) {
        std::uint64_t v = 0;
        for (std::size_t i = 0; i < 8; ++i) {
            v = (v << 8U) | bytes[idx + i];
        }
        return static_cast<std::int64_t>(v);
    }

    void canonicalize() {
        std::sort(records_.begin(), records_.end(), [](const auto& a, const auto& b) {
            if (a.kind != b.kind) return static_cast<std::uint8_t>(a.kind) < static_cast<std::uint8_t>(b.kind);
            if (a.key != b.key) return a.key < b.key;
            return a.value < b.value;
        });
    }

    std::vector<InvariantRecord> records_;
};

inline std::uint64_t stable_text_id(std::string_view text) {
    const auto digest = Sha256::digest(std::span<const std::uint8_t>(
        reinterpret_cast<const std::uint8_t*>(text.data()), text.size()));
    std::uint64_t out = 0;
    for (std::size_t i = 0; i < 8; ++i) {
        out = (out << 8U) | digest[i];
    }
    return out;
}

namespace detail {

inline void append_u32(std::vector<std::uint8_t>& out, std::uint32_t v) {
    for (int i = 3; i >= 0; --i) out.push_back(static_cast<std::uint8_t>((v >> (i * 8)) & 0xFFU));
}
inline void append_u64(std::vector<std::uint8_t>& out, std::uint64_t v) {
    for (int i = 7; i >= 0; --i) out.push_back(static_cast<std::uint8_t>((v >> (i * 8)) & 0xFFU));
}
inline std::uint32_t read_u32(std::span<const std::uint8_t> b, std::size_t i) {
    return (static_cast<std::uint32_t>(b[i]) << 24U) |
           (static_cast<std::uint32_t>(b[i + 1]) << 16U) |
           (static_cast<std::uint32_t>(b[i + 2]) << 8U) |
           static_cast<std::uint32_t>(b[i + 3]);
}
inline std::uint64_t read_u64(std::span<const std::uint8_t> b, std::size_t i) {
    std::uint64_t v = 0;
    for (std::size_t j = 0; j < 8; ++j) v = (v << 8U) | b[i + j];
    return v;
}
inline void append_crc(std::vector<std::uint8_t>& bytes) {
    const std::uint32_t crc = crc32c(bytes);
    append_u32(bytes, crc);
}
inline bool verify_crc(std::span<const std::uint8_t> bytes) {
    if (bytes.size() < 4) return false;
    const auto expected = read_u32(bytes, bytes.size() - 4);
    const auto actual = crc32c(bytes.first(bytes.size() - 4));
    return expected == actual;
}

} // namespace detail

inline std::array<std::uint8_t, 16> semantic_tag(
    std::uint64_t sequence, const Concept8D& state, const InvariantSet& invariants) {

    static constexpr std::array<std::uint8_t, 17> domain = {
        'Z','S','P','A','R','-','S','E','M','A','N','T','I','C','-','V','1'};
    std::vector<std::uint8_t> material;
    const auto inv = invariants.canonical_bytes();
    material.reserve(domain.size() + 8 + 4 + inv.size());
    material.insert(material.end(), domain.begin(), domain.end());
    detail::append_u64(material, sequence);
    detail::append_u32(material, state.to_dword());
    material.insert(material.end(), inv.begin(), inv.end());
    return sha256_128(material);
}

inline std::uint8_t axis_diff_mask(const Concept8D& a, const Concept8D& b) noexcept {
    const auto x = a.symbols();
    const auto y = b.symbols();
    std::uint8_t mask = 0;
    for (std::size_t i = 0; i < 8; ++i) {
        if (x[i] != y[i]) mask |= static_cast<std::uint8_t>(1U << i);
    }
    return mask;
}

struct RepairResult {
    enum class Status : std::uint8_t {
        Clean,
        Corrected,
        Uncorrectable,
        SemanticTagMismatch,
        InvalidFrame,
    };

    Status status = Status::InvalidFrame;
    Concept8D state{};
    std::uint8_t corrected_axis_mask = 0;
    Rs12_8::Syndromes syndromes{};

    [[nodiscard]] bool success() const noexcept {
        return status == Status::Clean || status == Status::Corrected;
    }
};

struct SystematicFrame {
    static constexpr std::array<std::uint8_t, 4> k_magic = {'Z','S','P','S'};
    static constexpr std::uint8_t k_version = 1;
    static constexpr std::size_t k_wire_size = 40;

    std::uint64_t sequence = 0;
    Concept8D state{};
    Rs12_8::Parity parity{};
    std::array<std::uint8_t, 16> tag{};
    bool has_invariants = false;

    static SystematicFrame create(std::uint64_t seq, const Concept8D& c, const InvariantSet& inv = {}) {
        SystematicFrame f;
        f.sequence = seq;
        f.state = c;
        f.parity = Rs12_8::parity_for(c.symbols());
        f.tag = semantic_tag(seq, c, inv);
        f.has_invariants = !inv.empty();
        return f;
    }

    [[nodiscard]] std::array<std::uint8_t, k_wire_size> serialize() const {
        std::vector<std::uint8_t> out;
        out.reserve(k_wire_size);
        out.insert(out.end(), k_magic.begin(), k_magic.end());
        out.push_back(k_version);
        out.push_back(has_invariants ? 0x01U : 0x00U);
        detail::append_u64(out, sequence);
        detail::append_u32(out, state.to_dword());
        const auto p = Rs12_8::pack_parity(parity);
        out.insert(out.end(), p.begin(), p.end());
        out.insert(out.end(), tag.begin(), tag.end());
        detail::append_crc(out);
        if (out.size() != k_wire_size) throw std::logic_error("systematic frame size invariant failed");
        std::array<std::uint8_t, k_wire_size> arr{};
        std::copy(out.begin(), out.end(), arr.begin());
        return arr;
    }

    static std::optional<SystematicFrame> parse(std::span<const std::uint8_t> bytes) {
        if (bytes.size() != k_wire_size || !detail::verify_crc(bytes)) return std::nullopt;
        if (!std::equal(k_magic.begin(), k_magic.end(), bytes.begin()) || bytes[4] != k_version) return std::nullopt;
        SystematicFrame f;
        f.has_invariants = (bytes[5] & 0x01U) != 0;
        f.sequence = detail::read_u64(bytes, 6);
        f.state = Concept8D::from_dword(detail::read_u32(bytes, 14));
        f.parity = Rs12_8::unpack_parity({bytes[18], bytes[19]});
        std::copy_n(bytes.begin() + 20, 16, f.tag.begin());
        return f;
    }

    [[nodiscard]] bool verify_semantics(const InvariantSet& inv = {}) const {
        return semantic_tag(sequence, state, inv) == tag;
    }
};

struct ParityOnlyFrame {
    static constexpr std::array<std::uint8_t, 4> k_magic = {'Z','S','P','P'};
    static constexpr std::uint8_t k_version = 1;
    static constexpr std::size_t k_wire_size = 36;

    std::uint64_t sequence = 0;
    Rs12_8::Parity parity{};
    std::array<std::uint8_t, 16> tag{};
    bool has_invariants = false;

    static ParityOnlyFrame create(std::uint64_t seq, const Concept8D& authoritative, const InvariantSet& inv = {}) {
        ParityOnlyFrame f;
        f.sequence = seq;
        f.parity = Rs12_8::parity_for(authoritative.symbols());
        f.tag = semantic_tag(seq, authoritative, inv);
        f.has_invariants = !inv.empty();
        return f;
    }

    [[nodiscard]] std::array<std::uint8_t, k_wire_size> serialize() const {
        std::vector<std::uint8_t> out;
        out.reserve(k_wire_size);
        out.insert(out.end(), k_magic.begin(), k_magic.end());
        out.push_back(k_version);
        out.push_back(has_invariants ? 0x01U : 0x00U);
        detail::append_u64(out, sequence);
        const auto p = Rs12_8::pack_parity(parity);
        out.insert(out.end(), p.begin(), p.end());
        out.insert(out.end(), tag.begin(), tag.end());
        detail::append_crc(out);
        if (out.size() != k_wire_size) throw std::logic_error("parity-only frame size invariant failed");
        std::array<std::uint8_t, k_wire_size> arr{};
        std::copy(out.begin(), out.end(), arr.begin());
        return arr;
    }

    static std::optional<ParityOnlyFrame> parse(std::span<const std::uint8_t> bytes) {
        if (bytes.size() != k_wire_size || !detail::verify_crc(bytes)) return std::nullopt;
        if (!std::equal(k_magic.begin(), k_magic.end(), bytes.begin()) || bytes[4] != k_version) return std::nullopt;
        ParityOnlyFrame f;
        f.has_invariants = (bytes[5] & 0x01U) != 0;
        f.sequence = detail::read_u64(bytes, 6);
        f.parity = Rs12_8::unpack_parity({bytes[14], bytes[15]});
        std::copy_n(bytes.begin() + 16, 16, f.tag.begin());
        return f;
    }

    // Repair a locally predicted/reconstructed semantic state using only the two-byte RS parity
    // carried by this frame. Low-confidence axes can be marked as erasures in erasure_axis_mask.
    // The decoder supports 2*errors + erasures <= 4. A 128-bit semantic tag then rejects any
    // out-of-radius miscorrection and also binds critical invariants reconstructed by the receiver.
    [[nodiscard]] RepairResult repair_prediction(
        const Concept8D& predicted,
        const InvariantSet& predicted_invariants = {},
        std::uint8_t erasure_axis_mask = 0) const {

        Rs12_8::Codeword received{};
        const auto data = predicted.symbols();
        std::copy(data.begin(), data.end(), received.begin());
        for (std::size_t i = 0; i < 4; ++i) received[8 + i] = parity[i];

        std::vector<std::uint8_t> erasures;
        for (std::size_t axis = 0; axis < 8; ++axis) {
            if ((erasure_axis_mask & (1U << axis)) != 0U) {
                erasures.push_back(static_cast<std::uint8_t>(axis));
            }
        }

        const auto decoded = Rs12_8::decode(received, erasures);
        RepairResult out;
        out.syndromes = decoded.syndromes_before;
        if (!decoded.success()) {
            out.status = RepairResult::Status::Uncorrectable;
            out.state = predicted;
            return out;
        }

        Rs12_8::Data corrected_data{};
        std::copy_n(decoded.codeword.begin(), 8, corrected_data.begin());
        const Concept8D corrected = Concept8D::from_symbols(corrected_data);
        out.state = corrected;
        out.corrected_axis_mask = axis_diff_mask(predicted, corrected);

        if (semantic_tag(sequence, corrected, predicted_invariants) != tag) {
            out.status = RepairResult::Status::SemanticTagMismatch;
            return out;
        }

        out.status = (corrected == predicted && erasure_axis_mask == 0)
            ? RepairResult::Status::Clean
            : RepairResult::Status::Corrected;
        return out;
    }
};

struct RepairRequest {
    static constexpr std::array<std::uint8_t, 4> k_magic = {'Z','S','R','Q'};
    static constexpr std::uint8_t k_version = 1;
    static constexpr std::size_t k_wire_size = 41;

    std::uint64_t sequence = 0;
    Concept8D predicted{};
    std::uint8_t erasure_axis_mask = 0;
    Rs12_8::Syndromes syndromes{};
    std::array<std::uint8_t, 16> expected_tag{};

    [[nodiscard]] std::array<std::uint8_t, k_wire_size> serialize() const {
        std::vector<std::uint8_t> out;
        out.reserve(k_wire_size);
        out.insert(out.end(), k_magic.begin(), k_magic.end());
        out.push_back(k_version);
        out.push_back(0);
        detail::append_u64(out, sequence);
        detail::append_u32(out, predicted.to_dword());
        out.push_back(erasure_axis_mask);
        out.push_back(static_cast<std::uint8_t>((syndromes[0] << 4U) | syndromes[1]));
        out.push_back(static_cast<std::uint8_t>((syndromes[2] << 4U) | syndromes[3]));
        out.insert(out.end(), expected_tag.begin(), expected_tag.end());
        detail::append_crc(out);
        if (out.size() != k_wire_size) throw std::logic_error("repair request size invariant failed");
        std::array<std::uint8_t, k_wire_size> arr{};
        std::copy(out.begin(), out.end(), arr.begin());
        return arr;
    }

    static RepairRequest from_failed_prediction(
        const ParityOnlyFrame& frame, const Concept8D& predicted, std::uint8_t erasure_axis_mask) {
        Rs12_8::Codeword received{};
        const auto d = predicted.symbols();
        std::copy(d.begin(), d.end(), received.begin());
        for (std::size_t i = 0; i < 4; ++i) received[8 + i] = frame.parity[i];
        RepairRequest r;
        r.sequence = frame.sequence;
        r.predicted = predicted;
        r.erasure_axis_mask = erasure_axis_mask;
        r.syndromes = Rs12_8::syndromes(received);
        r.expected_tag = frame.tag;
        return r;
    }
};

struct RepairResponse {
    static constexpr std::array<std::uint8_t, 4> k_magic = {'Z','S','R','P'};
    static constexpr std::uint8_t k_version = 1;
    static constexpr std::size_t k_wire_size = 40;

    std::uint64_t sequence = 0;
    Concept8D authoritative{};
    Rs12_8::Parity parity{};
    std::array<std::uint8_t, 16> tag{};
    bool has_invariants = false;

    static RepairResponse create(std::uint64_t seq, const Concept8D& c, const InvariantSet& inv = {}) {
        RepairResponse r;
        r.sequence = seq;
        r.authoritative = c;
        r.parity = Rs12_8::parity_for(c.symbols());
        r.tag = semantic_tag(seq, c, inv);
        r.has_invariants = !inv.empty();
        return r;
    }

    [[nodiscard]] std::array<std::uint8_t, k_wire_size> serialize() const {
        std::vector<std::uint8_t> out;
        out.reserve(k_wire_size);
        out.insert(out.end(), k_magic.begin(), k_magic.end());
        out.push_back(k_version);
        out.push_back(has_invariants ? 0x01U : 0x00U);
        detail::append_u64(out, sequence);
        detail::append_u32(out, authoritative.to_dword());
        const auto p = Rs12_8::pack_parity(parity);
        out.insert(out.end(), p.begin(), p.end());
        out.insert(out.end(), tag.begin(), tag.end());
        detail::append_crc(out);
        if (out.size() != k_wire_size) throw std::logic_error("repair response size invariant failed");
        std::array<std::uint8_t, k_wire_size> arr{};
        std::copy(out.begin(), out.end(), arr.begin());
        return arr;
    }
};

struct InvariantPatchFrame {
    static constexpr std::array<std::uint8_t, 4> k_magic = {'Z','S','I','P'};
    static constexpr std::uint8_t k_version = 1;

    std::uint64_t sequence = 0;
    Concept8D authoritative{};
    InvariantSet invariants{};
    std::array<std::uint8_t, 16> tag{};

    static InvariantPatchFrame create(std::uint64_t seq, const Concept8D& c, InvariantSet inv) {
        InvariantPatchFrame f;
        f.sequence = seq;
        f.authoritative = c;
        f.invariants = std::move(inv);
        f.tag = semantic_tag(seq, c, f.invariants);
        return f;
    }

    [[nodiscard]] std::vector<std::uint8_t> serialize() const {
        const auto inv = invariants.canonical_bytes();
        std::vector<std::uint8_t> out;
        out.reserve(4 + 1 + 1 + 8 + 4 + inv.size() + 16 + 4);
        out.insert(out.end(), k_magic.begin(), k_magic.end());
        out.push_back(k_version);
        out.push_back(0);
        detail::append_u64(out, sequence);
        detail::append_u32(out, authoritative.to_dword());
        out.insert(out.end(), inv.begin(), inv.end());
        out.insert(out.end(), tag.begin(), tag.end());
        detail::append_crc(out);
        return out;
    }

    static std::optional<InvariantPatchFrame> parse(std::span<const std::uint8_t> bytes) {
        if (bytes.size() < 39 || !detail::verify_crc(bytes)) return std::nullopt;
        if (!std::equal(k_magic.begin(), k_magic.end(), bytes.begin()) || bytes[4] != k_version) return std::nullopt;
        const std::size_t inv_start = 18;
        if (bytes.size() < inv_start + 1 + 16 + 4) return std::nullopt;
        const std::size_t count = bytes[inv_start];
        const std::size_t inv_len = 1 + count * 11;
        const std::size_t expected_size = inv_start + inv_len + 16 + 4;
        if (count > InvariantSet::k_max_records || bytes.size() != expected_size) return std::nullopt;
        const auto inv = InvariantSet::parse_canonical(bytes.subspan(inv_start, inv_len));
        if (!inv) return std::nullopt;
        InvariantPatchFrame f;
        f.sequence = detail::read_u64(bytes, 6);
        f.authoritative = Concept8D::from_dword(detail::read_u32(bytes, 14));
        f.invariants = *inv;
        std::copy_n(bytes.begin() + static_cast<std::ptrdiff_t>(inv_start + inv_len), 16, f.tag.begin());
        if (semantic_tag(f.sequence, f.authoritative, f.invariants) != f.tag) return std::nullopt;
        return f;
    }
};

} // namespace zspar
