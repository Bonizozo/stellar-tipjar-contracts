# TipJar Contract State Backup and Recovery

## Overview
Automated backup and recovery for TipJar contract state using shell scripts and optional S3 storage.

## Backup
```bash
./scripts/backup_state.sh <CONTRACT_ID> testnet s3://my-bucket/backups
```
Creates `backups/state_testnet_YYYYMMDD_HHMMSS.json` with a SHA-256 checksum file.

## Restore
```bash
./scripts/restore_state.sh <CONTRACT_ID> testnet backups/state_testnet_20250101_120000.json
```
Verifies checksum before restoring.

## Verify
```bash
./scripts/verify_state.sh backups/state_testnet_20250101_120000.json
```

## Scheduling
Add to cron for daily backups:
```
0 2 * * * /path/to/scripts/backup_state.sh $CONTRACT_ID testnet s3://bucket
```

## S3 Storage
Set `AWS_PROFILE` or `AWS_ACCESS_KEY_ID`/`AWS_SECRET_ACCESS_KEY` before running.

## Notes
- Backups are plain JSON — encrypt with `gpg` before uploading to shared storage
- Always test restore on testnet before using on mainnet
- Keep at least 7 daily backups
