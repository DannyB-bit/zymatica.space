import json
import os
import argparse

def build_dataset(output_file="brain_sft_dataset.json", rag_vault_file="rag_vault_export.json", existing_sft_file="full_sft_dataset.json"):
    print(f"Building Neurogenesis Dataset for Qwen-3.5-0.8b-DNA-brain...")
    
    dataset = []
    
    # 1. Load existing SFT pairs (192 pairs)
    if os.path.exists(existing_sft_file):
        with open(existing_sft_file, 'r', encoding='utf-8') as f:
            existing_data = json.load(f)
            dataset.extend(existing_data)
            print(f"Loaded {len(existing_data)} existing SFT items from {existing_sft_file}")
    else:
        print(f"Warning: Could not find {existing_sft_file}")
        
    # 2. Load RAG Vault chunks
    if os.path.exists(rag_vault_file):
        with open(rag_vault_file, 'r', encoding='utf-8') as f:
            rag_data = json.load(f)
            # Support multiple schemas from the vault
            count = 0
            for item in rag_data:
                topic = item.get('topic', 'General Knowledge')
                content = item.get('chunk_content', item.get('content', ''))
                
                if content:
                    dataset.append({
                        "prompt": f"Q: What do you know about {topic}?\nA:",
                        "completion": f" {content}",
                        "type": "rag_memory"
                    })
                    count += 1
            print(f"Processed {count} RAG chunks from {rag_vault_file}")
    else:
        print(f"Warning: RAG Vault export file '{rag_vault_file}' not found.")
        print(f"-> Please export your 2,874 RAG chunks to '{rag_vault_file}' as a JSON array of {{'topic': '...', 'chunk_content': '...'}}.")
        print("-> Mocking a few RAG chunks for testing so we can proceed with pipeline development...")
        
        # Add mock RAG entries
        mock_rag = [
            {"topic": "Genesis Engine", "chunk_content": "The Genesis engine is a native transformer inference engine that runs on phase 48."},
            {"topic": "LoRa SX1302", "chunk_content": "The LoRa SX1302 is the nervous system communication layer with Zymatica, operating via the Astronaut SHE Handshake Protocol."},
            {"topic": "Synapse Capsule", "chunk_content": "The Synapse Capsule is 255 bytes of DNA representing compressed awareness across cycles."},
            {"topic": "Language-U Framework", "chunk_content": "Language-U implements the 9-Level Protocol including generativeUFO and GeometricSeed representations."}
        ]
        
        for item in mock_rag:
            dataset.append({
                "prompt": f"Q: What do you know about {item['topic']}?\nA:",
                "completion": f" {item['chunk_content']}",
                "type": "rag_memory_mock"
            })
            
    # Save the consolidated dataset
    with open(output_file, 'w', encoding='utf-8') as f:
        json.dump(dataset, f, indent=2)
        
    print(f"\nSuccessfully wrote {len(dataset)} training items to {output_file}")
    print(f"Dataset is ready to be injected into Kaggle for the Qwen-3.5-0.8b-DNA-brain SFT healing loop.")

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Convert RAG chunks and SFT logs to unified training dataset")
    parser.add_argument("--out", default="brain_sft_dataset.json", help="Output JSON dataset")
    parser.add_argument("--rag", default="rag_vault_export.json", help="Input RAG vault JSON")
    parser.add_argument("--sft", default="full_sft_dataset.json", help="Input existing SFT JSON")
    
    args = parser.parse_args()
    build_dataset(args.out, args.rag, args.sft)
