# Contract Upgrade and Migration System — Design

## 1. Soroban Native Upgrade Mechanism

Soroban exposes a single host function for in-place WASM replacement:

```rust
env.deployer().update_current_contract_wasm(new_wasm_hash);
```

- `new_wasm_hash` is a `BytesN<32>` identifying WASM already uploaded to the network.
- The host atomically swaps the bytecode; all storage (instance + persistent) is untouched.
- The call succeeds or the entire transaction reverts — there is no partial state.

---

## 2. Version Tracking in Instance Storage

A new `DataKey::ContractVersion` variant stores a `u32` in instance storage:

```rust
DataKey::ContractVersion  // value: u32, default 1
```

On every successful `upgrade` call the version is incremented:

```rust
let v: u32 = env.storage().instance().get(&DataKey::ContractVersion).unwrap_or(1);
env.storage().instance().set(&DataKey::ContractVersion, &(v + 1));
```

Instance storage is chosen because the version is contract-global metadata, not per-user data.

---

## 3. Admin-Only Upgrade Authorization

The `upgrade` function follows the same authorization pattern used by `add_token`:

```rust
pub fn upgrade(env: Env, admin: Address, new_wasm_hash: soroban_sdk::BytesN<32>) {
    admin.require_auth();
    let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
    if admin != stored_admin {
        panic_with_error!(&env, TipJarError::UpgradeUnauthorized);
    }
    env.deployer().update_current_contract_wasm(new_wasm_hash);
    let v: u32 = env.storage().instance().get(&DataKey::ContractVersion).unwrap_or(1);
    env.storage().instance().set(&DataKey::ContractVersion, &(v + 1));
    env.events().publish((symbol_short!("upgrade"), env.current_contract_address()), v + 1);
}
```

---

## 4. Migration Function Pattern

For upgrades that change storage layout, include a one-shot `migrate` function in the new WASM:

```rust
pub fn migrate(env: Env, admin: Address) {
    admin.require_auth();
    let stored_admin: Address = env.storage().instance().get(&DataKey::Admin).unwrap();
    if admin != stored_admin { panic_with_error!(&env, TipJarError::UpgradeUnauthorized); }

    // Example: rename OldKey → NewKey
    if let Some(val) = env.storage().persistent().get::<_, SomeType>(&DataKey::OldKey) {
        env.storage().persistent().set(&DataKey::NewKey, &val);
        env.storage().persistent().remove(&DataKey::OldKey);
    }
}
```

Migration functions SHOULD be idempotent and removed in a subsequent upgrade once all instances have migrated.

---

## 5. Upgrade Script Design (`scripts/upgrade_contract.sh`)

```
Usage: ./scripts/upgrade_contract.sh <CONTRACT_ID> <NETWORK> <NEW_WASM_HASH>
```

Steps performed by the script:
1. Validate that all three arguments are provided.
2. Resolve `ADMIN_ADDRESS` from the `stellar keys address` command (or environment variable).
3. Invoke `stellar contract invoke` with the `upgrade` function.
4. Print success with the new version, or print the error and exit non-zero.

The script records `NEW_WASM_HASH` before invocation so operators can roll back by re-running with the previous hash.

---

## 6. Backward Compatibility Rules

| Change | Safe? | Notes |
|---|---|---|
| Add new `DataKey` variant | ✅ | Old keys unaffected |
| Add new contract function | ✅ | No selector collision |
| Remove `DataKey` variant | ❌ | Orphans stored data |
| Rename `DataKey` variant | ❌ | Requires migration |
| Change value type of existing key | ❌ | Requires migration |
| Reorder `TipJarError` discriminants | ❌ | Breaks client error handling |

---

## 7. Correctness Properties

- **P1 (Authorization):** For any caller `c ≠ stored_admin`, `upgrade(c, _)` always panics with `UpgradeUnauthorized`.
- **P2 (Version monotonicity):** `version()` after N upgrades equals `1 + N`.
- **P3 (State preservation):** Creator balances and totals are identical before and after an upgrade.
- **P4 (Event emission):** Every successful `upgrade` emits exactly one `("upgrade", contract_address)` event carrying the new version.
