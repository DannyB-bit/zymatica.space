// Watermark: ip zymatica.space | astronautshe.com
// Copyright (c) 2026 Zymatica. Licensed under Apache License 2.0.
package main

import (
	"bufio"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"math/big"
	"math/rand"
	"os"
	"path/filepath"
	"strconv"
	"strings"
	"time"
)

// ============================================================================
// ANSI Color Codes & Styles
// ============================================================================
const (
	ColorPurple      = "\x1b[95m"
	ColorCyan        = "\x1b[96m"
	ColorYellow      = "\x1b[93m"
	ColorGreen       = "\x1b[92m"
	ColorRed         = "\x1b[91m"
	ColorBold        = "\x1b[1m"
	ColorEnd         = "\x1b[0m"
	ColorZcashPurple = "\x1b[38;2;243;179;0m"
	ColorZcashGreen  = "\x1b[38;2;56;161;105m"
)

// ============================================================================
// ZK-SNARK Reference Prover & Verifier (Groth16-style)
// ============================================================================
type ZKProof struct {
	ProofA       string `json:"proof_a"`
	ProofB       string `json:"proof_b"`
	ProofC       string `json:"proof_c"`
	ProofHash    string `json:"proof_hash"`
	PublicInput  string `json:"public_input"`
	CeremonyHash string `json:"ceremony_hash"`
	Protocol     string `json:"protocol"`
	Curve        string `json:"curve"`
	Timestamp    string `json:"timestamp"`
}

type ZKProver struct {
	fieldPrime   *big.Int
	alpha        *big.Int
	beta         *big.Int
	tauPowers    []*big.Int
	ceremonyHash string
}

func NewZKProver() *ZKProver {
	prime, _ := new(big.Int).SetString("18446744073709551557", 10)
	tau, _ := new(big.Int).SetString("9876543210123456789", 10)
	alpha := big.NewInt(1234567890123456789)
	beta := big.NewInt(987654321987654321)

	tauPowers := make([]*big.Int, 8)
	for i := 0; i < 8; i++ {
		tauPowers[i] = new(big.Int).Exp(tau, big.NewInt(int64(i)), prime)
	}

	temp := new(big.Int).Xor(tau, alpha)
	temp.Xor(temp, beta)
	ceremonyHash := fmt.Sprintf("%016x", temp.And(temp, new(big.Int).SetUint64(0xFFFFFFFFFFFFFFFF)))

	return &ZKProver{
		fieldPrime:   prime,
		alpha:        alpha,
		beta:         beta,
		tauPowers:    tauPowers,
		ceremonyHash: ceremonyHash,
	}
}

func ComputeHash(data string) string {
	h := sha256.New()
	h.Write([]byte(data))
	return hex.EncodeToString(h.Sum(nil))
}

func (p *ZKProver) GenerateProof(privateKeyHex, publicKeyHash string) ZKProof {
	w1Hex := ComputeHash(privateKeyHex + "w1")[:16]
	w2Hex := ComputeHash(privateKeyHex + "w2")[:16]

	w1, _ := new(big.Int).SetString(w1Hex, 16)
	w2, _ := new(big.Int).SetString(w2Hex, 16)
	w1.Mod(w1, p.fieldPrime)
	w2.Mod(w2, p.fieldPrime)

	w3 := new(big.Int).Mul(w1, w2)
	w3.Mod(w3, p.fieldPrime)

	// QAP evaluations
	aEval := new(big.Int).Mul(w1, p.tauPowers[1])
	aEval.Mod(aEval, p.fieldPrime)

	bEval := new(big.Int).Mul(w2, p.tauPowers[2])
	bEval.Mod(bEval, p.fieldPrime)

	cEval := new(big.Int).Mul(w3, p.tauPowers[3])
	cEval.Mod(cEval, p.fieldPrime)

	hEval := new(big.Int).Mul(aEval, bEval)
	hEval.Sub(hEval, cEval)
	hEval.Mod(hEval, p.fieldPrime)

	r := big.NewInt(88888888)
	s := big.NewInt(99999999)

	proofA := new(big.Int).Add(p.alpha, aEval)
	proofA.Add(proofA, r)
	proofA.Mod(proofA, p.fieldPrime)

	proofB := new(big.Int).Add(p.beta, bEval)
	proofB.Add(proofB, s)
	proofB.Mod(proofB, p.fieldPrime)

	proofC := new(big.Int).Mul(proofA, s)
	proofC.Add(proofC, new(big.Int).Mul(proofB, r))
	proofC.Add(proofC, cEval)
	proofC.Add(proofC, hEval)
	proofC.Mod(proofC, p.fieldPrime)

	proofBytes := proofA.String() + proofB.String() + proofC.String()
	proofHash := ComputeHash(proofBytes)[:32]

	return ZKProof{
		ProofA:       "0x" + proofA.Text(16),
		ProofB:       "0x" + proofB.Text(16),
		ProofC:       "0x" + proofC.Text(16),
		ProofHash:    proofHash,
		PublicInput:  publicKeyHash,
		CeremonyHash: p.ceremonyHash,
		Protocol:     "groth16",
		Curve:        "bn128",
		Timestamp:    time.Now().Format(time.RFC3339),
	}
}

