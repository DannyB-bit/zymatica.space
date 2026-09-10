// ZYMATICA | Language-U Cross-Language Verification Engine (Java)
// Watermark: ip zymatica.space | astronautshe.com

import java.io.File;
import java.io.FileInputStream;
import java.io.IOException;
import java.util.ArrayList;
import java.util.List;

public class VerifyLanguageU {

    static class SparseTransition {
        long key;
        int sym;
        long count;
        SparseTransition(long key, int sym, long count) {
            this.key = key;
            this.sym = sym;
            this.count = count;
        }
    }

    static class RadicalPredictor {
        long alpha;
        long weight;
        List<SparseTransition> transRC = new ArrayList<>();
        List<SparseTransition> transRF = new ArrayList<>();
        List<SparseTransition> transRA = new ArrayList<>();
        int prevRC = 0;
        int prevRF = 0;
        int prevRA = 0;

        RadicalPredictor(long alpha, long weight) {
            this.alpha = alpha;
            this.weight = weight;
        }

        void observe(int rc, int rf, int ra) {
            long keyRC = prevRC;
            boolean found = false;
            for (SparseTransition entry : transRC) {
                if (entry.key == keyRC && entry.sym == rc) {
                    entry.count += weight;
                    found = true;
                    break;
                }
            }
            if (!found && transRC.size() < 256) {
                transRC.add(new SparseTransition(keyRC, rc, weight));
            }

            long keyRF = ((long)rc << 8) | prevRF;
            found = false;
            for (SparseTransition entry : transRF) {
                if (entry.key == keyRF && entry.sym == rf) {
                    entry.count += weight;
                    found = true;
                    break;
                }
            }
            if (!found && transRF.size() < 256) {
                transRF.add(new SparseTransition(keyRF, rf, weight));
            }

            long keyRA = ((long)rc << 16) | ((long)rf << 8) | prevRA;
            found = false;
            for (SparseTransition entry : transRA) {
                if (entry.key == keyRA && entry.sym == ra) {
                    entry.count += weight;
                    found = true;
                    break;
                }
            }
            if (!found && transRA.size() < 256) {
                transRA.add(new SparseTransition(keyRA, ra, weight));
            }

            prevRC = rc;
            prevRF = rf;
            prevRA = ra;
        }

        long[] getCumFreqsRC(int prevRC) {
            long[] freqs = new long[256];
            for (int i = 0; i < 256; i++) freqs[i] = alpha;
            for (SparseTransition entry : transRC) {
                if (entry.key == prevRC) {
                    freqs[entry.sym] += entry.count;
                }
            }
            long[] cumFreqs = new long[257];
            for (int i = 0; i < 256; i++) {
                cumFreqs[i + 1] = cumFreqs[i] + freqs[i];
            }
            return cumFreqs;
        }

        long[] getCumFreqsRF(int currRC, int prevRF) {
            long[] freqs = new long[256];
            for (int i = 0; i < 256; i++) freqs[i] = alpha;
            long key = ((long)currRC << 8) | prevRF;
            for (SparseTransition entry : transRF) {
                if (entry.key == key) {
                    freqs[entry.sym] += entry.count;
                }
            }
            long[] cumFreqs = new long[257];
            for (int i = 0; i < 256; i++) {
                cumFreqs[i + 1] = cumFreqs[i] + freqs[i];
            }
            return cumFreqs;
        }

        long[] getCumFreqsRA(int currRC, int currRF, int prevRA) {
            long[] freqs = new long[256];
            for (int i = 0; i < 256; i++) freqs[i] = alpha;
            long key = ((long)currRC << 16) | ((long)currRF << 8) | prevRA;
            for (SparseTransition entry : transRA) {
                if (entry.key == key) {
                    freqs[entry.sym] += entry.count;
                }
            }
            long[] cumFreqs = new long[257];
            for (int i = 0; i < 256; i++) {
                cumFreqs[i + 1] = cumFreqs[i] + freqs[i];
            }
            return cumFreqs;
        }
    }

    static class BitReader {
        byte[] buffer;
        int bitIndex = 0;
        int totalBits;

