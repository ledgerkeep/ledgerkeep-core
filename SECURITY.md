# Security Policy

## These contracts are unaudited

**Do not use them on mainnet with real funds.**

No third party has reviewed this code. It has tests and it has been exercised on testnet, and
neither of those is an audit. The contracts handle token balances and pay out tips, so a bug here
loses money.

`examples/long_escrow` is a fixture for demonstrating adoption of the standard. It is not a
production escrow and should not be treated as one under any circumstances.

If you deploy any of this to mainnet, you are responsible for having it reviewed first.

## Reporting a vulnerability

Email **<CONTACT>**. Do not open a public issue.

Include whatever you have:

- What the problem is and which contract it is in.
- The steps or transaction sequence that trigger it.
- What an attacker gets out of it — funds, a denial of service, corrupted state.
- Contract ID, network and transaction hash if you saw it on-chain.
- A test case, if you wrote one.

You will get an acknowledgement within 72 hours. Expect an assessment of whether the report is
valid within 7 days, and an estimate of when a fix will land if it is.

Please give us 90 days before publishing. If a fix ships sooner, we will say so and you are free to
publish then. If we go quiet on you, publish — silence from a maintainer is not a reason to sit on
a real vulnerability.

Reporters are credited in the release notes for the fix unless they ask not to be. There is no
bug bounty.

## Scope

**In scope** — anything in this repository that runs on-chain or produces something that does:

- `crates/maintainable` — the macro, its generated code, and the stored maintenance state.
- `contracts/registry` — registration, authorization, the swap-remove index, pagination.
- `contracts/rent_vault` — vault accounting, the three-condition claim check, token transfers.
- `examples/long_escrow` — as a fixture. Report bugs in it that reveal a flaw in the standard
  itself. Bugs that are only about escrow business logic are out of scope, since the example is not
  meant to be deployed.
- `scripts/` — anything that would cause a deployer to lose funds or hand over authority.
- The CI workflow, where a compromise would affect what gets built.

**Out of scope:**

- The Soroban runtime, `soroban-sdk`, and the Stellar network itself. Report those to
  [Stellar](https://github.com/stellar/stellar-protocol/security).
- The testnet deployments listed in the README. They were deployed by a throwaway identity that no
  longer exists, and testnet is periodically reset. They are there so you can read the contracts'
  behaviour, and they hold nothing worth attacking.
- Anything requiring a compromised private key or a malicious signer who already holds the relevant
  authority.
- The two limits below, which are design decisions we have already documented.

## Known limits, already documented

These are not vulnerabilities. They follow from a constraint in Soroban: **a contract cannot read
its own entries' TTL at runtime.** A contract can extend and record; it can never observe.

**The vault cannot prove maintenance was necessary.** `rent_vault` verifies that maintenance
happened and who did it, by cross-calling `lk_state()`. It cannot verify the keys were close to
expiring, because nothing on-chain can. The `interval` cap on claims is a rate limit, not a proof
of need. A keeper who calls `extend_all` on a schedule earns tips whether or not the work was
needed.

**Registry manifests can drift.** The `keys_xdr` a contract publishes is advisory metadata. Nothing
on-chain forces it to match the keys the contract's compiled `impl_maintainable!` actually extends,
and it is never decoded on-chain. `ledgerkeep-cli` detects drift by simulating `extend_all` and
comparing observed TTLs.

If you find a way to make either of these worse than described — for example, draining a vault
faster than one tip per interval — that is in scope and we want to hear about it.