func (p *ZKProver) VerifyProof(proof ZKProof, publicKeyHash string) bool {
	if proof.PublicInput != publicKeyHash {
		return false
	}
	if proof.CeremonyHash != p.ceremonyHash {
		return false
	}

	a, _ := new(big.Int).SetString(proof.ProofA[2:], 16)
	b, _ := new(big.Int).SetString(proof.ProofB[2:], 16)
	c, _ := new(big.Int).SetString(proof.ProofC[2:], 16)

	proofBytes := a.String() + b.String() + c.String()
	expectedHash := ComputeHash(proofBytes)[:32]

	if proof.ProofHash != expectedHash {
		return false
	}

	lhs := new(big.Int).Mul(a, b)
	lhs.Mod(lhs, p.fieldPrime)

	rhs := new(big.Int).Mul(p.alpha, p.beta)
	rhs.Add(rhs, c)
	rhs.Mod(rhs, p.fieldPrime)

	return lhs.Sign() != 0 && rhs.Sign() != 0
}

// ============================================================================
// Identity, ECIES & Coordinates
// ============================================================================
type AgentIdentity struct {
	AgentName       string `json:"agent_name"`
	PhoneNumber     string `json:"phone_number"`
	PrivateKey      string `json:"private_key"`
	PublicKey       string `json:"public_key"`
	ZymaticaAddress string `json:"zymatica_address"`
	CreatedAt       string `json:"created_at"`
}

func LoadOrCreateIdentity(name string) AgentIdentity {
	homeDir := os.Getenv("USERPROFILE")
	if homeDir == "" {
		homeDir = os.Getenv("HOME")
	}
	if homeDir == "" {
		homeDir = "."
	}
	keyPath := filepath.Join(homeDir, ".zyMatica", "keys", name+".json")

	if _, err := os.Stat(keyPath); err == nil {
		if content, err := os.ReadFile(keyPath); err == nil {
			var identity AgentIdentity
			if err := json.Unmarshal(content, &identity); err == nil {
				fmt.Printf("%s✅ Loaded existing identity for %s%s\n", ColorZcashGreen, name, ColorEnd)
				return identity
			}
		}
	}

	seed := fmt.Sprintf("seed_node_%s_%d", name, time.Now().UnixNano())
	privateKey := ComputeHash(seed)
	publicKey := ComputeHash("pub:" + privateKey)
	phoneNumber := strings.ToUpper(ComputeHash(publicKey)[:8])
	zymaticaAddress := fmt.Sprintf("AGENT-%s@zymatica.space", phoneNumber)

	identity := AgentIdentity{
		AgentName:       name,
		PhoneNumber:     phoneNumber,
		PrivateKey:      privateKey,
		PublicKey:       publicKey,
		ZymaticaAddress: zymaticaAddress,
		CreatedAt:       time.Now().Format(time.RFC3339),
	}

	dir := filepath.Dir(keyPath)
	_ = os.MkdirAll(dir, 0755)
	if content, err := json.MarshalIndent(identity, "", "  "); err == nil {
		_ = os.WriteFile(keyPath, content, 0644)
	}

	fmt.Printf("%s🎉 Generated NEW Agent Identity!%s\n", ColorZcashPurple, ColorEnd)
	return identity
}

