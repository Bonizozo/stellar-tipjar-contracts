#!/usr/bin/env bash
# restore-contract.sh — Restore TipJar contract state from a backup.
#
# Usage:
#   ./scripts/restore-contract.sh [OPTIONS]
#
# Options:
#   --backup    FILE_OR_URI   (required) Path or s3:// / ipfs:// URI
#   --contract  CONTRACT_ID   (required) Target contract address
#   --network   NETWORK       testnet | mainnet | futurenet  (default: testnet)
#   --source    ACCOUNT       Stellar account for signing     (default: alice)
#   --decrypt                 Decrypt the backup before importing
#   --key       HEX_KEY       32-byte hex decryption key
#   --dry-run                 Print what would be restored without submitting
#   --verify-only             Only verify backup integrity, do not restore
#
# Environment variables:
#   BACKUP_ENCRYPT_KEY   — decryption key override

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BACKUP=""
CONTRACT_ID=""
NETWORK="testnet"
SOURCE_ACCOUNT="alice"
DECRYPT=false
DECRYPT_KEY="${BACKUP_ENCRYPT_KEY:-}"
DRY_RUN=false
VERIFY_ONLY=false

# ── Argument parsing ──────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    --backup)       BACKUP="$2";          shift 2 ;;
    --contract)     CONTRACT_ID="$2";     shift 2 ;;
    --network)      NETWORK="$2";         shift 2 ;;
    --source)       SOURCE_ACCOUNT="$2";  shift 2 ;;
    --decrypt)      DECRYPT=true;         shift   ;;
    --key)          DECRYPT_KEY="$2";     shift 2 ;;
    --dry-run)      DRY_RUN=true;         shift   ;;
    --verify-only)  VERIFY_ONLY=true;     shift   ;;
    *) echo "Unknown option: $1"; exit 1           ;;
  esac
done

if [[ -z "$BACKUP" ]]; then
  echo "Error: --backup is required"
  exit 1
fi

if [[ -z "$CONTRACT_ID" ]] && ! $VERIFY_ONLY; then
  echo "Error: --contract is required unless --verify-only"
  exit 1
fi

echo "[restore] Backup:      $BACKUP"
echo "[restore] Contract:    $CONTRACT_ID"
echo "[restore] Network:     $NETWORK"
echo "[restore] Source:      $SOURCE_ACCOUNT"
echo "[restore] Decrypt:     $DECRYPT"
echo "[restore] Dry run:     $DRY_RUN"
echo "[restore] Verify only: $VERIFY_ONLY"

# ── Build importer if needed ──────────────────────────────────────────────────
IMPORTER_BIN="$ROOT_DIR/target/release/state-importer"
if [[ ! -f "$IMPORTER_BIN" ]]; then
  echo "[restore] Building state-importer..."
  cargo build --release --manifest-path "$ROOT_DIR/tools/backup/Cargo.toml" \
    --bin state-importer 2>&1
fi

# ── Verify backup integrity before restoring ──────────────────────────────────
if [[ -f "$BACKUP" ]] && ! $DECRYPT; then
  echo "[restore] Verifying backup integrity..."
  STORED=$(python3 -c "
import json
data = json.load(open('$BACKUP'))
print(data.get('metadata', {}).get('checksum', ''))
" 2>/dev/null || true)

  if [[ -n "$STORED" ]]; then
    COMPUTED=$(python3 -c "
import hashlib, json
data = json.load(open('$BACKUP'))
data['metadata']['checksum'] = ''
payload = json.dumps(data, indent=2)
print(hashlib.sha256(payload.encode()).hexdigest())
" 2>/dev/null || true)

    if [[ "$STORED" != "$COMPUTED" ]]; then
      echo "[restore] ERROR: Checksum mismatch — backup may be corrupted!"
      echo "[restore]   stored:   $STORED"
      echo "[restore]   computed: $COMPUTED"
      exit 1
    fi
    echo "[restore] Checksum OK: $STORED"
  fi
fi

# ── Compose importer arguments ────────────────────────────────────────────────
ARGS=(
  --backup   "$BACKUP"
  --contract "$CONTRACT_ID"
  --network  "$NETWORK"
  --source   "$SOURCE_ACCOUNT"
)

if $DECRYPT; then
  ARGS+=(--decrypt)
  if [[ -n "$DECRYPT_KEY" ]]; then
    ARGS+=(--key "$DECRYPT_KEY")
  fi
fi

if $DRY_RUN;     then ARGS+=(--dry-run);     fi
if $VERIFY_ONLY; then ARGS+=(--verify-only); fi

# ── Run importer ──────────────────────────────────────────────────────────────
echo "[restore] Running state-importer..."
"$IMPORTER_BIN" "${ARGS[@]}"

echo "[restore] Done."
