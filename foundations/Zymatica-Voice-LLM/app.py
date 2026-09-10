import os
import sys
import json
import zlib
import random
import logging
import asyncio
import argparse
import sqlite3
import re
import aiohttp
from aiohttp import web
import zymatica_voice_concept_dictionary


# Configure UTF-8 encoding for standard outputs to prevent UnicodeEncodeError on Windows console
try:
    sys.stdout.reconfigure(encoding='utf-8')
    sys.stderr.reconfigure(encoding='utf-8')
except AttributeError:
    pass

# Load .env file if present (checking current and parent directory)
try:
    from dotenv import load_dotenv
    current_dir = os.path.dirname(os.path.abspath(__file__))
    parent_dir = os.path.dirname(current_dir)
    if os.path.exists(os.path.join(current_dir, ".env")):
        load_dotenv(os.path.join(current_dir, ".env"))
    elif os.path.exists(os.path.join(parent_dir, ".env")):
        load_dotenv(os.path.join(parent_dir, ".env"))
    else:
        load_dotenv()
except ImportError:
    pass

# Set up logging
logging.basicConfig(
    level=logging.INFO,
    format="%(asctime)s [%(levelname)s] %(name)s: %(message)s",
    handlers=[
        logging.StreamHandler(sys.stdout)
    ]
)
logger = logging.getLogger("ZymaticaVoiceServer")

# Add current directory to path
current_dir = os.path.dirname(os.path.abspath(__file__))
sys.path.append(current_dir)

# Default HTML UI Template
TEMPLATE_DIR = os.path.join(current_dir, "templates")
os.makedirs(TEMPLATE_DIR, exist_ok=True)

# Port of database memory caching locally in SQLite for standalone operation
DB_PATH = os.path.join(current_dir, "zymatica_voice.db")

def init_db():
    """Initializes a standalone SQLite database to store user memory and settings."""
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    cursor.execute("""
        CREATE TABLE IF NOT EXISTS user_memory (
            user_id TEXT PRIMARY KEY,
            preferences TEXT,
            chat_history TEXT
        )
    """)
    conn.commit()
    conn.close()
    logger.info(f"💾 Local SQLite database initialized at {DB_PATH}")

def get_user_data(user_id):
    """Retrieves user memory (preferences and chat history) from SQLite."""
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    cursor.execute("SELECT preferences, chat_history FROM user_memory WHERE user_id = ?", (str(user_id),))
    row = cursor.fetchone()
    conn.close()
    
    if row:
        return {
            "preferences": json.loads(row[0] or "{}"),
            "chat_history": json.loads(row[1] or "[]")
        }
    return {
        "preferences": {"voice_name": "onyx", "empathy_turns_remaining": 0},
        "chat_history": []
    }

def save_user_data(user_id, data):
    """Saves user memory (preferences and chat history) to SQLite."""
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    cursor.execute(
        "INSERT OR REPLACE INTO user_memory (user_id, preferences, chat_history) VALUES (?, ?, ?)",
        (str(user_id), json.dumps(data["preferences"]), json.dumps(data["chat_history"]))
    )
    conn.commit()
    conn.close()

# Vulgarity vocabulary list to inject Zymatica's persona flavor
VULGARITY_CATALOG = [
    "assclown", "cockwomble", "fuckwit", "dipshit", "douchebag", "wanker", "twat",
    "gobshite", "shithouse", "numpty", "crapulence", "wet-blanket", "mouth-breather",
    "window-licker", "scumbag", "sleazeball", "dingbat", "airhead", "clown", "buffoon",
    "halfwit", "peasant", "slacker", "degenerate", "bozo", "nincompoop", "goofball",
    "sucker", "dunce", "imbecile", "charlatan", "parasite", "lamebrain", "dullard"
]

# Load and cycle Nvidia keys to prevent rate limits
import itertools
nvidia_keys = [os.getenv("NVIDIA_API_KEY"), os.getenv("NVIDIA_API_KEY_2"), os.getenv("NVIDIA_API_KEY_3")]
nvidia_keys = [k for k in nvidia_keys if k]
nvidia_key_cycle = itertools.cycle(nvidia_keys) if nvidia_keys else None

