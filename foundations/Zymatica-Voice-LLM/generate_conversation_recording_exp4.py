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
logger = logging.getLogger("ZymaticaRecorderExp4")

async def generate_full_recording():
    current_dir = os.path.dirname(os.path.abspath(__file__))
    report_path = os.path.join(current_dir, "zymatica_voice_zagents_report_exp4.md")
    output_mp3_path = os.path.join(current_dir, "zymatica_conversation_recording_exp4.mp3")
    
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
        elif "- **Zymatica**:" in line or "- **Zymatica (onyx)**:" in line:
            match = re.search(r'-\s+\*\*.*?\*\*:\s*"([^"]+)"', line)
            if match:
                turns.append(("zymatica", match.group(1)))
        elif "- **Frank**:" in line or "- **Frank (frank)**:" in line:
            match = re.search(r'-\s+\*\*.*?\*\*:\s*"([^"]+)"', line)
            if match:
                turns.append(("frank", match.group(1)))
        elif "- **Mediator**:" in line or "- **Mediator (mediator)**:" in line:
            match = re.search(r'-\s+\*\*.*?\*\*:\s*"([^"]+)"', line)
            if match:
                turns.append(("mediator", match.group(1)))
                
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
        elif speaker == "frank":
            voice = "en-US-GuyNeural"
            speaker_name = "Frank"
        else: # mediator
            voice = "en-US-JennyNeural"
            speaker_name = "Mediator"
            
        logger.info(f"[{turn_num}/{len(turns)}] Synthesizing {speaker_name}: \"{text[:40]}...\"")
        
        try:
            communicate = edge_tts.Communicate(text, voice)
            
            # Save chunk to temp file
            temp_chunk = f"temp_chunk_exp4_{idx}.mp3"
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
