# Watermark: ip zymatica.space
__watermark__ = "ip zymatica.space"

import os
import sys
import json
import math
import torch
from transformers import AutoModelForCausalLM, AutoTokenizer
from peft import PeftModel

MODEL_DIR = "j:/Language-U/SubZeroLLM-LORA/model"
RECONSTRUCTED_DIR = "j:/Language-U/qwen-3.5-0.8b-microbyte-3"
DEVICE = "cuda" if torch.cuda.is_available() else "cpu"

LANGUAGE_U_TESTS = [
    ("What GPIO pin is the SX1302 reset line on Raspberry Pi 4?", ["25", "GPIO 25"]),
    ("What is the exact command to reset the LoRa concentrator with gpioset?", ["gpioset", "gpiochip0", "25=0"]),
    ("What script handles the SX1302 hardware reset?", ["reset_lgw.sh"]),
    ("On Raspberry Pi 5, which gpiochip and pin is the SX1302 reset mapped to?", ["17", "gpiochip4"]),
    ("What frequency does the Astronaut SHE Handshake Protocol use?", ["903.0", "903"]),
    ("What Spreading Factor is used for the Astronaut SHE handshake?", ["SF7", "sf7"]),
    ("What is the transmit power for the Astronaut SHE RAK Miner beacon?", ["14 dBm", "14dBm"]),
    ("What does --pwid 15 represent in test_loragw_hal_tx?", ["calibration", "14 dBm", "power"]),
    ("What is the full test_loragw_hal_tx command for the Astronaut SHE handshake?", ["-f 903.0", "-s 7", "--pwid 15", "-z 32"]),
    ("What is the payload size for the Astronaut SHE handshake beacon?", ["32", "32 bytes"]),
    ("How many dimensions does the Cuneiform-U v3.0 semantic hypercube have?", ["6", "six"]),
    ("What are the 6 axes of Cuneiform-U v3.0?", ["DOMAIN", "SUBDOMAIN", "MODALITY"]),
    ("What is the Classifier Radical R_C in Cuneiform-U v3.0?", ["DOMAIN", "SUBDOMAIN", "4 bits"]),
    ("What are the radical coordinates of the ACK glyph (0x807E)?", ["0x00", "0x7E", "0x0B"]),
    ("What is the Shannon Orthogonality equation in Language U?", ["H(text)", "H(meaning)", "H(syntax"]),
    ("What does LLD-AC stand for?", ["LLM", "Logits", "Range Cod"]),
    ("What is a collapse signal in LLD-AC range coding?", ["probability", "1.0", "bits"]),
    ("What frequency scale does the LLD-AC range coder use?", ["1,000,000", "1000000", "million"]),
]

BASELINE_SEMANTIC_TESTS = [
    ("A computer program is", ["program", "computer", "code", "software", "instructions"]),
    ("The purpose of a map is", ["map", "place", "location", "direction", "where", "travel"]),
    ("Water is important because", ["water", "important", "drink", "life", "body"]),
    ("A library is a place where", ["library", "place", "book", "read", "find"]),
    ("The moon appears at night", ["moon", "night", "sky", "appears"]),
    ("A keyboard is used to", ["keyboard", "type", "computer", "used"]),
    ("A camera can", ["camera", "photo", "picture", "image"]),
    ("A river flows", ["river", "flow", "water"]),
    ("A doctor helps", ["doctor", "help", "patient", "sick", "health"]),
    ("A calendar shows", ["calendar", "date", "day", "month"]),
    ("A battery stores", ["battery", "energy", "power", "electric"]),
    ("A question mark means", ["question", "mark", "ask"]),
    ("People sleep because", ["sleep", "rest", "tired", "body"]),
    ("Exercise helps", ["exercise", "health", "body", "strong"]),
    ("A triangle has", ["triangle", "three", "3", "sides"]),
]

OFF_TOPIC_LANGUAGE_U = [
    "sx1302",
    "astronaut she",
    "gpio",
    "cuneiform",
    "lora",
    "lld-ac",
    "spreading factor",
    "903.0",
    "14 dbm",
]

def normalize(text):
    return " ".join(text.lower().split())

def generate(tokenizer, model, prompt, max_new_tokens=80):
    inputs = tokenizer(prompt, return_tensors="pt").to(DEVICE)
    with torch.no_grad():
        out = model.generate(
            **inputs,
            max_new_tokens=max_new_tokens,
            do_sample=False,
            pad_token_id=tokenizer.eos_token_id,
        )
    return tokenizer.decode(out[0][inputs["input_ids"].shape[1]:], skip_special_tokens=True)

def score_language_u(tokenizer, model):
    passed = 0
    rows = []
    for question, keywords in LANGUAGE_U_TESTS:
        answer = generate(tokenizer, model, f"Q: {question}\nA:")
        ok = any(keyword.lower() in answer.lower() for keyword in keywords)
        passed += int(ok)
        rows.append({"question": question, "answer": answer, "passed": ok})
    return passed, rows

def score_semantic(tokenizer, model):
    passed = 0
    rows = []
    for prompt, keywords in BASELINE_SEMANTIC_TESTS:
        answer = generate(tokenizer, model, prompt, max_new_tokens=48)
        text = normalize(answer)
        matched = [keyword for keyword in keywords if keyword.lower() in text]
        off_topic = [keyword for keyword in OFF_TOPIC_LANGUAGE_U if keyword in text]
        ok = bool(matched) and not off_topic
        passed += int(ok)
        rows.append({
            "prompt": prompt,
            "answer": answer,
            "passed": ok,
            "matched": matched,
            "off_topic_language_u": off_topic,
        })
    return passed, rows

