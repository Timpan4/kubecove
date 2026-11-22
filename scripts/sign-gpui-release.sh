#!/usr/bin/env bash
# The existing secret wraps a minisign key file in base64. Keep the decoded key
# outside the artifact directory and pass its password on stdin.
set -euo pipefail
dir=$1
version=$2
shift 2
prefix="$(node scripts/desktop-release-channel.mjs "$version" prefix "$@")"
umask 077
scratch="$(mktemp -d)"
trap 'rm -rf -- "$scratch"' EXIT
printf '%s' "$TAURI_SIGNING_PRIVATE_KEY" | base64 --decode > "$scratch/key"
for suffix in x86_64.AppImage aarch64.app.tar.gz x64-setup.exe; do
  name="${prefix}_${version}_${suffix}"
  comment="$(printf 'timestamp:%s\tfile:%s\tversion:%s' "$(date +%s)" "$name" "$version")"
  printf '%s\n' "${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}" |
    minisign -S -s "$scratch/key" -m "$dir/$name" -t "$comment" -x "$scratch/signature"
  base64 --wrap=0 "$scratch/signature" > "$dir/$name.sig"
done