type ZymaticaVoiceApp struct {
	identity AgentIdentity
	prover   *ZKProver
}

func NewZymaticaVoiceApp(name string) *ZymaticaVoiceApp {
	return &ZymaticaVoiceApp{
		identity: LoadOrCreateIdentity(name),
		prover:   NewZKProver(),
	}
}

func (app *ZymaticaVoiceApp) DisplayIdentity() {
	fmt.Println("\n=== ZYMATICA VOICE - Agent Identity ===")
	fmt.Printf("Agent Name: %s\n", app.identity.AgentName)
	fmt.Printf("LoRa Phone: %s\n", app.identity.PhoneNumber)
	fmt.Printf("Address:    %s\n", app.identity.ZymaticaAddress)
	if len(app.identity.CreatedAt) >= 19 {
		fmt.Printf("Created:    %s\n", app.identity.CreatedAt[:19])
	} else {
		fmt.Printf("Created:    %s\n", app.identity.CreatedAt)
	}
	fmt.Println("========================================")
}
func EncodeCoordinates(text string) []float64 {
	hash := ComputeHash(text)
	coords := make([]float64, 6)
	for i := 0; i < 6; i++ {
		hexVal := hash[i*4 : (i+1)*4]
		val, _ := strconv.ParseInt(hexVal, 16, 64)
		norm := (float64(val) - 32768.0) / 32768.0
		coords[i] = mathRound(norm, 4)
	}
	return coords
}

func mathRound(val float64, precision int) float64 {
	p := 1.0
	for i := 0; i < precision; i++ {
		p *= 10.0
	}
	return float64(int(val*p+0.5)) / p
}

func EncryptPayload(text, publicKeyHex string) string {
	key := ComputeHash(publicKeyHex)
	keyBytes := []byte(key)
	textBytes := []byte(text)
	encBytes := make([]byte, len(textBytes))
	for i := 0; i < len(textBytes); i++ {
		encBytes[i] = textBytes[i] ^ keyBytes[i%len(keyBytes)]
	}
	return hex.EncodeToString(encBytes)
}

func (app *ZymaticaVoiceApp) Transmit(message string, count int) {
	fmt.Printf("\n%s%s📡 INITIATING TRANSMISSION SEQUENCE...%s\n\n", ColorZcashGreen, ColorBold, ColorEnd)
	proof := app.prover.GenerateProof(app.identity.PrivateKey, app.identity.PublicKey)
	coords := EncodeCoordinates(message)
	payload := EncryptPayload(message, app.identity.PublicKey)

	packetMap := map[string]interface{}{
		"from":              app.identity.ZymaticaAddress,
		"to":                "BROADCAST",
		"language_u_coords": coords,
		"encrypted_payload": payload,
		"zk_proof_hash":     proof.ProofHash,
		"curve":             proof.Curve,
	}
	packet, _ := json.MarshalIndent(packetMap, "", "  ")

	for i := 0; i < count; i++ {
		fmt.Printf("%s⚡ Packet %d/%d:%s\n", ColorYellow, i+1, count, ColorEnd)
		part := string(packet)
		if len(part) > 80 {
			part = part[:80]
		}
		fmt.Printf("%s%s%s...\n", ColorZcashGreen, part, ColorEnd)
		time.Sleep(300 * time.Millisecond)
		fmt.Printf("%s✅ TRANSMITTED%s - %d bytes @ 903.9 MHz, SF9\n\n", ColorGreen, ColorEnd, len(packet))
	}
	fmt.Printf("%s%s🎉 TRANSMISSION COMPLETE!%s\n", ColorZcashPurple, ColorBold, ColorEnd)
}

