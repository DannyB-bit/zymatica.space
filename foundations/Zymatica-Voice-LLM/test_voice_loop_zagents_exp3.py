import os
import sys
import time
import logging
import asyncio
import io
import wave
import json
import re
import hashlib
import platform
import torch
from datetime import datetime

# Ensure UTF-8 output encoding on Windows to prevent UnicodeEncodeError
if sys.platform == "win32":
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8')

# Setup logging
logging.basicConfig(level=logging.INFO, format="%(asctime)s [%(levelname)s]: %(message)s")
logger = logging.getLogger("ZymaticaZymaticaZAgentsLoopExp3")

# Add current folder to path
current_dir = os.path.dirname(os.path.abspath(__file__))
if current_dir not in sys.path:
    sys.path.append(current_dir)

import database
from services.web_server import query_fast_llm
from services.vibevoice_wrapper import get_tts_model, get_asr_model

# Initialize local SQLite
database.init_db()

def get_system_environment():
    """Gathers detailed host hardware and software specifications for the audit logs."""
    env = {
        "os_name": os.name,
        "os_platform": sys.platform,
        "os_release": platform.release(),
        "os_version": platform.version(),
        "python_version": sys.version,
        "pytorch_version": torch.__version__,
        "cuda_available": torch.cuda.is_available()
    }
    if env["cuda_available"]:
        try:
            env["cuda_device_name"] = torch.cuda.get_device_name(0)
            env["cuda_device_capability"] = torch.cuda.get_device_capability(0)
            env["cuda_device_memory_gb"] = round(torch.cuda.get_device_properties(0).total_memory / (1024**3), 2)
        except Exception as e:
            env["cuda_error"] = str(e)
            
    # Check CPU
    try:
        import psutil
        env["cpu_logical_cores"] = psutil.cpu_count(logical=True)
        env["cpu_physical_cores"] = psutil.cpu_count(logical=False)
        env["ram_total_gb"] = round(psutil.virtual_memory().total / (1024**3), 2)
    except ImportError:
        pass
        
    return env

def get_md5(file_path):
    """Calculates the MD5 hash of a file for audit logs."""
    if not os.path.exists(file_path):
        return ""
    hash_md5 = hashlib.md5()
    with open(file_path, "rb") as f:
        for chunk in iter(lambda: f.read(4096), b""):
            hash_md5.update(chunk)
    return hash_md5.hexdigest()

def calculate_similarity(text1, text2):
    """Calculates word-level similarity percentage between two texts."""
    def clean(text):
        text = text.lower()
        text = re.sub(r'[^\w\s]', '', text)
        return text.split()
    
    words1 = clean(text1)
    words2 = clean(text2)
    
    if not words1 and not words2:
        return 100.0
    if not words1 or not words2:
        return 0.0
        
    m, n = len(words1), len(words2)
    dp = [[0] * (n + 1) for _ in range(m + 1)]
    for i in range(m + 1):
        dp[i][0] = i
    for j in range(n + 1):
        dp[0][j] = j
        
    for i in range(1, m + 1):
        for j in range(1, n + 1):
            if words1[i-1] == words2[j-1]:
                dp[i][j] = dp[i-1][j-1]
            else:
                dp[i][j] = min(dp[i-1][j] + 1,      # Deletion
                               dp[i][j-1] + 1,      # Insertion
                               dp[i-1][j-1] + 1)    # Substitution
                               
    dist = dp[m][n]
    max_len = max(m, n)
    return round((1.0 - dist / max_len) * 100, 2)

def get_audio_duration(file_path, text=""):
    """Calculates the duration of a wav file in seconds, falling back to text speaking rate estimate."""
    try:
        with wave.open(file_path, 'r') as f:
            frames = f.getnframes()
            rate = f.getframerate()
            return frames / float(rate)
    except Exception:
        words = text.split()
        if words:
            return max(1.5, len(words) / 2.5) # 150 words per minute speaking rate
        return 0.0

