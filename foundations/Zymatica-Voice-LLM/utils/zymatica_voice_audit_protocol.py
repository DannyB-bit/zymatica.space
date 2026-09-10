import os
import sys
import time
import json
import re
import hashlib
import platform
import logging
from datetime import datetime

# Setup standard logger
logging.basicConfig(level=logging.INFO, format="%(asctime)s [%(levelname)s]: %(message)s")
logger = logging.getLogger("ZymaticaVoiceAuditor")

class ZymaticaVoiceAuditor:
    """
    Official standard protocol framework for collecting, verifying, and logging
    cryptographic and performance evidence when training Zymatica Voice AI agents.
    """
    def __init__(self, experiment_name, output_dir="."):
        self.experiment_name = experiment_name
        self.output_dir = output_dir
        self.trace_logs = []
        self.metrics = []
        self.observer_logs = []
        self.system_env = self.gather_system_environment()
        
        logger.info(f"Initialized Zymatica Voice Auditor for: {self.experiment_name}")
        
    def gather_system_environment(self):
        """Gathers detailed host hardware and software specifications for the audit logs."""
        env = {
            "os_name": os.name,
            "os_platform": sys.platform,
            "os_release": platform.release(),
            "os_version": platform.version(),
            "python_version": sys.version,
        }
        
        # Check PyTorch and CUDA
        try:
            import torch
            env["pytorch_version"] = torch.__version__
            env["cuda_available"] = torch.cuda.is_available()
            if env["cuda_available"]:
                env["cuda_device_name"] = torch.cuda.get_device_name(0)
                env["cuda_device_capability"] = torch.cuda.get_device_capability(0)
                env["cuda_device_memory_gb"] = round(torch.cuda.get_device_properties(0).total_memory / (1024**3), 2)
        except ImportError:
            env["pytorch_version"] = "Not Installed"
            env["cuda_available"] = False
            
        # Check System RAM and CPU Specs
        try:
            import psutil
            env["cpu_logical_cores"] = psutil.cpu_count(logical=True)
            env["cpu_physical_cores"] = psutil.cpu_count(logical=False)
            env["ram_total_gb"] = round(psutil.virtual_memory().total / (1024**3), 2)
        except ImportError:
            pass
            
        return env
        
    def calculate_md5(self, file_path):
        """Calculates the MD5 hash of an audio file for audit checksum validation."""
        if not os.path.exists(file_path):
            return ""
        hash_md5 = hashlib.md5()
        with open(file_path, "rb") as f:
            for chunk in iter(lambda: f.read(4096), b""):
                hash_md5.update(chunk)
        return hash_md5.hexdigest()
        
    def calculate_similarity(self, text1, text2):
        """Calculates word-level similarity percentage between two transcripts."""
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
        
    def log_turn(self, turn_number, speaker, original_text, transcribed_text, audio_path, 
                 llm_latency_ms, tts_latency_ms, asr_latency_ms, provider, model, messages_input):
        """Logs a single conversational turn with complete telemetry parameters."""
        audio_md5 = self.calculate_md5(audio_path)
        similarity = self.calculate_similarity(original_text, transcribed_text)
        
        # Determine speaking duration estimation
        audio_duration = 0.0
        try:
            import wave
            with wave.open(audio_path, 'r') as f:
                frames = f.getnframes()
                rate = f.getframerate()
                audio_duration = frames / float(rate)
        except Exception:
            words = original_text.split()
            if words:
                audio_duration = max(1.5, len(words) / 2.5) # Estimate based on 150 WPM
                
        rtf = ttf = 0.0
        if audio_duration > 0:
            rtf = (tts_latency_ms / 1000.0) / audio_duration
            
        metrics_payload = {
            "turn": turn_number,
            "speaker": speaker,
            "similarity_pct": similarity,
            "tts_latency": tts_latency_ms / 1000.0 if tts_latency_ms else 0.0,
            "asr_latency": asr_latency_ms / 1000.0 if asr_latency_ms else 0.0,
            "llm_latency": llm_latency_ms / 1000.0 if llm_latency_ms else 0.0,
            "audio_duration": audio_duration,
            "rtf": rtf,
            "original_text": original_text,
            "audio_md5": audio_md5
        }
        self.metrics.append(metrics_payload)
        
        # Log to trace
        trace_record = {
            "timestamp_start": datetime.utcnow().isoformat() + "Z",
            "latency_ms": llm_latency_ms,
            "provider": provider,
            "model": model,
            "messages_input": messages_input,
            "response_output": original_text,
            "purpose": f"{speaker}_dialogue",
            "audio_md5": audio_md5,
            "audio_duration_seconds": audio_duration
        }
        self.trace_logs.append(trace_record)
        
        logger.info(f"Logged turn {turn_number} for {speaker}. MD5: {audio_md5} | Latency: {llm_latency_ms}ms")
        return metrics_payload

    def log_observer_feedback(self, turn_number, observer_name, feedback_text, latency_ms, provider, model, context):
        """Logs critique feedback generated by dual-observer Z-Agent Observers."""
        feedback_record = {
            "timestamp_start": datetime.utcnow().isoformat() + "Z",
            "latency_ms": latency_ms,
            "provider": provider,
            "model": model,
            "messages_input": [
                {"role": "system", "content": f"Critique feedback instructions for {observer_name}."},
                {"role": "user", "content": json.dumps(context)}
            ],
            "response_output": feedback_text,
            "purpose": f"observer_{observer_name.lower().replace(' ', '_')}"
        }
        self.trace_logs.append(feedback_record)
        self.observer_logs.append({
            "turn": turn_number,
            "agent": observer_name,
            "feedback": feedback_text
        })
        logger.info(f"Logged feedback from observer '{observer_name}' on turn {turn_number}")

    def write_audit_package(self, metalogs_filename="zymatica_voice_metalogs.json", 
                            report_filename="zymatica_voice_zagents_report.md"):
        """Saves both the trace JSON audit package and the telemetry Markdown report with log rotation."""
        metalogs_path = os.path.join(self.output_dir, metalogs_filename)
        report_path = os.path.join(self.output_dir, report_filename)
        
        # 1. Output Audit JSON Package with Log Rotation (5MB max_bytes, 5 backup files)
        max_bytes = 5 * 1024 * 1024
        backup_count = 5
        if os.path.exists(metalogs_path) and os.path.getsize(metalogs_path) > max_bytes:
            logger.info(f"Audit log {metalogs_path} size exceeds {max_bytes} bytes. Rotating history...")
            for i in range(backup_count - 1, 0, -1):
                sfn = os.path.join(self.output_dir, f"{metalogs_filename.replace('.json', '')}.{i}.json")
                dfn = os.path.join(self.output_dir, f"{metalogs_filename.replace('.json', '')}.{i+1}.json")
                if os.path.exists(sfn):
                    if os.path.exists(dfn):
                        os.remove(dfn)
                    os.rename(sfn, dfn)
            dfn = os.path.join(self.output_dir, f"{metalogs_filename.replace('.json', '')}.1.json")
            if os.path.exists(dfn):
                os.remove(dfn)
            os.rename(metalogs_path, dfn)
            logger.info(f"Rotated active log {metalogs_path} to {dfn}")

        audit_package = {
            "audit_meta_header": {
                "date": datetime.utcnow().strftime("%Y-%m-%d"),
                "target_system": "Zymatica-Voice-LLM-Standard-Auditable",
                "host_environment_spec": self.system_env
            },
            "generative_trace_logs": self.trace_logs
        }
        
        with open(metalogs_path, "w", encoding="utf-8") as f:
            json.dump(audit_package, f, indent=2)
        logger.info(f"Audit trace JSON package written to: {metalogs_path}")
        
        # 2. Output MD Report
        human_metrics = [m for m in self.metrics if "human" in m["speaker"]]
        bot_metrics = [m for m in self.metrics if "zymatica" in m["speaker"] or "boyfriend" in m["speaker"]]
        
        avg_human_tts = sum(m["tts_latency"] for m in human_metrics) / len(human_metrics) if human_metrics else 0
        avg_bot_tts = sum(m["tts_latency"] for m in bot_metrics) / len(bot_metrics) if bot_metrics else 0
        
        avg_human_asr = sum(m["asr_latency"] for m in human_metrics) / len(human_metrics) if human_metrics else 0
        avg_bot_asr = sum(m["asr_latency"] for m in bot_metrics) / len(bot_metrics) if bot_metrics else 0
        
        avg_human_sim = sum(m["similarity_pct"] for m in human_metrics) / len(human_metrics) if human_metrics else 0
        avg_bot_sim = sum(m["similarity_pct"] for m in bot_metrics) / len(bot_metrics) if bot_metrics else 0
        
        avg_bot_llm = sum(m["llm_latency"] for m in bot_metrics) / len(bot_metrics) if bot_metrics else 0
        total_audio = sum(m["audio_duration"] for m in self.metrics)
        
        md_content = f"""# Zymatica Voice Agent Dialectic Telemetry Evaluation Report
        
This report contains metrics, transcripts, and critiques validated dynamically according to the Zymatica Voice Audit Protocol.

## Summary Telemetry
- **Experiment Title**: {self.experiment_name}
- **Total Conversation Turns**: {len(self.metrics)}
- **Audio Duration**: {total_audio:.2f}s
- **Host Spec OS**: {self.system_env.get('os_platform')} | GPU: {self.system_env.get('cuda_device_name', 'None')}

## Metrics Summary Table

| Metric | human_simulator | zymatica_agent | Overall Average |
| :--- | :---: | :---: | :---: |
| **TTS Latency** | {avg_human_tts:.2f}s | {avg_bot_tts:.2f}s | {(avg_human_tts + avg_bot_tts)/2:.2f}s |
| **ASR Latency** | {avg_human_asr:.2f}s | {avg_bot_asr:.2f}s | {(avg_human_asr + avg_bot_asr)/2:.2f}s |
| **LLM Latency** | N/A | {avg_bot_llm:.2f}s | {avg_bot_llm:.2f}s |
| **ASR Accuracy (Similarity)** | {avg_human_sim:.1f}% | {avg_bot_sim:.1f}% | {(avg_human_sim + avg_bot_sim)/2:.1f}% |

## Observer Critiques
"""
        for log in self.observer_logs:
            md_content += f"- **{log['agent']} (Turn {log['turn']})**: *\"{log['feedback']}\"*\n"
            
        md_content += "\n## Transcripts & MD5 Signatures\n"
        for m in self.metrics:
            md_content += f"### Turn {m['turn']} | {m['speaker']}\n"
            md_content += f"- **Statement**: \"{m['original_text']}\"\n"
            md_content += f"- **Audio Checksum**: `{m['audio_md5']}`\n\n"
            
        with open(report_path, "w", encoding="utf-8") as rf:
            rf.write(md_content)
        logger.info(f"Quantitative report written to: {report_path}")
        
    def sync_to_huggingface(self, token, repo_id, folder_path):
        """Syncs the completed audit logs and report files to Hugging Face Model Hub."""
        try:
            from huggingface_hub import HfApi, upload_folder
            logger.info(f"Syncing folder '{folder_path}' to HF Hub repository '{repo_id}'...")
            api = HfApi(token=token)
            api.upload_folder(
                folder_path=folder_path,
                repo_id=repo_id,
                repo_type="model"
            )
            logger.info("🎉 Hugging Face folder upload completed successfully!")
        except Exception as e:
            logger.error(f"Failed to sync to Hugging Face: {e}")
