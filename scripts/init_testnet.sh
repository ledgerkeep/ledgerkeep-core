#!/usr/bin/env bash
#
# Initialize the LedgerKeep contracts deployed by scripts/deploy_testnet.sh, then
# read the registry back to confirm what landed on-chain.
#
# Usage:
#   scripts/init_testnet.sh <source-identity> <network>
#
# Reads the contract IDs from the environment. Run the export block that
# deploy_testnet.sh prints before running this:
#
#   LK_REGISTRY_ID, LK_RENT_VAULT_ID, LK_LONG_ESCROW_ID
#
# Optional overrides, all defaulting to the source identity's own address:
#
#   LK_SELLER, LK_APPROVER    Stored as escrow configuration. Any address works;
#                             neither authorizes anything this script calls.
#   LK_BUYER, LK_VAULT_OWNER  Both have to authorize a call below, so both must
#                             be the source identity's own address. The script
#                             checks this before it changes anything on-chain.
#
# No secret key appears in this script or its output.

set -euo pipefail

usage() {
    echo "usage: $(basename "$0") <source-identity> <network>" >&2
    echo "example: $(basename "$0") alice testnet" >&2
}

if [ "$#" -ne 2 ]; then
    usage
    exit 2
fi

SOURCE="$1"
NETWORK="$2"

if ! command -v stellar >/dev/null 2>&1; then
    echo "error: the stellar CLI is not on PATH." >&2
    exit 1
fi

for var in LK_REGISTRY_ID LK_RENT_VAULT_ID LK_LONG_ESCROW_ID; do
    if [ -z "${!var:-}" ]; then
        echo "error: $var is not set." >&2
        echo "run scripts/deploy_testnet.sh first and paste the export block it prints." >&2
        exit 1
    fi
done

# The escrow's maintenance manifest: the three ledger keys its
# `impl_maintainable!` extends, XDR-encoded as ScVal.
#
#   00000014                            ScVal::LedgerKeyContractInstance — the
#                                       instance entry, which holds Config
#   0000001000000001…42616c616e636500   ScVal::Vec([Symbol("Balance")])
#   0000001000000001…4d696c6573746f…    ScVal::Vec([Symbol("Milestones")])
#
# Nothing on-chain checks that these describe what the contract actually
# extends. They are kept in step with the contract by
# `manifest()` in examples/long_escrow/src/test.rs, which asserts against the
# same three values.
ESCROW_KEYS_XDR='["00000014","0000001000000001000000010000000f0000000742616c616e636500","0000001000000001000000010000000f0000000a4d696c6573746f6e65730000"]'

# Must match the values compiled into the escrow's `impl_maintainable!`.
ESCROW_THRESHOLD=100000
ESCROW_EXTEND_TO=500000

# One milestone of 1 XLM and one of 2 XLM, in stroops.
ESCROW_AMOUNTS='["10000000","20000000"]'

# What a keeper is paid per maintenance run, in stroops, and how often a claim
# is allowed.
#
# The interval is the escrow's real maintenance period, which is the gap between
# the two macro values and not `threshold` on its own: `extend_all` pushes the
# TTL up to `extend_to` and then does nothing until it falls back to
# `threshold`. With 500000 and 100000 that is 400000 ledgers. Deriving it here
# keeps it right if the escrow's macro values change.
#
# Setting it shorter overpays, and nothing on-chain catches that. The vault caps
# claims at one per interval, and `__lk_extend_all` records a maintenance run
# whether or not it actually extended anything, so a claim made inside the real
# period still passes all three of the vault's checks.
VAULT_TIP=1000000
VAULT_INTERVAL=$((ESCROW_EXTEND_TO - ESCROW_THRESHOLD))

invoke() {
    local id="$1"
    shift
    stellar contract invoke \
        --id "$id" \
        --source-account "$SOURCE" \
        --network "$NETWORK" \
        -- "$@"
}

