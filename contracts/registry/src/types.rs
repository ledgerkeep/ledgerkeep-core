//! Storage keys and the registry entry type.

use soroban_sdk::{contracttype, Address, Bytes, Vec};

/// Persistent storage keys.
#[contracttype]
pub enum DataKey {
    /// The manifest entry for a registered contract.
    Entry(Address),
    /// The index slot a registered contract occupies, for swap-remove.
    Slot(Address),
    /// The contract address at a given index position.
    Index(u32),
}

/// A published maintenance manifest for one contract.
///
/// `keys_xdr` holds XDR-encoded `ScVal` keys and is **never decoded on-chain**.
/// It exists so `ledgerkeep-cli` can construct ledger keys for
/// `ExtendFootprintTTLOp` without reading each protocol's source. The manifest
/// is advisory: it can drift from the keys a contract's compiled
/// `impl_maintainable!` actually extends, since nothing enforces a match. The
/// CLI detects drift by simulating `extend_all` and diffing observed TTLs.
#[contracttype]
#[derive(Clone)]
pub struct RegistryEntry {
    pub contract: Address,
    pub keys_xdr: Vec<Bytes>,
    pub threshold: u32,
    pub extend_to: u32,
    pub registered: u32,
    pub updated: u32,
}
