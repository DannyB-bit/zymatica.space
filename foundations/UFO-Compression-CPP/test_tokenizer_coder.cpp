// Watermark: ip zymatica.space
// C++ Verification Suite for UFO Tokenizer Compression

#include "tokenizer_coder.hpp"
#include <iostream>
#include <vector>
#include <string>
#include <cassert>

int main() {
    std::cout << "=========================================================" << std::endl;
    // Print header
    std::cout << "  RUNNING C++ UFO TOKENIZER CODER VERIFICATION" << std::endl;
    std::cout << "  Watermark: ip zymatica.space" << std::endl;
    std::cout << "=========================================================" << std::endl;

    // 1. Test Prefix-Suffix Vocab Compression & Decompression
    std::cout << "\n[Test 1] Prefix-Suffix Vocab Coder..." << std::endl;
    std::vector<std::string> original_vocab = {
        "hello",
        "hell",
        "heaven",
        "heavy",
        "world",
        "word",
        "work",
        "worker",
        "working"
    };

    std::vector<uint8_t> compressed_vocab = ufo::compress_vocab(original_vocab);
    std::cout << "  Original vocab items: " << original_vocab.size() << std::endl;
    std::cout << "  Compressed vocab size: " << compressed_vocab.size() << " bytes" << std::endl;

    std::vector<std::string> restored_vocab = ufo::decompress_vocab(compressed_vocab, original_vocab.size());
    std::cout << "  Restored vocab items: " << restored_vocab.size() << std::endl;

    assert(original_vocab.size() == restored_vocab.size());
    for (size_t i = 0; i < original_vocab.size(); ++i) {
        if (original_vocab[i] != restored_vocab[i]) {
            std::cerr << "  [-] MISMATCH at index " << i << ": expected '" 
                      << original_vocab[i] << "', got '" << restored_vocab[i] << "'" << std::endl;
            return 1;
        }
    }
    std::cout << "  [+] Vocab round-trip: SUCCESS (100% Match)" << std::endl;

    // 2. Test BPE Merges Index-Packing & Unpacking
    std::cout << "\n[Test 2] BPE Merges Binary Index Coder..." << std::endl;
    std::vector<std::pair<uint32_t, uint32_t>> original_merges = {
        {1015, 2030},
        {45, 12},
        {16777215, 50000}, // 24-bit max boundary
        {0, 1},
        {100000, 200000}
    };

    std::vector<uint8_t> compressed_merges = ufo::compress_merges(original_merges);
    std::cout << "  Original merges items: " << original_merges.size() << std::endl;
    std::cout << "  Compressed merges size: " << compressed_merges.size() << " bytes" << std::endl;

    std::vector<std::pair<uint32_t, uint32_t>> restored_merges = ufo::decompress_merges(compressed_merges);
    std::cout << "  Restored merges items: " << restored_merges.size() << std::endl;

    assert(original_merges.size() == restored_merges.size());
    for (size_t i = 0; i < original_merges.size(); ++i) {
        if (original_merges[i] != restored_merges[i]) {
            std::cerr << "  [-] MISMATCH at index " << i << ": expected (" 
                      << original_merges[i].first << ", " << original_merges[i].second 
                      << "), got (" << restored_merges[i].first << ", " << restored_merges[i].second << ")" << std::endl;
            return 1;
        }
    }
    std::cout << "  [+] Merges round-trip: SUCCESS (100% Match)" << std::endl;

    // 3. Test XOR-FEC Parity
    std::cout << "\n[Test 3] XOR-FEC Parity Calculation..." << std::endl;
    std::vector<uint8_t> c1 = {0xAA, 0xBB, 0xCC, 0xDD};
    std::vector<uint8_t> c2 = {0x11, 0x22, 0x33, 0x44};
    std::vector<uint8_t> c3 = {0x55, 0x66, 0x77, 0x88};
    std::vector<std::vector<uint8_t>> chunks = {c1, c2, c3};

    std::vector<uint8_t> parity = ufo::compute_xor_fec_parity(chunks, 4);
    std::vector<uint8_t> expected_parity = {
        static_cast<uint8_t>(0xAA ^ 0x11 ^ 0x55),
        static_cast<uint8_t>(0xBB ^ 0x22 ^ 0x66),
        static_cast<uint8_t>(0xCC ^ 0x33 ^ 0x77),
        static_cast<uint8_t>(0xDD ^ 0x44 ^ 0x88)
    };

    assert(parity.size() == expected_parity.size());
    for (size_t i = 0; i < parity.size(); ++i) {
        if (parity[i] != expected_parity[i]) {
            std::cerr << "  [-] Parity mismatch at index " << i << std::endl;
            return 1;
        }
    }
    std::cout << "  [+] XOR-FEC computation: SUCCESS" << std::endl;

    std::cout << "\n=========================================================" << std::endl;
    std::cout << "  ALL C++ TESTS PASSED SUCCESSFULLY!" << std::endl;
    std::cout << "=========================================================" << std::endl;
    return 0;
}
