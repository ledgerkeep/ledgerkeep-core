#!/usr/bin/env bash
#
# Create the planned LedgerKeep issues on GitHub in one run.
#
# Usage:
#   scripts/create_issues.sh [--dry-run] [--yes] [owner/repo]
#
# Defaults to the repository the current directory belongs to. Creates the
# labels it uses first, so a fresh repository does not need them set up by hand.
#
# This writes to a public issue tracker and there is no bulk undo. Run it with
# --dry-run first.

set -euo pipefail

DRY_RUN=0
ASSUME_YES=0
REPO=""

usage() {
    echo "usage: $(basename "$0") [--dry-run] [--yes] [owner/repo]" >&2
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --dry-run) DRY_RUN=1 ;;
        --yes | -y) ASSUME_YES=1 ;;
        -h | --help)
            usage
            exit 0
            ;;
        -*)
            echo "error: unknown option $1" >&2
            usage
            exit 2
            ;;
        *) REPO="$1" ;;
    esac
    shift
done

# gh is only needed to create issues, and to resolve the repository when one was
# not given. A dry run with an explicit repository works without it, so the
# issue text can be reviewed anywhere.
if [ "$DRY_RUN" -eq 0 ] || [ -z "$REPO" ]; then
    if ! command -v gh >/dev/null 2>&1; then
        echo "error: the gh CLI is not on PATH." >&2
        exit 1
    fi

    if ! gh auth status >/dev/null 2>&1; then
        echo "error: gh is not authenticated. run 'gh auth login'." >&2
        exit 1
    fi

    if [ -z "$REPO" ]; then
        REPO="$(gh repo view --json nameWithOwner -q .nameWithOwner)"
    fi
fi

# label name -> "colour|description"
declare -A LABELS=(
    ["complexity: low"]="c2e0c6|An hour or two, no design decisions"
    ["complexity: medium"]="fef2c0|Half a day, some design judgement needed"
    ["complexity: high"]="f9d0c4|Multi-day, changes a public surface"
    ["type: feature"]="a2eeef|Adds behaviour that does not exist yet"
    ["type: bug"]="d73a4a|Existing behaviour is wrong"
    ["type: test"]="bfd4f2|Adds or fixes coverage"
    ["type: docs"]="0075ca|Documentation only"
    ["area: maintainable"]="5319e7|crates/maintainable"
    ["area: registry"]="5319e7|contracts/registry"
    ["area: rent_vault"]="5319e7|contracts/rent_vault"
    ["area: example"]="5319e7|examples/long_escrow"
)

# Conventional-commit scope in the title -> area label suffix.
scope_to_area() {
    case "$1" in
        vault) echo "rent_vault" ;;
        *) echo "$1" ;;
    esac
}

CREATED=0

# issue <title> <complexity> <type>, body on stdin. The area label is derived
# from the scope in the title, so the two cannot drift apart.
issue() {
    local title="$1" complexity="$2" itype="$3"
    local body scope area
    body="$(cat)"

    scope="${title#*\(}"
    scope="${scope%%\)*}"
    area="area: $(scope_to_area "$scope")"

    CREATED=$((CREATED + 1))

    if [ "$DRY_RUN" -eq 1 ]; then
        echo "────────────────────────────────────────────────────────"
        echo "$title"
        echo "labels: $complexity, $itype, $area"
        echo
        echo "$body"
        return
    fi

    echo "==> $title" >&2
    gh issue create \
        --repo "$REPO" \
        --title "$title" \
        --label "$complexity" \
        --label "$itype" \
        --label "$area" \
        --body "$body"
}

# Every issue body ends with this. The stack is the same across the repository.
STACK_RUST=$(
    cat <<'EOF'
### Tech Stack

Rust (edition 2021, toolchain 1.93.0 pinned in `rust-toolchain.toml`), `soroban-sdk` 27.0.4,
target `wasm32v1-none`. Build with `stellar contract build`, never `cargo build`. Tests are
`#[cfg(test)]` modules on `Env::default()`; run them with `cargo test --all`.
EOF
)

