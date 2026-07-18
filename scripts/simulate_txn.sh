#!/usr/bin/env bash
set -euo pipefail

# Stellar Contract Transaction Simulator & Debugger (Issue #75)
# This tool helps debug contract interactions before submitting them to the network.

NETWORK="${1:-testnet}"
CONTRACT_ID="${2:-}"
FUNCTION="${3:-}"
shift 3 || true
ARGS="$@"

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [ -z "$CONTRACT_ID" ] || [ -z "$FUNCTION" ]; then
  echo "Usage: $0 [network] [contract_id] [function_name] [args...]"
  echo "Example: $0 testnet C... get_total_tips --creator G..."
  exit 1
fi

echo "--- [Simulator] Initializing Simulation for Function: $FUNCTION ---"
echo "--- [Simulator] Target Contract: $CONTRACT_ID ---"
echo "--- [Simulator] Target Network: $NETWORK ---"

# 1. Run standard simulation via Stellar CLI
# We use detailed output to capture gas and state footprints.
echo "[Step 1] Running Stellar contract simulation..."
SIM_OUTPUT=$(stellar contract invoke \
  --id "$CONTRACT_ID" \
  --network "$NETWORK" \
  --source alice \
  --simulate \
  -- "$FUNCTION" $ARGS 2>&1 || true)

# 2. Extract Gas Estimation
echo "[Step 2] Gas Estimation & Resources:"
echo "$SIM_OUTPUT" | grep -iE "gas|resource|budget" || echo "No explicit gas data in output."

# 3. Execution Trace / Error Debugging
echo "[Step 3] Execution Trace & Debugging Output:"
if echo "$SIM_OUTPUT" | grep -iq "error"; then
  echo "!!! Error Detected during simulation !!!"
  echo "$SIM_OUTPUT" | sed -n '/error/,$p'
else
  echo "Simulation successful. Detailed Trace Summary:"
  echo "$SIM_OUTPUT" | head -n 20
fi

# 4. State Change Visualization
# We check the footprint/ledger changes in the simulation output.
echo "[Step 4] State Change & Footprint Visualization:"
echo "$SIM_OUTPUT" | grep -iE "footprint|ledger|state" || echo "No state changes detected in this simulation."

# 5. Interactive Replay Recommendation
echo "[Step 5] Transaction Replay Tooling:"
echo "To replay this simulation with full Soroban host logs, use:"
echo "stellar contract invoke --id $CONTRACT_ID --network $NETWORK --source alice -- $FUNCTION $ARGS --log-level debug"

echo "--- [Simulator] Process Completed ---"
