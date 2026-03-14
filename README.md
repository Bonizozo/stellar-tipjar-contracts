# Stellar Tip Jar – Smart Contracts

This repository contains the **Soroban smart contracts** that power the Stellar Tip Jar platform.

The contracts are responsible for securely handling tip payments between supporters and creators on the Stellar blockchain.

These contracts ensure that funds are transferred transparently and securely without relying on centralized intermediaries.

---

## Project Overview

Stellar Tip Jar is an open-source platform that allows supporters to send tips to creators using Stellar tokens.

Example creator link:

```
https://tipjar.app/username
```

When a user sends a tip:

1. The transaction is sent to the smart contract.
2. The contract validates the payment.
3. The funds are transferred to the creator’s wallet.
4. The backend indexes the transaction for display in the frontend.

---

## Architecture

The project is structured as a multi-repository system:

```
stellar-tipjar
│
├── stellar-tipjar-frontend
├── stellar-tipjar-backend
└── stellar-tipjar-contracts
```

* **Frontend** – User interface for tipping creators
* **Backend** – API and transaction indexing
* **Contracts** – On-chain tipping logic

---

## Tech Stack

* Rust
* Soroban SDK
* Stellar CLI
* Cargo

---

## Project Structure

```
contracts
│
├── tipjar
│   ├── src
│   │   └── lib.rs
│   └── Cargo.toml
│
tests
│
scripts
│
README.md
```

Explanation:

* **contracts/** – Smart contract implementations
* **tests/** – Contract test cases
* **scripts/** – Deployment and helper scripts

---

## Core Contract Logic

The tip jar contract supports the following operations:

### Send Tip

Transfers tokens from the supporter to the creator.

```
tip(sender, creator, amount)
```

---

### Get Total Tips

Returns the total amount of tips received by a creator.

```
get_total_tips(creator)
```

---

### Withdraw Tips (optional)

Allows creators to withdraw accumulated tips.

```
withdraw(creator)
```

---

## Getting Started

### Install Stellar CLI

Install the CLI used for working with Soroban contracts.

```
stellar --version
```

---

### Build the contract

```
cargo build --target wasm32-unknown-unknown --release
```

---

### Run tests

```
cargo test
```

---

## Deployment

Contracts can be deployed to the **Stellar Testnet** for development and experimentation.

Example deployment command:

```
stellar contract deploy \
--wasm target/wasm32-unknown-unknown/release/tipjar.wasm \
--network testnet
```

After deployment, the contract ID can be used by the backend and frontend.

---

## Contributing

We welcome contributions from smart contract developers.

Ways to contribute:

* Improve contract security
* Add new tipping features
* Write additional tests
* Improve documentation

### Contribution Steps

1. Fork the repository
2. Create a feature branch

```
git checkout -b feature/new-feature
```

3. Commit your changes
4. Push your branch
5. Open a Pull Request

---

## Roadmap

Planned contract improvements:

* multi-token tipping
* creator tip statistics
* tipping messages
* NFT thank-you rewards
* tipping leaderboard

---

## License

MIT License

---

## Related Repositories

* stellar-tipjar-frontend
* stellar-tipjar-backend
