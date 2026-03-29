#!/usr/bin/env bash
# backup-contract.sh — Export TipJar contract state to a JSON backup.
#
# Usage:
#   ./scripts/backup-contract.sh [OPTIONS]
#
# Options:
#   --contract  CONTRACT_ID   (required) Strkey contract address
#   --network   NETWORK       testnet | mainnet | futurenet  (default: testnet)
#   --output    DIR           Local backup directory          (default: ./backups)
#   --encrypt                 Encrypt the backup with AES-256-GCM
#   --key       HEX_KEY       32-byte hex key (generated if omitted with --encrypt)
#   --incremental             Produce an incremental diff against the latest backup
#   --storage   BACKEND       local | s3 | ipfs               (default: local)
#
# Environment variables:
#   AWS_ACCESS_KEY_ID / AWS_SECRET_ACCESS_KEY / AWS_DEFAULT_REGION  — for S3
#   IPFS_API_URL                                                     — for IPFS
#   BACKUP_ENCRYPT_KEY                                               — key override

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BACKUP_DIR="$ROOT_DIR/backups"
NETWORK="testnet"
CONTRACT_ID=""
ENCRYPT=false
ENCRYPT_KEY="${BACKUP_ENCRYPT_KEY:-}"
INCREMENTAL=false
STORAGE="local"

# ── Argument parsing ──────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
  case "$1" in
    --contract)  CONTRACT_ID="$2"; shift 2 ;;
    --network)   NETWORK="$2";     shift 2 ;;
    --output)    BACKUP_DIR="$2";  shift 2 ;;
    --encrypt)   ENCRYPT=true;     shift   ;;
    --key)       ENCRYPT_KEY="$2"; shift 2 ;;
    --incremental) INCREMENTAL=true; shift ;;
    --storage)   STORAGE="$2";     shift 2 ;;
    *) echo "Unknown option: $1"; exit 1   ;;
  esac
done

if [[ -z "$CONTRACT_ID" ]]; then
  echo "Error: --contract is required"
  exit 1
fi

mkdir -p "$BACKUP_DIR"

TIMESTAMP=$(date -u +"%Y%m%dT%H%M%SZ")
BACKUP_ID="backup_${CONTRACT_ID:0:8}_${TIMESTAMP}"
BACKUP_FILE="$BACKUP_DIR/${BACKUP_ID}.json"

echo "[backup] Contract:    $CONTRACT_ID"
echo "[backup] Network:     $NETWORK"
echo "[backup] Output:      $BACKUP_FILE"
echo "[backup] Encrypt:     $ENCRYPT"
echo "[backup] Incremental: $INCREMENTAL"
echo "[backup] Storage:     $STORAGE"

# ── Build exporter if needed ──────────────────────────────────────────────────
EXPORTER_BIN="$ROOT_DIR/target/release/state-exporter"
if [[ ! -f "$EXPORTER_BIN" ]]; then
  echo "[backup] Building state-exporter..."
  cargo build --release --manifest-path "$ROOT_DIR/tools/backup/Cargo.toml" \
    --bin state-exporter 2>&1
fi

# ── Compose exporter arguments ────────────────────────────────────────────────
ARGS=(
  --contract "$CONTRACT_ID"
  --network  "$NETWORK"
  --output   "$BACKUP_FILE"
)

if $ENCRYPT; then
  ARGS+=(--encrypt)
  if [[ -n "$ENCRYPT_KEY" ]]; then
    ARGS+=(--key "$ENCRYPT_KEY")
  fi
fi

# Find the latest backup for incremental mode.
if $INCREMENTAL; then
  LATEST=$(ls -t "$BACKUP_DIR"/*.json 2>/dev/null | head -1 || true)
  if [[ -n "$LATEST" ]]; then
    BASE_ID=$(basename "$LATEST" .json)
    echo "[backup] Incremental base: $BASE_ID"
    ARGS+=(--incremental --base "$BASE_ID")
  else
    echo "[backup] No previous backup found — falling back to full backup"
  fi
fi

# ── Run exporter ──────────────────────────────────────────────────────────────
echo "[backup] Running state-exporter..."
"$EXPORTER_BIN" "${ARGS[@]}"

# ── Verify checksum ───────────────────────────────────────────────────────────
if [[ -f "$BACKUP_FILE" ]] && ! $ENCRYPT; then
  STORED_CHECKSUM=$(python3 -c "
import json, sys
data = json.load(open('$BACKUP_FILE'))
print(data.get('metadata', {}).get('checksum', ''))
" 2>/dev/null || true)

  if [[ -n "$STORED_CHECKSUM" ]]; then
    COMPUTED=$(python3 -c "
import hashlib, json
data = json.load(open('$BACKUP_FILE'))
data['metadata']['checksum'] = ''
payload = json.dumps(data, indent=2)
print(hashlib.sha256(payload.encode()).hexdigest())
" 2>/dev/null || true)

    if [[ "$STORED_CHECKSUM" == "$COMPUTED" ]]; then
      echo "[backup] Checksum verified: $STORED_CHECKSUM"
    else
      echo "[backup] WARNING: Checksum mismatch!"
      echo "[backup]   stored:   $STORED_CHECKSUM"
      echo "[backup]   computed: $COMPUTED"
    fi
  fi
fi

# ── Upload to remote storage ──────────────────────────────────────────────────
if [[ "$STORAGE" == "s3" ]]; then
  S3_BUCKET="${S3_BUCKET:-tipjar-backups}"
  S3_KEY="backups/${BACKUP_ID}.json"
  echo "[backup] Uploading to s3://$S3_BUCKET/$S3_KEY ..."
  aws s3 cp "$BACKUP_FILE" "s3://$S3_BUCKET/$S3_KEY"
  echo "[backup] S3 upload complete"

elif [[ "$STORAGE" == "ipfs" ]]; then
  IPFS_API="${IPFS_API_URL:-http://localhost:5001}"
  echo "[backup] Uploading to IPFS via $IPFS_API ..."
  CID=$(curl -s -X POST "$IPFS_API/api/v0/add" \
    -F "file=@$BACKUP_FILE" | python3 -c "import sys,json; print(json.load(sys.stdin)['Hash'])")
  echo "[backup] IPFS CID: $CID"
  echo "$CID" > "$BACKUP_DIR/${BACKUP_ID}.cid"
fi

# ── Record backup in manifest ─────────────────────────────────────────────────
MANIFEST="$BACKUP_DIR/manifest.json"
if [[ ! -f "$MANIFEST" ]]; then
  echo "[]" > "$MANIFEST"
fi

python3 - <<EOF
import json, os
manifest_path = "$MANIFEST"
with open(manifest_path) as f:
    manifest = json.load(f)
manifest.append({
    "backup_id": "$BACKUP_ID",
    "file": "$BACKUP_FILE",
    "contract": "$CONTRACT_ID",
    "network": "$NETWORK",
    "timestamp": "$TIMESTAMP",
    "encrypted": $( $ENCRYPT && echo "true" || echo "false" ),
    "storage": "$STORAGE"
})
with open(manifest_path, "w") as f:
    json.dump(manifest, f, indent=2)
print(f"[backup] Manifest updated: {manifest_path}")
EOF

echo "[backup] Done. Backup ID: $BACKUP_ID"
