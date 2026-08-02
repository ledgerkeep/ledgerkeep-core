#!/usr/bin/env bash
#
# Deploy the LedgerKeep contracts in dependency order: registry, rent_vault,
# long_escrow. Prints every contract ID in one block at the end.
#
# Usage:
#   scripts/deploy_testnet.sh <source-identity> <network>
#
#   source-identity   a name from `stellar keys ls`, e.g. alice
#   network           a name from `stellar network ls`, e.g. testnet
#
# No secret key appears in this script or its output. The stellar CLI resolves
# the identity from its own key store.

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
    echo "install it from https://github.com/stellar/stellar-cli/releases" >&2
    exit 1
fi

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

WASM_DIR="target/wasm32v1-none/release"

# The registry's own maintenance manifest. One key: the XDR encoding of
# ScVal::LedgerKeyContractInstance (0x00000014), the ledger key for a contract's
# instance entry. The registry maintains only its instance — its
# `impl_maintainable!` declares an empty persistent list — so one key is the
# whole manifest. It cannot be empty; the constructor rejects that with
# EmptyManifest.
REGISTRY_KEYS_XDR='["00000014"]'

# These must match the values compiled into the registry's `impl_maintainable!`.
# The manifest is advisory and nothing on-chain enforces a match, so a mismatch
# here would publish terms the contract does not actually apply.
REGISTRY_THRESHOLD=100000
REGISTRY_EXTEND_TO=500000

deploy() {
    local wasm="$1"
    shift
    stellar contract deploy \
        --wasm "$wasm" \
        --source-account "$SOURCE" \
        --network "$NETWORK" \
        "$@"
}

# Deploying a stale build is easy to do and hard to notice, so build first.
echo "==> Building contracts" >&2
stellar contract build

echo "==> Deploying registry" >&2
REGISTRY_ID="$(deploy "$WASM_DIR/registry.wasm" -- \
    --keys_xdr "$REGISTRY_KEYS_XDR" \
    --threshold "$REGISTRY_THRESHOLD" \
    --extend_to "$REGISTRY_EXTEND_TO")"

echo "==> Deploying rent_vault" >&2
RENT_VAULT_ID="$(deploy "$WASM_DIR/rent_vault.wasm")"

echo "==> Deploying long_escrow" >&2
LONG_ESCROW_ID="$(deploy "$WASM_DIR/long_escrow.wasm")"

cat <<EOF

# LedgerKeep deployed to $NETWORK. Copy this block into your shell;
# scripts/init_testnet.sh reads these variables.
export LK_NETWORK=$NETWORK
export LK_SOURCE=$SOURCE
export LK_REGISTRY_ID=$REGISTRY_ID
export LK_RENT_VAULT_ID=$RENT_VAULT_ID
export LK_LONG_ESCROW_ID=$LONG_ESCROW_ID
EOF