if [ "$DRY_RUN" -eq 0 ]; then
    echo "About to create issues on $REPO." >&2
    if [ "$ASSUME_YES" -eq 0 ]; then
        read -r -p "Continue? [y/N] " reply
        case "$reply" in
            y | Y | yes | YES) ;;
            *)
                echo "aborted." >&2
                exit 1
                ;;
        esac
    fi

    echo "==> Ensuring labels exist" >&2
    for name in "${!LABELS[@]}"; do
        IFS='|' read -r colour description <<<"${LABELS[$name]}"
        gh label create "$name" \
            --repo "$REPO" \
            --color "$colour" \
            --description "$description" \
            --force >/dev/null
    done
fi

# ───────────────────────────── maintainable ─────────────────────────────

issue "feat(maintainable): support temporary storage keys in impl_maintainable" \
    "complexity: medium" "type: feature" <<EOF
### Summary

\`impl_maintainable!\` takes an \`instance\` extension and a \`persistent:\` key list
(\`crates/maintainable/src/macros.rs\`). Temporary storage has a TTL too, and a contract that keeps
anything meaningful there has no way to declare it. Adding an optional \`temporary:\` list makes the
macro cover all three tiers.

Temporary entries differ from persistent ones in a way the macro has to respect: when a temporary
entry expires it is deleted outright, not archived, and Protocol 23's automatic restore does not
bring it back. Extending a key that no longer exists must not abort the whole run.

### Acceptance Criteria

- [ ] \`impl_maintainable!\` accepts an optional \`temporary: [...]\` list alongside \`persistent:\`.
- [ ] Omitting \`temporary:\` compiles and behaves exactly as it does today.
- [ ] The generated \`__lk_extend_all\` extends temporary keys with the same threshold and
      \`extend_to\` as the rest.
- [ ] The returned count includes temporary keys that were extended.
- [ ] A test covers a contract declaring both tiers, and a test covers a declared temporary key that
      has already been deleted.
- [ ] The rustdoc on the macro explains the delete-versus-archive difference.

$STACK_RUST
EOF

issue "test(maintainable): cover extend_all when a declared persistent key has no entry" \
    "complexity: low" "type: test" <<EOF
### Summary

The macro extends every key in its inlined list without checking that the entry exists. A contract
can legitimately declare a key it has not written yet — \`long_escrow\` declares \`DataKey::Balance\`
before any deposit. Nothing in the suite pins down what happens on that path, so a future change to
the generated code could turn it into a panic without any test noticing.

Find out what the host actually does for \`extend_ttl\` on a missing key, then write the test that
records it.

### Acceptance Criteria

- [ ] A test in \`crates/maintainable/src/test.rs\` calls \`extend_all\` on a contract with a declared
      persistent key that was never written.
- [ ] The test asserts the observed behaviour rather than assuming it — either the call succeeds and
      the key is skipped, or it returns a specific error.
- [ ] The returned count is asserted, so a change in what counts as "extended" fails the test.
- [ ] The macro's rustdoc states the behaviour the test pins down.

$STACK_RUST
EOF

issue "feat(maintainable): record how many keys were extended in MaintenanceState" \
    "complexity: medium" "type: feature" <<EOF
### Summary

\`MaintenanceState\` stores \`last_maintained\` and \`last_keeper\`
(\`crates/maintainable/src/types.rs\`). \`extend_all\` returns the number of keys it extended, but that
number is thrown away — a cross-contract reader only sees that *something* happened.

\`rent_vault\` pays a tip based on \`lk_state()\`. It currently cannot tell a run that extended every
declared key from one that extended none because they were all above the threshold. Storing the
count gives the vault something to check and gives keepers an auditable record.

This changes a stored \`#[contracttype]\`, so it needs a decision about existing deployments — the
testnet contracts hold state in the old shape.

### Acceptance Criteria

- [ ] \`MaintenanceState\` gains a field for the number of keys extended on the last run.
- [ ] \`__lk_extend_all\` writes it.
- [ ] The \`Maintained\` event carries the same count.
- [ ] A test asserts the count is 0 for a no-op run above the threshold, and the full key count for
      a run below it.
- [ ] The compatibility break is written down: what happens when \`lk_state()\` reads an entry
      written by the old layout, and what an adopter has to do about it.

$STACK_RUST
EOF

# ─────────────────────────────── registry ───────────────────────────────

issue "docs(registry): document the keys_xdr encoding a manifest must use" \
    "complexity: low" "type: docs" <<EOF
### Summary

\`RegistryEntry.keys_xdr\` holds XDR-encoded \`ScVal\` ledger keys and is never decoded on-chain. The
repository documents *that* rule but never shows an adopter how to produce the values. Working them
out currently means writing a throwaway test that runs \`ScVal::try_from_val\` and \`to_xdr\` and
prints the hex — which is what produced the three constants in \`scripts/init_testnet.sh\`.

That is a real barrier to adoption: the first thing a new contract has to do is the one thing
nothing explains.

### Acceptance Criteria

- [ ] Documentation covers what each entry in \`keys_xdr\` is: the XDR of the \`ScVal\` form of a
      storage key, hex-encoded.
- [ ] It shows the instance entry case (\`ScVal::LedgerKeyContractInstance\`, \`00000014\`).
- [ ] It shows a \`#[contracttype]\` unit enum variant case and explains why it encodes as
      \`ScVal::Vec([Symbol(name)])\`.
- [ ] It shows a reproducible way to generate the hex for an arbitrary key.
- [ ] The three constants in \`scripts/init_testnet.sh\` are cross-referenced as a worked example.
- [ ] It restates that the manifest is advisory and can drift from the compiled macro keys.

$STACK_RUST
EOF

issue "test(registry): re-register a contract after deregister and check the index" \
    "complexity: low" "type: test" <<EOF
### Summary

\`deregister\` removes an entry by swap-remove: the last index slot is moved into the freed one and
the count drops. There is a test for removing the middle of three entries, but none for registering
again afterwards. That is the path where a stale \`Slot\` or \`Index\` key would surface, and it is the
normal path for a contract that fixes a bad manifest.

### Acceptance Criteria

- [ ] A test registers three contracts, deregisters the middle one, then registers a fourth.
- [ ] It asserts \`count\` is correct at each step.
- [ ] It asserts \`page\` returns every live entry exactly once, with no gaps and no duplicates.
- [ ] It asserts the re-registered contract is reachable through \`get\`.
- [ ] A contract that deregisters and then re-registers itself is covered, since that is the
      manifest-correction path.

$STACK_RUST
EOF

issue "test(registry): page at the limit with maximum-size manifests" \
    "complexity: medium" "type: test" <<EOF
### Summary

\`page\` returns whole \`RegistryEntry\` values, each carrying its full \`keys_xdr\` vector. The guard on
\`limit\` bounds the number of entries but not their size, so a page of entries with large manifests
can exceed a transaction's read limits. A keeper hitting that gets a failure the registry never
warned about.

The point of this issue is to find where the ceiling actually is, not to guess at it.

### Acceptance Criteria

- [ ] A test registers entries with realistically large \`keys_xdr\` vectors and pages through them at
      the maximum permitted \`limit\`.
- [ ] The ledger-read budget the call consumes is measured, not assumed.
- [ ] The result is written into \`page\`'s rustdoc as concrete guidance on choosing \`limit\`.
- [ ] If the maximum \`limit\` can exceed the budget, a follow-up issue is opened for the fix — this
      issue only establishes the number.

$STACK_RUST
EOF

# ────────────────────────────── rent_vault ──────────────────────────────

issue "feat(vault): add close_vault so an owner can remove a vault entry" \
    "complexity: medium" "type: feature" <<EOF
### Summary

\`open\` creates a vault and \`withdraw\` takes funds out, but nothing removes the entry
(\`contracts/rent_vault/src/lib.rs\`). An owner who is finished with a target can drain the balance to
zero and still leaves a \`DataKey::Vault\` entry behind, which they keep paying rent on and which
\`open\` will refuse to replace.

### Acceptance Criteria

- [ ] \`close_vault(env, target)\` exists, authorized by the vault owner.
- [ ] It refuses to close a vault with a non-zero balance, with its own error code in the \`2xx\`
      range, so funds cannot be stranded.
- [ ] It removes the persistent entry rather than zeroing it.
- [ ] After closing, \`get_vault\` returns \`None\` and \`open\` succeeds for the same target.
- [ ] A \`#[contractevent]\` is emitted.
- [ ] Tests cover the happy path, the non-zero-balance rejection, and a close attempted by someone
      who is not the owner.

$STACK_RUST
EOF

issue "docs(vault): explain how to choose interval from threshold and extend_to" \
    "complexity: low" "type: docs" <<EOF
### Summary

A vault's \`interval\` is the minimum number of ledgers between paid claims. Nothing explains how to
pick it, and the obvious guess is wrong.

The maintenance period of a target is \`extend_to - threshold\`, not \`threshold\`. A contract with
\`threshold: 100_000, extend_to: 500_000\` has its TTL pushed to 500,000 ledgers and does not need
attention again until it falls to 100,000 — a gap of 400,000 ledgers. An owner who sets
\`interval\` to \`threshold\` pays for four times as many claims as the target needs. That mistake was
in \`scripts/init_testnet.sh\` until it was fixed by deriving the interval from the two macro values;
the contract's own documentation still says nothing about it.

### Acceptance Criteria

- [ ] \`open\` and \`set_terms\` rustdoc give the relationship as \`extend_to - threshold\` with the
      worked example above.
- [ ] It is stated that the vault cannot read the target's compiled values, so the owner has to
      supply a consistent \`interval\` and nothing on-chain checks it.
- [ ] The existing limit — the vault proves maintenance happened, not that it was needed — is
      linked rather than restated.
- [ ] The README's vault paragraph points at the guidance.

$STACK_RUST
EOF

issue "test(vault): claim when the balance is smaller than the tip" \
    "complexity: low" "type: test" <<EOF
### Summary

\`claim\` verifies three conditions and then pays \`tip\` from the vault balance. The suite covers the
full claim path, the keeper and timing rejections, and balance accounting across fund/claim/
withdraw — but not a vault whose balance has fallen below its own tip.

That is the state every underfunded vault ends in, so whatever it does is behaviour users will hit.
It must not underflow (\`overflow-checks\` is on, so it would trap) and it must not pay out more than
it holds.

### Acceptance Criteria

- [ ] A test funds a vault with less than \`tip\`, performs real maintenance, and claims.
- [ ] The outcome is asserted explicitly — a specific \`2xx\` error, or a partial payout — rather than
      just "does not panic".
- [ ] The vault balance after the attempt is asserted.
- [ ] \`last_claim\` is asserted, so a failed claim cannot silently consume the interval.
- [ ] A test covers a balance of exactly \`tip\`, the boundary case.

$STACK_RUST
EOF

# ──────────────────────────────── example ───────────────────────────────

issue "feat(example): add update_with and deregister_with wrappers" \
    "complexity: medium" "type: feature" <<EOF
### Summary

\`long_escrow\` has \`register_with\`, which lets the contract present its own address to the registry.
The registry also exposes \`update\` and \`deregister\`, and both authorize the same way — so both are
equally impossible to call from the command line, and neither has a wrapper.

The consequence is that registration is one-shot. An escrow registered with a wrong manifest is
stuck with it: \`register\` returns \`AlreadyRegistered\` forever and there is no path to fix it short
of deploying a new contract. Since the example exists to show the pattern an adopter should copy, it
is showing an incomplete one.

### Acceptance Criteria

- [ ] \`update_with\` and \`deregister_with\` exist, gated on \`config.buyer.require_auth()\` like
      \`register_with\`.
- [ ] Both go through the local \`RegistryClient\`; the \`registry\` crate is still not a dependency —
      linking it copies the registry's exported functions into the escrow's wasm.
- [ ] The \`Registrar\` trait gains matching declarations that stay in step with the registry.
- [ ] Tests against the \`fake_registry\` stub cover both, including rejection without buyer auth.
- [ ] The escrow's wasm size before and after is recorded in the pull request.

$STACK_RUST
EOF

issue "test(example): assert register_with rejects a caller who is not the buyer" \
    "complexity: low" "type: test" <<EOF
### Summary

\`register_with_needs_the_buyers_authorization\` calls \`env.set_auths(&[])\` and asserts the call
fails. That proves *some* authorization is required — it does not prove it is the buyer's. The test
would pass unchanged if the gate were \`config.seller.require_auth()\`, or any other address in the
config.

The buyer gate is the thing stopping an arbitrary caller from publishing an arbitrary manifest under
the escrow's address, so it deserves a test that actually pins it.

### Acceptance Criteria

- [ ] A test authorizes a specific non-buyer address — the seller, and a wholly unrelated address —
      and asserts \`register_with\` still fails.
- [ ] A test authorizes the buyer specifically, not "all auths", and asserts it succeeds.
- [ ] Changing the gate to any other address in \`EscrowConfig\` makes at least one test fail. Verify
      this by making the change locally before submitting.
- [ ] The existing \`set_auths(&[])\` test stays; it covers the no-auth case.

$STACK_RUST
EOF

issue "fix(example): check the register_with manifest against the compiled macro keys" \
    "complexity: medium" "type: bug" <<EOF
### Summary

\`register_with_presents_the_escrows_own_address\` asserts \`call.keys_xdr == keys\`, where \`keys\` is
the value the test itself passed in from its \`manifest()\` helper. The assertion compares
\`manifest()\` to \`manifest()\` and holds no matter what the contract's \`impl_maintainable!\` extends.

\`scripts/init_testnet.sh\` states in a comment that its manifest constants are "kept in step with the
contract by \`manifest()\` in \`examples/long_escrow/src/test.rs\`, which asserts against the same three
values." That safety net does not exist. If someone adds a key to the macro, every test still
passes and the published manifest silently goes stale — the exact drift the registry's
documentation warns about, manufactured by our own example.

Also worth fixing while here: \`register_with\` takes \`threshold\` and \`extend_to\` from the caller,
while \`extend_all\` uses the values compiled into the macro. The buyer can publish numbers that do
not match what the contract does.

### Acceptance Criteria

- [ ] A test derives the expected keys from the contract's actual maintenance behaviour rather than
      from the same constant it passes in — for example by extending TTLs through \`extend_all\` and
      checking which entries moved.
- [ ] Adding or removing a key in the escrow's \`impl_maintainable!\` without updating the manifest
      fails the suite. Verify by making the change locally before submitting.
- [ ] The comment in \`scripts/init_testnet.sh\` is either made true or corrected.
- [ ] A decision is recorded on \`threshold\` and \`extend_to\`: either \`register_with\` stops taking
      them and uses the compiled values, or the rustdoc states plainly that the caller can publish
      values the contract does not honour.

$STACK_RUST
EOF

# ─────────────────────────────────────────────────────────────────────────

echo >&2
if [ "$DRY_RUN" -eq 1 ]; then
    echo "dry run: $CREATED issues would be created on $REPO." >&2
else
    echo "created $CREATED issues on $REPO." >&2
fi
