# Quarantined examples

`lending_example.rs` was moved here because it does not compile: every method
body is a placeholder (`Ok(0)`/`Ok(())`) with the real call into
`lending::pool`/`lending::loan` left commented out, all parameters are
therefore unused, and the `Result<T, String>` return types used throughout
aren't valid Soroban contract return types (`String` doesn't implement the
conversions `#[contractimpl]` requires). It's illustrative documentation of
the intended call shape, not working code.

Cargo only auto-discovers top-level `examples/*.rs` as example targets, so
files in this subdirectory are excluded from `cargo build`/`cargo clippy`.

To re-enable: replace the commented-out real calls with actual working code
(wiring up real error types instead of `String`), then `git mv` the file back
up to `examples/`.