SOURCE_ADDRESS="$(stellar keys public-key "$SOURCE")"
BUYER="${LK_BUYER:-$SOURCE_ADDRESS}"
SELLER="${LK_SELLER:-$SOURCE_ADDRESS}"
APPROVER="${LK_APPROVER:-$SOURCE_ADDRESS}"
VAULT_OWNER="${LK_VAULT_OWNER:-$SOURCE_ADDRESS}"

# `stellar contract invoke` can only satisfy `require_auth()` for the identity it
# signs with, and that is the source account. Two of the overrides name addresses
# that have to authorize a call further down:
#
#   LK_BUYER        authorizes `register_with`, via config.buyer.require_auth()
#   LK_VAULT_OWNER  authorizes `open`, via owner.require_auth()
#
# Pointing either at an address this identity does not hold the key to cannot
# work from a single-signer script. That is checked here, before the first call
# that changes anything, because the failure would otherwise land partway
# through: `set -e` would abort at `register_with` with the escrow already
# initialized, and initialize is one-shot, so a re-run hits AlreadyInitialized
# and the only way forward is a fresh deploy.
#
# LK_SELLER and LK_APPROVER are deliberately not checked. They are stored as
# escrow configuration and authorize nothing this script calls, so any address
# is valid for them.
for pair in "LK_BUYER:$BUYER" "LK_VAULT_OWNER:$VAULT_OWNER"; do
    name="${pair%%:*}"
    address="${pair#*:}"
    if [ "$address" != "$SOURCE_ADDRESS" ]; then
        echo "error: $name is set to" >&2
        echo "         $address" >&2
        echo "       but this script signs every call as '$SOURCE'" >&2
        echo "         $SOURCE_ADDRESS" >&2
        echo >&2
        echo "$name has to authorize one of the calls below, and a single-signer" >&2
        echo "invoke cannot produce a signature for an address it does not hold" >&2
        echo "the key to. Either unset $name, or re-run this script with the" >&2
        echo "identity that owns that address as the source." >&2
        echo >&2
        echo "Nothing has been changed on-chain." >&2
        exit 1
    fi
done

# Native XLM as the tip asset, via its Stellar Asset Contract.
TOKEN_ID="$(stellar contract id asset --asset native --network "$NETWORK")"

echo "==> Vault: set the tip asset to native XLM ($TOKEN_ID)" >&2
invoke "$LK_RENT_VAULT_ID" initialize --token "$TOKEN_ID"

echo "==> Escrow: initialize" >&2
invoke "$LK_LONG_ESCROW_ID" initialize \
    --buyer "$BUYER" \
    --seller "$SELLER" \
    --approver "$APPROVER" \
    --token "$TOKEN_ID" \
    --amounts "$ESCROW_AMOUNTS"

# The registry authorizes by contract address, and no account holds the key to a
# contract address, so this cannot be a direct CLI call to `register`. It has to
# go through the escrow, which presents itself as the registering contract.
echo "==> Escrow: publish its manifest to the registry" >&2
invoke "$LK_LONG_ESCROW_ID" register_with \
    --registry "$LK_REGISTRY_ID" \
    --keys_xdr "$ESCROW_KEYS_XDR" \
    --threshold "$ESCROW_THRESHOLD" \
    --extend_to "$ESCROW_EXTEND_TO"

echo "==> Vault: open a vault that pays for maintaining the escrow" >&2
invoke "$LK_RENT_VAULT_ID" open \
    --target "$LK_LONG_ESCROW_ID" \
    --owner "$VAULT_OWNER" \
    --tip "$VAULT_TIP" \
    --interval "$VAULT_INTERVAL"

echo
echo "==> Read-back: registry contents" >&2

echo "--- count (expect 2: the registry's own constructor entry, plus the escrow)"
invoke "$LK_REGISTRY_ID" count

echo "--- get: registry (self-registered in its constructor)"
invoke "$LK_REGISTRY_ID" get --contract "$LK_REGISTRY_ID"

echo "--- get: long_escrow (registered by register_with above)"
invoke "$LK_REGISTRY_ID" get --contract "$LK_LONG_ESCROW_ID"

echo "--- get_vault: long_escrow"
invoke "$LK_RENT_VAULT_ID" get_vault --target "$LK_LONG_ESCROW_ID"
