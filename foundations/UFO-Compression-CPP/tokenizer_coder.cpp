// Watermark: ip zymatica.space
// Patent Pending — USPTO Provisional Application | Zymatica Project

#include "tokenizer_coder.hpp"
#include <algorithm>
#include <stdexcept>

namespace ufo {

    std::vector<uint8_t> write_varint(size_t val) {
        std::vector<uint8_t> res;
        size_t v = val;
        while (v >= 128) {
            res.push_back(static_cast<uint8_t>((v & 0x7F) | 0x80));
            v >>= 7;
        }
        res.push_back(static_cast<uint8_t>(v & 0x7F));
        return res;
    }

    size_t read_varint(const std::vector<uint8_t>& data, size_t& pos) {
        size_t val = 0;
        size_t shift = 0;
        while (true) {
            if (pos >= data.size()) {
                break;
            }
            uint8_t b = data[pos];
            pos++;
            val |= static_cast<size_t>(b & 0x7F) << shift;
            if (!(b & 0x80)) {
                break;
            }
            shift += 7;
        }
        return val;
    }

    std::vector<uint8_t> compress_vocab(const std::vector<std::string>& tokens) {
        std::vector<uint8_t> encoded;
        std::string prev = "";
        for (const auto& t : tokens) {
            size_t common = 0;
            size_t l = std::min(t.length(), prev.length());
            while (common < l && t[common] == prev[common]) {
                common++;
            }
            std::string suffix = t.substr(common);
            auto common_varint = write_varint(common);
            auto suffix_len_varint = write_varint(suffix.length());
            
            encoded.insert(encoded.end(), common_varint.begin(), common_varint.end());
            encoded.insert(encoded.end(), suffix_len_varint.begin(), suffix_len_varint.end());
            encoded.insert(encoded.end(), suffix.begin(), suffix.end());
            
            prev = t;
        }
        return encoded;
    }

    std::vector<std::string> decompress_vocab(const std::vector<uint8_t>& data, size_t num_tokens) {
        std::vector<std::string> tokens;
        tokens.reserve(num_tokens);
        size_t pos = 0;
        std::string prev = "";
        for (size_t i = 0; i < num_tokens; ++i) {
            if (pos >= data.size()) {
                break;
            }
            size_t common = read_varint(data, pos);
            size_t suffix_len = read_varint(data, pos);
            if (pos + suffix_len > data.size()) {
                break;
            }
            std::string suffix(data.begin() + pos, data.begin() + pos + suffix_len);
            pos += suffix_len;
            
            size_t len_to_keep = std::min(common, prev.length());
            std::string t = prev.substr(0, len_to_keep) + suffix;
            tokens.push_back(t);
            prev = t;
        }
        return tokens;
    }

    std::vector<uint8_t> compress_merges(const std::vector<std::pair<uint32_t, uint32_t>>& merges) {
        std::vector<uint8_t> encoded;
        encoded.reserve(merges.size() * 6);
        for (const auto& pair : merges) {
            uint32_t idx0 = pair.first;
            uint32_t idx1 = pair.second;
            
            // Pack each index into 3 bytes big-endian
            encoded.push_back(static_cast<uint8_t>((idx0 >> 16) & 0xFF));
            encoded.push_back(static_cast<uint8_t>((idx0 >> 8) & 0xFF));
            encoded.push_back(static_cast<uint8_t>(idx0 & 0xFF));
            
            encoded.push_back(static_cast<uint8_t>((idx1 >> 16) & 0xFF));
            encoded.push_back(static_cast<uint8_t>((idx1 >> 8) & 0xFF));
            encoded.push_back(static_cast<uint8_t>(idx1 & 0xFF));
        }
        return encoded;
    }

    std::vector<std::pair<uint32_t, uint32_t>> decompress_merges(const std::vector<uint8_t>& data) {
        size_t num_merges = data.size() / 6;
        std::vector<std::pair<uint32_t, uint32_t>> merges;
        merges.reserve(num_merges);
        for (size_t i = 0; i < num_merges; ++i) {
            size_t offset = i * 6;
            uint32_t idx0 = (static_cast<uint32_t>(data[offset]) << 16)
                          | (static_cast<uint32_t>(data[offset + 1]) << 8)
                          | static_cast<uint32_t>(data[offset + 2]);
                          
            uint32_t idx1 = (static_cast<uint32_t>(data[offset + 3]) << 16)
                          | (static_cast<uint32_t>(data[offset + 4]) << 8)
                          | static_cast<uint32_t>(data[offset + 5]);
            merges.push_back({idx0, idx1});
        }
        return merges;
    }

    std::vector<uint8_t> compute_xor_fec_parity(const std::vector<std::vector<uint8_t>>& chunks, size_t chunk_size) {
        std::vector<uint8_t> parity(chunk_size, 0);
        for (const auto& chunk : chunks) {
            size_t min_len = std::min(chunk.size(), chunk_size);
            for (size_t j = 0; j < min_len; ++j) {
                parity[j] ^= chunk[j];
            }
        }
        return parity;
    }
}
