import sys
import os

# Append current dir so we can import
sys.path.append("j:/Language-U")

from run_dna_grow_voice import DnaGrowVoiceSystem

def test_ehss():
    print("\n[START] Testing EHSS Cognitive Healing...")
    system = DnaGrowVoiceSystem()
    
    test_queries = [
        "What do you know about Genesis Engine?",
        "What do you know about Synapse Capsule?",
    ]
    
    for q in test_queries:
        print(f"\nPROMPT: {q}")
        response = system.generate_brain_response(q)
        print(f"RESPONSE: {response}")
        
    print("\n[SUCCESS] EHSS Steering Test Complete.")

if __name__ == "__main__":
    test_ehss()