        BitReader(byte[] buffer) {
            this.buffer = buffer;
            this.totalBits = buffer.length * 8;
        }

        int readBit() {
            if (bitIndex >= totalBits) return 0;
            int bytePos = bitIndex / 8;
            int bitPos = 7 - (bitIndex % 8);
            int bit = (buffer[bytePos] >> bitPos) & 1;
            bitIndex++;
            return bit;
        }
    }

    static class ConceptRadicals {
        int rc, rf, ra;
        ConceptRadicals(int rc, int rf, int ra) {
            this.rc = rc; this.rf = rf; this.ra = ra;
        }
    }

    static int readVarint(byte[] data, int[] state) {
        int val = 0;
        int shift = 0;
        while (true) {
            if (state[0] >= data.length) break;
            int b = data[state[0]] & 0xFF;
            state[0]++;
            val |= (b & 0x7F) << shift;
            if ((b & 0x80) == 0) break;
            shift += 7;
        }
        return val;
    }

    static List<String> decompressVocab(byte[] data, int numTokens) {
        List<String> tokens = new ArrayList<>();
        int[] state = {0};
        String prev = "";
        for (int i = 0; i < numTokens; i++) {
            if (state[0] >= data.length) break;
            int common = readVarint(data, state);
            int suffixLen = readVarint(data, state);
            byte[] suffixBytes = new byte[suffixLen];
            System.arraycopy(data, state[0], suffixBytes, 0, suffixLen);
            state[0] += suffixLen;
            
            String suffix = new String(suffixBytes);
            String token = prev.substring(0, Math.min(common, prev.length())) + suffix;
            tokens.add(token);
            prev = token;
        }
        return tokens;
    }

    static List<ConceptRadicals> decode(byte[] encodedBytes, int numConcepts, long alpha, long weight) {
        RadicalPredictor pred = new RadicalPredictor(alpha, weight);
        BitReader r = new BitReader(encodedBytes);

        long value = 0;
        for (int i = 0; i < 32; i++) {
            value = (value << 1) | r.readBit();
        }

        long low = 0;
        long high = 0xFFFFFFFFL;
        List<ConceptRadicals> decoded = new ArrayList<>();

        for (int cIdx = 0; cIdx < numConcepts; cIdx++) {
            int prevRC = pred.prevRC;
            int prevRF = pred.prevRF;
            int prevRA = pred.prevRA;
            int[] symbols = new int[3];

            for (int step = 0; step < 3; step++) {
                long[] cumFreqs;
                if (step == 0) {
                    cumFreqs = pred.getCumFreqsRC(prevRC);
                } else if (step == 1) {
                    cumFreqs = pred.getCumFreqsRF(symbols[0], prevRF);
                } else {
                    cumFreqs = pred.getCumFreqsRA(symbols[0], symbols[1], prevRA);
                }

                long total = cumFreqs[256];
                long rangeWidth = high - low + 1;
                long scaledVal = (((value - low) + 1) * total - 1) / rangeWidth;

                int sym = 0;
                int lIdx = 0, rIdx = 255;
                while (lIdx <= rIdx) {
                    int mIdx = (lIdx + rIdx) / 2;
                    if (cumFreqs[mIdx] <= scaledVal && scaledVal < cumFreqs[mIdx + 1]) {
                        sym = mIdx;
                        break;
                    } else if (scaledVal >= cumFreqs[mIdx + 1]) {
                        lIdx = mIdx + 1;
                    } else {
                        rIdx = mIdx - 1;
                    }
                }

                symbols[step] = sym;
                long cumLow = cumFreqs[sym];
                long cumHigh = cumFreqs[sym + 1];

                high = low + (rangeWidth * cumHigh) / total - 1;
                low = low + (rangeWidth * cumLow) / total;

                while (true) {
                    if (high < 0x80000000L) {
                        low <<= 1;
                        high = (high << 1) | 1;
                        value = (value << 1) | r.readBit();
                    } else if (low >= 0x80000000L) {
                        low = (low - 0x80000000L) << 1;
                        high = ((high - 0x80000000L) << 1) | 1;
                        value = ((value - 0x80000000L) << 1) | r.readBit();
                    } else if (low >= 0x40000000L && high < 0xC0000000L) {
                        low = (low - 0x40000000L) << 1;
                        high = ((high - 0x40000000L) << 1) | 1;
                        value = ((value - 0x40000000L) << 1) | r.readBit();
                    } else {
                        break;
                    }
                    low &= 0xFFFFFFFFL;
                    high &= 0xFFFFFFFFL;
                    value &= 0xFFFFFFFFL;
                }
            }

            decoded.add(new ConceptRadicals(symbols[0], symbols[1], symbols[2]));
            pred.observe(symbols[0], symbols[1], symbols[2]);
        }
        return decoded;
    }

