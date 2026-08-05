# Contributing to LedgerKeep

Thanks for taking a look. This document covers how to get the repository building, what a commit
has to look like, and what happens to a pull request.

Read [SECURITY.md](SECURITY.md) before reporting anything that looks like a vulnerability. Do not
open a public issue for one.

## Getting set up

Prerequisites are Rust 1.93.0 and the Stellar CLI 27.1.0. The toolchain version and the
`wasm32v1-none` target are declared in `rust-toolchain.toml`, so rustup installs both the first
time you build.

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install the Stellar CLI as a prebuilt binary
curl -sSfL https://github.com/stellar/stellar-cli/releases/download/v27.1.0/stellar-cli-27.1.0-x86_64-unknown-linux-gnu.tar.gz \
  | sudo tar xz -C /usr/local/bin stellar
```

Do not run `cargo install --locked stellar-cli` from inside this repository. It compiles the CLI
from source under our pinned toolchain, which takes 10–20 minutes and ties the CLI's minimum Rust
version to ours.

## Building and testing

```bash
stellar contract build      # compiles all contracts to wasm32v1-none
cargo test --all            # runs the full suite
```

Build with `stellar contract build`, never `cargo build`. `wasm32v1-none` is the only target the
Soroban runtime supports, and the CLI applies build settings the runtime requires. The CLI also
refuses Rust 1.81, 1.82, 1.83 and 1.91.0, which have known bad wasm codegen — that is why the pin
is 1.93.0.

Before you push, all four of these must be clean:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
stellar contract build
```

CI runs exactly these, in this order.

Tests are `#[cfg(test)]` modules using `Env::default()` and generated test contracts. Advancing
ledgers in a test is done through `env.ledger()`. Soroban writes snapshots under `test_snapshots/`;
those are gitignored and regenerate on every run, so do not commit them.

## Commits

[Conventional Commits](https://www.conventionalcommits.org/), `type(scope): description`.

- **Types:** `feat`, `fix`, `test`, `docs`, `chore`, `refactor`, `ci`
- **Scopes:** `workspace`, `maintainable`, `registry`, `vault`, `example`, `scripts`

The scope is the part of the repository the change belongs to, not the file you edited. A change to
`crates/maintainable` is `maintainable`; a change to `contracts/rent_vault` is `vault`.

One logical unit per commit — one function, one type, one block of tests. A commit that adds a
function and its tests and also renames something unrelated should be two or three commits.

Stage files by path. Do not use `git add .`; it is how gitignored-but-not-yet-ignored artifacts and
half-finished work end up in history.

Every commit must compile. Run `cargo check` before committing, and `cargo test` before committing
anything that touches logic.

Examples from this repository's history:

```
feat(registry): add deregister with swap-remove
test(vault): claim path, keeper/timing rejections, balance accounting
feat(example): add register_with cross-call to registry
ci(workspace): use prebuilt stellar-cli and move off blocklisted Rust 1.91.0
```

## Pull requests

Work happens on a branch and lands through a pull request. There are no direct pushes to `main`.

1. Fork, or branch if you have write access. Branch names are not enforced; `type/short-description`
   is what we use.
2. Make your changes as a series of scoped commits.
3. Confirm the four checks above pass locally.
4. Open a pull request. If it closes an issue, write `Closes #123` in the body.
5. CI must be green and the pull request must be approved before it merges.

Describe what changed and why. If you made a design decision that a reader might disagree with, say
what you decided and what the alternative was — that is more useful than a summary of the diff.

## Code standards

These are enforced by review, not by a linter:

- `#![no_std]` in every contract crate.
- No `unwrap()`, `expect()`, `panic!`, `assert!` or `unreachable!` outside `#[cfg(test)]`.
- No floats. Amounts are `i128` in stroops; rates are basis points.
- No unbounded iteration over storage. Every loop is bounded before it is entered.
- `overflow-checks = true` stays on in the release profile.
- Rustdoc on every public function: what it does, who is allowed to call it, and what it errors
  with.
- Errors are `#[contracterror]` enums with explicit codes. The ranges are fixed and must not be
  reused: registry `1xx`, rent_vault `2xx`, maintainable `3xx`, escrow `4xx`.
- Events use `#[contractevent]`, not `env.events().publish()`.
- Write contract state before any external token call.
- `require_auth()` is the first statement in a mutating function.

Two things are structurally impossible in Soroban and no amount of review will let them through: a
contract cannot read its own entries' TTL at runtime, and `keys_xdr` is never decoded on-chain. If
a change depends on either, it is going in the wrong direction.

`examples/long_escrow` is a fixture that demonstrates adoption. It is deliberately not a production
escrow, and pull requests that add escrow features to it will be declined.

## Reporting bugs

Open an issue with the version or commit you are on, what you ran, what you expected, and what
happened. For on-chain behaviour, include the contract ID, the network, and the transaction hash —
those make a report reproducible in a way a description cannot.
