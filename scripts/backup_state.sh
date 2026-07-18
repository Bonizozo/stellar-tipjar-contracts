#!/usr/bin/env bash
# backup_state.sh — Export TipJar contract state to JSON and upload to S3.
# Usage: ./scripts/backup_state.sh <CONTRACT_ID> <NETWORK> [s3://bucket/path]
set -euo pipefail
CONTRACT_ID="${1:?Usage: $0 <CONTRACT_ID> <NETWORK> [s3://bucket]}"
NETWORK="${2:?Usage: $0 <CONTRACT_ID> <NETWORK> [s3://bucket]}"
S3_BUCKET="${3:-}"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)
BACKUP_DIR="backups"
BACKUP_FILE="${BACKUP_DIR}/state_${NETWORK}_${TIMESTAMP}.json"
mkdir -p "$BACKUP_DIR"
echo "Backing up contract state..."
echo "Contract: $CONTRACT_ID | Network: $NETWORK"
stellar contract invoke --id "$CONTRACT_ID" --network "$NETWORK" -- get_state 2>/dev/null \
  > "$BACKUP_FILE" || { echo "ERROR: export failed" >&2; exit 1; }
sha256sum "$BACKUP_FILE" > "${BACKUP_FILE}.sha256"
echo "Backup: $BACKUP_FILE"
echo "Checksum: ${BACKUP_FILE}.sha256"
if [[ -n "$S3_BUCKET" ]]; then
  aws s3 cp "$BACKUP_FILE" "${S3_BUCKET}/${NETWORK}/"
  aws s3 cp "${BACKUP_FILE}.sha256" "${S3_BUCKET}/${NETWORK}/"
  echo "Uploaded to $S3_BUCKET"
fi
echo "Backup complete."
