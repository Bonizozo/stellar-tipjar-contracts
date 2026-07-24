#!/usr/bin/env bash
# keeper.sh — Scan for creators approaching TTL expiry and keep entries alive.
#
# Responsibilities:
# 1. Query indexer RPC for recent Tip events
# 2. Identify creators with last activity > EXPIRY_HORIZON days ago
# 3. Batch call extend_entries to bump their TTLs
# 4. Log results and alert on failures
#
# Usage:
#   bash scripts/keeper.sh                    # Run once (for cron)
#   bash scripts/keeper.sh --dry-run          # Show what would be extended
#   bash scripts/keeper.sh --creator GBXXXXXX # Extend specific creator

set -euo pipefail

# Configuration (overridable via env)
KEEPER_RPC_URL="${KEEPER_RPC_URL:-https://soroban-testnet.stellar.org}"
KEEPER_NETWORK="${KEEPER_NETWORK:-testnet}"
KEEPER_BATCH_SIZE="${KEEPER_BATCH_SIZE:-50}"
KEEPER_TTL_THRESHOLD="${KEEPER_TTL_THRESHOLD:-100000}"
KEEPER_EXPIRY_HORIZON="${KEEPER_EXPIRY_HORIZON:-1}"  # days before archival

DRY_RUN=false
SPECIFIC_CREATOR=""

# Parse arguments
while [[ $# -gt 0 ]]; do
  case "$1" in
    --dry-run) DRY_RUN=true; shift ;;
    --creator) SPECIFIC_CREATOR="$2"; shift 2 ;;
    *) echo "Unknown argument: $1"; exit 1 ;;
  esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
LOG_FILE="$REPO_ROOT/keeper.log"

log() {
  echo "[$(date +'%Y-%m-%d %H:%M:%S')] $*" | tee -a "$LOG_FILE"
}

log "=== Keeper Run Started ==="
log "RPC: $KEEPER_RPC_URL"
log "Network: $KEEPER_NETWORK"
log "Batch size: $KEEPER_BATCH_SIZE"
log "Expiry horizon: $KEEPER_EXPIRY_HORIZON days"
log "Dry run: $DRY_RUN"

# Load contract address from config
CONFIG_FILE="$REPO_ROOT/deployment/config.json"
if [[ ! -f "$CONFIG_FILE" ]]; then
  log "ERROR: deployment/config.json not found"
  exit 1
fi

CONTRACT_ID=$(jq -r ".networks[\"$KEEPER_NETWORK\"].active_contract_id" "$CONFIG_FILE")
if [[ -z "$CONTRACT_ID" || "$CONTRACT_ID" == "null" ]]; then
  log "ERROR: No contract ID for network '$KEEPER_NETWORK' in deployment/config.json"
  exit 1
fi

log "Contract: $CONTRACT_ID"

# If specific creator provided, extend immediately
if [[ -n "$SPECIFIC_CREATOR" ]]; then
  log "Extending entries for creator: $SPECIFIC_CREATOR"
  
  if [[ "$DRY_RUN" == "true" ]]; then
    log "[DRY RUN] Would call extend_entries($SPECIFIC_CREATOR, $KEEPER_TTL_THRESHOLD)"
  else
    stellar contract invoke \
      --id "$CONTRACT_ID" \
      --rpc-url "$KEEPER_RPC_URL" \
      --network-passphrase "Test SDF Network ; September 2015" \
      -- extend_entries --creator "$SPECIFIC_CREATOR" --threshold "$KEEPER_TTL_THRESHOLD" \
      || log "ERROR: Failed to extend entries for $SPECIFIC_CREATOR"
  fi
  exit 0
fi

# Query indexer for recent tip events
# This is a placeholder; integrate with actual indexer once deployed
# For now, log the query that would be made
log "Querying indexer for creators approaching expiry..."

# Placeholder: would query indexer DB like:
# SELECT creator, MAX(timestamp) as last_tip
# FROM tip_events
# WHERE timestamp < NOW() - (KEEPER_EXPIRY_HORIZON || ' days')::interval
# GROUP BY creator
# LIMIT 1000

# For this PoC, read from recent events via contract querying
log "Note: Indexer integration required for production keeper."
log "For now, creator must call extend_entries directly or be registered manually."

# Example: If you have a list of creators to monitor
# CREATORS=($(jq -r '.creators[]' scripts/keeper-config.json 2>/dev/null || echo ""))

# if [[ ${#CREATORS[@]} -eq 0 ]]; then
#   log "No creators to extend; use --creator to extend a specific creator"
#   log "Or populate scripts/keeper-config.json with creator addresses"
#   exit 0
# fi

# Batch and extend
# BATCH=()
# for creator in "${CREATORS[@]}"; do
#   BATCH+=("$creator")
#   if [[ ${#BATCH[@]} -ge $KEEPER_BATCH_SIZE ]]; then
#     log "Extending batch of ${#BATCH[@]} creators..."
#     # Call extend_entries_batch (once implemented)
#     BATCH=()
#   fi
# done

# if [[ ${#BATCH[@]} -gt 0 ]]; then
#   log "Extending final batch of ${#BATCH[@]} creators..."
#   # Call extend_entries_batch
# fi

log "=== Keeper Run Completed ==="
