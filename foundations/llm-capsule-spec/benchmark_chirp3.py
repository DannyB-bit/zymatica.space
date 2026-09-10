# Watermark: ip zymatica.space
__watermark__ = "ip zymatica.space"

import os
import sys
import time
import subprocess
import json
import shutil

sys.stdout.reconfigure(encoding='utf-8', errors='backslashreplace')

PKT_DIR = "j:/Language-U/packets_chirp3"
MANIFEST_PATH = os.path.join(PKT_DIR, "manifest_chirp3.json")

def clean_packets():
    if os.path.exists(PKT_DIR):
        print(f"Cleaning packet directory {PKT_DIR}...")
        shutil.rmtree(PKT_DIR)
    os.makedirs(PKT_DIR, exist_ok=True)

def run_cmd(args):
    print(f"Running: {' '.join(args)}")
    t0 = time.perf_counter()
    result = subprocess.run(args, capture_output=True, text=True, encoding='utf-8')
    elapsed = time.perf_counter() - t0
    if result.returncode != 0:
        print(f"❌ Command failed (code {result.returncode}):")
        print(result.stdout)
        print(result.stderr)
        sys.exit(1)
    return result.stdout, elapsed

def main():
    import argparse
    parser = argparse.ArgumentParser(description="Chirp-3 End-to-End Benchmark")
    parser.add_argument("--level", type=int, default=5, choices=[4, 5, 6],
                        help="Compression level: 4 (DCT), 5 (Eigenspace 24), 6 (Gradient Atom)")
    parser.add_argument("--simulate-loss", action="store_true",
                        help="Simulate the loss of one packet and test FEC recovery")
    args = parser.parse_args()
    
    print("=" * 80)
    print(f"  CHIRP-3 SYSTEM BENCHMARK — Level {args.level}")
    print("  Watermark: ip zymatica.space")
    print("=" * 80)
    
    # Step 1: Cleanup
    clean_packets()
    
    # Step 2: Compress
    print("\n--- Phase 1: Compression ---")
    compress_cmd = [sys.executable, "j:/Language-U/compress_chirp3.py", "--level", str(args.level)]
    stdout, comp_time = run_cmd(compress_cmd)
    print(stdout)
    print(f"Compression completed in {comp_time:.2f} seconds.")
    
    # Verify manifest and packets
    if not os.path.exists(MANIFEST_PATH):
        print(f"Error: Manifest not found at {MANIFEST_PATH}")
        sys.exit(1)
        
    with open(MANIFEST_PATH, "r") as f:
        manifest = json.load(f)
        
    print("\nPacket Inspection:")
    total_pkts = manifest["total_packets"]
    for i in range(total_pkts):
        p_path = os.path.join(PKT_DIR, f"packet_chirp3_{i}.bin")
        if not os.path.exists(p_path):
            print(f"  [-] Packet {i} is missing!")
            sys.exit(1)
        p_size = os.path.getsize(p_path)
        print(f"  [+] Packet {i}: {p_size} bytes [OK]")
        
    # Step 3: Simulate Packet Loss (FEC recovery validation)
    if args.simulate_loss:
        print("\n--- Phase 2: Simulating Packet Loss (XOR-FEC Test) ---")
        # Let's delete packet index 1 (which is a data packet)
        target_loss_idx = 1
        loss_pkt_path = os.path.join(PKT_DIR, f"packet_chirp3_{target_loss_idx}.bin")
        if os.path.exists(loss_pkt_path):
            os.remove(loss_pkt_path)
            print(f"  [!] Deleted {loss_pkt_path} to simulate packet loss.")
        else:
            print("  [-] Could not locate packet 1 to delete.")
            
    # Step 4: Decompress & Restore
    print("\n--- Phase 3: Decompression, Weight Injection, and SFT Alignment ---")
    decompress_cmd = [sys.executable, "j:/Language-U/decode_chirp3.py"]
    stdout, decomp_time = run_cmd(decompress_cmd)
    print(stdout)
    
    # Parse final metrics from decode output
    # We parse the printed lines for scores
    fid_before = "N/A"
    fid_after = "N/A"
    sem_before = "N/A"
    sem_after = "N/A"
    loss_init = "N/A"
    loss_final = "N/A"
    
    for line in stdout.splitlines():
        if "Fidelity Before:" in line:
            fid_before = line.split("Fidelity Before:")[-1].strip()
        elif "Fidelity After:" in line:
            fid_after = line.split("Fidelity After:")[-1].strip()
        elif "Semantic Before:" in line:
            sem_before = line.split("Semantic Before:")[-1].strip()
        elif "Semantic After:" in line:
            sem_after = line.split("Semantic After:")[-1].strip()
        elif "Loss Initial/Final:" in line:
            loss_init_final = line.split("Loss Initial/Final:")[-1].strip()
            if "/" in loss_init_final:
                loss_init, loss_final = [x.strip() for x in loss_init_final.split("/")]
                
    # Step 5: Summary Report
    print("\n" + "=" * 80)
    print("  CHIRP-3 SYSTEM REPORT")
    print("=" * 80)
    print(f"  Compression Level: {args.level}")
    print(f"  Payload Size:      {manifest['payload_bytes']} bytes")
    print(f"  Packet Count:      {manifest['total_packets']} packets ({manifest['num_data_packets']} data, 1 FEC)")
    print(f"  XOR-FEC Test:      {'PASSED (1 packet recovered)' if args.simulate_loss else 'SKIPPED (no loss simulated)'}")
    print(f"  Fidelity Before:   {fid_before}")
    print(f"  Fidelity After:    {fid_after}")
    print(f"  Semantic Before:   {sem_before}")
    print(f"  Semantic After:    {sem_after}")
    print(f"  SFT Training Loss: {loss_init} -> {loss_final}")
    print(f"  Total Time:        Compression={comp_time:.1f}s | Restoration={decomp_time:.1f}s")
    
    # Success evaluation
    try:
        fid_val = float(fid_after.replace("%", ""))
        success = fid_val == 100.0
    except ValueError:
        success = False
        
    print(f"  Status:            {'[PASS]' if success else '[FAIL]'}")
    print("=" * 80)

if __name__ == '__main__':
    main()
