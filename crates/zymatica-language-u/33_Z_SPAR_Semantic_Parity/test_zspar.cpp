#include <zspar/zspar.hpp>

#include <array>
#include <chrono>
#include <cstdint>
#include <iomanip>
#include <iostream>
#include <random>
#include <sstream>
#include <string>
#include <vector>

using namespace zspar;

#define CHECK(cond) do { if (!(cond)) { std::cerr << "CHECK FAILED: " #cond << " at " << __FILE__ << ":" << __LINE__ << "\n"; std::exit(1); } } while (0)

static std::string hex(std::span<const std::uint8_t> bytes) {
    std::ostringstream oss;
    oss << std::hex << std::setfill('0');
    for (auto b : bytes) oss << std::setw(2) << static_cast<unsigned>(b);
    return oss.str();
}

template <std::size_t N>
static std::string hex(const std::array<std::uint8_t, N>& bytes) {
    return hex(std::span<const std::uint8_t>(bytes.data(), bytes.size()));
}

static Concept8D random_concept(std::mt19937_64& rng) {
    std::uniform_int_distribution<int> nib(0, 15);
    return Concept8D(
        static_cast<std::uint8_t>(nib(rng)), static_cast<std::uint8_t>(nib(rng)),
        static_cast<std::uint8_t>(nib(rng)), static_cast<std::uint8_t>(nib(rng)),
        static_cast<std::uint8_t>(nib(rng)), static_cast<std::uint8_t>(nib(rng)),
        static_cast<std::uint8_t>(nib(rng)), static_cast<std::uint8_t>(nib(rng)));
}

static void test_hash_and_crc() {
    const std::string abc = "abc";
    const auto d = Sha256::digest(std::span<const std::uint8_t>(
        reinterpret_cast<const std::uint8_t*>(abc.data()), abc.size()));
    CHECK(hex(d) == "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");

    const std::string digits = "123456789";
    const auto crc = crc32c(std::span<const std::uint8_t>(
        reinterpret_cast<const std::uint8_t*>(digits.data()), digits.size()));
    CHECK(crc == 0xE3069283U);
}

static void test_gf16_and_rs_golden() {
    for (std::uint8_t a = 1; a < 16; ++a) {
        CHECK(GF16::mul(a, GF16::inverse(a)) == 1);
    }

    const Rs12_8::Data data = {1,2,3,4,5,6,7,8};
    const auto cw = Rs12_8::encode(data);
    const Rs12_8::Codeword expected = {1,2,3,4,5,6,7,8,8,13,11,7};
    CHECK(cw == expected);
    CHECK(Rs12_8::valid_codeword(cw));
    const auto packed = Rs12_8::pack_codeword(cw);
    CHECK(hex(packed) == "123456788db7");
}

static void test_all_single_symbol_errors() {
    const Rs12_8::Data data = {15,0,7,12,3,9,1,14};
    const auto original = Rs12_8::encode(data);
    for (std::size_t pos = 0; pos < 12; ++pos) {
        for (std::uint8_t delta = 1; delta < 16; ++delta) {
            auto corrupted = original;
            corrupted[pos] ^= delta;
            const auto decoded = Rs12_8::decode(corrupted);
            CHECK(decoded.success());
            CHECK(decoded.codeword == original);
        }
    }
}