    public static void main(String[] args) throws IOException {
        System.out.println("======================================================================");
        System.out.println("ZYMATICA | Cross-Language Java Decompressor & Range-Decoder");
        System.out.println("======================================================================\n");

        File namesFile = new File("Language-U-Browser/frameworks_names.bin");
        File coordsFile = new File("Language-U-Browser/frameworks_coordinates.bin");

        if (!namesFile.exists() || !coordsFile.exists()) {
            namesFile = new File("frameworks_names.bin");
            coordsFile = new File("frameworks_coordinates.bin");
        }

        if (!namesFile.exists() || !coordsFile.exists()) {
            System.err.println("[!] Error: Binary transport files not found. Run run_ultimate_pipeline.py first.");
            System.exit(1);
        }

        // 1. Read names binary
        byte[] namesBytes = new byte[(int) namesFile.length()];
        try (FileInputStream fis = new FileInputStream(namesFile)) {
            fis.read(namesBytes);
        }

        // 2. Read coordinates binary
        byte[] coordsBytes = new byte[(int) coordsFile.length()];
        try (FileInputStream fis = new FileInputStream(coordsFile)) {
            fis.read(coordsBytes);
        }

        // 3. Decompress vocabulary names
        List<String> names = decompressVocab(namesBytes, 49);
        System.out.println("[1] Java Vocab Decompression: SUCCESS (" + names.size() + " names restored).");

        // 4. Formulate expected radicals based on the same rules
        List<ConceptRadicals> expected = new ArrayList<>();
        for (String name : names) {
            int domain = 1;
            String lower = name.toLowerCase();
            if (lower.contains("pixi") || lower.contains("phaser") || lower.contains("away") || lower.contains("p5")) {
                domain = 2;
            } else if (lower.contains("scenejs") || lower.contains("glam") || lower.contains("deck") || lower.contains("cesium") || lower.contains("luma") || lower.contains("philo")) {
                domain = 7;
            }
            int rc = (domain << 4) | 2;
            int rf = (1 << 4) | 2;
            int ra = (15 << 4) | 12;
            expected.add(new ConceptRadicals(rc, rf, ra));
        }

        // 5. Decode radicals using Yang range decoder in Java
        List<ConceptRadicals> decoded = decode(coordsBytes, 49, 1, 128);
        System.out.println("[2] Java Yang Range Decoder execution: SUCCESS.");

        // 6. Assert exact equivalence (dynamic validation)
        boolean match = true;
        for (int i = 0; i < 49; i++) {
            ConceptRadicals exp = expected.get(i);
            ConceptRadicals dec = decoded.get(i);
            if (exp.rc != dec.rc || exp.rf != dec.rf || exp.ra != dec.ra) {
                System.err.println("[!] Mismatch at index " + i + " (" + names.get(i) + "): Expected RC=" + exp.rc + ", RF=" + exp.rf + ", RA=" + exp.ra + " | Decoded RC=" + dec.rc + ", RF=" + dec.rf + ", RA=" + dec.ra);
                match = false;
                break;
            }
        }

        if (match) {
            System.out.println("\n[SUCCESS] Java range-decoder verification: 100% MATCH!");
        } else {
            System.err.println("\n[ERROR] Java dynamic coordinate check failed!");
            System.exit(1);
        }
    }
}
