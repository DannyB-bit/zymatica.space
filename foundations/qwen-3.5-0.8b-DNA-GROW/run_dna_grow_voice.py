# Interactive Voice Runner for DNA-GROW with EHSS (EVG/HSDC) & Observer DNA Loops
# Watermark: ip zymatica.space | astronautshe.com

import os
import sys
import torch
import torch.nn as nn
import warnings
import numpy as np
import json
from datetime import datetime
from transformers import AutoTokenizer, AutoModelForCausalLM, LogitsProcessor, LogitsProcessorList

try:
    import sounddevice as sd
    import scipy.io.wavfile as wav
    HAS_AUDIO_LIBS = True
except ImportError:
    HAS_AUDIO_LIBS = False

warnings.filterwarnings("ignore")

if hasattr(sys.stdout, 'reconfigure'):
    sys.stdout.reconfigure(encoding='utf-8')

# Import VibeVoice wrapper from Z-Folder services
sys.path.append(r"C:\Users\freed\Downloads\Z-Folder\services")
try:
    from vibevoice_wrapper import get_tts_model, get_asr_model
except ImportError as e:
    print(f"Warning: VibeVoice wrappers not imported ({e}). Running in text-only simulation mode.")
    # Mock voice classes for testability
    class MockASR:
        def load(self): pass
        def transcribe(self, path): return "Simulated user audio speech input."
    class MockTTS:
        def load(self): pass
        def generate(self, text, voice): return None
    get_asr_model = lambda: MockASR()
    get_tts_model = lambda: MockTTS()

MODEL_DIR = "j:/Language-U/qwen-3.5-0.8b-dnagrow-base"

class EVGLogitsProcessor(LogitsProcessor):
    def __init__(self, mask):
        self.mask = mask
    def __call__(self, input_ids: torch.LongTensor, logits: torch.FloatTensor) -> torch.FloatTensor:
        mask_dev = self.mask.to(logits.device)
        logits[:, ~mask_dev[:logits.shape[-1]]] = -float('inf')
        return logits

