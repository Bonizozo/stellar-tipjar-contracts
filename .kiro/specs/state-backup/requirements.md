# Requirements: Contract State Backup and Recovery

## Overview
Provide automated backup and recovery capabilities for TipJar contract state to prevent data loss and enable disaster recovery.

## Requirements

### 1. Automated State Snapshots
- The system must support on-demand and scheduled snapshots of the full contract state
- Each snapshot must capture all relevant contract storage entries at a point in time
- Snapshots must be timestamped for identification and ordering

### 2. JSON Export/Import
- State must be exportable to a human-readable JSON format
- Exported JSON must be importable via a `restore_state` contract invocation
- The JSON schema must be versioned to support forward compatibility

### 3. Incremental Backups
- The system should support incremental backups that capture only state changes since the last snapshot
- Incremental backups must reference a parent snapshot for chain-of-custody tracking

### 4. SHA-256 Checksums
- Every backup file must have a corresponding `.sha256` checksum file generated at creation time
- Restore and verify scripts must validate the checksum before proceeding
- Checksum mismatch must abort the operation with a non-zero exit code

### 5. Backup Scheduling
- Backups must be schedulable via cron or equivalent task scheduler
- The backup script must be idempotent and safe to run concurrently
- Scheduling documentation must be provided for daily, weekly, and on-deploy triggers

### 6. S3 / IPFS Storage
- Backups must optionally upload to an S3-compatible bucket when a bucket path is provided
- IPFS upload support should be considered for decentralized, tamper-evident archival
- Remote upload must not block or fail the local backup on network error

### 7. Restore Testing
- A restore dry-run mode must be supported to validate backup integrity without modifying contract state
- Restore operations must always be tested on testnet before executing on mainnet
- The restore script must log all actions and exit with a clear success or failure message
