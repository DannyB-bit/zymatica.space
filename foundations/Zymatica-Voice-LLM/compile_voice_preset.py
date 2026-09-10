import os
import sys
import argparse
import torch
import numpy as np
import soundfile as sf
import logging

# Set up logging
logging.basicConfig(level=logging.INFO, format="%(asctime)s [%(levelname)s]: %(message)s")
logger = logging.getLogger("ZymaticaVoicePresetCompiler")

# Add temp_vibevoice to sys.path to resolve imports
current_dir = os.path.dirname(os.path.abspath(__file__))
parent_dir = os.path.dirname(current_dir) # Z-Folder
sys.path.append(os.path.join(parent_dir, "temp_vibevoice"))

try:
    from vibevoice.modular.modeling_vibevoice_streaming_inference import VibeVoiceStreamingForConditionalGenerationInference
    from vibevoice.processor.vibevoice_streaming_processor import VibeVoiceStreamingProcessor
    logger.info("✅ VibeVoice modules imported successfully for compiler")
except ImportError as e:
    logger.error(f"❌ Failed to import VibeVoice modules: {e}")
    logger.info("Trying direct flat imports fallback...")
    try:
        from modular.modeling_vibevoice_streaming_inference import VibeVoiceStreamingForConditionalGenerationInference
        from processor.vibevoice_streaming_processor import VibeVoiceStreamingProcessor
        logger.info("✅ VibeVoice modules imported successfully (flat fallback)")
    except Exception as e2:
        logger.error(f"❌ Failed to import VibeVoice modules (fallback): {e2}")

