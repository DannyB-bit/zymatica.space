import os
import sys
import argparse
import subprocess
import logging

# Set up logging
logging.basicConfig(level=logging.INFO, format="%(asctime)s [%(levelname)s]: %(message)s")
logger = logging.getLogger("ZymaticaASRTrainer")

def run_lora_training(model_path, data_dir, output_dir, epochs, batch_size, lr):
    """
    Spawns the VibeVoice ASR LoRA fine-tuning subprocess.
    Fine-tunes the speech-to-text language model so that it adapts to 
    specific voice qualities, accents, and custom vocabularies (e.g. crypto terminology).
    """
    logger.info("🎙️ Setting up VibeVoice ASR Transcription Fine-tuning...")
    
    # Locate the finetuning script in temp_vibevoice
    current_dir = os.path.dirname(os.path.abspath(__file__))
    parent_dir = os.path.dirname(current_dir) # Z-Folder
    lora_script_path = os.path.join(parent_dir, "temp_vibevoice", "finetuning-asr", "lora_finetune.py")
    
    if not os.path.exists(lora_script_path):
        logger.error(f"❌ Could not find training script at {lora_script_path}")
        logger.info("Please ensure temp_vibevoice is cloned and accessible in the parent directory.")
        return False
        
    os.makedirs(output_dir, exist_ok=True)
    os.makedirs(data_dir, exist_ok=True)
    
    logger.info(f"📊 Training Data Directory: {data_dir}")
    logger.info(f"💾 Checkpoints Output Directory: {output_dir}")
    
    # Assemble torchrun command
    cmd = [
        "torchrun", "--nproc_per_node=1", lora_script_path,
        "--model_path", model_path,
        "--data_dir", data_dir,
        "--output_dir", output_dir,
        "--num_train_epochs", str(epochs),
        "--per_device_train_batch_size", str(batch_size),
        "--learning_rate", str(lr),
        "--bf16",
        "--report_to", "none"
    ]
    
    logger.info(f"🚀 Launching training command: {' '.join(cmd)}")
    
    try:
        # Run training loop in subprocess
        process = subprocess.Popen(
            cmd,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            bufsize=1
        )
        
        # Stream output in real-time
        for line in process.stdout:
            print(line, end="")
            
        process.wait()
        if process.returncode == 0:
            logger.info("🎉 LoRA fine-tuning completed successfully!")
            return True
        else:
            logger.error(f"❌ Training failed with exit code: {process.returncode}")
            return False
            
    except Exception as e:
        logger.error(f"❌ Error executing training: {e}")
        return False

def main():
    parser = argparse.ArgumentParser(description="Zymatica Voice Transcription (ASR) LoRA Fine-tuner")
    parser.add_argument(
        "--model_path", 
        type=str, 
        default=os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "vibevoice_asr_model"),
        help="Path to the base VibeVoice ASR model directory"
    )
    parser.add_argument(
        "--data_dir", 
        type=str, 
        default="./train_dataset",
        help="Directory containing training audio and transcript .json metadata pairs"
    )
    parser.add_argument(
        "--output_dir", 
        type=str, 
        default="./weights_output",
        help="Output directory where LoRA adapter checkpoints will be saved"
    )
    parser.add_argument("--epochs", type=int, default=3, help="Number of training epochs")
    parser.add_argument("--batch_size", type=int, default=1, help="Training batch size per device")
    parser.add_argument("--lr", type=float, default=1e-4, help="Learning rate for adamw optimizer")
    
    args = parser.parse_args()
    
    success = run_lora_training(
        model_path=args.model_path,
        data_dir=args.data_dir,
        output_dir=args.output_dir,
        epochs=args.epochs,
        batch_size=args.batch_size,
        lr=args.lr
    )
    
    if success:
        sys.exit(0)
    else:
        sys.exit(1)

if __name__ == "__main__":
    main()