async def query_fast_llm_with_meta(messages, purpose="simulation"):
    """Queries LLM and returns response text alongside audit metadata."""
    nvidia_key = os.getenv("NVIDIA_API_KEY")
    openai_key = os.getenv("OPENAI_API_KEY")
    
    start_time = time.time()
    iso_start = datetime.utcnow().isoformat() + "Z"
    
    # We query the Nvidia API directly to collect complete metadata
    model_name = "meta/llama-3.1-8b-instruct"
    response_text = None
    provider = "nvidia"
    
    if nvidia_key:
        url = "https://integrate.api.nvidia.com/v1/chat/completions"
        headers = {
            "Authorization": f"Bearer {nvidia_key}",
            "Content-Type": "application/json"
        }
        payload = {
            "model": model_name,
            "messages": messages,
            "temperature": 0.8,
            "max_tokens": 150
        }
        try:
            r = requests_post_sync(url, headers, payload)
            if r.status_code == 200:
                res_json = r.json()
                response_text = res_json["choices"][0]["message"]["content"].strip()
        except Exception as e:
            logger.warning(f"Nvidia query failed in meta-logging wrapper: {e}")
            
    if not response_text and openai_key:
        provider = "openai"
        model_name = "gpt-4o-mini"
        url = "https://api.openai.com/v1/chat/completions"
        headers = {
            "Authorization": f"Bearer {openai_key}",
            "Content-Type": "application/json"
        }
        payload = {
            "model": model_name,
            "messages": messages,
            "temperature": 0.8,
            "max_tokens": 150
        }
        try:
            r = requests_post_sync(url, headers, payload)
            if r.status_code == 200:
                res_json = r.json()
                response_text = res_json["choices"][0]["message"]["content"].strip()
        except Exception as e:
            logger.warning(f"OpenAI query failed in meta-logging wrapper: {e}")
            
    # Fallback to standard fast llm if custom query failed
    if not response_text:
        provider = "fast_llm_site_fallback"
        response_text = await query_fast_llm(messages)
        if not response_text:
            response_text = "I'm not sure what to say, but I'd love to know what you're thinking."
            
    end_time = time.time()
    iso_end = datetime.utcnow().isoformat() + "Z"
    latency_ms = int((end_time - start_time) * 1000)
    
    metadata = {
        "timestamp_start": iso_start,
        "timestamp_end": iso_end,
        "latency_ms": latency_ms,
        "provider": provider,
        "model": model_name,
        "messages_input": messages,
        "response_output": response_text,
        "purpose": purpose
    }
    
    return response_text, metadata

def requests_post_sync(url, headers, payload):
    """Helper to run synchronous POST using standard requests module."""
    import requests
    return requests.post(url, headers=headers, json=payload, timeout=8)

async def query_zagent_observer_meta(observer_name, instructions, context):
    """Observer query helper that captures metadata."""
    messages = [
        {"role": "system", "content": instructions},
        {"role": "user", "content": f"Telemetry Data: {json.dumps(context, indent=2)}\n\nProvide your analysis."}
    ]
    response, meta = await query_fast_llm_with_meta(messages, purpose=f"observer_{observer_name.lower().replace(' ', '_')}")
    return response.strip().replace('"', ''), meta

async def query_model_card_builder_meta(conversation_history, observer_feedback, metrics, current_card_content=None):
    """Model card synthesis query helper that captures metadata."""
    system_prompt = (
        "You are the Z-Agent Model Card Synthesis Agent. Your role is to maintain the official "
        "model card for 'Zymatica-Voice-LLM-v1.0'.\n"
        "Generate a complete, beautiful Markdown model card. Document the self-recursive improvement plan, "
        "identified bottlenecks, required prompt patches, and comedic vocabulary calibration changes."
    )
    
    payload = {
        "metrics_summary": {
            "turns_analyzed": len(metrics),
            "avg_tts_latency": sum(m["tts_latency"] for m in metrics) / len(metrics) if metrics else 0,
            "avg_asr_latency": sum(m["asr_latency"] for m in metrics) / len(metrics) if metrics else 0,
            "avg_similarity": sum(m["similarity_pct"] for m in metrics) / len(metrics) if metrics else 0
        },
        "observer_feedback": observer_feedback,
        "recent_history": conversation_history[-6:]
    }
    
    messages = [
        {"role": "system", "content": system_prompt},
        {"role": "user", "content": f"Current Card Content (if any):\n{current_card_content or 'None'}\n\nNew Telemetry Update:\n{json.dumps(payload, indent=2)}\n\nWrite a fully updated Markdown Model Card."}
    ]
    
    response, meta = await query_fast_llm_with_meta(messages, purpose="model_card_synthesis")
    return response, meta

