# Architecture Overview

## Repository Layout

```
stellar-tipjar-contracts/
├── contracts/tipjar/       # Core Soroban smart contract
│   ├── src/lib.rs          # Contract entry point and all function implementations
│   └── Cargo.toml
├── sdk/typescript/         # TypeScript SDK wrapping contract calls
│   └── src/
│       ├── TipJarContract.ts
│       ├── types.ts
│       ├── errors.ts
│       ├── events.ts
│       └── utils.ts
├── indexer/                # Off-chain event indexer (Rust + SQLite/Postgres)
├── analytics/              # Metrics aggregation and dashboard
├── security/               # Rate limiter, circuit breaker, anomaly detector
├── simulator/              # Local contract simulator for development
├── tools/
│   ├── gas-estimator/      # Fee estimation tool
│   ├── doc-generator/      # Auto-generates API docs from source
│   └── backup/             # State export/import utilities
├── monitoring/             # Prometheus + Grafana dashboards
├── scripts/                # Deploy, upgrade, backup, and migration scripts
└── docs/                   # This documentation
```

## Request / Data Flow

```
Frontend / CLI
     │
     ▼
TypeScript SDK (sdk/typescript/)
     │  builds + signs XDR transactions
     ▼
Stellar RPC (soroban-testnet / mainnet)
     │  submits transaction
     ▼
Soroban Runtime
     │  executes contract WASM
     ▼
TipJar Contract (contracts/tipjar/src/lib.rs)
     │  reads/writes persistent storage
     │  emits contract events
     ▼
Stellar Ledger (immutable)
     │
     ├──► Indexer (indexer/) — streams events → DB
     └──► Analytics (analytics/) — aggregates metrics
```

## Contract Architecture

The contract is a single Soroban contract with the following logical modules:

| Module | File | Responsibility |
|---|---|---|
| Core tip/withdraw | `lib.rs` | `init`, `tip`, `withdraw`, `get_total_tips` |
| Tip with message | `lib.rs` | `tip_with_message`, `get_messages` |
| Batch tips | `lib.rs` | `tip_batch` |
| Locked tips | `lib.rs` | `tip_locked`, `withdraw_locked` |
| Subscriptions | `lib.rs` | `create_subscription`, `execute_subscription_payment` |
| Leaderboard | `lib.rs` | `get_leaderboard` |
| Matching programs | `lib.rs` | `create_matching_program`, `cancel_matching_program` |
| Milestones | `lib.rs` | `create_milestone`, `complete_milestone` |
| Roles / RBAC | `lib.rs` | `grant_role`, `revoke_role` |
| Pause / unpause | `lib.rs` | `pause`, `unpause` |
| Upgrade | `upgrade.rs` | `upgrade` (WASM hash replacement) |
| Events | `events/` | Structured event emission helpers |
| Privacy | `privacy/` | Commitment-based private tips |
| Governance | `governance/` | On-chain proposals and voting |
| Staking | `staking/` | Reward distribution |
| AMM | `amm/` | Automated market maker primitives |

## Storage Model

All state lives in Soroban's key-value store. Keys are typed via `DataKey`:

- **Instance storage** — shared contract-level data (token, admin, paused flag).
- **Persistent storage** — per-creator data (balance, total, messages, leaderboard entries).

See `docs/STORAGE.md` for the full key catalogue.

## Off-Chain Components

### Indexer
Streams `getEvents` from the RPC, parses tip/withdraw events, and writes them to a relational database. Exposes a REST API for historical queries.

### Analytics
Reads from the indexer DB, computes aggregates (daily volume, top creators, tip counts), and serves a dashboard at `analytics/dashboard/index.html`.

### Security Monitor
Runs alongside the indexer. Detects anomalies (large single tips, rapid-fire tips), triggers circuit breakers, and sends alerts.

## Diagrams

### Wallet Interaction Flow

```
User clicks "Tip"
      │
      ▼
SDK validates params (amount > 0, creator valid)
      │
      ▼
Build TransactionBuilder with contract invocation
      │
      ▼
simulateTransaction (dry-run, get fee)
      │
      ▼
Sign with Keypair / Freighter wallet
      │
      ▼
sendTransaction → poll getTransaction until SUCCESS/FAILED
      │
      ▼
Parse result / emit UI feedback
```

### Component Hierarchy (TypeScript SDK)

```
TipJarContract
  ├── connect(keypair)
  ├── sendTip(params)          → buildTx → simulate → sign → submit
  ├── withdraw(params)         → buildTx → simulate → sign → submit
  ├── getTotalTips(creator)    → simulateTransaction (read-only)
  └── getWithdrawableBalance   → simulateTransaction (read-only)
```