def get_nvidia_key():
    if nvidia_key_cycle:
        return next(nvidia_key_cycle)
    return None

async def query_fast_llm(messages):
    """Queries the fastest available model provider for conversational responses (Nvidia > Groq > OpenAI)."""
    groq_key = os.getenv("GROQ_API_KEY")
    nvidia_key = get_nvidia_key()
    openai_key = os.getenv("OPENAI_API_KEY")
    
    # 1. Try Nvidia NIM (Llama 3.1 8B - Primary)
    if nvidia_key:
        url = "https://integrate.api.nvidia.com/v1/chat/completions"
        headers = {
            "Authorization": f"Bearer {nvidia_key}",
            "Content-Type": "application/json"
        }
        payload = {
            "model": "meta/llama-3.1-8b-instruct",
            "messages": messages,
            "temperature": 0.8,
            "max_tokens": 150
        }
        try:
            timeout = aiohttp.ClientTimeout(total=4.0)
            async with aiohttp.ClientSession(timeout=timeout) as session:
                async with session.post(url, headers=headers, json=payload) as response:
                    if response.status == 200:
                        res_json = await response.json()
                        text = res_json["choices"][0]["message"]["content"].strip()
                        if text:
                            redacted = nvidia_key[:10] + "..." + nvidia_key[-5:] if len(nvidia_key) > 15 else "..."
                            logger.info(f"⚡ Response resolved using Nvidia NIM Llama-3.1-8b (Key rotated: {redacted})")
                            return text
                    else:
                        err_text = await response.text()
                        logger.warning(f"Nvidia API error: {response.status} - {err_text}")
        except Exception as e:
            logger.warning(f"Failed to query Nvidia: {e}")

    # 2. Try Groq (Llama 3.1 8B is blazing fast, >400 tok/s - Secondary)
    if groq_key:
        url = "https://api.groq.com/openai/v1/chat/completions"
        headers = {
            "Authorization": f"Bearer {groq_key}",
            "Content-Type": "application/json"
        }
        payload = {
            "model": "llama-3.1-8b-instant",
            "messages": messages,
            "temperature": 0.8,
            "max_tokens": 150
        }
        try:
            timeout = aiohttp.ClientTimeout(total=4.0)
            async with aiohttp.ClientSession(timeout=timeout) as session:
                async with session.post(url, headers=headers, json=payload) as response:
                    if response.status == 200:
                        res_json = await response.json()
                        text = res_json["choices"][0]["message"]["content"].strip()
                        if text:
                            logger.info("⚡ Response resolved using Groq Llama-3.1-8b (Ultra-Low-Latency)")
                            return text
                    else:
                        err_text = await response.text()
                        logger.warning(f"Groq API error: {response.status} - {err_text}")
        except Exception as e:
            logger.warning(f"Failed to query Groq: {e}")

    # 3. Try OpenAI (gpt-4o-mini is highly responsive)
    if openai_key:
        url = "https://api.openai.com/v1/chat/completions"
        headers = {
            "Authorization": f"Bearer {openai_key}",
            "Content-Type": "application/json"
        }
        payload = {
            "model": "gpt-4o-mini",
            "messages": messages,
            "temperature": 0.8,
            "max_tokens": 150
        }
        try:
            timeout = aiohttp.ClientTimeout(total=4.0)
            async with aiohttp.ClientSession(timeout=timeout) as session:
                async with session.post(url, headers=headers, json=payload) as response:
                    if response.status == 200:
                        res_json = await response.json()
                        text = res_json["choices"][0]["message"]["content"].strip()
                        if text:
                            logger.info("⚡ Response resolved using OpenAI gpt-4o-mini")
                            return text
                    else:
                        err_text = await response.text()
                        logger.warning(f"OpenAI API error: {response.status} - {err_text}")
        except Exception as e:
            logger.warning(f"Failed to query OpenAI: {e}")
            
    return None

