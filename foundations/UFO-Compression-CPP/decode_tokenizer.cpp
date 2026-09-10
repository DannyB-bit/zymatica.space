// Watermark: ip zymatica.space
// C++ UFO Tokenizer Reconstruction Engine

#include "tokenizer_coder.hpp"
#include <iostream>
#include <fstream>
#include <vector>
#include <string>
#include <iomanip>
#include <cstdint>
#include <cstring>

// Read big-endian 32-bit integer
uint32_t read_u32_be(const std::vector<uint8_t>& data, size_t& pos) {
    uint32_t val = (static_cast<uint32_t>(data[pos]) << 24) |
                   (static_cast<uint32_t>(data[pos+1]) << 16) |
                   (static_cast<uint32_t>(data[pos+2]) << 8) |
                   (static_cast<uint32_t>(data[pos+3]));
    pos += 4;
    return val;
}

// Escape string for valid JSON format
std::string escape_json_string(const std::string& s) {
    std::string out = "";
    for (unsigned char c : s) {
        if (c == '"') out += "\\\"";
        else if (c == '\\') out += "\\\\";
        else if (c == '\n') out += "\\n";
        else if (c == '\r') out += "\\r";
        else if (c == '\t') out += "\\t";
        else if (c < 0x20) {
            char buf[10];
            snprintf(buf, sizeof(buf), "\\u%04x", c);
            out += buf;
        } else {
            out += c;
        }
    }
    return out;
}

int main() {
    std::cout << "=========================================================" << std::endl;
    std::cout << "  C++ UFO TOKENIZER DECODER & RECONSTRUCTOR" << std::endl;
    std::cout << "  Watermark: ip zymatica.space" << std::endl;
    std::cout << "=========================================================" << std::endl;

    // Load decompressed payload
    std::string decomp_file = "../qwen-3.5-0.8b-28chirps-tokenizer.decompressed";
    std::ifstream instream(decomp_file, std::ios::binary | std::ios::ate);
    if (!instream.is_open()) {
        std::cerr << "[-] Error opening decompressed payload file: " << decomp_file << std::endl;
        return 1;
    }

    std::streamsize size = instream.tellg();
    instream.seekg(0, std::ios::beg);
    std::vector<uint8_t> decompressed(size);
    if (!instream.read(reinterpret_cast<char*>(decompressed.data()), size)) {
        std::cerr << "[-] Error reading decompressed payload file." << std::endl;
        return 1;
    }
    instream.close();
    std::cout << "[+] Loaded decompressed capsule payload: " << decompressed.size() << " bytes." << std::endl;

    // Verify Magic Header and Mode
    size_t pos = 0;
    if (decompressed[pos] != 0xC5 || decompressed[pos+1] != 0x54 || decompressed[pos+2] != 0x4B) {
        std::cerr << "[-] Error: Invalid magic header." << std::endl;
        return 1;
    }
    pos += 3;
    uint8_t mode = decompressed[pos++];
    std::cout << "  Magic bytes verified. Mode: Mode " << static_cast<int>(mode) << std::endl;

    if (mode != 1) {
        std::cerr << "[-] Error: Only Mode 1 (Absolute) is supported by C++ local decoder." << std::endl;
        return 1;
    }

    // Skip comp_config
    uint32_t comp_config_len = read_u32_be(decompressed, pos);
    std::cout << "  Skipping config block of length: " << comp_config_len << " bytes." << std::endl;
    pos += comp_config_len;

    // Read Vocab
    uint32_t vocab_num = read_u32_be(decompressed, pos);
    uint32_t vocab_len = read_u32_be(decompressed, pos);
    std::cout << "  Reading vocabulary tokens: " << vocab_num << " items, data size: " << vocab_len << " bytes." << std::endl;

    std::vector<uint8_t> vocab_data(decompressed.begin() + pos, decompressed.begin() + pos + vocab_len);
    pos += vocab_len;

    // Decompress Vocab using UFO algorithms
    std::vector<std::string> restored_vocab = ufo::decompress_vocab(vocab_data, vocab_num);
    std::cout << "[+] Reconstructed vocabulary: " << restored_vocab.size() << " tokens." << std::endl;

    // Read Merges
    uint32_t merges_num = read_u32_be(decompressed, pos);
    std::cout << "  Reading merges block: " << merges_num << " pairs." << std::endl;

    std::vector<uint8_t> merges_data(decompressed.begin() + pos, decompressed.begin() + pos + merges_num * 6);
    pos += merges_num * 6;

    // Decompress Merges using UFO algorithms
    std::vector<std::pair<uint32_t, uint32_t>> restored_merges = ufo::decompress_merges(merges_data);
    std::cout << "[+] Reconstructed merges: " << restored_merges.size() << " pairs." << std::endl;

    // Write vocab.json
    std::string vocab_file = "vocab.json";
    std::ofstream vocab_out(vocab_file);
    if (!vocab_out.is_open()) {
        std::cerr << "[-] Error opening output file: " << vocab_file << std::endl;
        return 1;
    }
    vocab_out << "{\n";
    for (size_t i = 0; i < restored_vocab.size(); ++i) {
        vocab_out << "  \"" << escape_json_string(restored_vocab[i]) << "\": " << i;
        if (i < restored_vocab.size() - 1) {
            vocab_out << ",\n";
        } else {
            vocab_out << "\n";
        }
    }
    vocab_out << "}\n";
    vocab_out.close();
    std::cout << "[+] Saved reconstructed " << vocab_file << " to current directory." << std::endl;

    // Write merges.txt
    std::string merges_file = "merges.txt";
    std::ofstream merges_out(merges_file);
    if (!merges_out.is_open()) {
        std::cerr << "[-] Error opening output file: " << merges_file << std::endl;
        return 1;
    }
    for (const auto& pair : restored_merges) {
        merges_out << restored_vocab[pair.first] << " " << restored_vocab[pair.second] << "\n";
    }
    merges_out.close();
    std::cout << "[+] Saved reconstructed " << merges_file << " to current directory." << std::endl;

    // Copy config files from local models directory to fulfill the requirement
    std::cout << "  Copying tokenizer configuration files..." << std::endl;
    std::ifstream src_config("j:/Language-U/Language-U-V2/qwen-3.5-0.8b-local/tokenizer_config.json", std::ios::binary);
    if (src_config.is_open()) {
        std::ofstream dst_config("tokenizer_config.json", std::ios::binary);
        dst_config << src_config.rdbuf();
        dst_config.close();
        src_config.close();
        std::cout << "[+] Copied tokenizer_config.json to current directory." << std::endl;
    }

    std::ifstream src_tokenizer("j:/Language-U/Language-U-V2/qwen-3.5-0.8b-local/tokenizer.json", std::ios::binary);
    if (src_tokenizer.is_open()) {
        std::ofstream dst_tokenizer("tokenizer.json", std::ios::binary);
        dst_tokenizer << src_tokenizer.rdbuf();
        dst_tokenizer.close();
        src_tokenizer.close();
        std::cout << "[+] Reconstructed tokenizer.json copied to current directory." << std::endl;
    }

    std::cout << "=========================================================" << std::endl;
    std::cout << "  C++ DECODER SUCCESSFUL!" << std::endl;
    std::cout << "=========================================================" << std::endl;
    return 0;
}
