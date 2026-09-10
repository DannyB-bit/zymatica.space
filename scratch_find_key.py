import os, json
from pathlib import Path

home = Path(os.environ.get('USERPROFILE', 'C:/Users/freed'))

search_roots = [
    home / '.config',
    home / '.solana',
    home / 'Downloads',
    home / 'Desktop',
    home / 'Documents',
    Path('C:/200amsterdam-Book'),
]

skip_dirs = {'node_modules', '.git', 'target', 'AppData', '.cache', '.rustup', '.cargo', '.vscode', '.idea'}

found_keypairs = []

for root in search_roots:
    if not root.exists():
        continue
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in skip_dirs]
        for fname in filenames:
            if fname.endswith('.json') or 'keypair' in fname.lower() or 'id' in fname.lower() or 'wallet' in fname.lower():
                full_p = Path(dirpath) / fname
                try:
                    if full_p.stat().st_size < 10000:
                        data = json.loads(full_p.read_text(encoding='utf-8', errors='ignore'))
                        if isinstance(data, list) and len(data) == 64 and all(isinstance(x, int) and 0 <= x <= 255 for x in data):
                            found_keypairs.append(str(full_p))
                except Exception:
                    pass

print(f"Total Solana Keypairs Found: {len(found_keypairs)}")
for k in found_keypairs:
    print(f"Found Keypair File: {k}")