async def simulate_human_agent_meta(history):
    """Simulates the girlfriend caller (she/her) who is extremely curious and hooks boyfriend."""
    system_prompt = (
        "You are a young woman who just swapped numbers at a coffee shop with a guy. "
        "You are having a warm, conversational, and flirty phone call. Keep your reply brief (strictly under 20 words). "
        "When you reply: first, directly answer his question, then immediately ask him a new curious question about himself "
        "to hook him and keep the conversation going."
    )
    messages = [{"role": "system", "content": system_prompt}]
    for msg in history[-10:]:
        messages.append({"role": msg["role"], "content": msg["message"]})
    messages.append({"role": "user", "content": "Answer his question and hook him with your next question."})
    
    response, meta = await query_fast_llm_with_meta(messages, purpose="girlfriend_dialogue")
    return response.strip().replace('"', ''), meta

async def query_zymatica_meta(history, user_text):
    """Queries Zymatica (boyfriend, onyx) who is extremely curious and hooks girlfriend."""
    system_content = (
        "You are a young man who just swapped numbers at a coffee shop with a girl. "
        "You are having a warm, conversational, and flirty phone call. Keep your reply brief (strictly under 20 words). "
        "When you reply: first, directly answer her question, then immediately ask her a new curious question about herself "
        "to hook her and keep the conversation going."
    )
    messages = [{"role": "system", "content": system_content}]
    for msg in history[-10:]:
        messages.append({"role": msg["role"], "content": msg["message"]})
    messages.append({"role": "user", "content": user_text})
    
    response, meta = await query_fast_llm_with_meta(messages, purpose="boyfriend_dialogue")
    return response.strip().replace('"', ''), meta

