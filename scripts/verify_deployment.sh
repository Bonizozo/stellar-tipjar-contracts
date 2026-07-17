#!/usr/bin/env bash
set -e

CONTRACT_ID="${1:?Usage: verify_deployment.sh <contract_id> <network>}"
NETWORK="${2:?Usage: verify_deployment.sh <contract_id> <network>}"

echo "[verify] Checking contract $CONTRACT_ID on $NETWORK..."

# Invoke a read-only function to confirm the contract is live and responsive.
# get_total_tips requires a creator address; use a zero-address as a probe.
PROBE_ADDRESS="GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN"

stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  --source "$DEPLOYER_SECRET" \
  -- get_total_tips \
  --creator "$PROBE_ADDRESS" \
  > /dev/null

echo "[verify] Contract $CONTRACT_ID is live on $NETWORK."