static void test_random_error_erasure_patterns() {
    std::mt19937_64 rng(0x5A504152ULL);
    std::uniform_int_distribution<int> pos_dist(0, 11);
    std::uniform_int_distribution<int> data_pos_dist(0, 7);
    std::uniform_int_distribution<int> delta_dist(1, 15);

    for (int round = 0; round < 3000; ++round) {
        const auto semantic_state = random_concept(rng);
        const auto original = Rs12_8::encode(semantic_state.symbols());
        auto corrupted = original;

        if ((round % 3) == 0) {
            // Two unknown errors: 2e = 4.
            int p1 = pos_dist(rng), p2 = pos_dist(rng);
            while (p2 == p1) p2 = pos_dist(rng);
            corrupted[static_cast<std::size_t>(p1)] ^= static_cast<std::uint8_t>(delta_dist(rng));
            corrupted[static_cast<std::size_t>(p2)] ^= static_cast<std::uint8_t>(delta_dist(rng));
            const auto decoded = Rs12_8::decode(corrupted);
            CHECK(decoded.success());
            CHECK(decoded.codeword == original);
        } else if ((round % 3) == 1) {
            // Four erasures: s = 4.
            std::vector<std::uint8_t> erasures;
            while (erasures.size() < 4) {
                const auto p = static_cast<std::uint8_t>(data_pos_dist(rng));
                if (std::find(erasures.begin(), erasures.end(), p) == erasures.end()) erasures.push_back(p);
            }
            for (const auto p : erasures) corrupted[p] = static_cast<std::uint8_t>(delta_dist(rng)); // ignored by decoder
            const auto decoded = Rs12_8::decode(corrupted, erasures);
            CHECK(decoded.success());
            CHECK(decoded.codeword == original);
        } else {
            // One error + two erasures: 2e+s = 4.
            std::vector<std::uint8_t> erasures;
            while (erasures.size() < 2) {
                const auto p = static_cast<std::uint8_t>(data_pos_dist(rng));
                if (std::find(erasures.begin(), erasures.end(), p) == erasures.end()) erasures.push_back(p);
            }
            int ep = pos_dist(rng);
            while (std::find(erasures.begin(), erasures.end(), ep) != erasures.end()) ep = pos_dist(rng);
            corrupted[static_cast<std::size_t>(ep)] ^= static_cast<std::uint8_t>(delta_dist(rng));
            for (const auto p : erasures) corrupted[p] = static_cast<std::uint8_t>(delta_dist(rng));
            const auto decoded = Rs12_8::decode(corrupted, erasures);
            CHECK(decoded.success());
            CHECK(decoded.codeword == original);
        }
    }
}

static InvariantSet make_invariants() {
    return InvariantSet({
        {InvariantKind::Entity, 1, static_cast<std::int64_t>(stable_text_id("VALVE-7"))},
        {InvariantKind::Quantity, 2, 50},
        {InvariantKind::Unit, 2, static_cast<std::int64_t>(stable_text_id("PSI"))},
        {InvariantKind::Negation, 9, 1},
    });
}

static void test_systematic_and_parity_frames() {
    const Concept8D authoritative(2, 5, 9, 14, 15, 3, 1, 12);
    const auto inv = make_invariants();
    constexpr std::uint64_t seq = 0x0102030405060708ULL;

    const auto sf = SystematicFrame::create(seq, authoritative, inv);
    const auto wire = sf.serialize();
    CHECK(wire.size() == 40);
    const auto parsed = SystematicFrame::parse(wire);
    CHECK(parsed.has_value());
    CHECK(parsed->state == authoritative);
    CHECK(parsed->verify_semantics(inv));

    auto damaged = wire;
    damaged[17] ^= 0x01;
    CHECK(!SystematicFrame::parse(damaged).has_value()); // CRC catches raw-wire corruption.

    const auto pf = ParityOnlyFrame::create(seq, authoritative, inv);
    const auto p_wire = pf.serialize();
    CHECK(p_wire.size() == 36);
    const auto p_parsed = ParityOnlyFrame::parse(p_wire);
    CHECK(p_parsed.has_value());

    // Different model reconstructed two semantic axes incorrectly.
    Concept8D predicted = authoritative;
    predicted.operation ^= 0x07;
    predicted.modality ^= 0x03;
    const auto repaired = p_parsed->repair_prediction(predicted, inv);
    CHECK(repaired.success());
    CHECK(repaired.state == authoritative);
    CHECK(repaired.corrected_axis_mask != 0);

    // Mixed confidence: two low-confidence erased axes + one actual wrong axis satisfies 2e+s = 4.
    predicted = authoritative;
    predicted.domain = 0;
    predicted.subdomain = 0;
    predicted.polarity ^= 0x04;
    const auto mixed = p_parsed->repair_prediction(predicted, inv, 0b00000011);
    CHECK(mixed.success());
    CHECK(mixed.state == authoritative);

    // Wrong critical invariant must fail even when the 8D concept repairs perfectly.
    auto wrong_inv = inv;
    wrong_inv.add({InvariantKind::Argument, 77, 99});
    const auto tag_fail = p_parsed->repair_prediction(authoritative, wrong_inv);
    CHECK(tag_fail.status == RepairResult::Status::SemanticTagMismatch);
}

