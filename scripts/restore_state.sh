#!/usr/bin/env bash
# restore_state.sh — Restore TipJar contract state from a JSON backup.
# Usage: ./scripts/restore_state.sh <CONTRACT_ID> <NETWORK> <BACKUP_FILE>
set -euo pipefail
CONTRACT_ID="${1:?Usage: $0 <CONTRACT_ID> <NETWORK> <BACKUP_FILE>}"
NETWORK="${2:?Usage: $0 <CONTRACT_ID> <NETWORK> <BACKUP_FILE>}"
BACKUP_FILE="${3:?Usage: $0 <CONTRACT_ID> <NETWORK> <BACKUP_FILE>}"
[[ -f "$BACKUP_FILE" ]] || { echo "ERROR: $BACKUP_FILE not found" >&2; exit 1; }
if [[ -f "${BACKUP_FILE}.sha256" ]]; then
  sha256sum --check "${BACKUP_FILE}.sha256" || { echo "ERROR: checksum mismatch" >&2; exit 1; }
  echo "Checksum verified."
fi
echo "Restoring from $BACKUP_FILE to $CONTRACT_ID on $NETWORK..."
stellar contract invoke --id "$CONTRACT_ID" --network "$NETWORK" \
  -- restore_state --data "$(cat "$BACKUP_FILE")" \
  && echo "Restore complete." || { echo "ERROR: restore failed" >&2; exit 1; }
