#!/usr/bin/env bash
# One-time setup for releases. Run it yourself, on your own Mac, from the
# repository folder:   bash scripts/setup-release-keys.sh
#
# It creates two keys, keeps backups in ~/.tauri/ and stores them as GitHub
# Actions secrets of this repository (with the GitHub CLI, `gh`):
#
# 1. Updater key: signs every update, so SlideRelay only installs updates
#    that really come from this project. Its public half goes into
#    companion/src-tauri/tauri.release.conf.json (commit that change).
# 2. macOS signing certificate (self-made): gives every Mac release the
#    same identity, so Screen Recording permission survives updates.
#    It is NOT an Apple Developer ID; Gatekeeper still asks on first open.
#
# BACK UP ~/.tauri/sliderelay-* somewhere safe (e.g. a password manager).
# If the updater key is lost, existing installs can't auto-update any more.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
KEYS="$HOME/.tauri"
mkdir -p "$KEYS"
chmod 700 "$KEYS"
umask 077
command -v gh >/dev/null || { echo "Install the GitHub CLI first: https://cli.github.com"; exit 1; }
gh auth status >/dev/null || { echo "Sign in first: gh auth login"; exit 1; }

# ---- 1. Updater key ----
UPD_KEY="$KEYS/sliderelay-updater.key"
UPD_PASS="$KEYS/sliderelay-updater.password"
if [ -f "$UPD_KEY" ]; then
  echo "Updater key already exists in $KEYS (keeping it)."
else
  openssl rand -base64 32 | tr -d '\n' > "$UPD_PASS"
  (cd "$REPO_ROOT/companion" && npx tauri signer generate --ci -w "$UPD_KEY" -p "$(cat "$UPD_PASS")")
fi
gh secret set TAURI_SIGNING_PRIVATE_KEY < "$UPD_KEY"
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD < "$UPD_PASS"
node -e '
  const fs = require("fs");
  const file = process.argv[1];
  const conf = JSON.parse(fs.readFileSync(file, "utf8"));
  conf.plugins.updater.pubkey = fs.readFileSync(process.argv[2], "utf8").trim();
  fs.writeFileSync(file, JSON.stringify(conf, null, 2) + "\n");
' "$REPO_ROOT/companion/src-tauri/tauri.release.conf.json" "$UPD_KEY.pub"
echo "Updater public key written to companion/src-tauri/tauri.release.conf.json"

# ---- 2. macOS signing certificate (self-made) ----
NAME="SlideRelay Open Source"
CERT_DIR="$KEYS/sliderelay-macos"
if [ -f "$CERT_DIR/cert.p12" ]; then
  echo "macOS certificate already exists in $CERT_DIR (keeping it)."
else
  mkdir -p "$CERT_DIR"
  openssl rand -base64 24 | tr -d '\n' > "$CERT_DIR/p12.password"
  /usr/bin/openssl req -x509 -newkey rsa:2048 -sha256 -days 3650 -nodes \
    -keyout "$CERT_DIR/key.pem" -out "$CERT_DIR/cert.pem" -subj "/CN=$NAME" \
    -extensions v3 -config <(printf '[req]\ndistinguished_name=dn\n[dn]\n[v3]\nbasicConstraints=critical,CA:false\nkeyUsage=critical,digitalSignature\nextendedKeyUsage=critical,codeSigning\n')
  /usr/bin/openssl pkcs12 -export -inkey "$CERT_DIR/key.pem" -in "$CERT_DIR/cert.pem" \
    -name "$NAME" -out "$CERT_DIR/cert.p12" -passout "pass:$(cat "$CERT_DIR/p12.password")"
fi
base64 -i "$CERT_DIR/cert.p12" | tr -d '\n' | gh secret set MACOS_CERTIFICATE
gh secret set MACOS_CERTIFICATE_PASSWORD < "$CERT_DIR/p12.password"
printf '%s' "$NAME" | gh secret set MACOS_SIGNING_IDENTITY

echo
echo "Done. Secrets stored on GitHub:"
gh secret list
echo
echo "Next: commit companion/src-tauri/tauri.release.conf.json and back up $KEYS/sliderelay-*"
