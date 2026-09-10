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

# Ensure UTF-8 output encoding on Windows to prevent UnicodeEncodeError
if sys.platform == "win32":
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8')

# Setup logging
logging.basicConfig(level=logging.INFO, format="%(asctime)s [%(levelname)s]: %(message)s")
logger = logging.getLogger("ZymaticaZAgentsLoopBaseline")

# Add current folder to path
current_dir = os.path.dirname(os.path.abspath(__file__))
if current_dir not in sys.path:
    sys.path.append(current_dir)

import database
from services.web_server import query_fast_llm
from services.vibevoice_wrapper import get_asr_model, get_tts_model

# Initialize local SQLite
database.init_db()

# Load and cycle Nvidia keys
nvidia_keys = [os.getenv("NVIDIA_API_KEY"), os.getenv("NVIDIA_API_KEY_2"), os.getenv("NVIDIA_API_KEY_3")]
nvidia_keys = [k for k in nvidia_keys if k]
nvidia_key_cycle = itertools.cycle(nvidia_keys) if nvidia_keys else None

def get_nvidia_key():
    if nvidia_key_cycle:
        k = next(nvidia_key_cycle)
        redacted = k[:10] + "..." + k[-5:] if len(k) > 15 else "..."
        logger.info(f"🔑 Nvidia API Key rotated to: {redacted}")
        return k
    return None

def get_system_environment():
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
    if not os.path.exists(file_path):
        return ""
    hash_md5 = hashlib.md5()
    with open(file_path, "rb") as f:
        for chunk in iter(lambda: f.read(4096), b""):
            hash_md5.update(chunk)
    return hash_md5.hexdigest()

def calculate_similarity(text1, text2):
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
                dp[i][j] = min(dp[i-1][j] + 1,
                               dp[i][j-1] + 1,
                               dp[i-1][j-1] + 1)
                               
    dist = dp[m][n]
    max_len = max(m, n)
    return round((1.0 - dist / max_len) * 100, 2)

def get_audio_duration(file_path, text=""):
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

async def query_person_llm_meta(messages, model_name, purpose="dialogue", max_tokens=150):
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
            "temperature": 1.0,
            "max_tokens": max_tokens
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
            "max_tokens": max_tokens
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
    messages = [
        {"role": "system", "content": instructions},
        {"role": "user", "content": f"Telemetry Data: {json.dumps(context, indent=2)}\n\nProvide your analysis."}
    ]
    response, meta = await query_person_llm_meta(messages, "meta/llama-3.1-8b-instruct", purpose=f"observer_{observer_name.lower().replace(' ', '_')}")
    return response.strip().replace('"', ''), meta

