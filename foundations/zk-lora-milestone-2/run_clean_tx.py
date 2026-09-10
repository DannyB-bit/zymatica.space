#!/usr/bin/env python3
import subprocess
import time
import sys

def main():
    print("🚀 Starting ZK-LoRa Transmitter (TX) Node...")
    # Start the operator binary
    p = subprocess.Popen(
        ["cargo", "run", "--release"],
        cwd="Full_Projects/rust",
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        bufsize=1
    )
    
    # 1. Select option 1 (Transmit)
    p.stdin.write("1\n")
    p.stdin.flush()
    time.sleep(1)
    
    # 2. Enter custom message
    p.stdin.write("Hello from RAK-Miner-A - Synchronized Real Test\n")
    p.stdin.flush()
    time.sleep(1)
    
    # 3. Enter packet count (1)
    p.stdin.write("1\n")
    p.stdin.flush()
    
    # Wait for transmission to complete
    time.sleep(5)
    
    # 4. Press Enter to continue
    p.stdin.write("\n")
    p.stdin.flush()
    time.sleep(1)
    
    # 5. Exit menu (0)
    p.stdin.write("0\n")
    p.stdin.flush()
    
    stdout, stderr = p.communicate()
    print(stdout)
    if stderr:
        print("Errors:", stderr, file=sys.stderr)

if __name__ == "__main__":
    main()