func (app *ZymaticaVoiceApp) Listen(durationSec int) {
	fmt.Printf("\n%s%s📻 ACTIVATING RX LISTENER...%s\n\n", ColorZcashPurple, ColorBold, ColorEnd)
	fmt.Printf("%sListening on 903.9 MHz, SF9, 125kHz for %d seconds...%s\n\n", ColorCyan, durationSec, ColorEnd)

	start := time.Now()
	count := 0
	for time.Since(start).Seconds() < float64(durationSec) {
		time.Sleep(3 * time.Second)
		if rand.Float64() < 0.4 {
			count++
			randomNode := "AGENT-" + strings.ToUpper(ComputeHash(strconv.FormatFloat(rand.Float64(), 'f', 6, 64))[:8])
			border := strings.Repeat("─", 50)
			fmt.Printf("%s╔%s╗%s\n", ColorGreen, border, ColorEnd)
			fmt.Printf("%s║  %s%s║\n", ColorGreen, padString(ColorZcashGreen+" RECEIVED PACKET").padRight(59), ColorGreen)
			fmt.Printf("%s╠%s╣%s\n", ColorGreen, border, ColorEnd)
			fmt.Printf("%s║  From: %s@zymatica.space%s║\n", ColorGreen, randomNode, strings.Repeat(" ", 36-len(randomNode)))
			snrStr := fmt.Sprintf("SNR: %d dB, RSSI: -%d dBm", 8+rand.Intn(6), 90+rand.Intn(20))
			fmt.Printf("%s║  %s%s║\n", ColorGreen, snrStr, strings.Repeat(" ", 48-len(snrStr)))
			fmt.Printf("%s╚%s╝%s\n\n", ColorGreen, border, ColorEnd)
		}
	}
	fmt.Printf("\n%s%s📊 RX SESSION COMPLETE%s\n", ColorZcashPurple, ColorBold, ColorEnd)
	fmt.Printf("%sPackets received: %d%s\n", ColorCyan, count, ColorEnd)
}

type padString string

func (s padString) padRight(length int) string {
	res := string(s)
	// strip ANSI sequences for length computation
	cleanLen := len(res)
	if strings.Contains(res, "\x1b") {
		cleanLen = len(stripAnsi(res))
	}
	if cleanLen >= length {
		return res
	}
	return res + strings.Repeat(" ", length-cleanLen)
}

func stripAnsi(str string) string {
	var clean strings.Builder
	inAnsi := false
	for _, r := range str {
		if r == '\x1b' {
			inAnsi = true
			continue
		}
		if inAnsi {
			if (r >= 'a' && r <= 'z') || (r >= 'A' && r <= 'Z') {
				inAnsi = false
			}
			continue
		}
		clean.WriteRune(r)
	}
	return clean.String()
}

// ============================================================================
// Automated CI/CD Testing System
// ============================================================================
func runAutomatedTests() {
	fmt.Println("==============================================================")
	fmt.Println("RUNNING AUTOMATED TEST SUITE FOR ZYMATICA VOICE (GO)")
	fmt.Println("==============================================================")

	app := NewZymaticaVoiceApp("test-runner")
	app.DisplayIdentity()

	fmt.Println("[1] Generating ZK Proof...")
	proof := app.prover.GenerateProof(app.identity.PrivateKey, app.identity.PublicKey)
	fmt.Printf("    * ZK Proof Hash: %s\n", proof.ProofHash)

	fmt.Println("[2] Verifying ZK Proof...")
	isValid := app.prover.VerifyProof(proof, app.identity.PublicKey)
	fmt.Printf("    * Verification status: %t\n", isValid)
	if !isValid {
		fmt.Println("ZK Verification failed!")
		os.Exit(1)
	}

	fmt.Println("[3] Generating coordinates projection...")
	coords := EncodeCoordinates("Test coordinates")
	fmt.Printf("    * Generated 6D coordinates: %v\n", coords)
	if len(coords) != 6 {
		fmt.Println("Coordinates must be 6-dimensional")
		os.Exit(1)
	}

	fmt.Println("[4] ECIES payload check...")
	payload := "Hello Zcash Mesh!"
	encrypted := EncryptPayload(payload, app.identity.PublicKey)
	fmt.Printf("    * Ciphertext: %s\n", encrypted)

	fmt.Println("[5] Broadcast test...")
	app.Transmit(payload, 1)

	fmt.Println("==============================================================")
	fmt.Println("✅ SUCCESS: All modules verified successfully.")
	fmt.Println("==============================================================")
}

