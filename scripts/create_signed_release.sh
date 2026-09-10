#!/usr/bin/env bash
set -euo pipefail
TAG="${1:?usage: create_signed_release.sh vX.Y.Z [message]}"
MESSAGE="${2:-Zymatica verified release ${TAG}}"
if [[ -n "$(git status --porcelain)" ]]; then
  echo "working tree is not clean" >&2
  exit 1
fi
git tag -s "$TAG" -m "$MESSAGE"
git tag -v "$TAG"
git push origin "$TAG"
echo "PASS: signed release tag pushed: $TAG"
