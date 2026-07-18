# Mainnet Readiness Checklist

Complete every item before deploying to mainnet. Check off each item as it is verified.

---

## Security

- [ ] Security audit completed by an independent Soroban specialist
- [ ] All audit findings resolved or formally accepted with documented rationale
- [ ] Admin key stored in HSM or secrets manager — never in source control
- [ ] Admin key is a multisig account (recommended for high-value deployments)
- [ ] `DEPLOYER_SECRET` is separate from the admin key
- [ ] No secret keys committed to git (`git log --all -S 'S...' --oneline` returns nothing)
- [ ] Token whitelist reviewed — only legitimate SAC or trusted token contracts included
- [ ] Threat model reviewed: [docs/THREAT_MODEL.md](THREAT_MODEL.md)
- [ ] Emergency pause runbook documented and shared with on-call team

## Code Quality

- [ ] All unit tests pass: `cargo test -p tipjar`
- [ ] Full test suite passes: `cargo test --workspace`
- [ ] No compiler warnings: `cargo build -p tipjar --target wasm32v1-none --release 2>&1 | grep -c warning` returns 0
- [ ] WASM size is within acceptable limits (check with `wc -c target/wasm32v1-none/release/tipjar.optimized.wasm`)
- [ ] Contract version is set correctly in storage

## Testnet Validation

- [ ] Contract deployed and initialized on testnet
- [ ] `tip` flow tested end-to-end on testnet with real token
- [ ] `withdraw` flow tested on testnet
- [ ] `tip_batch` tested on testnet
- [ ] `tip_locked` and `withdraw_locked` tested on testnet
- [ ] Role management (`grant_role`, `revoke_role`) tested on testnet
- [ ] `pause` / `unpause` tested on testnet
- [ ] Matching campaign creation and matching tested on testnet
- [ ] Withdrawal limits and cooldown tested on testnet
- [ ] Integration tests run against testnet deployment
- [ ] Testnet contract ID recorded in `deployment/config.json`

## Configuration

- [ ] `deployment/config.json` has correct mainnet RPC URL and network passphrase
- [ ] Mainnet token address(es) confirmed and verified
- [ ] Admin address confirmed and funded (minimum 10 XLM recommended)
- [ ] Deployer account funded (minimum 10 XLM for deployment fees)
- [ ] Frontend / SDK updated with testnet contract ID for final pre-launch testing

## Monitoring

- [ ] Prometheus + Grafana stack deployed and accessible
- [ ] Alert rules configured: `monitoring/prometheus/alert_rules.yml`
- [ ] Event indexer running and synced to testnet
- [ ] On-call rotation established with alert notification channels (PagerDuty / Slack)
- [ ] Dashboard bookmarked: `monitoring/dashboard.html`
- [ ] Runbook for each alert documented

## Documentation

- [ ] [DEPLOYMENT.md](DEPLOYMENT.md) reviewed and up to date
- [ ] [API.md](API.md) reflects current contract interface
- [ ] [SECURITY.md](SECURITY.md) reviewed
- [ ] Rollback procedure tested on testnet: `bash scripts/rollback.sh testnet`
- [ ] Post-deployment announcement drafted (blog post / social / Discord)

## Go / No-Go Sign-Off

| Role | Name | Date | Approved |
|---|---|---|---|
| Lead Engineer | | | ☐ |
| Security Reviewer | | | ☐ |
| Product Owner | | | ☐ |

---

## Post-Deployment Checklist

Complete these steps immediately after mainnet deployment.

- [ ] Contract ID recorded in `deployment/config.json`
- [ ] `verify_deployment.sh` passes: `bash scripts/verify_deployment.sh $CONTRACT_ID mainnet`
- [ ] Contract initialized with correct admin and token addresses
- [ ] First tip transaction submitted and confirmed on-chain
- [ ] First withdrawal confirmed on-chain
- [ ] Monitoring alerts firing correctly (trigger a test alert)
- [ ] Event indexer synced to mainnet and processing events
- [ ] Frontend updated with mainnet contract ID
- [ ] Deployment announced to users
- [ ] Deployment recorded in team changelog / release notes