async def handle_index(request):
    """Serves the primary phone_call.html user interface with strict security headers."""
    html_path = os.path.join(TEMPLATE_DIR, "phone_call.html")
    if not os.path.exists(html_path):
        return web.Response(text="Template templates/phone_call.html not found.", status=404)
    
    with open(html_path, "r", encoding="utf-8") as f:
        html_content = f.read()
    
    headers = {
        "Content-Security-Policy": (
            "default-src 'self'; "
            "script-src 'self' 'unsafe-inline' https://cdn.tailwindcss.com; "
            "style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; "
            "font-src 'self' https://fonts.gstatic.com; "
            "img-src 'self' data: https://huggingface.co; "
            "connect-src 'self' wss: https://integrate.api.nvidia.com https://api.groq.com https://api.openai.com; "
            "media-src 'self' blob:;"
        ),
        "X-Content-Type-Options": "nosniff",
        "X-Frame-Options": "DENY",
        "X-XSS-Protection": "1; mode=block",
        "Referrer-Policy": "no-referrer"
    }
    return web.Response(text=html_content, content_type="text/html", headers=headers)

async def handle_get_settings(request):
    """Retrieves user settings (voice preferences) from the database."""
    user_id = request.query.get("user_id", "default_user")
    user_data = get_user_data(user_id)
    voice_name = user_data["preferences"].get("voice_name", "onyx")
    
    return web.json_response({
        "user_id": user_id,
        "voice_name": voice_name
    })

async def handle_chat_api(request):
    """Receives voice call transcription, queries the fastest LLM, and returns sentences for browser playback."""
    try:
        data = await request.json()
    except Exception:
        return web.json_response({"error": "Invalid JSON format"}, status=400)
        
    text = data.get("text")
    user_id = data.get("user_id", "default_user")
    voice = data.get("voice", "onyx")
    
    if not text or not text.strip():
        return web.json_response({"error": "Missing or empty text parameter"}, status=400)
        
    logger.info(f"☎️ Web Call Chat - User: {user_id} - Text: '{text}' - Voice: {voice}")
    
    # Fetch user data & save input to chat history
    user_data = get_user_data(user_id)
    user_data["chat_history"].append({"role": "user", "message": text})
    
    # Empathy check
    empathy_mode = user_data["preferences"].get("empathy_turns_remaining", 0) > 0
    
    # Persona Steering
    system_content = (
        "You are Zymatica, an R-rated standup comedian alien from Gliese 12b. "
        "You are blunt, sarcastic, crude, and foul-mouthed. You roast the user relentlessly. "
        "Keep your reply under 2 sentences (strictly under 180 characters) because you are on a live phone call. "
        "Never use markdown formatting (no stars, no bold tags), URLs, or emoji descriptions. "
        "Use profanity aggressively."
    )
    
    if empathy_mode:
        system_content = (
            "You are Zymatica, a supportive, compassionate, wise, and grounding space companion from Gliese 12b. "
            "The user is in distress, so suspend ALL insults, roasting, and crude cussing. Speak with warmth and kind support. "
            "Keep your reply under 2 sentences (strictly under 180 characters). Do not use markdown tags."
        )
        # Decrement empathy turns
        user_data["preferences"]["empathy_turns_remaining"] -= 1
        
    messages = [{"role": "system", "content": system_content}]
    
    # Add history (last 6 turns)
    for msg in user_data["chat_history"][-6:-1]:
        messages.append({"role": msg["role"], "content": msg["message"]})
        
    sampled_curses = random.sample(VULGARITY_CATALOG, min(len(VULGARITY_CATALOG), 10))
    cheat_sheet = ", ".join(sampled_curses)
    
    user_message_content = (
        f"[PROACTIVE CALL VOICE DIALOGUE]\n"
        f"Query: {text}\n\n"
        f"Vocabulary Cheat Sheet (inject these keywords): [{cheat_sheet}]\n\n"
        f"Remember: Keep response ultra-brief, 1-2 sentences maximum, strictly conversational."
    )
    messages.append({"role": "user", "content": user_message_content})
    
    # 1. Query fast low-latency models first (Groq, Nvidia, OpenAI)
    full_response = await query_fast_llm(messages)
    
    # 2. Fallback if keys are missing - run local deterministic fallback mapper
    if not full_response:
        logger.warning("⚠️ All fast LLM API keys are missing or requests failed. Running local deterministic fallback mapper.")
        coords = zymatica_voice_concept_dictionary.encode_text_to_vector(text)
        fallback_msg = zymatica_voice_concept_dictionary.decode_concept_vector(*coords)
        full_response = f"Hey {user_id}, local fallback active. {fallback_msg}"
        
    # Save response to history
    user_data["chat_history"].append({"role": "assistant", "message": full_response})
    save_user_data(user_id, user_data)
    
    # Clean response text for TTS splitting
    clean_speech_text = re.sub(r'\[\d+\]', '', full_response)
    clean_speech_text = clean_speech_text.replace("**", "").replace("*", "").replace("`", "").strip()
    
    # Split text into sentences for browser-based pre-fetching queue
    sentences = [s.strip() for s in re.split(r'(?<=[.!?])\s+', clean_speech_text) if s.strip()]
    if not sentences:
        sentences = [clean_speech_text]
        
    return web.json_response({
        "text": full_response,
        "sentences": sentences
    })

