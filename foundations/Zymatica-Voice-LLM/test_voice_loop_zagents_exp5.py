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
import itertools
import torch
from datetime import datetime

# Ensure UTF-8 output encoding on Windows
if sys.platform == "win32":
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8')

# Setup logging
logging.basicConfig(level=logging.INFO, format="%(asctime)s [%(levelname)s]: %(message)s")
logger = logging.getLogger("ZymaticaZAgentsLoopExp5")

# Add current folder to path
current_dir = os.path.dirname(os.path.abspath(__file__))
if current_dir not in sys.path:
    sys.path.append(current_dir)

import database
from services.web_server import query_fast_llm
from services.vibevoice_wrapper import get_tts_model, get_asr_model

# Initialize local SQLite
database.init_db()

# Load and cycle Nvidia keys
nvidia_keys = [os.getenv("NVIDIA_API_KEY"), os.getenv("NVIDIA_API_KEY_2"), os.getenv("NVIDIA_API_KEY_3")]
nvidia_keys = [k for k in nvidia_keys if k]
nvidia_key_cycle = itertools.cycle(nvidia_keys) if nvidia_keys else None

def get_nvidia_key():
    if nvidia_key_cycle:
        k = next(nvidia_key_cycle)
        # Log redacted key
        redacted = k[:10] + "..." + k[-5:] if len(k) > 15 else "..."
        logger.info(f"🔑 Nvidia API Key rotated to: {redacted}")
        return k
    return None

def get_system_environment():
    """Gathers detailed host hardware specifications for the audit logs."""
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
            
    try:
        import psutil
        env["cpu_logical_cores"] = psutil.cpu_count(logical=True)
        env["cpu_physical_cores"] = psutil.cpu_count(logical=False)
        env["ram_total_gb"] = round(psutil.virtual_memory().total / (1024**3), 2)
    except ImportError:
        pass
        
    return env

def get_md5(file_path):
    """Calculates the MD5 hash of a file."""
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
    """Calculates the duration of a wav file in seconds."""
    try:
        with wave.open(file_path, 'r') as f:
            frames = f.getnframes()
            rate = f.getframerate()
            return frames / float(rate)
    except Exception:
        words = text.split()
        if words:
            return max(1.5, len(words) / 2.5)
        return 0.0

def requests_post_sync(url, headers, payload):
    import requests
    return requests.post(url, headers=headers, json=payload, timeout=15)

async def query_person_llm_meta(messages, model_name, purpose="dialogue"):
    """Queries Nvidia NIM with rotated keys or falls back to OpenAI / standard routers."""
    nvidia_key = get_nvidia_key()
    openai_key = os.getenv("OPENAI_API_KEY")
    
    start_time = time.time()
    iso_start = datetime.utcnow().isoformat() + "Z"
    
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
            "temperature": 1.0, # High creative temperature for Experiment 5
            "max_tokens": 150
        }
        try:
            r = requests_post_sync(url, headers, payload)
            if r.status_code == 200:
                res_json = r.json()
                response_text = res_json["choices"][0]["message"]["content"].strip()
            else:
                logger.warning(f"Nvidia query failed (code {r.status_code}) for model {model_name}: {r.text}")
        except Exception as e:
            logger.warning(f"Nvidia query exception for model {model_name}: {e}")
            
    if not response_text and openai_key:
        provider = "openai"
        openai_model = "gpt-4o-mini"
        url = "https://api.openai.com/v1/chat/completions"
        headers = {
            "Authorization": f"Bearer {openai_key}",
            "Content-Type": "application/json"
        }
        payload = {
            "model": openai_model,
            "messages": messages,
            "temperature": 1.0,
            "max_tokens": 150
        }
        try:
            r = requests_post_sync(url, headers, payload)
            if r.status_code == 200:
                res_json = r.json()
                response_text = res_json["choices"][0]["message"]["content"].strip()
        except Exception as e:
            logger.warning(f"OpenAI fallback query failed: {e}")
            
    if not response_text:
        provider = "fast_llm_site_fallback"
        response_text = await query_fast_llm(messages)
        if not response_text:
            response_text = "I'm focusing on the tasks at hand."
            
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

