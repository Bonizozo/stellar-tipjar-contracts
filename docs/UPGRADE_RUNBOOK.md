# TipJar Upgrade Runbook

Operational procedure for shipping a WASM upgrade to a deployed
`contracts/tipjar` instance: propose → monitor → execute → verify, plus the
rollback stance. For the mechanism itself (storage layout, function
semantics, backward-compatibility rules), see
[`docs/UPGRADE_GUIDE.md`](./UPGRADE_GUIDE.md).

> **Every command below targets a `contracts/tipjar` `<CONTRACT_ID>`, and
> only that contract.** `contracts/tipjar-legacy` is a frozen reference
> crate — not deployed anywhere, not the production TipJar — that happens to
> carry its own `upgrade`/`get_version`/`migrate_state`/`migrate` functions,
> including a same-named, same-signature `migrate(admin)` that is **not**
> the timelocked flow documented here. If you land on this runbook after a
> mistyped contract ID, a copy-pasted CLI command against the wrong deployed
> address, or general confusion about which crate you're looking at:
> `tipjar-legacy`'s upgrade-shaped functions are retired and must never be
> invoked as a substitute for the `propose_upgrade`/`execute_upgrade`/
> `cancel_upgrade`/`migrate` sequence below. If `stellar contract invoke`
> against your `<CONTRACT_ID>` doesn't emit the events described in §3, stop
> and re-verify the contract ID before proceeding — do not assume a
> different-shaped response just means a different function signature.

Two separate tools are involved and it's easy to conflate them:

- **`stellar` CLI** — the only thing that actually calls the contract
  (`propose_upgrade`, `execute_upgrade`, `cancel_upgrade`, `migrate`).
- **`scripts/migrate`** (`cargo run --manifest-path scripts/migrate/Cargo.toml
  --bin migrate -- <subcommand>`) — an independent, read-only-by-default
  state export/verification toolkit that talks to the network over RPC. It
  never calls `propose_upgrade`/`execute_upgrade` itself; it exists so you
  have an off-chain, byte-level record of contract state before and after
  the swap, to detect any discrepancy the contract's own storage-preservation
  guarantee didn't actually hold.

Prerequisites: [`stellar` CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli)
installed and configured, admin key available (`stellar keys ls`), new
contract WASM built (`cargo build --target wasm32v1-none --release
--manifest-path contracts/tipjar/Cargo.toml`).

---

## 1. Snapshot state before touching anything

Take an off-chain export of the live contract's state. This is the
rollback stance: **the only supported rollback path is re-deploying from a
pre-upgrade snapshot plus the original WASM**, since Soroban has no native
"undo" for a WASM swap or a `migrate()` that already ran (see §6).

```bash
cp scripts/migrate/config.toml migrate.local.toml
# edit migrate.local.toml: source_contract_id = target_contract_id = <CONTRACT_ID>,
# network, rpc_url, backup_filename

cargo run --manifest-path scripts/migrate/Cargo.toml --bin migrate -- \
    export --config migrate.local.toml
```

Keep the resulting snapshot (`snapshot_dir/backup_filename`) somewhere
durable outside the repo — it's your only way back.

## 2. Upload and propose

```bash
stellar contract upload \
  --wasm target/wasm32v1-none/release/tipjar.wasm \
  --source <admin-key> --network testnet
# → <new_wasm_hash>, save it

stellar contract invoke --id <CONTRACT_ID> --source <admin-key> --network testnet \
  -- propose_upgrade --admin <ADMIN_ADDRESS> --new_wasm_hash <new_wasm_hash>
```

This emits `UpgradeProposed { hash, unlock_ledger }` and fails outright if a
proposal is already pending (`UpgradeAlreadyPending`) — cancel the existing
one first (§5) if you need to replace it. `unlock_ledger` is
`UpgradeTimelockLedgers` (fixed at `init`, ~48h of ledgers by convention)
ledgers past the proposal, and is public on-chain state — anyone can compute
when the upgrade becomes executable, including the watchers in the next
step.

## 3. Monitor

`UpgradeProposed`/`UpgradeExecuted`/`UpgradeCancelled` (and
`AdminTransferProposed`/`AdminTransferAccepted`) are exactly the events a
watcher should alert on — see `docs/EVENTS.md` for their exact topic/data
shape and `monitoring/` for this repo's existing event-monitor scaffolding.
At minimum, confirm the proposal is visible before the timelock starts
counting down:

```bash
stellar contract invoke --id <CONTRACT_ID> --network testnet -- get_admin
```

During the timelock window is the time to socialize the change, let anyone
depending on the contract review the new WASM, and abort (§5) if anything
looks wrong — that review window is the entire point of the delay.

## 4. Execute

`execute_upgrade` is permissionless (no admin key needed for this step — see
`docs/UPGRADE_GUIDE.md` for why) but will panic with `TimelockNotElapsed`
until the current ledger reaches `unlock_ledger`:

```bash
stellar contract invoke --id <CONTRACT_ID> --network testnet \
  -- execute_upgrade
```

Success emits `UpgradeExecuted { hash }` and swaps the WASM in place — all
storage is preserved by the host automatically, no action needed for that
part.

If the new WASM's `DATA_VERSION` is ahead of the currently stored
`DataVersion`, run the migration once, by hand, right after:

```bash
stellar contract invoke --id <CONTRACT_ID> --source <admin-key> --network testnet \
  -- migrate --admin <ADMIN_ADDRESS>
```

`migrate` is idempotent and version-gated — re-running it (accidentally, or
to double-check) is always safe and a no-op once the version already
matches.

## 5. Abort instead, if needed

Any time before `execute_upgrade` succeeds — timelock elapsed or not:

```bash
stellar contract invoke --id <CONTRACT_ID> --source <admin-key> --network testnet \
  -- cancel_upgrade --admin <ADMIN_ADDRESS>
```

Emits `UpgradeCancelled { hash }` and clears the pending proposal. There is
nothing to undo on-chain — no WASM swap happened yet.

## 6. Verify, and the rollback stance

Compare live state against the pre-upgrade snapshot from §1 with
`scripts/migrate/verify_migration.rs`:

```bash
cargo run --manifest-path scripts/migrate/Cargo.toml --bin migrate -- \
    verify --config migrate.local.toml --snapshot <path-to-step-1-snapshot>
```

This checks every category `verify_migration.rs` knows about (instance
storage, creator balances/totals, and the rest of its 12-category report)
against the live contract and prints a discrepancy report. For a same-shape
upgrade (no `migrate()` needed) every category should read identical to the
pre-upgrade snapshot; for one that ran a real migration, expect exactly the
transformed fields to differ and everything else to match.

**Rollback stance:** Soroban does not provide a native "undo" for
`update_current_contract_wasm`, and if `migrate()` already transformed
storage, the old WASM may not even be able to read the new layout correctly.
Treat rollback as a forward action, not an undo:

1. Confirm the old WASM hash is still uploaded (re-upload if not:
   `stellar contract upload --wasm <old-wasm> --source <admin-key> --network
   testnet`).
2. Go through §2–§4 again with the old hash as the proposal — this still
   waits out the full timelock. There is no fast-path rollback; that's the
   same safety property that makes the forward path safe.
3. If `migrate()` ran and transformed storage in a way the old WASM can't
   read, restore from the §1 snapshot via `scripts/migrate`'s `import`
   subcommand into a *new* contract instance instead, and redirect clients —
   don't attempt to force incompatible storage back onto the original
   instance.

This is why §1 isn't optional: it's the only way to know, after the fact,
exactly what changed and whether a forward-rollback or a fresh-instance
restore is the right call.