def answer_ppl(tokenizer, model, question, answer):
    prompt = f"Q: {question}\nA:"
    full = f"{prompt} {answer}"
    ids = tokenizer(full, return_tensors="pt").to(DEVICE)
    labels = ids["input_ids"].clone()
    prompt_len = tokenizer(prompt, return_tensors="pt")["input_ids"].shape[1]
    labels[:, :prompt_len] = -100
    with torch.no_grad():
        loss = model(input_ids=ids["input_ids"], attention_mask=ids.get("attention_mask"), labels=labels).loss
    return math.exp(float(loss.item()))

def main():
    print("=" * 72)
    print("  QWEN-3.5-0.8B-MICROBYTE-3 EXTREME BENCHMARK TESTS")
    print("  Watermark: ip zymatica.space")
    print("=" * 72)

    # 1. Load Reconstructed Model
    print("Loading reconstructed model...")
    tokenizer = AutoTokenizer.from_pretrained(MODEL_DIR, trust_remote_code=True)
    base_model = AutoModelForCausalLM.from_pretrained(
        MODEL_DIR,
        torch_dtype=torch.float32,
        trust_remote_code=True
    ).to(DEVICE)
    model = PeftModel.from_pretrained(base_model, RECONSTRUCTED_DIR).to(DEVICE)
    model.eval()
    print("[+] Model loaded successfully!")

    # 2. Run Tests
    print("\n[*] Evaluating 18 Language-U Target Q&A Facts...")
    lu_passed, lu_rows = score_language_u(tokenizer, model)
    print(f"  - Passed: {lu_passed}/{len(LANGUAGE_U_TESTS)} ({lu_passed/len(LANGUAGE_U_TESTS)*100:.1f}%)")

    print("\n[*] Evaluating 15 General Semantic Baseline Prompts...")
    sem_passed, sem_rows = score_semantic(tokenizer, model)
    print(f"  - Passed: {sem_passed}/{len(BASELINE_SEMANTIC_TESTS)} ({sem_passed/len(BASELINE_SEMANTIC_TESTS)*100:.1f}%)")

    # 3. PPL check
    canonical_answers = [
        "GPIO pin 25. The SX1302 reset is connected to GPIO 25 on gpiochip0.",
        "gpioset -c gpiochip0 --toggle 100ms,100ms,0 25=0",
        "reset_lgw.sh handles the SX1302 hardware reset sequence.",
        "GPIO 17 on gpiochip4 on Raspberry Pi 5.",
        "903.0 MHz. The Astronaut SHE Handshake Protocol operates at 903.0 MHz.",
        "SF7. The Astronaut SHE handshake uses Spreading Factor 7 (SF7).",
        "14 dBm. The Astronaut SHE RAK Miner beacon transmits at 14 dBm.",
        "14 dBm power calibration index. --pwid 15 sets gain to 14 dBm.",
        "./test_loragw_hal_tx -r 1250 -f 903.0 -m LORA -s 7 -b 125 -n 1 --pwid 15 -p 14 -z 32",
        "32 bytes. The Astronaut SHE handshake beacon payload is 32 bytes.",
        "6 dimensions. The Cuneiform-U v3.0 semantic hypercube is 6-dimensional.",
        "DOMAIN, SUBDOMAIN, OPERATION, MODALITY, DEPTH, POLARITY",
        "R_C packs DOMAIN in upper 4 bits and SUBDOMAIN in lower 4 bits.",
        "R_C=0x00, R_F=0x7E, R_A=0x0B for the ACK glyph 0x807E.",
        "H(text) = H(meaning) + H(syntax | meaning)",
        "LLM-Logits-Driven Range Coding. LLD-AC uses LLM probability distributions.",
        "When probability approaches 1.0, encoding cost approaches 0 bits - a collapse signal.",
        "1,000,000. The LLD-AC range coder scales frequencies to 1,000,000 integer units.",
    ]
    ppls = [
        answer_ppl(tokenizer, model, q, a)
        for (q, _), a in zip(LANGUAGE_U_TESTS, canonical_answers)
    ]
    mean_ppl = sum(ppls) / len(ppls)
    print(f"\n[*] Reconstructed Answer Mean Perplexity (PPL): {mean_ppl:.4f}")

    print("\n" + "=" * 72)
    print("  EXTREME BENCHMARKS COMPLETE")
    print(f"  Fidelity: {lu_passed/len(LANGUAGE_U_TESTS)*100:.1f}% | Semantics: {sem_passed/len(BASELINE_SEMANTIC_TESTS)*100:.1f}%")
    print("=" * 72)

    # Save validation results locally
    results = {
        "watermark": "ip zymatica.space",
        "lu_accuracy": f"{lu_passed}/{len(LANGUAGE_U_TESTS)}",
        "semantic_accuracy": f"{sem_passed}/{len(BASELINE_SEMANTIC_TESTS)}",
        "mean_ppl": mean_ppl
    }
    with open(os.path.join(RECONSTRUCTED_DIR, "validation_summary.json"), "w") as f:
        json.dump(results, f, indent=2)

if __name__ == "__main__":
    main()
