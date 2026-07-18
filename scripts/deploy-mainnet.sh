#!/usr/bin/env bash
# deploy-mainnet.sh — alias wrapper for deploy_mainnet.sh
# Allows both `bash scripts/deploy-mainnet.sh` and `bash scripts/deploy_mainnet.sh`.
set -e
exec "$(dirname "$0")/deploy_mainnet.sh" "$@"
