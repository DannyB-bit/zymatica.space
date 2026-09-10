import os
import sys
import io
import re
import asyncio
import logging
import edge_tts

# Ensure UTF-8 output encoding on Windows
if sys.platform == "win32":
    sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding='utf-8')
    sys.stderr = io.TextIOWrapper(sys.stderr.buffer, encoding='utf-8')

# Setup logging
logging.basicConfig(level=logging.INFO, format="%(asctime)s [%(levelname)s]: %(message)s")
logger = logging.getLogger("ZymaticaRecorderExp5")

async def generate_full_recording():
    current_dir = os.path.dirname(os.path.abspath(__file__))
    report_path = os.path.join(current_dir, "zymatica_voice_zagents_report_exp5.md")
    output_mp3_path = os.path.join(current_dir, "zymatica_conversation_recording_exp5.mp3")
    
    if not os.path.exists(report_path):
        logger.error(f"Report file not found at {report_path}. Run the simulation first!")
        return
        
    logger.info(f"Reading transcript from {report_path}...")
    with open(report_path, "r", encoding="utf-8") as f:
        content = f.read()
        
    turns = []
    lines = content.split('\n')
    current_turn_num = None
    
    for line in lines:
        if line.startswith("### Turn "):
            try:
                current_turn_num = int(line.split("|")[0].replace("### Turn ", "").strip())
            except (ValueError, IndexError):
                pass
        elif "- **Zymatica**:" in line or "- **Zymatica (onyx)**:" in line or "- **Zymatica (brian)**:" in line:
            match = re.search(r'-\s+\*\*.*?\*\*:\s*"([^"]+)"', line)
            if match:
                turns.append(("zymatica", match.group(1)))
        elif "- **Boss**:" in line or "- **The boss**:" in line or "- **Boss (arthur)**:" in line or "- **Boss (alloy)**:" in line:
            match = re.search(r'-\s+\*\*.*?\*\*:\s*"([^"]+)"', line)
            if match:
                turns.append(("boss", match.group(1)))
        elif "- **Sarah**:" in line or "- **Sarah (aria)**:" in line or "- **Sarah (nova)**:" in line:
            match = re.search(r'-\s+\*\*.*?\*\*:\s*"([^"]+)"', line)
            if match:
                turns.append(("sarah", match.group(1)))
        elif "- **Claire**:" in line or "- **Claire (michelle)**:" in line or "- **Claire (shimmer)**:" in line:
            match = re.search(r'-\s+\*\*.*?\*\*:\s*"([^"]+)"', line)
            if match:
                turns.append(("claire", match.group(1)))
                
    if not turns:
        logger.error("Failed to parse any conversation turns from the report!")
        return
        
    logger.info(f"Found {len(turns)} dialogue turns. Synthesizing conversation...")
    
    master_bytes = bytearray()
    
    for idx, (speaker, text) in enumerate(turns):
        turn_num = idx + 1
        # Determine voice
        if speaker == "zymatica":
            voice = "en-US-BrianNeural"
            speaker_name = "Zymatica"
        elif speaker == "boss":
            voice = "en-US-SteffanNeural"
            speaker_name = "Boss (Arthur)"
        elif speaker == "sarah":
            voice = "en-US-AriaNeural"
            speaker_name = "Sarah"
        else: # claire
            voice = "en-US-MichelleNeural"
            speaker_name = "Claire"
            
        logger.info(f"[{turn_num}/{len(turns)}] Synthesizing {speaker_name}: \"{text[:40]}...\"")
        
        try:
            communicate = edge_tts.Communicate(text, voice)
            
            # Save chunk to temp file
            temp_chunk = f"temp_chunk_exp5_{idx}.mp3"
            await communicate.save(temp_chunk)
            
            # Read bytes
            if os.path.exists(temp_chunk):
                with open(temp_chunk, "rb") as tf:
                    master_bytes.extend(tf.read())
                os.remove(temp_chunk)
        except Exception as e:
            logger.error(f"Failed to synthesize turn {turn_num}: {e}")
            
    # Write full recording
    with open(output_mp3_path, "wb") as out_f:
        out_f.write(master_bytes)
        
    logger.info(f"Recording generated successfully: {output_mp3_path}")
    logger.info(f"File size: {len(master_bytes) / 1024 / 1024:.2f} MB")

if __name__ == "__main__":
    asyncio.run(generate_full_recording())
