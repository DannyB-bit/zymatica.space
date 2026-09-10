// Watermark: ip zymatica.space
// JVM UFO Tokenizer Reconstruction Engine

import ufo.TokenizerCoder;
import java.io.*;
import java.nio.ByteBuffer;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.List;

public class DecodeTokenizer {

    private static String escapeJsonString(String str) {
        StringBuilder sb = new StringBuilder();
        for (int i = 0; i < str.length(); i++) {
            char c = str.charAt(i);
            if (c == '"') sb.append("\\\"");
            else if (c == '\\') sb.append("\\\\");
            else if (c == '\n') sb.append("\\n");
            else if (c == '\r') sb.append("\\r");
            else if (c == '\t') sb.append("\\t");
            else if (c < 0x20) {
                sb.append(String.format("\\u%04x", (int) c));
            } else {
                sb.append(c);
            }
        }
        return sb.toString();
    }

    public static void main(String[] args) throws Exception {
        System.out.println("=========================================================");
        System.out.println("  JAVA UFO TOKENIZER DECODER & RECONSTRUCTOR");
        System.out.println("  Watermark: ip zymatica.space");
        System.out.println("=========================================================");

        String decompFile = "../qwen-3.5-0.8b-28chirps-tokenizer.decompressed";
        Path path = Paths.get(decompFile);
        if (!Files.exists(path)) {
            System.err.println("[-] Error: Decompressed payload not found at: " + decompFile);
            System.exit(1);
        }

        byte[] decompressed = Files.readAllBytes(path);
        System.out.println("[+] Loaded decompressed capsule payload: " + String.format("%,d", decompressed.length) + " bytes.");

        ByteBuffer buf = ByteBuffer.wrap(decompressed);

        // Verify Magic
        if (buf.get() != (byte)0xC5 || buf.get() != (byte)0x54 || buf.get() != (byte)0x4B) {
            System.err.println("[-] Error: Invalid magic header.");
            System.exit(1);
        }
        int mode = buf.get() & 0xFF;
        System.out.println("  Magic bytes verified. Mode: Mode " + mode);

        if (mode != 1) {
            System.err.println("[-] Error: Only Mode 1 (Absolute) is supported by local Java decoder.");
            System.exit(1);
        }

        // Skip Config
        int compConfigLen = buf.getInt();
        System.out.println("  Skipping config block of length: " + compConfigLen + " bytes.");
        buf.position(buf.position() + compConfigLen);

        // Read Vocab
        int vocabNum = buf.getInt();
        int vocabLen = buf.getInt();
        System.out.println("  Reading vocabulary tokens: " + String.format("%,d", vocabNum) + " items, data size: " + String.format("%,d", vocabLen) + " bytes.");

        byte[] vocabData = new byte[vocabLen];
        buf.get(vocabData);

        // Decompress Vocab using UFO algorithms
        List<byte[]> restoredVocab = TokenizerCoder.decompressVocab(vocabData, vocabNum);
        System.out.println("[+] Reconstructed vocabulary: " + String.format("%,d", restoredVocab.size()) + " tokens.");

        // Read Merges
        int mergesNum = buf.getInt();
        System.out.println("  Reading merges block: " + String.format("%,d", mergesNum) + " pairs.");

        byte[] mergesData = new byte[mergesNum * 6];
        buf.get(mergesData);

        // Decompress Merges using UFO algorithms
        List<int[]> restoredMerges = TokenizerCoder.decompressMerges(mergesData);
        System.out.println("[+] Reconstructed merges: " + String.format("%,d", restoredMerges.size()) + " pairs.");

        // Write vocab.json using BufferedWriter for speed
        String vocabFile = "vocab.json";
        try (BufferedWriter writer = new BufferedWriter(new FileWriter(vocabFile))) {
            writer.write("{\n");
            for (int i = 0; i < restoredVocab.size(); i++) {
                String tokenStr = new String(restoredVocab.get(i), StandardCharsets.UTF_8);
                String escaped = escapeJsonString(tokenStr);
                if (i < restoredVocab.size() - 1) {
                    writer.write("  \"" + escaped + "\": " + i + ",\n");
                } else {
                    writer.write("  \"" + escaped + "\": " + i + "\n");
                }
            }
            writer.write("}\n");
        }
        System.out.println("[+] Saved reconstructed " + vocabFile + " to current directory.");

        // Write merges.txt using BufferedWriter
        String mergesFile = "merges.txt";
        try (BufferedWriter writer = new BufferedWriter(new FileWriter(mergesFile))) {
            for (int[] pair : restoredMerges) {
                String t0 = new String(restoredVocab.get(pair[0]), StandardCharsets.UTF_8);
                String t1 = new String(restoredVocab.get(pair[1]), StandardCharsets.UTF_8);
                writer.write(t0 + " " + t1 + "\n");
            }
        }
        System.out.println("[+] Saved reconstructed " + mergesFile + " to current directory.");

        // Copy config files from local models directory
        System.out.println("  Copying tokenizer configuration files...");
        Path srcConfig = Paths.get("j:/Language-U/Language-U-V2/qwen-3.5-0.8b-local/tokenizer_config.json");
        Path fallbackConfig = Paths.get("/mnt/j/Language-U/Language-U-V2/qwen-3.5-0.8b-local/tokenizer_config.json");
        if (!Files.exists(srcConfig) && Files.exists(fallbackConfig)) {
            srcConfig = fallbackConfig;
        }

        if (Files.exists(srcConfig)) {
            Files.copy(srcConfig, Paths.get("tokenizer_config.json"), java.nio.file.StandardCopyOption.REPLACE_EXISTING);
            System.out.println("[+] Copied tokenizer_config.json to current directory.");
        }

        Path srcTokenizer = Paths.get("j:/Language-U/Language-U-V2/qwen-3.5-0.8b-local/tokenizer.json");
        Path fallbackTokenizer = Paths.get("/mnt/j/Language-U/Language-U-V2/qwen-3.5-0.8b-local/tokenizer.json");
        if (!Files.exists(srcTokenizer) && Files.exists(fallbackTokenizer)) {
            srcTokenizer = fallbackTokenizer;
        }

        if (Files.exists(srcTokenizer)) {
            Files.copy(srcTokenizer, Paths.get("tokenizer.json"), java.nio.file.StandardCopyOption.REPLACE_EXISTING);
            System.out.println("[+] Reconstructed tokenizer.json copied to current directory.");
        }

        System.out.println("=========================================================");
        System.out.println("  JAVA DECODER SUCCESSFUL!");
        System.out.println("=========================================================");
    }
}
