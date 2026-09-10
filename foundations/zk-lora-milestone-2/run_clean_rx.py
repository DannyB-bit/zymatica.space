#!/usr/bin/env python3
import subprocess
import time
import sys

def main():
    print("📻 Starting ZK-LoRa Receiver (RX) Node...")
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
    
    # 1. Select option 2 (Listen)
    p.stdin.write("2\n")
    p.stdin.flush()
    time.sleep(1)
    
    # 2. Enter duration (15 seconds)
    p.stdin.write("15\n")
    p.stdin.flush()
    
    # Wait for listening session to complete
    time.sleep(18)
    
    # 3. Press Enter to continue
    p.stdin.write("\n")
    p.stdin.flush()
    time.sleep(1)
    
    # 4. Exit menu (0)
    p.stdin.write("0\n")
    p.stdin.flush()
    
    stdout, stderr = p.communicate()
    print(stdout)
    if stderr:
        print("Errors:", stderr, file=sys.stderr)

if __name__ == "__main__":
    main()