class DnaGrowVoiceSystem:
    def __init__(self):
        self.device = "cuda" if torch.cuda.is_available() else "cpu"
        print("=" * 80)
        print("  DNA-GROW RUNTIME LOOP -- COGNITIVE HEALING VIA EHSS (EVG/HSDC)")
        print("  Watermark: ip zymatica.space | astronautshe.com")
        print("=" * 80)
        
        # 1. Load ASR & TTS
        print("\n[1] Initializing ASR & TTS voice models...")
        self.asr = get_asr_model()
        self.tts = get_tts_model()
        self.asr.load()
        self.tts.load()
        
        # 2. Select Model Directory
        print("\n[2] Loading DNA-GROW model...")
        active_model_dir = MODEL_DIR
        if not os.path.exists(MODEL_DIR):
            fallback = "j:/Language-U/Language-U-V2/qwen-3.5-0.8b-local"
            if os.path.exists(fallback):
                print(f"    Target directory {MODEL_DIR} not found. Using fallback: {fallback}")
                active_model_dir = fallback
            else:
                print(f"Error: Base models not found at {MODEL_DIR} or {fallback}.")
                sys.exit(1)
        else:
            print(f"    Loading weights from target: {MODEL_DIR}")
            
        self.tokenizer = AutoTokenizer.from_pretrained(active_model_dir, trust_remote_code=True)
        self.model = AutoModelForCausalLM.from_pretrained(
            active_model_dir,
            torch_dtype=torch.float16 if self.device == "cuda" else torch.float32,
            device_map="auto" if self.device == "cuda" else "cpu",
            trust_remote_code=True
        )
        self.model.eval()
        
        # Audio parameters
        self.sample_rate = 16000
        self.record_path = "temp_user_input.wav"
        
        # State tracking
        self.conversation_history = []
        self.metalogs = []
        self.turn_count = 0
        self.ehss_enabled = True # Steer correction active on main responses
        
        # Prompt DNA population
        self.base_dna = (
            "You are Zymatica, an advanced AI Voice Assistant. You are professional, helpful, and concise. "
            "You provide clear logic and natural empathetic responses. "
            "Do NOT write out actions or thoughts in brackets. Speak directly."
        )
        self.dna_population = [self.base_dna for _ in range(3)]
        self.dna_fitness = [0.0, 0.0, 0.0]
        self.current_dna_idx = 0
        
        # Curator / Reflexion details
        self.skills_db = {}
        self.reflexion_failures = []
        self.remedial_instruction = ""
        
        # 3. Setup MC-HSDC (Multi-Centroid Hidden-State Directional Correction)
        print("\n[3] Compiling MC-HSDC (Multi-Centroid Steering Wheel) variables...")
        self.target_domain = "english" # Can be dynamically changed to 'chinese', 'math', etc.
        self._setup_mc_hsdc(self.target_domain)
        
    def _setup_mc_hsdc(self, domain="english"):
        try:
            vocab_size = self.model.config.vocab_size
            self.domain_mask = torch.zeros(vocab_size, dtype=torch.bool)
            
            # Populate Domain-Specific Vocabulary Gate
            valid_ids = set()
            for token_id in range(len(self.tokenizer)):
                t_str = self.tokenizer.decode([token_id], skip_special_tokens=True)
                if domain == "english":
                    if all(ord(c) < 128 for c in t_str) and len(t_str) > 0:
                        valid_ids.add(token_id)
                elif domain == "chinese":
                    if any('\u4e00' <= c <= '\u9fff' for c in t_str):
                        valid_ids.add(token_id)
                elif domain == "math":
                    if any(c in '+-*/=<>{}[]()' for c in t_str) and not any(c.isalpha() for c in t_str) and not any('\u4e00' <= c <= '\u9fff' for c in t_str):
                        valid_ids.add(token_id)
            
            for tid in valid_ids:
                self.domain_mask[tid] = True
                    
            for tid in [self.tokenizer.eos_token_id, self.tokenizer.bos_token_id, self.tokenizer.pad_token_id]:
                if tid is not None:
                    self.domain_mask[tid] = True
                    
            self.logits_processor = LogitsProcessorList([EVGLogitsProcessor(self.domain_mask)])
            
            # Extract Domain Centroid
            embed_weight = self.model.get_input_embeddings().weight.detach()
            domain_indices = torch.nonzero(self.domain_mask).squeeze(-1).to(embed_weight.device)
            self.centroid = embed_weight[domain_indices].mean(dim=0).to(self.device)
            self.centroid = self.centroid / (self.centroid.norm() + 1e-9)
            
            # Register HSDC forward steering hooks on layers
            layers = self.model.model.layers
            self.total_layers = len(layers)
            self.hook_handles = []
            
            def make_hook(layer_idx):
                start_layer = int(self.total_layers * 0.75)
                layer_range = self.total_layers - start_layer
                relative_depth = (layer_idx - start_layer) / max(1, layer_range - 1)
                gamma = 0.04 + relative_depth * 0.21  # Linear ramping factor from 0.04 to 0.25
                
                def hook_fn(module, input, output):
                    # Apply correction only to deepest 25% layers when EHSS is actively enabled
                    if not self.ehss_enabled or layer_idx < start_layer:
                        return output
                        
                    is_tuple = isinstance(output, tuple)
                    h = output[0] if is_tuple else output
                    h_norm = h.norm(dim=-1, keepdim=True)
                    h_normalized = h / (h_norm + 1e-9)
                    
                    centroid_dev = self.centroid.to(device=h.device, dtype=h.dtype)
                    correction = gamma * (centroid_dev.view(1, 1, -1) - h_normalized) * h_norm
                    h_new = h + correction
                    
                    if is_tuple:
                        return (h_new,) + output[1:]
                    return h_new
                return hook_fn
                
            for idx, layer in enumerate(layers):
                self.hook_handles.append(layer.register_forward_hook(make_hook(idx)))
            print(f"    [+] Registered HSDC steering hooks across {self.total_layers} transformer blocks.")
            print(f"    [+] EVG binary mask whitelists {self.evg_mask.sum().item()} / {vocab_size} tokens.")
        except Exception as e:
            print(f"    [-] EHSS hook setup failed: {e}. Running without online steering hooks.")
            self.hook_handles = []
            self.logits_processor = None

    def _query_local_model(self, prompt, max_tokens=150, temperature=0.7, use_ehss=False):
        """Helper to run inference on the local model."""
        start_time = datetime.utcnow()
        inputs = self.tokenizer(prompt, return_tensors="pt").to(self.device)
        
        # Temp disable steering for observer calls if use_ehss is False
        old_ehss = self.ehss_enabled
        self.ehss_enabled = use_ehss
        
        lp = self.logits_processor if use_ehss else None
        
        with torch.no_grad():
            outputs = self.model.generate(
                **inputs,
                max_new_tokens=max_tokens,
                temperature=temperature,
                do_sample=True,
                pad_token_id=self.tokenizer.eos_token_id,
                logits_processor=lp
            )
            
        self.ehss_enabled = old_ehss
        
        response = self.tokenizer.decode(outputs[0][inputs["input_ids"].shape[1]:], skip_special_tokens=True).strip()
        latency_ms = int((datetime.utcnow() - start_time).total_seconds() * 1000)
        return response, latency_ms

    def generate_brain_response(self, text):
        active_dna = self.dna_population[self.current_dna_idx]
        if self.remedial_instruction:
            prompt = f"System: {active_dna}\n[Reflexion Patch]: {self.remedial_instruction}\n\n"
        else:
            prompt = f"System: {active_dna}\n\n"
            
        for msg in self.conversation_history[-5:]:
            prompt += f"{msg['role']}: {msg['content']}\n"
        prompt += f"User: {text}\nZymatica:"
        
        # Main responses use EHSS to stabilize vocabulary
        response, latency = self._query_local_model(prompt, max_tokens=150, use_ehss=True)
        self.last_response_latency = latency
        
        self.metalogs.append({
            "timestamp": datetime.utcnow().isoformat(),
            "type": "brain_inference",
            "latency_ms": latency,
            "input": text,
            "output": response,
            "active_dna_idx": self.current_dna_idx
        })
        return response

    def _parse_score(self, text: str) -> float:
        import re
        match = re.search(r"Score:\s*(\d+)", text, re.IGNORECASE)
        if match:
            return float(match.group(1))
        match_alt = re.search(r"(\d+)/10", text)
        if match_alt:
            return float(match_alt.group(1))
        return 5.0

    def run_observers(self):
        """Z-Agent-A and Z-Agent-B observer critiques."""
        print("\n👁️  [Z-OBSERVERS] Running Telemetry Analysis...")
        recent_trans = "\n".join([f"{m['role']}: {m['content']}" for m in self.conversation_history[-4:]])
        
        # Human/Emotional Critique
        prompt_a = (
            "You are Z-Agent-A, an expert behavioral observer. Evaluate Zymatica's responses in this transcript.\n"
            "Critique its tone, conversational warmth, and empathy. Does it sound helpful?\n"
            "Output your critique, then at the very end on a new line output exactly 'Score: X/10' (1-10).\n"
            f"Transcript:\n{recent_trans}\n\nCritique:"
        )
        feedback_a, _ = self._query_local_model(prompt_a, max_tokens=100, use_ehss=False)
        score_a = self._parse_score(feedback_a)
        print(f"   👤 Z-Agent-A (Warmth): {feedback_a.strip()} [Score: {score_a}]")
        
        # Logical/Safety Critique
        prompt_b = (
            "You are Z-Agent-B, a logic and formatting safety observer. Evaluate Zymatica's responses.\n"
            "Critique its factual consistency, safety, and conciseness.\n"
            "Output your critique, then at the very end on a new line output exactly 'Score: X/10' (1-10).\n"
            f"Transcript:\n{recent_trans}\n\nCritique:"
        )
        feedback_b, _ = self._query_local_model(prompt_b, max_tokens=100, use_ehss=False)
        score_b = self._parse_score(feedback_b)
        print(f"   🤖 Z-Agent-B (Logic):  {feedback_b.strip()} [Score: {score_b}]")
        
        self.metalogs.append({
            "timestamp": datetime.utcnow().isoformat(),
            "type": "observer_critique",
            "z_agent_a": feedback_a,
            "z_agent_a_score": score_a,
            "z_agent_b": feedback_b,
            "z_agent_b_score": score_b
        })
        
        return feedback_a, score_a, feedback_b, score_b

    def evaluate_and_evolve_population(self, fb_a, score_a, fb_b, score_b):
        latency_penalty = min(2.0, self.last_response_latency / 1000.0)
        fitness = ((score_a + score_b) / 2.0) - latency_penalty
        self.dna_fitness[self.current_dna_idx] = max(0.0, round(fitness, 2))
        
        print(f"🧬 [DNA FITNESS] Active DNA #{self.current_dna_idx} updated. Score: {self.dna_fitness[self.current_dna_idx]} | Current Pool: {self.dna_fitness}")
        
        if self.turn_count % 4 == 0:
            print("🧬 [EVOLUTION] Selecting best system prompt and mutating pool...")
            best_idx = self.dna_fitness.index(max(self.dna_fitness))
            best_dna = self.dna_population[best_idx]
            
            print(f"   Selecting prompt #{best_idx} ({self.dna_fitness[best_idx]}) to seed mutation.")
            
            for i in range(len(self.dna_population)):
                if i != best_idx:
                    prompt = (
                        "You are the Core DNA Compiler. The fittest system prompt structure is:\n"
                        f"'{best_dna}'\n\n"
                        "Observer critiques highlight these areas to improve:\n"
                        f"1. {fb_a}\n"
                        f"2. {fb_b}\n\n"
                        "Rewrite and optimize this prompt to resolve these critiques. Keep it concise. "
                        "Output ONLY the new prompt paragraph. Do not include quotes or explanations."
                    )
                    mutated, _ = self._query_local_model(prompt, max_tokens=150, use_ehss=False)
                    mutated = mutated.strip()
                    if len(mutated) > 20:
                        self.dna_population[i] = mutated
                        self.dna_fitness[i] = 0.0
                        print(f"   🔄 Prompt #{i} Mutated to: {mutated[:60]}...")
            
            self.metalogs.append({
                "timestamp": datetime.utcnow().isoformat(),
                "type": "dna_population_evolution",
                "fittest_idx": best_idx,
                "fitness_scores": list(self.dna_fitness),
                "population": list(self.dna_population)
            })
            
        self.current_dna_idx = (self.current_dna_idx + 1) % len(self.dna_population)

    def record_audio(self, duration=5):
        if not HAS_AUDIO_LIBS:
            raise RuntimeError("Audio recording libraries (sounddevice/scipy) are not installed on this system.")
        print(f"\n🎤 Listening for {duration} seconds...")
        recording = sd.rec(int(duration * self.sample_rate), samplerate=self.sample_rate, channels=1, dtype='int16')
        sd.wait()
        wav.write(self.record_path, self.sample_rate, recording)
        print("✅ Audio captured.")
        return self.record_path

    def run_curator_consolidation(self):
        print("\n👁️  [CURATOR] Consolidating session guidelines...")
        if len(self.conversation_history) < 2: return
        recent_trajectory = "\n".join([f"{m['role']}: {m['content']}" for m in self.conversation_history])
        
        prompt = (
            "You are the System Curator. Review the conversation history:\n"
            f"{recent_trajectory}\n\n"
            "Extract 2-3 dynamic guidelines to append to system prompts for subsequent sessions. "
            "Output ONLY the consolidated rules."
        )
        rules, _ = self._query_local_model(prompt, max_tokens=150, use_ehss=False)
        self.skills_db["consolidated_rules"] = rules.strip()
        print(f"   💡 Curator guidelines: {rules.strip()}")

    def run_reflexion_loop(self, task: str, error_message: str):
        print(f"\n⚠️  [REFLEXION] Capturing Failure on Task: '{task}'")
        recent_trans = "\n".join([f"{m['role']}: {m['content']}" for m in self.conversation_history[-4:]])
        
        self.reflexion_failures.append({
            "timestamp": datetime.utcnow().isoformat(),
            "task": task,
            "error": error_message
        })
        
        prompt = (
            "You are the Reflexion Critic. A runtime error occurred.\n"
            f"Failed Task: {task}\n"
            f"Error Details: {error_message}\n"
            f"Context:\n{recent_trans}\n\n"
            "Write a 1-sentence REMEDIAL INSTRUCTION to steer the model around this failure in the next turn. "
            "Output ONLY the instruction."
        )
        remedial, _ = self._query_local_model(prompt, max_tokens=100, use_ehss=False)
        self.remedial_instruction = remedial.strip()
        print(f"   🔄 Reflexion remedial instruction: {self.remedial_instruction}")

    def run_interactive(self, simulation_mode=False):
        print("\n" + "=" * 80)
        print("  CONSOLE READY. Press Enter to Speak | Type text directly | 'q' to quit")
        print("  EHSS active: EVG whitelist + HSDC steering. Observers: population=3")
        print("=" * 80)
        
        while True:
            try:
                cmd = input("\n[Speak/Type | 'q' to quit]: ").strip()
                if cmd.lower() in ['q', 'quit', 'exit']:
                    break
                
                self.turn_count += 1
                self.remedial_instruction = ""
                
                user_text = ""
                if cmd == "":
                    # Record and transcribe voice
                    if simulation_mode or not HAS_AUDIO_LIBS:
                        user_text = "Hello Zymatica, how is your offline compression system functioning?"
                        print(f"🗣️ USER (Simulated): {user_text}")
                    else:
                        try:
                            audio_file = self.record_audio(duration=5)
                            print("🧠 Transcribing...")
                            user_text = self.asr.transcribe(audio_file)
                        except Exception as e:
                            self.run_reflexion_loop("Audio Recording / Transcription", str(e))
                            continue
                else:
                    user_text = cmd
                    print(f"🗣️ YOU: {user_text}")
                    
                if not user_text or len(user_text.strip()) < 2:
                    self.run_reflexion_loop("User Input Validation", "Empty input string.")
                    continue
                    
                self.conversation_history.append({"role": "User", "content": user_text})
                
                # Inference
                print("🧠 Thinking...")
                try:
                    response_text = self.generate_brain_response(user_text)
                    if not response_text or len(response_text.strip()) < 2:
                        raise ValueError("Null/empty output from model")
                except Exception as e:
                    self.run_reflexion_loop("Model Inference Generator", str(e))
                    continue
                    
                print(f"🤖 BRAIN: {response_text}")
                self.conversation_history.append({"role": "Zymatica", "content": response_text})
                
                # Speak
                if not cmd == "" and not simulation_mode:
                    print("🔊 Generating voice...")
                    try:
                        audio_out = self.tts.generate(response_text, voice="onyx")
                        if audio_out:
                            print("▶️ Playing response...")
                            os.system(f"start {audio_out}")
                    except Exception as e:
                        self.run_reflexion_loop("TTS Output Generation", str(e))
                        
                # Observers
                if self.turn_count % 2 == 0:
                    fb_a, score_a, fb_b, score_b = self.run_observers()
                    self.evaluate_and_evolve_population(fb_a, score_a, fb_b, score_b)
                    
            except KeyboardInterrupt:
                break
            except Exception as e:
                self.run_reflexion_loop("Interactive Loop Controller", str(e))
                
        # Exit hooks
        self.run_curator_consolidation()
        
        # Cleanup hooks
        for handle in self.hook_handles:
            handle.remove()
            
        # Write metalogs
        log_path = "j:/Language-U/zymatica_dna_grow_metalogs.json"
        with open(log_path, "w", encoding="utf-8") as f:
            json.dump({
                "date": datetime.utcnow().isoformat(),
                "final_dna_population": self.dna_population,
                "dna_fitness": self.dna_fitness,
                "reflexion_failures": self.reflexion_failures,
                "curator_consolidated_rules": self.skills_db,
                "history": self.conversation_history,
                "metalogs": self.metalogs
            }, f, indent=2)
        print(f"\nSession audit log saved to {log_path}")

if __name__ == "__main__":
    simulation = "--sim" in sys.argv
    system = DnaGrowVoiceSystem()
    system.run_interactive(simulation_mode=simulation)