def extract_and_compile_preset(model_path, audio_path, transcript, output_preset_path):
    """
    Extracts the key-value activations (KV-cache) for both the base text LM and TTS LM
    from a short, high-quality reference audio (3-10s) and its transcription.
    Saves the extracted state dictionary as a .pt file which VibeVoice uses to clone the speaker's timbre.
    """
    logger.info("🔊 Loading VibeVoice Realtime Model & Processor...")
    device = "cuda" if torch.cuda.is_available() else "cpu"
    dtype = torch.bfloat16 if (device == "cuda" and torch.cuda.is_bf16_supported()) else torch.float32
    
    try:
        processor = VibeVoiceStreamingProcessor.from_pretrained(model_path)
        model = VibeVoiceStreamingForConditionalGenerationInference.from_pretrained(
            model_path,
            torch_dtype=dtype
        ).to(device)
        model.eval()
    except Exception as err:
        logger.error(f"❌ Failed to load VibeVoice for preset compilation: {err}")
        return False
        
    logger.info(f"🎙️ Reading studio-quality reference audio from: {audio_path}")
    if not os.path.exists(audio_path):
        logger.error("❌ Audio path does not exist.")
        return False
        
    try:
        # Load audio file (convert to mono, 24kHz)
        audio_data, samplerate = sf.read(audio_path)
        if samplerate != 24000:
            logger.warning(f"⚠️ Audio sample rate is {samplerate}Hz. VibeVoice expects 24,000Hz PCM mono.")
            # Simple downsampling/upsampling placeholder if scipy is installed
            try:
                import scipy.signal
                num_samples = int(len(audio_data) * 24000 / samplerate)
                audio_data = scipy.signal.resample(audio_data, num_samples)
                samplerate = 24000
                logger.info("🔄 Audio resampled to 24000Hz successfully.")
            except ImportError:
                logger.error("❌ Audio is not 24000Hz. Install scipy or provide a 24000Hz wave file.")
                return False
                
        # Handle stereo downmixing
        if len(audio_data.shape) > 1:
            audio_data = np.mean(audio_data, axis=1)
            logger.info("🔄 Audio downmixed to mono.")
            
    except Exception as err:
        logger.error(f"❌ Failed to parse reference wave: {err}")
        return False
        
    logger.info(f"✍️ Compiling prompt transcription: '{transcript}'")
    
    # Process inputs through text and audio encoders
    try:
        # Encode speaker transcript
        prompt_tokens = processor.tokenizer.encode(transcript.strip() + "\n", add_special_tokens=False)
        
        # Quantize audio into acoustic tokens using VibeVoice's acoustic tokenizer
        speech_array = torch.tensor(audio_data, dtype=torch.float32, device=device).unsqueeze(0)
        
        # Run forward pass of model encoders to populate cache
        logger.info("⚡ Computing prompt cached activations...")
        with torch.no_grad():
            # 1. Base Text LM Prefilling Pass
            input_ids = torch.tensor([prompt_tokens], dtype=torch.long, device=device)
            lm_outputs = model.forward_lm(
                input_ids=input_ids,
                use_cache=True,
                return_dict=True
            )
            
            # 2. Extract Acoustic latents
            # Scale and tokenize audio bytes
            normalized_speech = processor.audio_processor._normalize_audio(audio_data)
            speech_tensor = torch.tensor(normalized_speech, dtype=torch.float32, device=device).unsqueeze(0)
            
            with torch.no_grad():
                # Extract latents via acoustic tokenizer
                latents = model.model.acoustic_tokenizer.encode(speech_tensor)
                # Apply connector scaling
                acoustic_embed = model.model.acoustic_connector(latents)
                
            # 3. TTS LM Prefilling Pass
            tts_lm_input_ids = torch.tensor([prompt_tokens], dtype=torch.long, device=device)
            tts_text_masks = torch.ones_like(tts_lm_input_ids)
            
            tts_lm_outputs = model.forward_tts_lm(
                input_ids=tts_lm_input_ids,
                tts_text_masks=tts_text_masks,
                lm_last_hidden_state=acoustic_embed,
                use_cache=True,
                return_dict=True
            )
            
            # 4. Compile negative conditions (unconditional classifier-free priors)
            neg_tok = processor.tokenizer.convert_tokens_to_ids("<|image_pad|>")
            neg_ids = torch.tensor([[neg_tok]], dtype=torch.long, device=device)
            
            neg_lm_outputs = model.forward_lm(
                input_ids=neg_ids,
                use_cache=True,
                return_dict=True
            )
            
            neg_tts_lm_outputs = model.forward_tts_lm(
                input_ids=neg_ids,
                tts_text_masks=torch.ones_like(neg_ids),
                lm_last_hidden_state=acoustic_embed[:, :1, :], # truncated
                use_cache=True,
                return_dict=True
            )
            
        logger.info("💾 Formatting prefilled activation cache dict...")
        # Compile final outputs into preset dict
        all_prefilled_outputs = {
            "lm": lm_outputs,
            "tts_lm": tts_lm_outputs,
            "neg_lm": neg_lm_outputs,
            "neg_tts_lm": neg_tts_lm_outputs
        }
        
        # Save output preset file
        torch.save(all_prefilled_outputs, output_preset_path)
        logger.info(f"🎉 Studio-quality voice preset successfully saved to: {output_preset_path}")
        return True
        
    except Exception as err:
        logger.error(f"❌ Failed to extract KV-cache: {err}")
        import traceback
        logger.error(traceback.format_exc())
        return False

def main():
    parser = argparse.ArgumentParser(description="Zymatica Voice Studio Preset KV-Cache Compiler")
    parser.add_argument(
        "--model_path", 
        type=str, 
        default=os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "vibevoice_realtime_model"),
        help="Path to the local VibeVoice Realtime 0.5B model folder"
    )
    parser.add_argument(
        "--audio_path", 
        type=str, 
        required=True, 
        help="Path to 3-10s studio-recorded 24kHz mono reference audio (.wav)"
    )
    parser.add_argument(
        "--transcript", 
        type=str, 
        required=True, 
        help="Literal textual transcription of the reference audio"
    )
    parser.add_argument(
        "--output", 
        type=str, 
        default="./my_voice_preset.pt",
        help="Output path for the compiled speaker preset file (.pt)"
    )
    
    args = parser.parse_args()
    
    success = extract_and_compile_preset(
        model_path=args.model_path,
        audio_path=args.audio_path,
        transcript=args.transcript,
        output_preset_path=args.output
    )
    
    if success:
        sys.exit(0)
    else:
        sys.exit(1)

if __name__ == "__main__":
    main()
