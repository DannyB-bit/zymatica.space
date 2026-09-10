// Watermark: ip zymatica.space
// Verification suite for JVM UFO Tokenizer Coder

import ufo.TokenizerCoder;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;

public class TestTokenizerCoder {
    public static void main(String[] args) {
        System.out.println("=========================================================");
        System.out.println("  RUNNING JAVA UFO TOKENIZER CODER VERIFICATION");
        System.out.println("  Watermark: ip zymatica.space");
        System.out.println("=========================================================");

        // 1. Test Prefix-Suffix Vocab Compression & Decompression
        System.out.println("\n[Test 1] Prefix-Suffix Vocab Coder...");
        String[] originalVocabStrings = {
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
        List<byte[]> originalVocab = new ArrayList<>();
        for (String s : originalVocabStrings) {
            originalVocab.add(s.getBytes(StandardCharsets.UTF_8));
        }

        byte[] compressedVocab = TokenizerCoder.compressVocab(originalVocab);
        System.out.println("  Original vocab items: " + originalVocab.size());
        System.out.println("  Compressed vocab size: " + compressedVocab.length + " bytes");

        List<byte[]> restoredVocab = TokenizerCoder.decompressVocab(compressedVocab, originalVocab.size());
        System.out.println("  Restored vocab items: " + restoredVocab.size());

        if (originalVocab.size() != restoredVocab.size()) {
            throw new RuntimeException("Mismatch in vocab count!");
        }
        for (int i = 0; i < originalVocab.size(); i++) {
            if (!Arrays.equals(originalVocab.get(i), restoredVocab.get(i))) {
                throw new RuntimeException("Mismatch in vocab content at index " + i + "!");
            }
        }
        System.out.println("  [+] Vocab round-trip: SUCCESS (100% Match)");

        // 2. Test BPE Merges index pack/unpack
        System.out.println("\n[Test 2] BPE Merges Binary Index Coder...");
        List<int[]> originalMerges = new ArrayList<>();
        originalMerges.add(new int[]{1015, 2030});
        originalMerges.add(new int[]{45, 12});
        originalMerges.add(new int[]{16777215, 50000});
        originalMerges.add(new int[]{0, 1});
        originalMerges.add(new int[]{100000, 200000});

        byte[] compressedMerges = TokenizerCoder.compressMerges(originalMerges);
        System.out.println("  Original merges items: " + originalMerges.size());
        System.out.println("  Compressed merges size: " + compressedMerges.length + " bytes");

        List<int[]> restoredMerges = TokenizerCoder.decompressMerges(compressedMerges);
        System.out.println("  Restored merges items: " + restoredMerges.size());

        if (originalMerges.size() != restoredMerges.size()) {
            throw new RuntimeException("Mismatch in merges count!");
        }
        for (int i = 0; i < originalMerges.size(); i++) {
            if (originalMerges.get(i)[0] != restoredMerges.get(i)[0] ||
                originalMerges.get(i)[1] != restoredMerges.get(i)[1]) {
                throw new RuntimeException("Mismatch in merges content at index " + i + "!");
            }
        }
        System.out.println("  [+] Merges round-trip: SUCCESS (100% Match)");

        // 3. Test XOR-FEC Parity
        System.out.println("\n[Test 3] XOR-FEC Parity Calculation...");
        byte[] c1 = {(byte)0xAA, (byte)0xBB, (byte)0xCC, (byte)0xDD};
        byte[] c2 = {(byte)0x11, (byte)0x22, (byte)0x33, (byte)0x44};
        byte[] c3 = {(byte)0x55, (byte)0x66, (byte)0x77, (byte)0x88};
        List<byte[]> chunks = new ArrayList<>();
        chunks.add(c1);
        chunks.add(c2);
        chunks.add(c3);

        byte[] parity = TokenizerCoder.computeXorFecParity(chunks, 4);
        byte[] expectedParity = {
            (byte)(0xAA ^ 0x11 ^ 0x55),
            (byte)(0xBB ^ 0x22 ^ 0x66),
            (byte)(0xCC ^ 0x33 ^ 0x77),
            (byte)(0xDD ^ 0x44 ^ 0x88)
        };

        if (!Arrays.equals(parity, expectedParity)) {
            throw new RuntimeException("Mismatch in XOR-FEC parity!");
        }
        System.out.println("  [+] XOR-FEC computation: SUCCESS");

        System.out.println("\n=========================================================");
        System.out.println("  ALL JAVA TESTS PASSED SUCCESSFULLY!");
        System.out.println("=========================================================");
    }
}
