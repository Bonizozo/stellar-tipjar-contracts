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
