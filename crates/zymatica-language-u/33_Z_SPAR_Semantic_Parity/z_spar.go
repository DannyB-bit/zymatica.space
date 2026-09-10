// Package main implements Class 33: Z-SPAR (Zymatica Semantic Parity & Repair) in Go.
// Author: Danny Bouldiez | Codebase by Devs One
// License: SPDX-License-Identifier: LicenseRef-Zymatica-Covenant-2.0

package main

import "fmt"

var gf16Exp = [32]byte{
	1, 2, 4, 8, 3, 6, 12, 11, 5, 10, 7, 14, 15, 13, 9, 1,
	2, 4, 8, 3, 6, 12, 11, 5, 10, 7, 14, 15, 13, 9, 1, 2,
}

var gf16Log = [16]byte{
	0, 0, 1, 4, 2, 8, 5, 10, 3, 14, 9, 7, 6, 13, 11, 12,
}

func gf16Add(a, b byte) byte {
	return (a ^ b) & 0x0F
}

func gf16Mul(a, b byte) byte {
	a &= 0x0F
	b &= 0x0F
	if a == 0 || b == 0 {
		return 0
	}
	return gf16Exp[(int(gf16Log[a])+int(gf16Log[b]))%15]
}

func gf16Div(a, b byte) byte {
	a &= 0x0F
	b &= 0x0F
	if b == 0 || a == 0 {
		return 0
	}
	return gf16Exp[(int(gf16Log[a])-int(gf16Log[b])+15)%15]
}

func gf16Power(a byte, exp int) byte {
	a &= 0x0F
	if a == 0 {
		return 0
	}
	return gf16Exp[(int(gf16Log[a])*exp)%15]
}

func EncodeZSPAR8D(state [8]byte) [4]byte {
	var parity [4]byte
	for j := 0; j < 4; j++ {
		root := gf16Exp[j+1]
		var sum byte = 0
		for i := 0; i < 8; i++ {
			w := gf16Power(root, i+1)
			sum = gf16Add(sum, gf16Mul(state[i], w))
		}
		parity[j] = sum
	}
	return parity
}

func VerifyAndRepairZSPAR(reconstructed [8]byte, parity [4]byte) (bool, [8]byte) {
	var syndromes [4]byte
	allZero := true
	for j := 0; j < 4; j++ {
		root := gf16Exp[j+1]
		var sum byte = 0
		for i := 0; i < 8; i++ {
			w := gf16Power(root, i+1)
			sum = gf16Add(sum, gf16Mul(reconstructed[i], w))
		}
		syndromes[j] = gf16Add(parity[j], sum)
		if syndromes[j] != 0 {
			allZero = false
		}
	}

	repaired := reconstructed
	if allZero {
		return true, repaired
	}

	// 1-error correction
	for target := 0; target < 8; target++ {
		var candErr byte = 0
		match := true
		for j := 0; j < 4; j++ {
			root := gf16Exp[j+1]
			w := gf16Power(root, target+1)
			err := gf16Div(syndromes[j], w)
			if j == 0 {
				candErr = err
			} else if candErr != err {
				match = false
				break
			}
		}
		if match && candErr != 0 {
			repaired[target] = gf16Add(repaired[target], candErr)
			return true, repaired
		}
	}

	// 2-error correction
	for i1 := 0; i1 < 8; i1++ {
		for i2 := i1 + 1; i2 < 8; i2++ {
			r0, r1 := gf16Exp[1], gf16Exp[2]
			a11 := gf16Power(r0, i1+1)
			a12 := gf16Power(r0, i2+1)
			a21 := gf16Power(r1, i1+1)
			a22 := gf16Power(r1, i2+1)
			det := gf16Add(gf16Mul(a11, a22), gf16Mul(a12, a21))
			if det == 0 {
				continue
			}

			num1 := gf16Add(gf16Mul(a22, syndromes[0]), gf16Mul(a12, syndromes[1]))
			num2 := gf16Add(gf16Mul(a11, syndromes[1]), gf16Mul(a21, syndromes[0]))
			e1 := gf16Div(num1, det)
			e2 := gf16Div(num2, det)

			r2, r3 := gf16Exp[3], gf16Exp[4]
			s2 := gf16Add(gf16Mul(gf16Power(r2, i1+1), e1), gf16Mul(gf16Power(r2, i2+1), e2))
			s3 := gf16Add(gf16Mul(gf16Power(r3, i1+1), e1), gf16Mul(gf16Power(r3, i2+1), e2))

			if s2 == syndromes[2] && s3 == syndromes[3] {
				repaired[i1] = gf16Add(repaired[i1], e1)
				repaired[i2] = gf16Add(repaired[i2], e2)
				return true, repaired
			}
		}
	}

	return false, repaired
}

func main() {
	state := [8]byte{1, 4, 8, 15, 10, 1, 2, 14}
	parity := EncodeZSPAR8D(state)
	drifted := [8]byte{1, 4, 3, 15, 10, 1, 2, 14}

	ok, repaired := VerifyAndRepairZSPAR(drifted, parity)
	fmt.Printf("[Go Z-SPAR] Success: %v | Repaired OP: %d\n", ok, repaired[2])
}