static void test_out_of_radius_is_rejected_by_semantic_tag() {
    std::mt19937_64 rng(0x33445566778899ULL);
    const auto inv = make_invariants();
    std::uniform_int_distribution<int> data_pos(0, 7);
    std::uniform_int_distribution<int> delta(1, 15);

    for (int round = 0; round < 1000; ++round) {
        const auto authoritative = random_concept(rng);
        const auto frame = ParityOnlyFrame::create(static_cast<std::uint64_t>(round + 1), authoritative, inv);
        auto predicted = authoritative;
        auto s = predicted.symbols();
        std::array<int, 3> p{};
        p[0] = data_pos(rng);
        do { p[1] = data_pos(rng); } while (p[1] == p[0]);
        do { p[2] = data_pos(rng); } while (p[2] == p[0] || p[2] == p[1]);
        for (int i = 0; i < 3; ++i) s[static_cast<std::size_t>(p[static_cast<std::size_t>(i)])] ^= static_cast<std::uint8_t>(delta(rng));
        predicted = Concept8D::from_symbols(s);
        const auto r = frame.repair_prediction(predicted, inv);
        // >2 unknown errors are outside the profile. A decoder may find another codeword within
        // radius two, but the 128-bit semantic tag must never accept it as the original state.
        CHECK(!(r.success() && r.state != authoritative));
    }
}

static void test_repair_protocol_and_invariant_patch() {
    const Concept8D c(1,4,12,1,0,15,2,13);
    auto inv = make_invariants();
    const auto p = ParityOnlyFrame::create(42, c, inv);
    Concept8D bad = c;
    bad.operation ^= 1;
    bad.modality ^= 2;
    bad.strength ^= 4; // three unknown errors -> intentionally beyond 2-error radius

    const auto failed = p.repair_prediction(bad, inv);
    if (!failed.success()) {
        const auto rq = RepairRequest::from_failed_prediction(p, bad, 0);
        const auto rq_wire = rq.serialize();
        CHECK(rq_wire.size() == 41);

        const auto response = RepairResponse::create(42, c, inv);
        const auto rp_wire = response.serialize();
        CHECK(rp_wire.size() == 40);

        const auto patch = InvariantPatchFrame::create(42, c, inv);
        const auto patch_wire = patch.serialize();
        const auto parsed_patch = InvariantPatchFrame::parse(patch_wire);
        CHECK(parsed_patch.has_value());
        CHECK(parsed_patch->authoritative == c);
        CHECK(parsed_patch->invariants == inv);
    }
}

static void run_benchmark() {
    std::mt19937_64 rng(1234567);
    const auto inv = make_invariants();
    constexpr int N = 50000;
    int repaired = 0;
    const auto start = std::chrono::steady_clock::now();
    for (int i = 0; i < N; ++i) {
        const auto c = random_concept(rng);
        const auto frame = ParityOnlyFrame::create(static_cast<std::uint64_t>(i), c, inv);
        auto s = c.symbols();
        s[static_cast<std::size_t>((i * 3) % 8)] ^= static_cast<std::uint8_t>((i % 15) + 1);
        s[static_cast<std::size_t>((i * 5 + 1) % 8)] ^= static_cast<std::uint8_t>(((i + 7) % 15) + 1);
        // If both formulae hit same axis, this is a one-symbol error; still valid.
        const auto r = frame.repair_prediction(Concept8D::from_symbols(s), inv);
        if (r.success() && r.state == c) ++repaired;
    }
    const auto end = std::chrono::steady_clock::now();
    const double sec = std::chrono::duration<double>(end - start).count();
    std::cout << "BENCH parity-only repair: " << repaired << "/" << N
              << " exact, " << (N / sec) << " frames/sec\n";
    CHECK(repaired == N);
}

int main() {
    test_hash_and_crc();
    test_gf16_and_rs_golden();
    test_all_single_symbol_errors();
    test_random_error_erasure_patterns();
    test_systematic_and_parity_frames();
    test_out_of_radius_is_rejected_by_semantic_tag();
    test_repair_protocol_and_invariant_patch();
    run_benchmark();

    const Concept8D golden(1,4,12,1,0,15,2,13);
    const auto inv = make_invariants();
    const auto sf = SystematicFrame::create(0x0102030405060708ULL, golden, inv);
    const auto pf = ParityOnlyFrame::create(0x0102030405060708ULL, golden, inv);
    std::cout << "GOLDEN concept_dword=" << std::hex << std::setw(8) << std::setfill('0') << golden.to_dword() << "\n";
    std::cout << "GOLDEN parity=" << hex(Rs12_8::pack_parity(sf.parity)) << "\n";
    std::cout << "GOLDEN semantic_tag=" << hex(sf.tag) << "\n";
    std::cout << "GOLDEN systematic_frame=" << hex(sf.serialize()) << "\n";
    std::cout << "GOLDEN parity_frame=" << hex(pf.serialize()) << "\n";
    std::cout << "ALL Z-SPAR C++20 TESTS PASSED\n";
    return 0;
}
