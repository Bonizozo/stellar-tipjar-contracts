# Contract Upgrade and Migration System — Tasks

> **Status: implemented** (issue #358). Rewritten to reflect the task list
> actually completed — the original list described a single-step
> `upgrade()`/`TipJarError` design that was superseded by the timelocked
> flow below before any of it was built.

## Task List

- [x] 1. Add `DataKey::Admin`, `PendingAdmin`, `UpgradeTimelockLedgers`, `PendingUpgrade`, `DataVersion` variants to `contracts/tipjar/src/lib.rs`
- [x] 2. Add `Error::Unauthorized`, `NoPendingAdmin`, `InvalidTimelock`, `UpgradeAlreadyPending`, `NoPendingUpgrade`, `TimelockNotElapsed`
- [x] 3. Extend `init` to take `admin: Address` and `upgrade_timelock_ledgers: u32`, storing both plus the initial `DataVersion`
- [x] 4. Implement `propose_upgrade`, `execute_upgrade`, `cancel_upgrade` (timelocked, admin-gated proposal/cancellation, permissionless execution past the timelock)
- [x] 5. Implement `propose_admin` / `accept_admin` two-step admin transfer
- [x] 6. Implement `migrate(admin)` — idempotent, version-gated against `DATA_VERSION`
- [x] 7. Emit `UpgradeProposed`, `UpgradeExecuted`, `UpgradeCancelled`, `Migrated`, `AdminTransferProposed`, `AdminTransferAccepted`
- [x] 8. Build `contracts/tipjar-v2-fixture` — a genuinely distinct compiled WASM used to test the swap end-to-end via `soroban_sdk::contractimport!`
- [x] 9. Write `contracts/tipjar/src/test_upgrade.rs`: balance/total preservation, timelock boundary (unlock−1 / unlock), cancellation, non-admin rejection, double-proposal rejection, migrate idempotency, admin transfer, event-schema assertions
- [x] 10. Update existing `test.rs`/`test_exhaustive.rs` call sites for the new `init` signature; regenerate golden event XDR fixtures
- [x] 11. Retire competing upgrade paths: delete orphaned `migrations/upgrade_v1_to_v2.rs` and unreferenced `contracts/tipjar-legacy/src/proxy.rs`; document `contracts/tipjar-legacy`'s own `upgrade()`/`get_version()`/`migrate_state()` as retired (crate kept alive for `simulator`/`tools/gas-estimator`, which depend on its broader feature set)
- [x] 12. Point `tests/integration/tip_flow_test.rs` at `contracts/tipjar` instead of `tipjar-legacy`
- [x] 13. Rewrite `docs/UPGRADE_GUIDE.md`; write `docs/UPGRADE_RUNBOOK.md` (propose → monitor → execute → verify, rollback stance, `scripts/migrate` wiring); update `docs/EVENTS.md`
- [x] 14. Wire `.github/workflows/test.yml` and `contract-ci.yml` to build the v2 fixture WASM before running `tipjar` tests
- [x] 15. `cargo test -p tipjar` (29 tests), `cargo test -p tipjar-integration-tests` (11 tests), `cargo fmt --check`, `cargo clippy --all-targets -D warnings`, `cargo build -p tipjar --target wasm32v1-none --release`
