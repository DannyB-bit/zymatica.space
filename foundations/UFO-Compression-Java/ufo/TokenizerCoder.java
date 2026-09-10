// Watermark: ip zymatica.space
// Patent Pending — USPTO Provisional Application | Zymatica Project

package ufo;

import java.io.ByteArrayOutputStream;
import java.util.ArrayList;
import java.util.List;

public class TokenizerCoder {

    public static byte[] writeVarint(int val) {
        ByteArrayOutputStream bos = new ByteArrayOutputStream();
        int v = val;
        while (v >= 128) {
            bos.write((v & 0x7F) | 0x80);
            v >>>= 7;
        }
        bos.write(v & 0x7F);
        return bos.toByteArray();
    }

    public static int readVarint(byte[] data, int[] state) {
        int val = 0;
        int shift = 0;
        while (true) {
            if (state[0] >= data.length) {
                break;
            }
            byte b = data[state[0]];
            state[0]++;
            val |= (b & 0x7F) << shift;
            if ((b & 0x80) == 0) {
                break;
            }
            shift += 7;
        }
        return val;
    }

    /**
     * Level 4 Prefix-Suffix Vocabulary String Compression
     */
    public static byte[] compressVocab(List<byte[]> tokens) {
        ByteArrayOutputStream bos = new ByteArrayOutputStream();
        byte[] prev = new byte[0];
        for (byte[] t : tokens) {
            int common = 0;
            int l = Math.min(t.length, prev.length);
            while (common < l && t[common] == prev[common]) {
                common++;
            }
            int suffixLen = t.length - common;
            byte[] suffix = new byte[suffixLen];
            System.arraycopy(t, common, suffix, 0, suffixLen);
            
            try {
                bos.write(writeVarint(common));
                bos.write(writeVarint(suffixLen));
                bos.write(suffix);
            } catch (Exception e) {
                // Ignore
            }
            prev = t;
        }
        return bos.toByteArray();
    }

    /**
     * Level 4 Prefix-Suffix Vocabulary String Restoration
     */
    public static List<byte[]> decompressVocab(byte[] data, int numTokens) {
        List<byte[]> tokens = new ArrayList<>(numTokens);
        int[] state = new int[]{0};
        byte[] prev = new byte[0];
        for (int i = 0; i < numTokens; i++) {
            if (state[0] >= data.length) {
                break;
            }
            int common = readVarint(data, state);
            int suffixLen = readVarint(data, state);
            if (state[0] + suffixLen > data.length) {
                break;
            }
            byte[] suffix = new byte[suffixLen];
            System.arraycopy(data, state[0], suffix, 0, suffixLen);
            state[0] += suffixLen;
            
            byte[] t = new byte[common + suffixLen];
            System.arraycopy(prev, 0, t, 0, Math.min(common, prev.length));
            System.arraycopy(suffix, 0, t, common, suffixLen);
            tokens.add(t);
            prev = t;
        }
        return tokens;
    }

    /**
     * Level 3 BPE Merges Binary Index-Packing (24-bit integer pairs)
     */
    public static byte[] compressMerges(List<int[]> merges) {
        byte[] encoded = new byte[merges.size() * 6];
        int offset = 0;
        for (int[] pair : merges) {
            int idx0 = pair[0];
            int int1 = pair[1];
            
            encoded[offset]     = (byte)((idx0 >> 16) & 0xFF);
            encoded[offset + 1] = (byte)((idx0 >> 8) & 0xFF);
            encoded[offset + 2] = (byte)(idx0 & 0xFF);
            
            encoded[offset + 3] = (byte)((int1 >> 16) & 0xFF);
            encoded[offset + 4] = (byte)((int1 >> 8) & 0xFF);
            encoded[offset + 5] = (byte)(int1 & 0xFF);
            offset += 6;
        }
        return encoded;
    }

    /**
     * Level 3 BPE Merges Binary Index-Unpacking (24-bit integer pairs)
     */
    public static List<int[]> decompressMerges(byte[] data) {
        int numMerges = data.length / 6;
        List<int[]> merges = new ArrayList<>(numMerges);
        for (int i = 0; i < numMerges; i++) {
            int offset = i * 6;
            int idx0 = ((data[offset] & 0xFF) << 16) |
                       ((data[offset + 1] & 0xFF) << 8) |
                       (data[offset + 2] & 0xFF);
                       
            int idx1 = ((data[offset + 3] & 0xFF) << 16) |
                       ((data[offset + 4] & 0xFF) << 8) |
                       (data[offset + 5] & 0xFF);
            merges.add(new int[]{idx0, idx1});
        }
        return merges;
    }

    /**
     * Level 7 XOR-FEC Parity computation for error resilient transmission
     */
    public static byte[] computeXorFecParity(List<byte[]> chunks, int chunkSize) {
        byte[] parity = new byte[chunkSize];
        for (byte[] chunk : chunks) {
            int limit = Math.min(chunk.length, chunkSize);
            for (int j = 0; j < limit; j++) {
                parity[j] ^= chunk[j];
            }
        }
        return parity;
    }
}
