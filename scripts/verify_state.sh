#!/usr/bin/env bash
# verify_state.sh — Verify a backup file's checksum integrity.
# Usage: ./scripts/verify_state.sh <BACKUP_FILE>
set -euo pipefail
BACKUP_FILE="${1:?Usage: $0 <BACKUP_FILE>}"
[[ -f "$BACKUP_FILE" ]] || { echo "ERROR: $BACKUP_FILE not found" >&2; exit 1; }
CHECKSUM_FILE="${BACKUP_FILE}.sha256"
[[ -f "$CHECKSUM_FILE" ]] || { echo "ERROR: checksum file not found: $CHECKSUM_FILE" >&2; exit 1; }
sha256sum --check "$CHECKSUM_FILE" && echo "Verification passed." || { echo "FAILED: checksum mismatch" >&2; exit 1; }
