package ufo

import (
	"bytes"
	"fmt"
	"testing"
)

func TestTokenizerCoder(t *testing.T) {
	fmt.Println("=========================================================")
	fmt.Println("  RUNNING GO UFO TOKENIZER CODER VERIFICATION")
	fmt.Println("  Watermark: ip zymatica.space")
	fmt.Println("=========================================================")

	// 1. Test Vocab Coder
	fmt.Println("\n[Test 1] Prefix-Suffix Vocab Coder...")
	originalVocab := [][]byte{
		[]byte("hello"),
		[]byte("hell"),
		[]byte("heaven"),
		[]byte("heavy"),
		[]byte("world"),
		[]byte("word"),
		[]byte("work"),
		[]byte("worker"),
		[]byte("working"),
	}

	compressedVocab := CompressVocab(originalVocab)
	fmt.Printf("  Original vocab items: %d\n", len(originalVocab))
	fmt.Printf("  Compressed vocab size: %d bytes\n", len(compressedVocab))

	restoredVocab := DecompressVocab(compressedVocab, len(originalVocab))
	fmt.Printf("  Restored vocab items: %d\n", len(restoredVocab))

	if len(originalVocab) != len(restoredVocab) {
		t.Fatalf("Vocab length mismatch: expected %d, got %d", len(originalVocab), len(restoredVocab))
	}
	for i := range originalVocab {
		if !bytes.Equal(originalVocab[i], restoredVocab[i]) {
			t.Fatalf("Vocab item mismatch at index %d: expected %s, got %s", i, originalVocab[i], restoredVocab[i])
		}
	}
	fmt.Println("  [+] Vocab round-trip: SUCCESS (100% Match)")

	// 2. Test BPE Merges Coder
	fmt.Println("\n[Test 2] BPE Merges Binary Index Coder...")
	originalMerges := [][2]uint32{
		{1015, 2030},
		{45, 12},
		{16777215, 50000},
		{0, 1},
		{100000, 200000},
	}

	compressedMerges := CompressMerges(originalMerges)
	fmt.Printf("  Original merges items: %d\n", len(originalMerges))
	fmt.Printf("  Compressed merges size: %d bytes\n", len(compressedMerges))

	restoredMerges := DecompressMerges(compressedMerges)
	fmt.Printf("  Restored merges items: %d\n", len(restoredMerges))

	if len(originalMerges) != len(restoredMerges) {
		t.Fatalf("Merges length mismatch: expected %d, got %d", len(originalMerges), len(restoredMerges))
	}
	for i := range originalMerges {
		if originalMerges[i] != restoredMerges[i] {
			t.Fatalf("Merges item mismatch at index %d: expected %v, got %v", i, originalMerges[i], restoredMerges[i])
		}
	}
	fmt.Println("  [+] Merges round-trip: SUCCESS (100% Match)")

	// 3. Test XOR-FEC Parity
	fmt.Println("\n[Test 3] XOR-FEC Parity Calculation...")
	c1 := []byte{0xAA, 0xBB, 0xCC, 0xDD}
	c2 := []byte{0x11, 0x22, 0x33, 0x44}
	c3 := []byte{0x55, 0x66, 0x77, 0x88}
	chunks := [][]byte{c1, c2, c3}

	parity := ComputeXorFecParity(chunks, 4)
	expectedParity := []byte{
		0xAA ^ 0x11 ^ 0x55,
		0xBB ^ 0x22 ^ 0x66,
		0xCC ^ 0x33 ^ 0x77,
		0xDD ^ 0x44 ^ 0x88,
	}

	if !bytes.Equal(parity, expectedParity) {
		t.Fatalf("Parity mismatch: expected %v, got %v", expectedParity, parity)
	}
	fmt.Println("  [+] XOR-FEC computation: SUCCESS")

	fmt.Println("\n=========================================================")
	fmt.Println("  ALL GO TESTS PASSED SUCCESSFULLY!")
	fmt.Println("=========================================================")
}
