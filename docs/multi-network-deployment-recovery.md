# Multi-Network Deployment Failure Recovery

This guide covers recovery when `scripts/deploy-multi-network.sh` partially
succeeds across `TESTNET` and `MAINNET`. Run commands from the repository root.
Replace every `<...>` placeholder with real values; never paste a seed or secret
into a manifest or a shell command that is logged.

## Required inputs

`deploy-multi-network.sh` accepts only these flags:

| Flag | Requirement |
|------|-------------|
| `--networks` | Comma-separated list; default `TESTNET`. Supported values: `TESTNET`, `MAINNET`. |
| `--admin` | Required unless supplied by `--config`; must be a Stellar address. |
| `--operator` | Optional; defaults to the admin address. |
| `--config` | Optional JSON file with `admin`, `operator`, and `networks` keys. Values from this file override the flags. |
| `--manifest` | Optional output path; default `deployment-manifest.json`. |
| `--dry-run` | Print the plan and write a manifest without building or deploying. |
| `--help` | Print usage and exit without validation. |

Prerequisites: `stellar-cli` in `PATH`, a configured Freighter wallet for
`--source auto`, signed-in source accounts with XLM on every target network, and
`jq` when `--config` is used.

`scripts/pre-deploy-verify.ts` has no admin or manifest flags. It takes
`--network <NETWORK>` or `DEPLOY_NETWORK`, with optional
`DEPLOY_NETWORK_PASSPHRASE`, and reads fixed artifact paths.

## Artifact verification

Run verification before each network attempt, and again after any rebuild:

```bash
npx ts-node --transpile-only scripts/pre-deploy-verify.ts --network <NETWORK>
```

The verifier checks the WASM at
`sla_calculator/target/wasm32-unknown-unknown/release/sla_calculator.wasm`,
its size against the 100 KB budget, its SHA-256 against
`sla_calculator/manifest.sha256`, and `target_network` in
`sla_calculator/build-metadata.json` against the configured network and
passphrase. A rebuilt artifact has a new hash, so re-run this step after
`stellar contract build` / `stellar contract optimize` and regenerate the
manifest with `sha256sum <WASM_PATH> > sla_calculator/manifest.sha256` when the
artifact intentionally changes. Any failed check blocks the rollout.

`deploy-multi-network.sh` rebuilds and optimizes the contract inside each
network loop, so a hash verified before the run belongs to the artifact that is
deployed only if nothing rebuilds it in between. Do not edit sources between
network attempts.

## Rollout and dry run

```bash
./scripts/deploy-multi-network.sh \
  --networks TESTNET,MAINNET \
  --admin <ADMIN_ADDRESS> \
  --operator <OPERATOR_ADDRESS> \
  --manifest <MANIFEST_FILE> \
  --dry-run

./scripts/deploy-multi-network.sh \
  --networks TESTNET,MAINNET \
  --admin <ADMIN_ADDRESS> \
  --operator <OPERATOR_ADDRESS> \
  --manifest <MANIFEST_FILE>
```

The dry run records `DRY_RUN:PENDING` per network and performs no build, deploy,
or initialize. Missing `--admin` exits 1; `--help` exits 0.

## Per-network outcomes

Each network is processed independently. The script continues after a failure
and always writes the manifest before exiting nonzero if any network failed.

| Outcome | Meaning | Recovery |
|---------|---------|----------|
| `SUCCESS:<CONTRACT_ID>` | Deploy and `initialize` both succeeded. | Keep the ID; do not redeploy this network. |
| `FAILED:<cause>` | Build, deploy, or initialize failed. | Read `results.<NETWORK>.error`; retry only this network. |
| `FAILED:Unknown network` | Name not in `TESTNET,MAINNET`. | Fix `--networks` and rerun. |
| `DRY_RUN:PENDING` | Planning run only. | Rerun without `--dry-run`. |
| `NOT_STARTED` | No result was recorded for that name. | Check spelling in `--networks` and the manifest. |

The legacy `networks` field keeps the `STATUS:value` strings. The machine-readable
`results` field holds `status`, `contractId`, and `error` per network. Failure
causes are truncated and secret-looking seeds are redacted. When deploy succeeds
but `initialize` fails, `contractId` still records that partial deployment;
treat it as uninitialized and finish it manually or redeploy the network
deliberately.

## Retry behavior

There is no automatic rollback and no automatic retry. Successful networks are
not revisited; the script retries only what you pass in `--networks`.

```bash
./scripts/deploy-multi-network.sh \
  --networks <FAILED_NETWORK> \
  --admin <ADMIN_ADDRESS> \
  --operator <OPERATOR_ADDRESS> \
  --manifest <RETRY_MANIFEST_FILE>
```

Before retrying, read the failed entry in `<MANIFEST_FILE>`:

- `contractId: null`: no deployment was recorded; a normal retry is safe.
- `contractId: "<PARTIAL_CONTRACT_ID>"`: deployment happened before the failure.
  Either complete initialization against that ID with `stellar contract invoke`
  using the same `--network`, `--source auto`, `--rpc-url`, and
  `--network-passphrase` values the script uses, or start a fresh deployment for
  that network and record the new ID. Do not run `initialize` twice.

Use a different `--manifest` path for the retry so the successful IDs from the
first run remain auditable. After the retry exits 0, merge the successful IDs
into backend config and run post-deploy smoke verification against each
contract before routing traffic. If a second run fails on the same network, stop
and resolve the cause instead of looping retries.
