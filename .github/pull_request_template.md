## Summary

- 

## Testing

- [ ] `cargo test --workspace`
- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --all-targets -- -D warnings`

## Snapshot diff review

If this PR changes any of these human-readable contract snapshots, reviewers must inspect the JSON diff before approval:

- `contracts/derivatives/test_snapshots/tests/*.1.json`
- `contracts/risk-management/test_snapshots/tests/*.1.json`
- `contracts/limit-orders/test_snapshots/tests/*.1.json`
- `contracts/arbitrage/test_snapshots/tests/*.1.json`

- [ ] No snapshot files changed in those directories.
- [ ] Snapshot files changed, and the PR description explains the intended storage/event diff.
- [ ] Snapshot files changed, and a reviewer has explicitly confirmed the JSON diff is expected.

## Fixture diff review

If this PR regenerated any XDR or JSON event-schema fixture files under
`contracts/tipjar/tests/fixtures/` (i.e. `UPDATE_FIXTURES=1` was used), the
companion `.json` files must be reviewed before merging. The JSON diff shows
exactly which fields, types, or field ordering changed in the on-chain event
schema — a change that is invisible in the binary `.xdr` diff.

**Before ticking any box below, open the "Files changed" tab and read every
changed `.json` file in `contracts/tipjar/tests/fixtures/`.**

- [ ] No fixture files changed in this PR.
- [ ] Fixture files changed, and the JSON companion diff was reviewed. The changes are intentional and described in the summary above.
- [ ] Fixture files changed, and a reviewer has explicitly confirmed the JSON diff matches the intended event-schema change.
