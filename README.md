<!-- Optional: replace with a banner image. Keep it plain — a wordmark, not a stock illustration. -->
<h1 align="center">LedgerKeep</h1>

<p align="center">
  A maintenance standard for Soroban contract state.
</p>

<p align="center">
  <a href="https://github.com/ledgerkeep/ledgerkeep-core/actions"><img alt="CI" src="https://github.com/ledgerkeep/ledgerkeep-core/actions/workflows/ci.yml/badge.svg"></a>
  <img alt="License: Apache-2.0" src="https://img.shields.io/badge/License-Apache_2.0-blue.svg">
  <img alt="soroban-sdk 27.0.4" src="https://img.shields.io/badge/soroban--sdk-27.0.4-orange">
</p>

---

## What this is

Every entry a Soroban contract stores has a time-to-live measured in ledgers. When it runs out,
persistent data is archived and has to be restored at a cost, and a contract's instance and code
become unusable until someone restores them. Since Protocol 23 the restore is automatic during a
normal call, so nothing is lost forever — but the cost is real, and there is no way for a contract
to watch its own entries approach expiry, because a contract cannot read its own TTL at runtime.

Today every protocol handles this privately, with a script someone remembers to run. There is no
shared tooling, because there is no standard way for a contract to say which of its stored keys are
the ones that matter.

LedgerKeep is that standard. It has three parts:

- **`maintainable`** — a Rust crate a contract adopts with one macro. It generates a permissionless
  `extend_all` function that extends the contract's own critical keys, and records who did it and
  when.
- **`registry`** — a permissionless on-chain directory. A contract publishes the list of keys it
  wants maintained, so external tooling can find work to do without reading the contract's source.
- **`rent_vault`** — a funding contract. A protocol pre-funds it, and it pays a tip to whoever
  performs the maintenance.

