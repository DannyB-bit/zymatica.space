#!/usr/bin/env bash
set -euo pipefail
REPO="${1:-DannyB-bit/zymatica.space}"
BRANCH="${2:-main}"
REQUIRE_SIGNATURES="${REQUIRE_SIGNATURES:-0}"
command -v gh >/dev/null || { echo "gh CLI is required" >&2; exit 1; }
gh auth status

cat <<'JSON' | gh api --method PUT "repos/${REPO}/branches/${BRANCH}/protection" --input -
{
  "required_status_checks": {
    "strict": true,
    "contexts": [
      "Multi-Architecture Native Build & Test (ubuntu-latest)",
      "Multi-Architecture Native Build & Test (macos-latest)",
      "Multi-Architecture Native Build & Test (windows-latest)"
    ]
  },
  "enforce_admins": true,
  "required_pull_request_reviews": null,
  "restrictions": null,
  "required_linear_history": true,
  "allow_force_pushes": false,
  "allow_deletions": false,
  "block_creations": false,
  "required_conversation_resolution": true,
  "lock_branch": false,
  "allow_fork_syncing": true
}
JSON

if [[ "$REQUIRE_SIGNATURES" == "1" ]]; then
  gh api --method POST "repos/${REPO}/branches/${BRANCH}/protection/required_signatures" >/dev/null
fi

echo "PASS: branch protection applied to ${REPO}/${BRANCH}"