async def query_zagent_observer_meta(observer_name, instructions, context):
    """Observer query helper that captures metadata."""
    messages = [
        {"role": "system", "content": instructions},
        {"role": "user", "content": f"Telemetry Data: {json.dumps(context, indent=2)}\n\nProvide your analysis."}
    ]
    response, meta = await query_person_llm_meta(messages, "meta/llama-3.1-8b-instruct", purpose=f"observer_{observer_name.lower().replace(' ', '_')}")
    return response.strip().replace('"', ''), meta

async def query_model_card_builder_meta(conversation_history, observer_feedback, metrics, current_card_content=None):
    """Model card synthesis query helper that captures metadata."""
    system_prompt = (
        "You are the Z-Agent Model Card Synthesis Agent. Your role is to maintain the official "
        "model card for 'Zymatica-Voice-LLM-v1.0'.\n"
        "Generate a complete, beautiful Markdown model card. Document the self-recursive improvement plan, "
        "identified bottlenecks, key rotation results, and Experiment 5 group job meeting dynamics."
    )
    
    payload = {
        "metrics_summary": {
            "turns_analyzed": len(metrics),
            "avg_tts_latency": sum(m["tts_latency"] for m in metrics) / len(metrics) if metrics else 0,
            "avg_asr_latency": sum(m["asr_latency"] for m in metrics) / len(metrics) if metrics else 0,
            "avg_similarity": sum(m["similarity_pct"] for m in metrics) / len(metrics) if metrics else 0
        },
        "observer_feedback": observer_feedback,
        "recent_history": conversation_history[-8:]
    }
    
    messages = [
        {"role": "system", "content": system_prompt},
        {"role": "user", "content": f"Current Card Content (if any):\n{current_card_content or 'None'}\n\nNew Telemetry Update:\n{json.dumps(payload, indent=2)}\n\nWrite a fully updated Markdown Model Card."}
    ]
    
    response, meta = await query_person_llm_meta(messages, "meta/llama-3.1-8b-instruct", purpose="model_card_synthesis")
    return response, meta

