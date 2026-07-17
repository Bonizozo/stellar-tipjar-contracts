# Contributing Guide

See the root [`CONTRIBUTING.md`](../CONTRIBUTING.md) for the full contribution workflow.

This page supplements it with contract-specific details.

## Development Setup

```bash
# Install Rust toolchain
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add Soroban WASM target
rustup target add wasm32v1-none

# Install Stellar CLI
cargo install --locked stellar-cli

# Clone and build
git clone https://github.com/your-org/stellar-tipjar-contracts
cd stellar-tipjar-contracts
cargo build -p tipjar --target wasm32v1-none --release
```

## Running Tests

```bash
# Unit + integration tests
cargo test -p tipjar

# All workspace tests
cargo test --workspace

# Specific test
cargo test -p tipjar test_tipping_functionality
```

## Branching Strategy

| Branch prefix | Purpose |
|---|---|
| `feature/<name>` | New functionality |
| `fix/<name>` | Bug fixes |
| `docs/<name>` | Documentation only |
| `chore/<name>` | Tooling, CI, dependencies |
| `refactor/<name>` | Code restructuring without behavior change |

Branch from `main`, keep branches focused on one concern.

## Pull Request Checklist

- [ ] `cargo test --workspace` passes
- [ ] `cargo fmt --all -- --check` passes
- [ ] `cargo clippy --all-targets -- -D warnings` passes
- [ ] New behavior is covered by tests
- [ ] Storage key changes are backward-compatible or include a migration
- [ ] Events are emitted for all observable state changes
- [ ] Docs updated if public API changed
- [ ] No secrets committed

## Test Requirements

Every PR that changes contract behavior must include:

1. A **happy-path test** demonstrating the new behavior.
2. At least one **error/edge-case test** (invalid input, unauthorized caller, paused state).
3. Tests must use the Soroban test environment (`soroban_sdk::testutils`).

## Commit Message Format

```
feat(contract): add tip_batch function
fix(sdk): handle missing keypair in sendTip
docs(api): document withdraw_locked parameters
test(integration): add multi-token withdrawal test
```

See `docs/CODE_STYLE.md` for the full style guide.
