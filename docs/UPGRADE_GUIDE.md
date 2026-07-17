# TipJar Contract Upgrade Guide

This guide covers the end-to-end procedure for upgrading a deployed TipJar contract, including version tracking, migration patterns, rollback, and access control requirements.

---

## Prerequisites

- [`stellar` CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli) installed and configured.
- Admin key available locally (`stellar keys ls`).
- The new contract WASM compiled: `cargo build --target wasm32v1-none --release`.

---

## Upgrade Procedure

### Step 1 — Build the new WASM

```bash
cargo build --target wasm32v1-none --release \
  --manifest-path contracts/tipjar/Cargo.toml
```

The artifact is at `target/wasm32v1-none/release/tipjar.wasm`.

### Step 2 — Upload the WASM to the network

```bash
stellar contract upload \
  --wasm target/wasm32v1-none/release/tipjar.wasm \
  --source <admin-key> \
  --network testnet
# Output: <new_wasm_hash>  ← save this value
```

### Step 3 — Run the upgrade script

```bash
./scripts/upgrade_contract.sh <CONTRACT_ID> testnet <new_wasm_hash>
```

The script will:
1. Print the current version before upgrading (save it for rollback reference).
2. Invoke `upgrade` on-chain, signed by the admin key.
3. Print the new version on success, or exit non-zero on failure.

### Step 4 — Verify

```bash
stellar contract invoke \
  --id <CONTRACT_ID> \
  --network testnet \
  -- version
# Expected: previous_version + 1
```

---

## Version Tracking

The contract stores a `u32` version in instance storage under `DataKey::ContractVersion`.

- Default value before any upgrade: `1`.
- Incremented by `1` on every successful `upgrade` call.
- Readable at any time via the `version()` contract function.

---

## Migration Patterns

Soroban preserves all storage across a WASM swap. For additive changes (new keys, new functions) no migration is needed.

For breaking storage changes, include a one-shot `migrate` function in the new WASM:

```rust
pub fn migrate(env: Env, admin: Address) {
    admin.require_auth();
    let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
    if admin != stored_admin { panic_with_error!(&env, TipJarError::UpgradeUnauthorized); }

    // Transform old storage layout → new layout
    if let Some(val) = env.storage().persistent().get::<_, OldType>(&DataKey::OldKey) {
        env.storage().persistent().set(&DataKey::NewKey, &transform(val));
        env.storage().persistent().remove(&DataKey::OldKey);
    }
}
```

Call `migrate` once after `upgrade`, then remove it in the next upgrade.

Migration functions should be **idempotent** — safe to call multiple times without corrupting state.

---

## Rollback Considerations

Soroban does not provide a native rollback mechanism. To revert to a previous version:

1. Re-upload the old WASM (if not already on-chain):
   ```bash
   stellar contract upload --wasm path/to/old/tipjar.wasm \
     --source <admin-key> --network testnet
   # Output: <old_wasm_hash>
   ```
2. Run the upgrade script with the old hash:
   ```bash
   ./scripts/upgrade_contract.sh <CONTRACT_ID> testnet <old_wasm_hash>
   ```

The upgrade script prints the current version before invoking, making it easy to record the previous WASM hash for rollback.

> Note: If a migration function was run and storage was transformed, rolling back the WASM alone may leave storage in an incompatible state. Always test migrations on testnet before mainnet.

---

## Access Control Requirements

- Only the address stored as `DataKey::Admin` during `init` may call `upgrade`.
- The `upgrade` function enforces `admin.require_auth()` — the transaction must be signed by the admin key.
- Unauthorized calls fail with `TipJarError::UpgradeUnauthorized` (error code 23).
- To transfer admin rights before an upgrade, use the role management functions.

---

## Backward Compatibility Rules

| Change type | Safe? |
|---|---|
| Add new `DataKey` variant | Yes |
| Add new contract function | Yes |
| Remove existing `DataKey` variant | No — orphans stored data |
| Rename existing `DataKey` variant | No — requires migration |
| Change value type of existing key | No — requires migration |
| Reorder `TipJarError` discriminants | No — breaks client error handling |