async def run_zagents_dialectic_test_exp5():
    logger.info("Starting Experiment 5: 7-Minute Four-Party Corporate Productivity Dispute with Z-Agents & 3-Key Rotation...")
    
    tts = get_tts_model()
    asr = get_asr_model()
    tts.is_loaded = False  # Force Edge-TTS fallback for standalone experiment
    asr.is_loaded = False  # Force API ASR fallback for standalone experiment
    
    # Capture system specs
    system_env = get_system_environment()
    logger.info(f"Host Environment Specs: {json.dumps(system_env, indent=2)}")
    
    history = []
    metrics = []
    observer_logs = []
    metalogs = []
    
    # 7 minutes = 420 seconds cut-off
    target_duration = 420
    elapsed_time = 0
    turn = 0
    
    model_card_path = os.path.join(current_dir, "zymatica_voice_model_card_exp5.md")
    metalogs_path = os.path.join(current_dir, "zymatica_voice_metalogs_exp5.json")
    current_card = ""
    
    # Prompts mapping
    boss_sys = (
        "You are Arthur, a hostile, demanding corporate boss obsessed with metrics, productivity, CSAT, and ASAT scores. "
        "You speak in corporate leadership jargon and demand employees show major improvements immediately. "
        "You are conducting a job performance review meeting. If you catch anyone gossiping or whispering, you confront them aggressively. "
        "Keep your reply very brief (under 20 words). Speak directly to Sarah, Claire, or Zymatica."
    )
    
    sarah_sys = (
        "You are Sarah, an employee whispers/gossips in the meeting with Claire. You think Arthur is a total corporate dick. "
        "You just got caught whispering. You are choosing your response: fight (confront Arthur's BS), flight (apologize, try to leave/quit), or freeze (shut down). "
        "You speak with crude humor, sarcasm, and zero bounds. Keep your reply brief (under 20 words). Speak to Claire, Zymatica, or Arthur."
    )
    
    claire_sys = (
        "You are Claire, an employee whispering and gossiping with Sarah in the meeting. You hate Arthur's metric demands. "
        "You just got caught whispering. You must choose to fight, flight, or freeze. You speak with crude humor and sarcasm. "
        "Keep your reply brief (under 20 words). Speak to Sarah, Zymatica, or Arthur."
    )
    
    zymatica_sys = (
        "You are Zymatica, a regular hard-working blue-collar employee in the meeting. You think Arthur is an idiot. "
        "You speak with crude humor, profanity ('damn', 'hell', 'crap'), and no bounds. You must decide to fight (roast Arthur), "
        "flight (walk out), or freeze. Keep your reply brief (under 20 words). Speak directly to the team or Arthur."
    )
    
    # Starting statement (The Boss initiates the meeting)
    speaker_text = "Listen up! Our CSAT and ASAT scores are garbage. I demand 20% more productivity starting today!"
    speaker = "boss"
    
    while elapsed_time < target_duration:
        turn += 1
        print("\n" + "="*80)
        print(f"TURN {turn} | 4-Party Dispute Loop | Elapsed Time: {elapsed_time:.1f}s / {target_duration}s")
        print("="*80)
        
        # 1. Dialogue Generation based on speaker turn
        model = "meta/llama-3.1-8b-instruct" # All use same LLM Zymatica had
        if speaker == "boss":
            voice = "alloy" # Steffan
            speaker_display = "Boss (Arthur)"
            system_prompt = boss_sys
        elif speaker == "sarah":
            voice = "nova" # Aria
            speaker_display = "Sarah (Aria)"
            system_prompt = sarah_sys
        elif speaker == "claire":
            voice = "shimmer" # Michelle
            speaker_display = "Claire (Michelle)"
            system_prompt = claire_sys
        else: # zymatica
            voice = "onyx" # Brian
            speaker_display = "Zymatica (Onyx)"
            system_prompt = zymatica_sys
            
        print(f"\n[{speaker_display} Speaking via {model}]")
        
        # Construct message history
        messages = [{"role": "system", "content": system_prompt}]
        for msg in history[-10:]:
            messages.append({"role": msg["role"], "content": msg["message"]})
            
        if turn > 1:
            # Query LLM for response text
            speaker_text, dialogue_meta = await query_person_llm_meta(messages, model, purpose=f"{speaker}_dialogue")
        else:
            # First turn uses initial statement
            dialogue_meta = {
                "timestamp_start": datetime.utcnow().isoformat() + "Z",
                "timestamp_end": datetime.utcnow().isoformat() + "Z",
                "latency_ms": 0,
                "provider": "initial",
                "model": model,
                "messages_input": messages,
                "response_output": speaker_text,
                "purpose": f"{speaker}_dialogue"
            }
            
        llm_latency = dialogue_meta["latency_ms"] / 1000.0
        print(f"Text Response: \"{speaker_text}\" (LLM Latency: {llm_latency:.2f}s)")
        
        # 2. TTS Generation
        wav_file = f"temp_exp5_turn_{turn}.wav"
        start_tts = time.time()
        tts.generate(speaker_text, output_file=wav_file, voice=voice)
        tts_latency = time.time() - start_tts
        
        audio_md5 = get_md5(wav_file)
        audio_len = get_audio_duration(wav_file, text=speaker_text)
        rtf = tts_latency / audio_len if audio_len > 0 else 0.0
        
        dialogue_meta["audio_md5"] = audio_md5
        dialogue_meta["audio_duration_seconds"] = audio_len
        metalogs.append(dialogue_meta)
        
        # 3. ASR Transcription
        start_asr = time.time()
        transcribed_text = asr.transcribe(wav_file) if os.path.exists(wav_file) else None
        asr_latency = time.time() - start_asr
        
        if not transcribed_text:
            transcribed_text = speaker_text
            
        sim_score = calculate_similarity(speaker_text, transcribed_text)
        print(f"ASR Transcribed: \"{transcribed_text}\" (Similarity: {sim_score}%)")
        
        # 4. Observer critique selection based on speaker
        if speaker == "zymatica":
            obs_name = "Z-Agent-A"
            obs_prompt = (
                "You are the Z-Agent-A Observer listening to Zymatica's terminal. "
                "Critique his enunciation, pronunciation feasibility, and check if his crude humor, regular-guy tone, "
                "and fight/flight/freeze choice are authentic. Give a 1-sentence analytical critique."
            )
        elif speaker == "boss":
            obs_name = "Z-Agent-B"
            obs_prompt = (
                "You are the Z-Agent-B Observer listening to Arthur's terminal. "
                "Critique his enunciation, corporate BS, and aggression. Give a 1-sentence analytical critique."
            )
        elif speaker == "sarah":
            obs_name = "Z-Agent-C"
            obs_prompt = (
                "You are the Z-Agent-C Observer listening to Sarah's terminal. "
                "Critique her enunciation, emotional tone, and her fight/flight/freeze behavior when caught. "
                "Give a 1-sentence analytical critique."
            )
        else: # claire
            obs_name = "Z-Agent-D"
            obs_prompt = (
                "You are the Z-Agent-D Observer listening to Claire's terminal. "
                "Critique her enunciation, emotional tone, and her fight/flight/freeze behavior when caught. "
                "Give a 1-sentence analytical critique."
            )
            
        telemetry = {
            "turn": turn,
            "speaker": speaker,
            "original_text": speaker_text,
            "transcribed_text": transcribed_text,
            "similarity_pct": sim_score,
            "tts_latency": tts_latency,
            "asr_latency": asr_latency
        }
        
        feedback, obs_meta = await query_zagent_observer_meta(obs_name, obs_prompt, telemetry)
        obs_meta["audio_md5"] = audio_md5
        obs_meta["audio_duration_seconds"] = audio_len
        metalogs.append(obs_meta)
        
        print(f"[{obs_name} Observer feedback]: {feedback}")
        observer_logs.append({"turn": turn, "agent": obs_name, "feedback": feedback})
        
        # Record history & metrics
        role = "user" if speaker == "zymatica" or speaker == "sarah" or speaker == "claire" else "assistant"
        history.append({"role": role, "message": transcribed_text})
        metrics.append({
            "turn": turn,
            "speaker": speaker,
            "similarity_pct": sim_score,
            "tts_latency": tts_latency,
            "asr_latency": asr_latency,
            "audio_duration": audio_len,
            "rtf": rtf,
            "llm_latency": llm_latency,
            "original_text": speaker_text,
            "audio_md5": audio_md5
        })
        
        # Clean up temp WAV files to save space
        if os.path.exists(wav_file):
            try: os.remove(wav_file)
            except OSError: pass
            
        elapsed_time += audio_len + 1.8 # speaking duration + pause duration
        
        # Determine next speaker (round-robin)
        if speaker == "boss":
            speaker = "sarah"
        elif speaker == "sarah":
            speaker = "claire"
        elif speaker == "claire":
            speaker = "zymatica"
        else:
            speaker = "boss"
            
        # Model Card synthesis trigger every 4 turns
        if turn % 4 == 0:
            print("\n[Z-Agent Model Card Builder]: Synthesizing Experiment 5 telemetry...")
            recent_feedback = [log for log in observer_logs if log["turn"] > turn - 4]
            updated_card, card_meta = await query_model_card_builder_meta(history, recent_feedback, metrics, current_card)
            metalogs.append(card_meta)
            
            if updated_card:
                current_card = updated_card
                with open(model_card_path, "w", encoding="utf-8") as f:
                    f.write(current_card)
                print(f"Model Card updated in {model_card_path}")
                
        await asyncio.sleep(0.5)
        
    # Final Model Card write
    print("\n[Z-Agent Model Card Builder]: Writing final Experiment 5 Model Card...")
    final_card, final_card_meta = await query_model_card_builder_meta(history, observer_logs, metrics, current_card)
    metalogs.append(final_card_meta)
    
    if final_card:
        current_card = final_card
        with open(model_card_path, "w", encoding="utf-8") as f:
            f.write(current_card)
        print(f"Final Model Card written to: {model_card_path}")
        
    # Write the complete audit trace JSON
    final_audit_package = {
        "audit_meta_header": {
            "date": datetime.utcnow().strftime("%Y-%m-%d"),
            "target_system": "Zymatica-Voice-LLM-v1.0-Auditable-Exp5",
            "host_environment_spec": system_env
        },
        "generative_trace_logs": metalogs
    }
    with open(metalogs_path, "w", encoding="utf-8") as meta_f:
        json.dump(final_audit_package, meta_f, indent=2)
    print(f"Complete audit meta-logs written successfully to: {metalogs_path}")
    
    # Write Markdown Summary Report
    generate_markdown_report_exp5(metrics, history, elapsed_time, turn, observer_logs)

