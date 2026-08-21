# Tests directory

Most contract unit tests live alongside the code in `contracts/tipjar/src/`
(`test.rs`, `test_invariants.rs`, `test_upgrade.rs`, ...) and use Soroban's
testing framework.

This top-level directory holds integration-style suites that are wired up as
explicit `[[test]]` targets of the packages that own them, rather than living
under those packages' own `tests/` directories:

- `pause_tests.rs`, `partial_pause_tests.rs` (+ `common/mod.rs`) — circuit-breaker
  coverage for `contracts/tipjar`, declared via `[[test]]` entries in
  `contracts/tipjar/Cargo.toml` (see `common/mod.rs` for why they live here
  instead of `contracts/tipjar/tests/`).

`integration/` and `gas/` are their own things: `integration/` is a separate
workspace member (`tests/integration/Cargo.toml`) exercising `contracts/tipjar`
as an external dependency, and `gas/benchmarks.rs` is not currently wired into
any package's `[[test]]`/`[[bench]]` list.

A prior version of this directory held 23 additional suites (plus a root
`src/` "SDK" tree) with no `[package]` anywhere to compile them as — they were
never part of any `cargo build`/`test`/`clippy` invocation and had been dead
since the commit that added them (see #414). Of that tree:
- the `src/config`/`src/simulation` SDK code was real and has been moved into
  the (previously placeholder) `sdk` workspace member, where its tests now run
  as `sdk/tests/simulation_tests.rs`.
- the remaining 20 test files were written against a `TipJarContract`/
  `TipJarError`/`Role`/`Subscription`/bridge/privacy/swap-shaped API that
  matches `contracts/tipjar-legacy`, imported under the name `tipjar`
  (`contracts/tipjar`'s real package name, but not its API shape) rather than
  `tipjar_legacy`. Wiring them up as `tipjar-legacy` test targets is blocked on
  a separate, pre-existing problem: `contracts/tipjar-legacy`'s library itself
  does not currently compile (`cargo check -p tipjar-legacy` fails independent
  of this tree, e.g. missing `TipJarError` variants and calls to the removed
  `Env::invoker()` API in `lending`/`recovery`). Since fixing that is out of
  scope here and the suites were never exercised in the first place, they were
  deleted rather than left as dead weight that looks like real coverage.
