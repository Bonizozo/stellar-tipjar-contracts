# Code Style Guide

## Rust

### Formatting

All Rust code is formatted with `rustfmt` using the default configuration:

```bash
cargo fmt --all
```

CI will reject PRs that are not formatted.

### Linting

```bash
cargo clippy --all-targets -- -D warnings
```

All Clippy warnings are treated as errors. Common patterns to follow:

- Prefer `if let` over `match` for single-arm matches.
- Use `?` for error propagation instead of `unwrap()` in library code.
- Avoid `clone()` where a reference suffices.

### Contract-Specific Rules

- All public contract functions must have a `///` doc comment.
- Use `panic_with_error!(env, TipJarError::X)` instead of `panic!()` for user-facing errors.
- Storage reads must use typed `DataKey` variants — no raw string keys.
- Emit events for every state change visible to external observers.

### Naming Conventions

| Item | Convention | Example |
|---|---|---|
| Types / Structs | `PascalCase` | `TipWithMessage` |
| Functions | `snake_case` | `tip_with_message` |
| Constants | `SCREAMING_SNAKE_CASE` | `MAX_MESSAGE_LEN` |
| Storage keys | `PascalCase` enum variants | `DataKey::CreatorBalance` |

---

## TypeScript (SDK)

### Formatting

The SDK uses Prettier with default settings:

```bash
cd sdk/typescript && npx prettier --write src/
```

### Linting

```bash
cd sdk/typescript && npx eslint src/ --ext .ts
```

### Type Safety

- All public SDK functions must have explicit return types.
- Avoid `any`; use `unknown` and narrow with type guards.
- Use `bigint` for all on-chain amounts (Soroban `i128` maps to `bigint`).

### Naming Conventions

| Item | Convention | Example |
|---|---|---|
| Classes | `PascalCase` | `TipJarContract` |
| Functions / methods | `camelCase` | `sendTip` |
| Interfaces | `PascalCase` | `TipParams` |
| Constants | `SCREAMING_SNAKE_CASE` | `BASE_FEE` |

---

## Documentation

- Every public Rust function gets a `///` doc comment with `# Parameters`, `# Errors`, and `# Example` sections.
- Every public TypeScript method gets a JSDoc comment with `@param`, `@returns`, `@throws`, and `@example`.
- Markdown files use ATX-style headings (`#`, `##`, `###`).
- Code blocks must specify the language (` ```rust `, ` ```typescript `, ` ```bash `).

---

## Git Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <short description>

[optional body]
```

Types: `feat`, `fix`, `docs`, `test`, `refactor`, `chore`, `perf`.

Examples:
```
feat(contract): add tip_batch function
fix(sdk): handle missing keypair in sendTip
docs(api): document withdraw_locked parameters
test(integration): add multi-token withdrawal test
```
