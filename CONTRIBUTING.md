# Contributing to Stellar Tip Jar Contracts

This guide covers the `tipjar` contract (`contracts/tipjar/`) and its
frontend client (`packages/contract-client/`). It does not apply to the
unrelated code under `contracts/tipjar-legacy/` or the other workspace
members.

## Branching Strategy

- `main` is protected: always reviewable and releasable, no direct pushes.
- `feature/<short-description>` — new functionality.
- `fix/<short-description>` — bug fixes.
- `chore/<short-description>` — non-functional maintenance (deps, CI, docs).

Keep branches focused on one concern; avoid mixing unrelated changes in a
single PR.

## Coding Standards

### Rust (`contracts/tipjar/`)

- Run `cargo fmt -p tipjar` before committing.
- `cargo clippy -p tipjar --all-targets -- -D warnings` must be clean —
  warnings are treated as errors.
- `#![no_std]`: don't introduce `std` dependencies; use `soroban_sdk` types
  (`Address`, `Env`, `Vec`, `Map`, etc.) instead of `alloc`/`std` collections.
- Errors: extend the `Error` enum (`#[contracterror]`) rather than panicking
  with a bare string, and raise them with `panic_with_error!`. Keep error
  variants and their discriminants stable once shipped — clients and indexers
  may depend on the numeric codes.
- Storage keys: extend `DataKey` rather than introducing ad-hoc keys, and bump
  TTL (`extend_ttl`) on any persistent/instance entry you write.
- Events: define new events with `#[contractevent]` (not the deprecated
  `Events::publish`), matching the existing `Tip`/`Withdraw` pattern.

### TypeScript (`packages/contract-client/`)

- No ESLint/Prettier config exists yet — until one is added, match the
  existing style (2-space indent, semicolons, single quotes).
- `npm run build` (`tsc`) must pass with no type errors.
- Don't hand-edit `src/generated.ts` — it's vendored output from
  `scripts/generate-bindings.sh`. Hand-write changes in `src/index.ts` or
  `src/networks.ts` instead.

## Test Requirements

- Any new or changed contract behavior needs a corresponding test in
  `contracts/tipjar/src/test.rs`.
- `cargo test -p tipjar` must pass before opening a PR.
- Cover the success path, the relevant `Error` variant(s), and any emitted
  event for new behavior — see the existing tests for the expected shape
  (real SAC test token, `mock_all_auths()`, `try_<fn>()` for expected errors,
  `env.events().all().filter_by_contract(...)` for event assertions).

## Commit Convention

Use [Conventional Commits](https://www.conventionalcommits.org/):

```text
feat: add minimum tip amount check
fix: bump persistent TTL on withdraw
docs: update README build instructions
chore: bump soroban-sdk to 26.1.0
```

## Pull Request Checklist

- [ ] `cargo build -p tipjar --target wasm32v1-none --release` succeeds.
- [ ] `cargo test -p tipjar` passes.
- [ ] `cargo fmt -p tipjar --check` and `cargo clippy -p tipjar --all-targets -- -D warnings` are clean.
- [ ] If `packages/contract-client/` changed: `npm run build` passes.
- [ ] `README.md` / `packages/contract-client/README.md` updated if behavior, commands, or exports changed.
- [ ] Any `DataKey` or event (topics/data) change is called out explicitly in the PR description — these are part of the contract's on-chain interface.
- [ ] No secrets, identity seed phrases, or private keys committed — `deployment/config.json` should only ever contain public contract IDs, RPC URLs, and network passphrases.