Observation of TTL — knowing *when* a key is close to expiry — happens off-chain in a companion
command-line keeper, [`ledgerkeep-cli`](https://github.com/ledgerkeep/ledgerkeep-cli). This
repository is the on-chain half.

## How the pieces fit

```
   ┌───────────────────────┐
   │       registry        │  who opted in, and which keys they care about
   └───────────┬───────────┘
               │  1. read manifests
               │
   ┌───────────┴───────────┐
   │   off-chain keeper    │  ledgerkeep-cli — the only piece that can see TTL
   └────┬─────────────┬────┘
        │             │
        │ 2.          │ 3. claim(target, keeper)
        │ extend_all  │
        ▼             ▼
   ┌──────────┐   ┌──────────────┐
   │   your   │   │  rent_vault  │  4. cross-calls lk_state() to confirm the
   │ contract │◄──┤              │     work happened and who did it, then pays
   └──────────┘   └──────────────┘
```

A keeper reads the registry to find contracts and their declared keys, calls `extend_all` on one,
then claims a tip from that contract's vault. The vault confirms the work happened and who did it
by cross-calling `lk_state()` on the maintained contract.

**One honest limit worth stating up front:** the vault can prove maintenance *occurred* and *who
did it*, but not that it was *necessary* — no contract can read TTL at runtime to check. It caps
payouts at one tip per interval, set by the vault owner. It is a rate limit, not a proof of need.
The keys a contract publishes to the registry are also advisory: nothing on-chain forces them to
match the keys the contract's macro actually extends. The CLI checks for that drift by simulating
`extend_all` and comparing observed TTLs.

## Deployments

Testnet. Verify each on the explorer before trusting it.

| Contract | ID | Explorer |
|---|---|---|
| registry | `CB7K56KG3KHC43FROV534M55FMVGBW24NUFQSXSRMH7OS54242GFYMGN` | [view](https://stellar.expert/explorer/testnet/contract/CB7K56KG3KHC43FROV534M55FMVGBW24NUFQSXSRMH7OS54242GFYMGN) |
| rent_vault | `CACRDSINFHJFMH4ADZO3PA376VZQW7PXPWCZAFIFFEB5X4ZJLFJZMUTF` | [view](https://stellar.expert/explorer/testnet/contract/CACRDSINFHJFMH4ADZO3PA376VZQW7PXPWCZAFIFFEB5X4ZJLFJZMUTF) |
| long_escrow (example) | `CASBZNG6KRKZYRQ22TVOGEYSRDIV7QSCJDFIMSII5LA7XXKIUXOX6NZ6` | [view](https://stellar.expert/explorer/testnet/contract/CASBZNG6KRKZYRQ22TVOGEYSRDIV7QSCJDFIMSII5LA7XXKIUXOX6NZ6) |

> These exist so you can verify the contracts behave as described. Do not point your own deployment
> at them: they were deployed by a throwaway identity that no longer exists, so the escrow's roles
> and the vault's owner are unrecoverable, and testnet is periodically reset. Run
> `scripts/deploy_testnet.sh` to get your own.

## Adopting the standard

A contract adopts LedgerKeep with one macro call and two thin wrappers. This is the whole
integration:

```rust
use maintainable::{MaintainableError, MaintenanceState};

maintainable::impl_maintainable!(
    threshold: 100_000,
    extend_to: 500_000,
    persistent: [DataKey::Balance, DataKey::Milestones],
);

#[contractimpl]
impl MyContract {
    pub fn extend_all(env: Env, keeper: Address) -> Result<u32, MaintainableError> {
        __lk_extend_all(&env, keeper)
    }

    pub fn lk_state(env: Env) -> Result<MaintenanceState, MaintainableError> {
        maintainable::lk_state(&env)
    }
}
```

The keys are inlined at compile time, so there is no on-chain key list to store or spoof.
`extend_all` is permissionless: anyone can call it, and the worst a caller can do is pay a fee to
extend a key that did not need it.

Appearing in the registry takes one more function, and it has to live in the contract. Registration
cannot be done from the command line: the registry authorizes by contract address, and no private
key exists for one, so the call must originate inside the contract being registered. See
`register_with` in `examples/long_escrow` for the pattern.

That example is a fixture for demonstrating the standard, not a production escrow — the Stellar
ecosystem already has mature escrow infrastructure, and this does not compete with it.

## Quick start

Prerequisites: Rust 1.93.0 and the Stellar CLI. The pinned toolchain and the `wasm32v1-none` target
are declared in `rust-toolchain.toml`, so rustup installs both on the first build.

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install the Stellar CLI as a prebuilt binary
curl -sSfL https://github.com/stellar/stellar-cli/releases/download/v27.1.0/stellar-cli-27.1.0-x86_64-unknown-linux-gnu.tar.gz \
  | sudo tar xz -C /usr/local/bin stellar

# Create an identity to sign with. `testnet` is a built-in network name.
stellar keys generate alice --network testnet --fund
```

> Do not run `cargo install --locked stellar-cli` from inside this repository. It compiles the CLI
> from source under this repository's pinned toolchain, which takes 10–20 minutes and ties the
> CLI's minimum Rust version to ours. The prebuilt binary has neither problem.

Build and test:

```bash
stellar contract build      # builds all contracts to wasm32v1-none
cargo test                  # runs the test suite
```

> Build with `stellar contract build`, not `cargo build`. `wasm32v1-none` is the only target the
> Soroban runtime supports, and the Stellar CLI applies build settings the runtime requires. It
> also refuses Rust 1.81, 1.82, 1.83 and 1.91.0, which have known bad wasm codegen — hence the
> 1.93.0 pin.

Deploy to testnet:

```bash
./scripts/deploy_testnet.sh alice testnet
# paste the export block it prints into your shell, then:
./scripts/init_testnet.sh alice testnet
```

`deploy_testnet.sh` builds first, deploys registry → rent_vault → long_escrow in dependency order,
and prints every contract ID in one block at the end. `init_testnet.sh` initializes the vault and
the escrow, registers the escrow against the registry, opens a vault pointing at it, and then reads
the registry back so you can see what landed.

## Repository layout

```
crates/maintainable      the standard: macro, state, client
contracts/registry       permissionless manifest directory
contracts/rent_vault     maintenance funding and payouts
examples/long_escrow     a contract that adopts the standard
scripts/                 testnet deploy and init
```

## Contributing

Issues and pull requests are welcome. [CONTRIBUTING.md](CONTRIBUTING.md) has the build steps, the
commit format, and what a pull request needs to pass.

The short version: commits follow [Conventional Commits](https://www.conventionalcommits.org/) —
`type(scope): description`, one logical change per commit. `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, `cargo test --all` and `stellar contract build` must
all be clean before anything merges; CI enforces all four.

The contracts are not audited. Do not use them on mainnet with real funds without your own review.
To report a vulnerability, see [SECURITY.md](SECURITY.md) — please do not open a public issue for
one.

## Maintainers

| Name | Role | GitHub | Contact |
|---|---|---|---|
| Dillon Ofili | Maintainer | [@0dillon](https://github.com/0dillon) | `<CONTACT>` |

## Contributors

<!-- Renders contributor avatars once the repo has any. -->
<a href="https://github.com/ledgerkeep/ledgerkeep-core/graphs/contributors">
  <img src="https://contrib.rocks/image?repo=ledgerkeep/ledgerkeep-core" />
</a>

## License

Apache-2.0. See [LICENSE](LICENSE).
