# ledgerkeep-core

A maintenance standard for Soroban contract state.

Every Soroban ledger entry has a time-to-live measured in ledgers. When it runs
out, persistent entries are archived and must be restored at cost; contract
instances and code become unusable until restored. `ledgerkeep-core` gives
protocols a shared way to declare which stored keys are critical and to pay for
keeping them alive.

## The core constraint

A Soroban contract cannot read the TTL of its own entries at runtime. On-chain
code can extend TTL and record that it did so; it cannot observe TTL. All
observation happens off-chain via RPC in the companion `ledgerkeep-cli`
repository.

## Components

- **`crates/maintainable`** — a library any contract implements to expose
  permissionless self-maintenance (`lk_declare`, `extend_all`, state accessors).
- **`contracts/registry`** — a permissionless, admin-free on-chain directory
  where protocols publish their critical-key manifests.
- **`contracts/rent_vault`** — a vault that pre-funds maintenance and pays
  whoever performs it, rate-limited per interval.
- **`examples/long_escrow`** — a fixture showing the standard on a contract
  whose state must outlive default TTL. An example, not a product.

## Tech stack

| Item | Value |
|---|---|
| Rust edition | 2021 |
| Minimum Rust | 1.93.0 |
| Build target | `wasm32v1-none` |
| Build command | `stellar contract build` |
| soroban-sdk | 27.0.4 |

Build contracts with `stellar contract build`, never `cargo build`. Run tests
with `cargo test`.

## Status

Scaffold in place. Component implementation follows the build sequence in the
project specification.

## License

Apache-2.0. See [LICENSE](LICENSE).