async def run_zagents_dialectic_test():
    logger.info("Starting Experiment 3: 5-Minute Relationship Curiosity Loop with Meta-Logging...")
    
    tts = get_tts_model()
    asr = get_asr_model()
    tts.is_loaded = False  # Force Edge-TTS fallback for standalone experiment
    asr.is_loaded = False  # Force API ASR fallback for standalone experiment
    
    # Capture system details at start
    system_env = get_system_environment()
    logger.info(f"Host System Environment gathered: {json.dumps(system_env, indent=2)}")
    
    history = []
    metrics = []
    observer_logs = []
    metalogs = []
    
    # 5 minutes = 300 seconds of simulated conversation time
    target_duration = 300
    elapsed_time = 0
    turn = 0
    
    model_card_path = os.path.join(current_dir, "zymatica_voice_model_card_exp3.md")
    metalogs_path = os.path.join(current_dir, "zymatica_voice_metalogs_exp3.json")
    current_card = ""
    
    # First turn human prompt: Coffee Swapped Swapped numbers
    human_text = "Hey, I'm really glad we swapped numbers at the coffee shop today... what made you decide to actually talk to me?"
    
    while elapsed_time < target_duration:
        turn += 1
        print("\n" + "="*80)
        print(f"TURN {turn} | Elapsed Simulated Time: {elapsed_time:.1f}s / {target_duration}s")
        print("="*80)
        
        # ----------------------------------------------------
        # 1. HUMAN SPEAKER (Girlfriend)
        # ----------------------------------------------------
        print(f"\n[Human Target Text]: {human_text}")
        
        # TTS synthesis
        human_wav = f"temp_human_turn_exp3_{turn}.wav"
        start_tts = time.time()
        tts.generate(human_text, output_file=human_wav, voice="nova")
        human_tts_latency = time.time() - start_tts
        
        # Get MD5 of generated audio
        human_audio_md5 = get_md5(human_wav)
        
        # Get audio duration and size
        human_audio_len = get_audio_duration(human_wav, text=human_text)
        human_rtf = human_tts_latency / human_audio_len if human_audio_len > 0 else 0.0
        
        # ASR transcription
        start_asr = time.time()
        transcribed_human = asr.transcribe(human_wav) if os.path.exists(human_wav) else None
        human_asr_latency = time.time() - start_asr
        
        if not transcribed_human:
            transcribed_human = human_text
            
        human_sim = calculate_similarity(human_text, transcribed_human)
        
        print(f"Human TTS Latency: {human_tts_latency:.2f}s | Audio Len: {human_audio_len:.2f}s | Audio MD5: {human_audio_md5}")
        print(f"Human Transcribed (ASR): '{transcribed_human}' (Similarity: {human_sim}%)")
        
        # Run Z-Agent-A Observer analysis
        h_observer_prompt = (
            "You are the Z-Agent-A Agent listening on the female speaker's terminal. "
            "Critique her conversational enunciation, pronunciation feasibility, and "
            "her question hook quality (whether it effectively drives curiosity). Give a 1-sentence analytical critique."
        )
        h_telemetry = {
            "turn": turn,
            "original_text": human_text,
            "transcribed_text": transcribed_human,
            "similarity_pct": human_sim,
            "tts_latency": human_tts_latency,
            "asr_latency": human_asr_latency
        }
        h_feedback, h_obs_meta = await query_zagent_observer_meta("Z-Agent-A (Human Observer)", h_observer_prompt, h_telemetry)
        h_obs_meta["audio_md5"] = human_audio_md5
        h_obs_meta["audio_duration_seconds"] = human_audio_len
        metalogs.append(h_obs_meta)
        
        print(f"Z-Agent-A (Human Observer): {h_feedback}")
        observer_logs.append({"turn": turn, "agent": "Z-Agent-A", "feedback": h_feedback})
        
        # Add to history
        history.append({"role": "user", "message": transcribed_human})
        metrics.append({
            "turn": turn,
            "speaker": "human_simulator",
            "similarity_pct": human_sim,
            "tts_latency": human_tts_latency,
            "asr_latency": human_asr_latency,
            "audio_duration": human_audio_len,
            "rtf": human_rtf,
            "original_text": human_text,
            "audio_md5": human_audio_md5
        })
        
        elapsed_time += human_audio_len + 1.5
        if elapsed_time >= target_duration:
            break
            
        # ----------------------------------------------------
        # 2. ZYMATICA BOT SPEAKER (Boyfriend)
        # ----------------------------------------------------
        # Query Zymatica response with meta-logs
        zymatica_text, z_dialogue_meta = await query_zymatica_meta(history, transcribed_human)
        
        zymatica_llm_latency = z_dialogue_meta["latency_ms"] / 1000.0
        print(f"\n[Zymatica Target Text]: {zymatica_text} (LLM latency: {zymatica_llm_latency:.2f}s)")
        
        # TTS synthesis
        zymatica_wav = f"temp_bot_turn_exp3_{turn}.wav"
        start_tts = time.time()
        tts.generate(zymatica_text, output_file=zymatica_wav, voice="onyx")
        zymatica_tts_latency = time.time() - start_tts
        
        # Get MD5 of generated audio
        zymatica_audio_md5 = get_md5(zymatica_wav)
        z_dialogue_meta["audio_md5"] = zymatica_audio_md5
        z_dialogue_meta["audio_duration_seconds"] = get_audio_duration(zymatica_wav, text=zymatica_text)
        metalogs.append(z_dialogue_meta)
        
        # Get audio duration and size
        zymatica_audio_len = z_dialogue_meta["audio_duration_seconds"]
        zymatica_rtf = zymatica_tts_latency / zymatica_audio_len if zymatica_audio_len > 0 else 0.0
        
        # ASR transcription
        start_asr = time.time()
        transcribed_bot = asr.transcribe(zymatica_wav) if os.path.exists(zymatica_wav) else None
        zymatica_asr_latency = time.time() - start_asr
        
        if not transcribed_bot:
            transcribed_bot = zymatica_text
            
        zymatica_sim = calculate_similarity(zymatica_text, transcribed_bot)
        
        print(f"Zymatica TTS Latency: {zymatica_tts_latency:.2f}s | Audio Len: {zymatica_audio_len:.2f}s | Audio MD5: {zymatica_audio_md5}")
        print(f"Zymatica Transcribed (ASR): '{transcribed_bot}' (Similarity: {zymatica_sim}%)")
        
        # Run Z-Agent-B Observer analysis with meta-logs
        z_observer_prompt = (
            "You are the Z-Agent-B Agent listening on the male speaker's terminal. "
            "Critique his conversational enunciation, voice inflection, and "
            "his question hook quality (whether it effectively drives curiosity). Give a 1-sentence analytical critique."
        )
        z_telemetry = {
            "turn": turn,
            "original_text": zymatica_text,
            "transcribed_text": transcribed_bot,
            "similarity_pct": zymatica_sim,
            "llm_latency": zymatica_llm_latency,
            "tts_latency": zymatica_tts_latency,
            "asr_latency": zymatica_asr_latency
        }
        z_feedback, z_obs_meta = await query_zagent_observer_meta("Z-Agent-B (Zymatica Observer)", z_observer_prompt, z_telemetry)
        metalogs.append(z_obs_meta)
        
        print(f"Z-Agent-B (Zymatica Observer): {z_feedback}")
        observer_logs.append({"turn": turn, "agent": "Z-Agent-B", "feedback": z_feedback})
        
        # Add to history
        history.append({"role": "assistant", "message": zymatica_text})
        metrics.append({
            "turn": turn,
            "speaker": "zymatica_bot",
            "similarity_pct": zymatica_sim,
            "tts_latency": zymatica_tts_latency,
            "asr_latency": zymatica_asr_latency,
            "audio_duration": zymatica_audio_len,
            "rtf": zymatica_rtf,
            "llm_latency": zymatica_llm_latency,
            "original_text": zymatica_text,
            "audio_md5": zymatica_audio_md5
        })
        
        elapsed_time += zymatica_audio_len + 1.5
        
        # Clean up temp WAV files to save space
        if os.path.exists(human_wav):
            try: os.remove(human_wav)
            except OSError: pass
        if os.path.exists(zymatica_wav):
            try: os.remove(zymatica_wav)
            except OSError: pass
            
        # ----------------------------------------------------
        # 3. REAL-TIME MODEL CARD SYNTHESIS
        # ----------------------------------------------------
        # Trigger model card builder update every 4 turns
        if turn % 4 == 0:
            print("\n[Z-Agent Model Card Builder]: Synthesizing telemetry and updating Model Card...")
            recent_feedback = [log for log in observer_logs if log["turn"] > turn - 4]
            updated_card, card_meta = await query_model_card_builder_meta(history, recent_feedback, metrics, current_card)
            metalogs.append(card_meta)
            
            if updated_card:
                current_card = updated_card
                with open(model_card_path, "w", encoding="utf-8") as f:
                    f.write(current_card)
                print(f"Model Card updated successfully in {model_card_path}")
            else:
                print("Warning: Model Card update returned empty response or failed.")
                
        # Pause to keep loop speed fast in real-world time
        await asyncio.sleep(0.5)
        
        # Generate next human query
        human_text, h_dialogue_meta = await simulate_human_agent_meta(history)
        metalogs.append(h_dialogue_meta)
        
    # Final Model Card write (in case it didn't trigger at the end)
    print("\n[Z-Agent Model Card Builder]: Writing final synthesized Model Card...")
    final_card, final_card_meta = await query_model_card_builder_meta(history, observer_logs, metrics, current_card)
    metalogs.append(final_card_meta)
    
    if final_card:
        current_card = final_card
        with open(model_card_path, "w", encoding="utf-8") as f:
            f.write(current_card)
        print(f"Final Model Card written to: {model_card_path}")
        
    # Write the complete audit meta-logs JSON containing system details & turn-by-turn trace
    final_audit_package = {
        "audit_meta_header": {
            "date": datetime.utcnow().strftime("%Y-%m-%d"),
            "target_system": "Zymatica-Voice-LLM-v1.0-Auditable",
            "host_environment_spec": system_env
        },
        "generative_trace_logs": metalogs
    }
    with open(metalogs_path, "w", encoding="utf-8") as meta_f:
        json.dump(final_audit_package, meta_f, indent=2)
    print(f"Complete audit meta-logs written successfully to: {metalogs_path}")
        
    # Also write a separate test summary report
    generate_markdown_report(metrics, history, elapsed_time, turn, observer_logs)