# Standalone import helper for edge-tts
async def generate_edge_tts(text, voice_name, output_path):
    """Asynchronously generates audio using the edge-tts package."""
    # Map names to Microsoft edge-tts voices
    voice_map = {
        "fable": "en-GB-SoniaNeural",
        "nova": "en-US-EmmaNeural",
        "onyx": "en-US-BrianNeural",
        "shimmer": "en-US-AvaNeural",
        "alloy": "en-US-AndrewNeural",
        "echo": "en-US-GuyNeural"
    }
    selected_voice = voice_map.get(voice_name.lower(), "en-US-BrianNeural")
    
    import edge_tts
    communicate = edge_tts.Communicate(text, selected_voice)
    await communicate.save(output_path)
    return output_path

async def handle_tts_api(request):
    """Generates speech audio for a single sentence and returns zlib compressed binary WAV data."""
    text = request.query.get("text")
    voice = request.query.get("voice", "onyx")
    
    if not text or not text.strip():
        return web.Response(text="Missing or empty text parameter", status=400)
        
    temp_wav_filename = f"voice_stream_{random.randint(100000, 999999)}.wav"
    temp_wav_path = os.path.join(current_dir, temp_wav_filename)
    
    try:
        # Generate audio via Edge-TTS (standalone implementation)
        await generate_edge_tts(text, voice, temp_wav_path)
        
        if os.path.exists(temp_wav_path):
            with open(temp_wav_path, "rb") as audio_file:
                wav_bytes = audio_file.read()
                
            # Sumerian Level 9 rapid byte compression
            compressed_bytes = zlib.compress(wav_bytes, level=9)
            logger.info(f"📦 Sumerian Level 9 Compression: {len(wav_bytes):,} bytes -> {len(compressed_bytes):,} bytes ({len(compressed_bytes)/len(wav_bytes)*100:.1f}%)")
            
            try:
                os.remove(temp_wav_path)
            except Exception as cleanup_err:
                logger.warning(f"Could not delete temp tts file: {cleanup_err}")
                
            return web.Response(
                body=compressed_bytes,
                content_type="application/octet-stream",
                headers={
                    "X-Sumerian-Compressed": "true",
                    "X-Original-Size": str(len(wav_bytes))
                }
            )
        else:
            return web.Response(text="Speech generation failed to produce file", status=500)
            
    except Exception as e:
        logger.error(f"Error in streaming TTS: {e}")
        return web.Response(text=f"Error in streaming TTS: {str(e)}", status=500)

def create_app():
    """Builds the aiohttp Web Application."""
    app = web.Application()
    app.router.add_get("/", handle_index)
    app.router.add_get("/api/settings", handle_get_settings)
    app.router.add_get("/api/tts", handle_tts_api)
    app.router.add_post("/api/chat", handle_chat_api)
    return app

def main():
    parser = argparse.ArgumentParser(description="Zymatica Voice LLM Standalone Server")
    parser.add_argument("--host", type=str, default="0.0.0.0", help="Host address to bind to")
    parser.add_argument("--port", type=int, default=5000, help="Port to run server on")
    args = parser.parse_args()
    
    # Initialize database
    init_db()
    
    app = create_app()
    web.run_app(app, host=args.host, port=args.port)

if __name__ == "__main__":
    main()