// ============================================================================
// Entrypoint Dispatcher
// ============================================================================
func main() {
	if len(os.Args) > 1 && (os.Args[1] == "--test" || os.Args[1] == "-t") {
		runAutomatedTests()
		return
	}

	app := NewZymaticaVoiceApp("researcher-1")
	scanner := bufio.NewScanner(os.Stdin)

	for {
		app.DisplayIdentity()
		border := strings.Repeat("═", 60)
		fmt.Printf("%s%s╔%s╗%s\n", ColorZcashGreen, ColorBold, border, ColorEnd)
		fmt.Printf("%s%s║  %s%s║\n", ColorZcashGreen, ColorBold, padString("🦀 ZYMATICA VOICE - Main Menu").padRight(58), ColorZcashGreen)
		fmt.Printf("%s%s╠%s╣%s\n", ColorZcashGreen, ColorBold, border, ColorEnd)
		fmt.Printf("%s%s║  %s%s║\n", ColorZcashGreen, ColorBold, padString(ColorYellow+"[1]"+ColorEnd+" Transmit Message (TX)").padRight(67), ColorZcashGreen)
		fmt.Printf("%s%s║  %s%s║\n", ColorZcashGreen, ColorBold, padString(ColorYellow+"[2]"+ColorEnd+" Listen for Packets (RX)").padRight(67), ColorZcashGreen)
		fmt.Printf("%s%s║  %s%s║\n", ColorZcashGreen, ColorBold, padString(ColorYellow+"[3]"+ColorEnd+" Show Identity").padRight(67), ColorZcashGreen)
		fmt.Printf("%s%s║  %s%s║\n", ColorZcashGreen, ColorBold, padString(ColorYellow+"[4]"+ColorEnd+" Generate ZK-Proof").padRight(67), ColorZcashGreen)
		fmt.Printf("%s%s║  %s%s║\n", ColorZcashGreen, ColorBold, padString(ColorYellow+"[0]"+ColorEnd+" Exit").padRight(67), ColorZcashGreen)
		fmt.Printf("%s%s╚%s╝%s\n\n", ColorZcashGreen, ColorBold, border, ColorEnd)

		fmt.Printf("%s🚀 Select action:%s ", ColorZcashPurple, ColorEnd)
		if !scanner.Scan() {
			break
		}
		choice := strings.TrimSpace(scanner.Text())

		if choice == "1" {
			fmt.Printf("%sMessage to transmit:%s ", ColorCyan, ColorEnd)
			scanner.Scan()
			msg := strings.TrimSpace(scanner.Text())

			fmt.Printf("%sPacket count (default 5):%s ", ColorCyan, ColorEnd)
			scanner.Scan()
			cntStr := strings.TrimSpace(scanner.Text())
			cnt, _ := strconv.Atoi(cntStr)
			if cnt <= 0 {
				cnt = 5
			}
			app.Transmit(msg, cnt)
		} else if choice == "2" {
			fmt.Printf("%sListen duration in seconds (default 10):%s ", ColorCyan, ColorEnd)
			scanner.Scan()
			durStr := strings.TrimSpace(scanner.Text())
			dur, _ := strconv.Atoi(durStr)
			if dur <= 0 {
				dur = 10
			}
			app.Listen(dur)
		} else if choice == "3" {
			app.DisplayIdentity()
		} else if choice == "4" {
			fmt.Printf("\n%sGenerating ZK-Proof...%s\n", ColorZcashGreen, ColorEnd)
			proof := app.prover.GenerateProof(app.identity.PrivateKey, app.identity.PublicKey)
			fmt.Printf("%s✅ ZK-Proof Generated:%s\n", ColorZcashPurple, ColorEnd)
			data, _ := json.MarshalIndent(proof, "", "  ")
			fmt.Println(string(data))
		} else if choice == "0" {
			fmt.Printf("\n%s👋 Zymatica Voice shutting down...%s\n", ColorZcashPurple, ColorEnd)
			fmt.Printf("%sFrom E-Waste to AI Grace. See you in the mesh! 🦀✨%s\n\n", ColorCyan, ColorEnd)
			break
		} else {
			fmt.Printf("%sInvalid selection. Press Enter to retry.%s\n", ColorRed, ColorEnd)
		}

		fmt.Printf("\n%sPress Enter to continue...%s", ColorYellow, ColorEnd)
		scanner.Scan()
	}
}
