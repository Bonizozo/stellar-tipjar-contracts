# Contract Upgrade and Migration System — Tasks

## Task List

- [ ] 1. Add `DataKey::ContractVersion` variant to the `DataKey` enum in `contracts/tipjar/src/lib.rs`
- [ ] 2. Add `TipJarError::UpgradeUnauthorized = 23` to the `TipJarError` enum in `contracts/tipjar/src/lib.rs`
- [ ] 3. Implement `pub fn upgrade(env: Env, admin: Address, new_wasm_hash: soroban_sdk::BytesN<32>)` inside the `#[contractimpl]` block
  - [ ] 3.1 Call `admin.require_auth()`
  - [ ] 3.2 Verify `admin` matches stored admin; panic with `UpgradeUnauthorized` otherwise
  - [ ] 3.3 Call `env.deployer().update_current_contract_wasm(new_wasm_hash)`
  - [ ] 3.4 Increment `DataKey::ContractVersion` in instance storage
  - [ ] 3.5 Emit `("upgrade", contract_address)` event with new version
- [ ] 4. Implement `pub fn version(env: Env) -> u32` inside the `#[contractimpl]` block — returns `ContractVersion` from instance storage (default `1`)
- [ ] 5. Write unit tests in `contracts/tipjar/src/lib.rs` (or a test module)
  - [ ] 5.1 Test: admin can call `upgrade` successfully and version increments
  - [ ] 5.2 Test: non-admin call to `upgrade` panics with `UpgradeUnauthorized`
  - [ ] 5.3 Test: `version()` returns `1` before any upgrade
- [ ] 6. Create `scripts/upgrade_contract.sh` — Bash upgrade script
- [ ] 7. Create `docs/UPGRADE_GUIDE.md` — end-to-end upgrade documentation
- [ ] 8. Run `cargo test --manifest-path contracts/tipjar/Cargo.toml` and confirm all 52+ tests pass
