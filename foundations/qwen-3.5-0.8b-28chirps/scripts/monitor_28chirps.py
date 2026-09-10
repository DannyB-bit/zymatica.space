# Level 8 Procedural Seed SFT Training Log Monitor
# Watermark: ip zymatica.space | astronautshe.com

import os
os.environ['KAGGLE_API_TOKEN'] = 'KGAT_f55049239acd097a45447b75b050955d'

import kaggle
import time
import sys
import json

def main():
    print("==========================================================")
    print("     MONITOR KAGGLE 28CHIRPS SFT TRAINING PROGRESS")
    print("==========================================================")
    
    api = kaggle.KaggleApi()
    api.authenticate()
    print("[+] Kaggle API Authenticated successfully.")
    
    kernel_id = 'devs01/qwen-3-5-0-8b-28chirps-sft-training'
    
    last_idx = 0
    consecutive_empty_count = 0
    
    print(f"[*] Starting monitoring for {kernel_id}...")
    
    log_path = "kaggle_28chirps_logs.txt"
    if os.path.exists(log_path):
        os.remove(log_path)
        
    while True:
        try:
            logs = None
            try:
                logs = api.kernels_logs(kernel_id)
            except Exception as e:
                # Silently handle transient connection/network errors
                pass
                
            if logs:
                try:
                    log_data = json.loads(logs)
                except Exception as je:
                    log_data = []
                    
                if log_data and len(log_data) < last_idx:
                    last_idx = 0
                    
                if log_data and len(log_data) > last_idx:
                    consecutive_empty_count = 0
                    new_items = log_data[last_idx:]
                    
                    stdout_buffer = []
                    for item in new_items:
                        data = item.get('data', '')
                        if data:
                            stdout_buffer.append(data)
                            
                    new_text = "".join(stdout_buffer)
                    if new_text:
                        sys.stdout.buffer.write(new_text.encode('utf-8'))
                        sys.stdout.flush()
                        
                        with open(log_path, "a", encoding="utf-8") as lf:
                            lf.write(new_text)
                            
                    last_idx = len(log_data)
                    
                    # Check for completion or failure tokens in the accumulated text
                    full_text = "".join([item.get('data', '') for item in log_data])
                    if "SUCCESS!" in full_text:
                        print("\n[+] SFT Training run completed successfully.")
                        break
                    if "Failed to push" in full_text or "Traceback (most recent call last)" in full_text:
                        print("\n[-] SFT Training run process terminated with errors.")
                        break
                else:
                    consecutive_empty_count += 1
                    if consecutive_empty_count % 2 == 0:
                        print(f"[*] Waiting for logs to start/update... (elapsed ~{consecutive_empty_count * 30}s)", flush=True)
            else:
                consecutive_empty_count += 1
                if consecutive_empty_count % 2 == 0:
                    print(f"[*] Waiting for logs to start/update... (elapsed ~{consecutive_empty_count * 30}s)", flush=True)
                    
            time.sleep(30)
            
        except Exception as e:
            print(f"\n[-] Error in main loop: {e}")
            time.sleep(30)

if __name__ == "__main__":
    main()