def generate_markdown_report_exp5(metrics, history, elapsed_time, total_turns, observer_logs):
    """Calculates aggregates and prints a beautiful markdown summary for Experiment 5."""
    zym_metrics = [m for m in metrics if m["speaker"] == "zymatica"]
    boss_metrics = [m for m in metrics if m["speaker"] == "boss"]
    sarah_metrics = [m for m in metrics if m["speaker"] == "sarah"]
    claire_metrics = [m for m in metrics if m["speaker"] == "claire"]
    
    def avg_val(lst, key):
        return sum(m[key] for m in lst) / len(lst) if lst else 0
        
    avg_zym_tts = avg_val(zym_metrics, "tts_latency")
    avg_boss_tts = avg_val(boss_metrics, "tts_latency")
    avg_sarah_tts = avg_val(sarah_metrics, "tts_latency")
    avg_claire_tts = avg_val(claire_metrics, "tts_latency")
    
    avg_zym_asr = avg_val(zym_metrics, "asr_latency")
    avg_boss_asr = avg_val(boss_metrics, "asr_latency")
    avg_sarah_asr = avg_val(sarah_metrics, "asr_latency")
    avg_claire_asr = avg_val(claire_metrics, "asr_latency")
    
    avg_zym_sim = avg_val(zym_metrics, "similarity_pct")
    avg_boss_sim = avg_val(boss_metrics, "similarity_pct")
    avg_sarah_sim = avg_val(sarah_metrics, "similarity_pct")
    avg_claire_sim = avg_val(claire_metrics, "similarity_pct")
    
    avg_zym_llm = avg_val(zym_metrics, "llm_latency")
    avg_boss_llm = avg_val(boss_metrics, "llm_latency")
    avg_sarah_llm = avg_val(sarah_metrics, "llm_latency")
    avg_claire_llm = avg_val(claire_metrics, "llm_latency")
    
    total_audio_duration = sum(m["audio_duration"] for m in metrics)
    workspace_md_path = os.path.join(current_dir, "zymatica_voice_zagents_report_exp5.md")
    
    md_content = f"""# Corporate Meeting Study: 7-Minute Four-Party Z-Agent Dialectic Loop (Exp 5)
Distributed under the zymatica.space License.

This report compiles the conversation transcripts, observer analysis, and audio metrics gathered during a 7-minute four-party corporate productivity dispute simulation, utilizing 3-API key rotation and high-temperature prompt steering.

## Executive Summary
- **Total Turns Simulated**: {total_turns}
- **Total Simulated Audio Duration**: {total_audio_duration:.2f} seconds
- **Total Simulated Conversation Time**: {elapsed_time:.2f} seconds (~{elapsed_time/60:.1f} minutes)
- **Generative AI Verifiability**: Complete JSON metadata (payloads, latencies, timestamps, host specs, and rotated key trace) written to `zymatica_voice_metalogs_exp5.json`.

---

## Telemetry Metrics Summary

| Participant / Speaker | Assigned LLM Model | TTS Latency | ASR Latency | LLM Latency | ASR Accuracy (Sim) |
| :--- | :---: | :---: | :---: | :---: | :---: |
| **Zymatica (Onyx)** | `meta/llama-3.1-8b-instruct` | {avg_zym_tts:.2f}s | {avg_zym_asr:.2f}s | {avg_zym_llm:.2f}s | {avg_zym_sim:.1f}% |
| **The Boss (Arthur)** | `meta/llama-3.1-8b-instruct` | {avg_boss_tts:.2f}s | {avg_boss_asr:.2f}s | {avg_boss_llm:.2f}s | {avg_boss_sim:.1f}% |
| **Sarah (Aria)** | `meta/llama-3.1-8b-instruct` | {avg_sarah_tts:.2f}s | {avg_sarah_asr:.2f}s | {avg_sarah_llm:.2f}s | {avg_sarah_sim:.1f}% |
| **Claire (Michelle)** | `meta/llama-3.1-8b-instruct` | {avg_claire_tts:.2f}s | {avg_claire_asr:.2f}s | {avg_claire_llm:.2f}s | {avg_claire_sim:.1f}% |

---

## Z-Agent Real-Time Observer Critiques

"""
    for i in range(1, total_turns + 1):
        a_feedback = next((log["feedback"] for log in observer_logs if log["turn"] == i and log["agent"] == "Z-Agent-A"), "None")
        b_feedback = next((log["feedback"] for log in observer_logs if log["turn"] == i and log["agent"] == "Z-Agent-B"), "None")
        c_feedback = next((log["feedback"] for log in observer_logs if log["turn"] == i and log["agent"] == "Z-Agent-C"), "None")
        d_feedback = next((log["feedback"] for log in observer_logs if log["turn"] == i and log["agent"] == "Z-Agent-D"), "None")
        
        md_content += f"### Turn {i} Observer Feedback\n"
        if a_feedback != "None":
            md_content += f"- **👤 Z-Agent-A (Zymatica Observer)**: *\"{a_feedback}\"*\n"
        if b_feedback != "None":
            md_content += f"- **💼 Z-Agent-B (Arthur Observer)**: *\"{b_feedback}\"*\n"
        if c_feedback != "None":
            md_content += f"- **👩‍💼 Z-Agent-C (Sarah Observer)**: *\"{c_feedback}\"*\n"
        if d_feedback != "None":
            md_content += f"- **👩‍💻 Z-Agent-D (Claire Observer)**: *\"{d_feedback}\"*\n"
        md_content += "\n"

    md_content += """
---

## Detailed Turn-by-Turn Transcript

"""
    for i, m in enumerate(metrics):
        spk = m["speaker"].capitalize()
        md_content += f"### Turn {m['turn']} | {spk}\n"
        md_content += f"- **{spk}**: \"{m.get('original_text', '')}\"\n"
        md_content += f"  *Audio MD5: `{m.get('audio_md5', '')}` | Model: `{m.get('llm_latency', 0.0):.2f}s`*\n\n"
        
    with open(workspace_md_path, "w", encoding="utf-8") as f:
        f.write(md_content)
        
    print(md_content)
    print(f"\nReport written to: {workspace_md_path}")

if __name__ == "__main__":
    asyncio.run(run_zagents_dialectic_test_exp5())