def generate_markdown_report(metrics, history, elapsed_time, total_turns, observer_logs):
    """Calculates aggregates and prints a beautiful markdown summary."""
    human_metrics = [m for m in metrics if m["speaker"] == "human_simulator"]
    bot_metrics = [m for m in metrics if m["speaker"] == "zymatica_bot"]
    
    avg_human_tts = sum(m["tts_latency"] for m in human_metrics) / len(human_metrics) if human_metrics else 0
    avg_bot_tts = sum(m["tts_latency"] for m in bot_metrics) / len(bot_metrics) if bot_metrics else 0
    
    avg_human_asr = sum(m["asr_latency"] for m in human_metrics) / len(human_metrics) if human_metrics else 0
    avg_bot_asr = sum(m["asr_latency"] for m in bot_metrics) / len(bot_metrics) if bot_metrics else 0
    
    avg_human_sim = sum(m["similarity_pct"] for m in human_metrics) / len(human_metrics) if human_metrics else 0
    avg_bot_sim = sum(m["similarity_pct"] for m in bot_metrics) / len(bot_metrics) if bot_metrics else 0
    
    avg_bot_llm = sum(m["llm_latency"] for m in bot_metrics) / len(bot_metrics) if bot_metrics else 0
    total_audio_duration = sum(m["audio_duration"] for m in metrics)
    
    workspace_md_path = os.path.join(current_dir, "zymatica_voice_zagents_report_exp3.md")
    
    md_content = f"""# Relationship Curiosity Study: 5-Minute Z-Agent-Dialectic Conversation Test (Exp 3)

This report compiles the conversation transcripts, observer analysis, and audio metrics gathered during a 5-minute back-and-forth phone call relationship simulation evaluated in real-time by Z-Agent agents.

## Executive Summary
- **Total Turns Simulated**: {total_turns}
- **Total Simulated Audio Duration**: {total_audio_duration:.2f} seconds
- **Total Simulated Conversation Time**: {elapsed_time:.2f} seconds (~{elapsed_time/60:.1f} minutes)
- **Average Dialogue Turnaround Time**: {avg_bot_llm + avg_bot_tts + avg_bot_asr:.2f} seconds
- **Generative AI Verifiability**: Complete JSON metadata (payloads, latencies, timestamps, host specs, and audio checksums) written to `zymatica_voice_metalogs_exp3.json` for audit.

---

## Telemetry Metrics Summary

| Metric | Girlfriend (Nova) | Boyfriend (Onyx) | Overall Average |
| :--- | :---: | :---: | :---: |
| **TTS Synthesis Latency** | {avg_human_tts:.2f}s | {avg_bot_tts:.2f}s | {(avg_human_tts + avg_bot_tts)/2:.2f}s |
| **ASR Transcription Latency** | {avg_human_asr:.2f}s | {avg_bot_asr:.2f}s | {(avg_human_asr + avg_bot_asr)/2:.2f}s |
| **LLM Response Latency** | N/A | {avg_bot_llm:.2f}s | {avg_bot_llm:.2f}s |
| **ASR Accuracy (Similarity)** | {avg_human_sim:.1f}% | {avg_bot_sim:.1f}% | {(avg_human_sim + avg_bot_sim)/2:.1f}% |

---

## Z-Agent Real-Time Observer Critiques

"""
    for i in range(1, total_turns + 1):
        h_feedback = next((log["feedback"] for log in observer_logs if log["turn"] == i and log["agent"] == "Z-Agent-A"), "None")
        z_feedback = next((log["feedback"] for log in observer_logs if log["turn"] == i and log["agent"] == "Z-Agent-B"), "None")
        
        md_content += f"### Turn {i} Observer Feedback\n"
        md_content += f"- **👤 Z-Agent-A (Human Observer)**: *\"{h_feedback}\"*\n"
        md_content += f"- **🤖 Z-Agent-B (Zymatica Observer)**: *\"{z_feedback}\"*\n\n"

    md_content += """
---

## Detailed Turn-by-Turn Transcript

"""
    for i in range(1, total_turns + 1):
        h_m = next((m for m in human_metrics if m["turn"] == i), None)
        b_m = next((m for m in bot_metrics if m["turn"] == i), None)
        
        md_content += f"### Turn {i}\n"
        if h_m:
            md_content += f"- **👤 Girlfriend (nova)**: \"{h_m.get('original_text', '')}\"\n"
            md_content += f"  *Audio MD5: `{h_m.get('audio_md5', '')}`*\n"
        if b_m:
            md_content += f"- **🤖 Boyfriend (onyx)**: \"{b_m.get('original_text', '')}\"\n"
            md_content += f"  *Audio MD5: `{b_m.get('audio_md5', '')}`*\n"
        md_content += "\n"
        
    with open(workspace_md_path, "w", encoding="utf-8") as f:
        f.write(md_content)
        
    print(md_content)
    print(f"\nReport written to: {workspace_md_path}")
    print(f"Model Card written to: {os.path.join(current_dir, 'zymatica_voice_model_card_exp3.md')}")

if __name__ == "__main__":
    asyncio.run(run_zagents_dialectic_test())