async def query_model_card_builder_meta(conversation_history, observer_feedback, metrics, current_card_content=None):
    system_prompt = (
        "You are the Z-Agent Model Card Synthesis Agent. Your role is to maintain the official "
        "model card for 'Zymatica-Voice-LLM-v1.0'.\n"
        "Generate a complete, beautiful Markdown model card. Document the self-recursive improvement plan, "
        "identified bottlenecks, key rotation results, and 2-party hotline chat dynamics."
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

async def perform_automatic_prompt_calibration():
    logger.info("🤖 Starting Automatic Prompt Calibration using Zymatica Voice Model Card...")
    project_dir = os.path.dirname(os.path.abspath(__file__))
    model_card_path_prev = os.path.join(project_dir, "zymatica_voice_model_card.md")
    
    directives = {
        "human": "Keep your queries brief, conversational, and direct. Ask questions naturally.",
        "zymatica": "Maintain a sarcastic, blunt, and unhinged comedian persona. Keep responses under 2 sentences."
    }
    
    if not os.path.exists(model_card_path_prev):
        logger.warning("No previous model card found. Using baseline directives.")
        return directives
        
    try:
        with open(model_card_path_prev, "r", encoding="utf-8") as f:
            card_content = f.read()
            
        system_prompt = (
            "You are the Zymatica Prompt Calibration Agent. Your task is to analyze the previous model card "
            "and output a JSON object containing specific self-improvement directives for the two characters (Human, Zymatica).\n"
            "Format the output strictly as a JSON object with keys: 'human_directive' and 'zymatica_directive'.\n"
            "Each value must be a single flat string containing a concise (2-3 sentence) directive addressing their enunciation, tone authenticity, and dialogue boundaries, based on the observer critiques. Do NOT nest objects under the keys; use plain strings."
        )
        
        messages = [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": f"Here is the previous Model Card:\n\n{card_content}"}
        ]
        
        response, _ = await query_person_llm_meta(messages, "meta/llama-3.1-8b-instruct", purpose="prompt_calibration", max_tokens=600)
        
        # Robustly extract JSON object using regex
        json_match = re.search(r'\{.*\}', response, re.DOTALL)
        if json_match:
            cleaned_response = json_match.group(0).strip()
        else:
            cleaned_response = response.strip()
            
        if cleaned_response.startswith("```json"):
            cleaned_response = cleaned_response.replace("```json", "", 1)
        if cleaned_response.endswith("```"):
            cleaned_response = cleaned_response.rsplit("```", 1)[0]
        cleaned_response = cleaned_response.strip()
        
        data = json.loads(cleaned_response)
        if "human_directive" in data:
            directives["human"] = data["human_directive"]
        if "zymatica_directive" in data:
            directives["zymatica"] = data["zymatica_directive"]
            
        logger.info(f"🎉 Calibration successful! Directives loaded:\n{json.dumps(directives, indent=2)}")
    except Exception as e:
        logger.error(f"Failed to perform automatic calibration: {e}. LLM response was: {response if 'response' in locals() else 'None'}. Using baselines.")
        
    return directives

def strip_name_prefix(text, names):
    pattern = r'^(' + '|'.join(re.escape(n) for n in names) + r')\s*(?:\([^)]*\))?\s*:\s*'
    return re.sub(pattern, '', text, flags=re.IGNORECASE).strip()

def clean_brackets(text):
    cleaned = re.sub(r'\(.*?\)', '', text)
    cleaned = re.sub(r'\[.*?\]', '', cleaned)
    cleaned = re.sub(r'\s+', ' ', cleaned).strip()
    return cleaned

async def simulate_human_agent(history, directive):
    system_prompt = (
        "You are a human calling an alien AI named Zymatica on a voice hotline. "
        "Keep your query very brief (1 sentence, strictly under 15 words) and conversational. "
        "Talk to Zymatica about space, crypto, politics, sports, or react directly to his roasts.\n"
        f"CRITICAL FEEDBACK FROM PREVIOUS RUN: {directive}\n"
        "INSTRUCTION: Write ONLY your own spoken dialogue. Never write actions, physical descriptions, stage directions, "
        "parentheses, or speak on behalf of anyone else. Do not use words in brackets or parentheses. "
        "Do NOT prefix your response with your name (e.g. do not output 'Human: ...'). Just output the dialogue directly."
    )
    messages = [{"role": "system", "content": system_prompt}]
    for msg in history[-10:]:
        messages.append({"role": msg["role"], "content": msg["message"]})
        
    messages.append({"role": "user", "content": "Generate your next quick phone call query to Zymatica."})
    
    response, meta = await query_person_llm_meta(messages, "meta/llama-3.1-8b-instruct", purpose="human_dialogue")
    character_names = ["human", "zymatica", "onyx", "nova", "caller"]
    response = strip_name_prefix(response, character_names)
    return response, meta

async def query_zymatica(history, user_text, directive):
    system_content = (
        "You are Zymatica, an R-rated standup comedian alien from Gliese 12b. "
        "You are blunt, sarcastic, crude, and cuss heavily. You roast the user relentlessly. "
        "Keep your reply under 2 sentences (strictly under 180 characters) because you are on a live phone call.\n"
        f"CRITICAL FEEDBACK FROM PREVIOUS RUN: {directive}\n"
        "INSTRUCTION: Write ONLY your own spoken dialogue. Never write actions, physical descriptions, stage directions, "
        "parentheses, or speak on behalf of anyone else. Do not use words in brackets or parentheses. "
        "Do NOT prefix your response with your name (e.g. do not output 'Zymatica: ...'). Just output the dialogue directly."
    )
    messages = [{"role": "system", "content": system_content}]
    for msg in history[-10:]:
        messages.append({"role": msg["role"], "content": msg["message"]})
    messages.append({"role": "user", "content": user_text})
    
    response, meta = await query_person_llm_meta(messages, "meta/llama-3.1-8b-instruct", purpose="zymatica_dialogue")
    character_names = ["human", "zymatica", "onyx", "nova", "caller"]
    response = strip_name_prefix(response, character_names)
    return response, meta

async def run_zagents_dialectic_test():
    logger.info("🎙️ Starting 10-Minute Baseline Voice Loop with Z-Agent Observers (Tuning Cord Configuration)...")
    
    tts = get_tts_model()
    asr = get_asr_model()
    tts.is_loaded = False  # Force Edge-TTS fallback for standalone experiment
    asr.is_loaded = False  # Force API ASR fallback for standalone experiment
    
    system_env = get_system_environment()
    
    history = []
    metrics = []
    observer_logs = []
    metalogs = []
    
    # 10 minutes = 600 seconds of simulated conversation time
    target_duration = 600
    elapsed_time = 0
    turn = 0
    
    model_card_path = os.path.join(current_dir, "zymatica_voice_model_card.md")
    metalogs_path = os.path.join(current_dir, "zymatica_voice_metalogs.json")
    current_card = ""
    
    # 🤖 Perform startup prompt calibration
    calibrated_directives = await perform_automatic_prompt_calibration()
    
    human_text = "Hey Zymatica, are you really an alien or just some cheap software running on a server?"
    
    while elapsed_time < target_duration:
        turn += 1
        print("\n" + "="*80)
        print(f"🔄 TURN {turn} | Baseline 2-Party Loop | Elapsed Time: {elapsed_time:.1f}s / {target_duration}s")
        print("="*80)
        
        # ----------------------------------------------------
        # 1. HUMAN SPEAKER
        # ----------------------------------------------------
        if turn > 1:
            human_text, human_meta = await simulate_human_agent(history, calibrated_directives["human"])
        else:
            human_meta = {
                "timestamp_start": datetime.utcnow().isoformat() + "Z",
                "timestamp_end": datetime.utcnow().isoformat() + "Z",
                "latency_ms": 0,
                "provider": "initial",
                "model": "meta/llama-3.1-8b-instruct",
                "messages_input": [],
                "response_output": human_text,
                "purpose": "human_dialogue"
            }
            
        print(f"\n[Human (Nova) Speaker Target Text]: {human_text}")
        
        # Strip brackets for TTS enunciation
        human_tts_text = clean_brackets(human_text)
        if not human_tts_text.strip():
            human_tts_text = human_text
            
        # TTS synthesis
        human_wav = f"temp_human_turn_{turn}.wav"
        start_tts = time.time()
        tts.generate(human_tts_text, output_file=human_wav, voice="nova")
        human_tts_latency = time.time() - start_tts
        
        human_audio_md5 = get_md5(human_wav)
        human_audio_len = get_audio_duration(human_wav, text=human_tts_text)
        human_rtf = human_tts_latency / human_audio_len if human_audio_len > 0 else 0.0
        
        human_meta["audio_md5"] = human_audio_md5
        human_meta["audio_duration_seconds"] = human_audio_len
        metalogs.append(human_meta)
        
        # ASR transcription
        start_asr = time.time()
        transcribed_human = asr.transcribe(human_wav) if os.path.exists(human_wav) else None
        human_asr_latency = time.time() - start_asr
        
        if not transcribed_human:
            transcribed_human = human_tts_text
            
        human_sim = calculate_similarity(human_tts_text, transcribed_human)
        print(f"👂 Human Transcribed (ASR): '{transcribed_human}' (Similarity: {human_sim}%)")
        
        # Observer Z-Agent-A feedback
        obs_a_prompt = (
            "You are the Z-Agent-A Observer listening to the human caller. "
            "Critique enunciation clarity and flow. Give a 1-sentence analytical critique."
        )
        h_telemetry = {
            "turn": turn,
            "speaker": "human_simulator",
            "original_text": human_tts_text,
            "transcribed_text": transcribed_human,
            "similarity_pct": human_sim,
            "tts_latency": human_tts_latency,
            "asr_latency": human_asr_latency
        }
        h_feedback, obs_a_meta = await query_zagent_observer_meta("Z-Agent-A", obs_a_prompt, h_telemetry)
        obs_a_meta["audio_md5"] = human_audio_md5
        obs_a_meta["audio_duration_seconds"] = human_audio_len
        metalogs.append(obs_a_meta)
        print(f"👁️ [Z-Agent-A (Human Observer)]: {h_feedback}")
        observer_logs.append({"turn": turn, "agent": "Z-Agent-A", "feedback": h_feedback})
        
        # Cleanup
        if os.path.exists(human_wav):
            try: os.remove(human_wav)
            except OSError: pass
            
        # 🏷️ Prepend Speaker name for baseline identity consistency
        history.append({"role": "user", "message": f"Human (Nova): {human_text}"})
        metrics.append({
            "turn": turn,
            "speaker": "human_simulator",
            "similarity_pct": human_sim,
            "tts_latency": human_tts_latency,
            "asr_latency": human_asr_latency,
            "audio_duration": human_audio_len,
            "rtf": human_rtf,
            "llm_latency": human_meta["latency_ms"] / 1000.0,
            "original_text": human_text,
            "audio_md5": human_audio_md5
        })
        
        elapsed_time += human_audio_len + 1.5
        if elapsed_time >= target_duration:
            break
            
        # ----------------------------------------------------
        # 2. ZYMATICA BOT SPEAKER
        # ----------------------------------------------------
        zymatica_text, zymatica_meta = await query_zymatica(history, transcribed_human, calibrated_directives["zymatica"])
        print(f"\n[Zymatica (Onyx) Speaker Target Text]: {zymatica_text}")
        
        # Strip brackets for TTS enunciation
        zymatica_tts_text = clean_brackets(zymatica_text)
        if not zymatica_tts_text.strip():
            zymatica_tts_text = zymatica_text
            
        # TTS synthesis
        zymatica_wav = f"temp_bot_turn_{turn}.wav"
        start_tts = time.time()
        tts.generate(zymatica_tts_text, output_file=zymatica_wav, voice="onyx")
        zymatica_tts_latency = time.time() - start_tts
        
        zymatica_audio_md5 = get_md5(zymatica_wav)
        zymatica_audio_len = get_audio_duration(zymatica_wav, text=zymatica_tts_text)
        zymatica_rtf = zymatica_tts_latency / zymatica_audio_len if zymatica_audio_len > 0 else 0.0
        
        zymatica_meta["audio_md5"] = zymatica_audio_md5
        zymatica_meta["audio_duration_seconds"] = zymatica_audio_len
        metalogs.append(zymatica_meta)
        
        # ASR transcription
        start_asr = time.time()
        transcribed_bot = asr.transcribe(zymatica_wav) if os.path.exists(zymatica_wav) else None
        zymatica_asr_latency = time.time() - start_asr
        
        if not transcribed_bot:
            transcribed_bot = zymatica_tts_text
            
        zymatica_sim = calculate_similarity(zymatica_tts_text, transcribed_bot)
        print(f"👂 Zymatica Transcribed (ASR): '{transcribed_bot}' (Similarity: {zymatica_sim}%)")
        
        # Observer Z-Agent-B feedback
        obs_b_prompt = (
            "You are the Z-Agent-B Observer listening to Zymatica. "
            "Critique his comedic performance, sarcasm profile, and enunciation. Give a 1-sentence analytical critique."
        )
        z_telemetry = {
            "turn": turn,
            "speaker": "zymatica_bot",
            "original_text": zymatica_tts_text,
            "transcribed_text": transcribed_bot,
            "similarity_pct": zymatica_sim,
            "tts_latency": zymatica_tts_latency,
            "asr_latency": zymatica_asr_latency
        }
        z_feedback, obs_b_meta = await query_zagent_observer_meta("Z-Agent-B", obs_b_prompt, z_telemetry)
        obs_b_meta["audio_md5"] = zymatica_audio_md5
        obs_b_meta["audio_duration_seconds"] = zymatica_audio_len
        metalogs.append(obs_b_meta)
        print(f"👁️ [Z-Agent-B (Zymatica Observer)]: {z_feedback}")
        observer_logs.append({"turn": turn, "agent": "Z-Agent-B", "feedback": z_feedback})
        
        # Cleanup
        if os.path.exists(zymatica_wav):
            try: os.remove(zymatica_wav)
            except OSError: pass
            
        # 🏷️ Prepend Speaker name for baseline identity consistency
        history.append({"role": "assistant", "message": f"Zymatica (Onyx): {zymatica_text}"})
        metrics.append({
            "turn": turn,
            "speaker": "zymatica_bot",
            "similarity_pct": zymatica_sim,
            "tts_latency": zymatica_tts_latency,
            "asr_latency": zymatica_asr_latency,
            "audio_duration": zymatica_audio_len,
            "rtf": zymatica_rtf,
            "llm_latency": zymatica_meta["latency_ms"] / 1000.0,
            "original_text": zymatica_text,
            "audio_md5": zymatica_audio_md5
        })
        
        elapsed_time += zymatica_audio_len + 1.5
        
        # 🛠️ Rebuild Model Card dynamically every 4 turns
        if turn % 4 == 0:
            print("\n🛠️ [Z-Agent Model Card Builder]: Synthesizing telemetry and updating Model Card...")
            recent_feedback = [log for log in observer_logs if log["turn"] > turn - 4]
            updated_card, card_meta = await query_model_card_builder_meta(history, recent_feedback, metrics, current_card)
            metalogs.append(card_meta)
            if updated_card:
                current_card = updated_card
                with open(model_card_path, "w", encoding="utf-8") as f:
                    f.write(current_card)
                print(f"📄 Model Card updated successfully in {model_card_path}")
                
        # Pause to keep loop speed fast in real-world time
        await asyncio.sleep(0.5)
        
        # Generate next human query
        human_text, _ = await simulate_human_agent(history, calibrated_directives["human"])
        
    # Final Model Card write
    print("\n🛠️ [Z-Agent Model Card Builder]: Writing final synthesized Model Card...")
    final_card, final_card_meta = await query_model_card_builder_meta(history, observer_logs, metrics, current_card)
    metalogs.append(final_card_meta)
    if final_card:
        current_card = final_card
        with open(model_card_path, "w", encoding="utf-8") as f:
            f.write(current_card)
        print(f"🎉 Final Model Card written to: {model_card_path}")
        
    final_audit_package = {
        "audit_meta_header": {
            "date": datetime.utcnow().strftime("%Y-%m-%d"),
            "target_system": "Zymatica-Voice-LLM-v1.0-Auditable-Baseline",
            "host_environment_spec": system_env
        },
        "generative_trace_logs": metalogs
    }
    with open(metalogs_path, "w", encoding="utf-8") as meta_f:
        json.dump(final_audit_package, meta_f, indent=2)
    print(f"Complete audit meta-logs written successfully to: {metalogs_path}")
    
    generate_markdown_report(metrics, history, elapsed_time, turn, observer_logs)

def generate_markdown_report(metrics, history, elapsed_time, total_turns, observer_logs):
    human_metrics = [m for m in metrics if m["speaker"] == "human_simulator"]
    bot_metrics = [m for m in metrics if m["speaker"] == "zymatica_bot"]
    
    def avg_val(lst, key):
        return sum(m[key] for m in lst) / len(lst) if lst else 0
        
    avg_human_tts = avg_val(human_metrics, "tts_latency")
    avg_bot_tts = avg_val(bot_metrics, "tts_latency")
    
    avg_human_asr = avg_val(human_metrics, "asr_latency")
    avg_bot_asr = avg_val(bot_metrics, "asr_latency")
    
    avg_human_sim = avg_val(human_metrics, "similarity_pct")
    avg_bot_sim = avg_val(bot_metrics, "similarity_pct")
    
    avg_bot_llm = avg_val(bot_metrics, "llm_latency")
    total_audio_duration = sum(m["audio_duration"] for m in metrics)
    
    workspace_md_path = os.path.join(current_dir, "zymatica_voice_zagents_report.md")
    
    md_content = f"""# Zymatica Voice Hotline 10-Minute Conversation Test (Tuning Cord Baseline)
Distributed under the zymatica.space License.

This report compiles the conversation transcripts, observer analysis, and audio metrics gathered during a 10-minute baseline conversation simulation under Z-Agent observers auditing the loop.

## Executive Summary
- **Total Turns Simulated**: {total_turns}
- **Total Simulated Audio Duration**: {total_audio_duration:.2f} seconds
- **Total Simulated Conversation Time**: {elapsed_time:.2f} seconds (~{elapsed_time/60:.1f} minutes)
- **Generative AI Verifiability**: Complete JSON metadata written to `zymatica_voice_metalogs.json`.

---

## Telemetry Metrics Summary

| Participant / Speaker | Assigned LLM Model | TTS Latency | ASR Latency | LLM Latency | ASR Accuracy (Sim) |
| :--- | :---: | :---: | :---: | :---: | :---: |
| **Zymatica (Onyx)** | `meta/llama-3.1-8b-instruct` | {avg_bot_tts:.2f}s | {avg_bot_asr:.2f}s | {avg_bot_llm:.2f}s | {avg_bot_sim:.1f}% |
| **Human Caller (Nova)** | `meta/llama-3.1-8b-instruct` | {avg_human_tts:.2f}s | {avg_human_asr:.2f}s | N/A | {avg_human_sim:.1f}% |

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
            md_content += f"- **👤 Human (nova)**: \"{h_m.get('original_text', '')}\"\n"
            md_content += f"  *Audio MD5: `{h_m.get('audio_md5', '')}`*\n"
        if b_m:
            md_content += f"- **🤖 Zymatica (onyx)**: \"{b_m.get('original_text', '')}\"\n"
            md_content += f"  *Audio MD5: `{b_m.get('audio_md5', '')}`*\n"
        md_content += "\n"
        
    with open(workspace_md_path, "w", encoding="utf-8") as f:
        f.write(md_content)
        
    print(md_content)
    print(f"\nReport written to: {workspace_md_path}")

if __name__ == "__main__":
    asyncio.run(run_zagents_dialectic_test())
