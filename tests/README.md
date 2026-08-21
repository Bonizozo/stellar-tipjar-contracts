# Tests directory

This folder holds **integration tests that are compiled as part of the
`tipjar` package** (`contracts/tipjar`), even though the files live at the
repo root rather than under `contracts/tipjar/tests/`.

## Why the files are here, not under `contracts/tipjar/tests/`

`pause_tests.rs` and `partial_pause_tests.rs` share a common harness in
`tests/common/mod.rs`. Cargo compiles `mod common;` fresh into each
`[[test]]` binary, so a single shared `tests/common/` directory is the
simplest way to avoid duplicating the harness across two packages.

Because Cargo's test auto-discovery only looks in a package's *own*
`tests/` directory, these root-level files are **not** auto-discovered.
They are wired in explicitly via `[[test]]` entries in
`contracts/tipjar/Cargo.toml`:

```toml
[[test]]
name = "pause_tests"
path = "../../tests/pause_tests.rs"

[[test]]
name = "partial_pause_tests"
path = "../../tests/partial_pause_tests.rs"
```

## How to verify the wiring

These targets are real `[[test]]` targets of the `tipjar` package — they
are **not** orphaned. You can confirm this directly:

```bash
# 1. They appear as `test` targets of the `tipjar` package in cargo metadata:
cargo metadata --no-deps --format-version 1 \
  | jq -r '.packages[] | select(.name=="tipjar") | .targets[] | select(.kind[0]=="test") | .name'
# => pause_tests
# => partial_pause_tests

# 2. They enumerate their expected test functions:
cargo test -p tipjar --test pause_tests --test partial_pause_tests -- --list

# 3. They pass:
cargo test -p tipjar --test pause_tests --test partial_pause_tests
```

> Note: `cargo test -p tipjar` (the full suite) additionally compiles the
> in-tree unit tests under `contracts/tipjar/src/`, which embed a v2 upgrade
> fixture WASM via `soroban_sdk::contractimport!`. That fixture must be built
> first — see `docs/UPGRADE_GUIDE.md` and `.github/workflows/test.yml`.
> The two integration targets above do **not** depend on that fixture and can
> be run independently.

## Other files in this directory

The remaining `*.rs` files (`core_functionality.rs`, `edge_cases.rs`,
`security_tests.rs`, etc.) are **not** wired to any package and are not
compiled by `cargo test`. They are legacy/scratch integration tests kept
for reference; see `tests/README.md` history and the audit notes in
`docs/SECURITY.md` for the orphaned-test-tree context.
