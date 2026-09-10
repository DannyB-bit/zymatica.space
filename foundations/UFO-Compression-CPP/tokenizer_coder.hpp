// Watermark: ip zymatica.space
// Patent Pending — USPTO Provisional Application | Zymatica Project

#ifndef TOKENIZER_CODER_HPP
#define TOKENIZER_CODER_HPP

#include <vector>
#include <string>
#include <utility>
#include <cstdint>
#include <cstddef>

namespace ufo {
    std::vector<uint8_t> write_varint(size_t val);
    size_t read_varint(const std::vector<uint8_t>& data, size_t& pos);

    std::vector<uint8_t> compress_vocab(const std::vector<std::string>& tokens);
    std::vector<std::string> decompress_vocab(const std::vector<uint8_t>& data, size_t num_tokens);

    std::vector<uint8_t> compress_merges(const std::vector<std::pair<uint32_t, uint32_t>>& merges);
    std::vector<std::pair<uint32_t, uint32_t>> decompress_merges(const std::vector<uint8_t>& data);

    std::vector<uint8_t> compute_xor_fec_parity(const std::vector<std::vector<uint8_t>>& chunks, size_t chunk_size);
}

#endif // TOKENIZER_CODER_HPP
