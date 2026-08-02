//! Storage key and the vault state type.

use soroban_sdk::{contracttype, Address};

/// Persistent storage keys.
#[contracttype]
pub enum DataKey {
    /// The vault for a given maintenance target.
    Vault(Address),
}

/// One vault: a pre-funded balance and the terms for paying keepers.
///
/// All amounts are `i128` in stroops. There are no floats anywhere in this
/// contract.
#[contracttype]
#[derive(Clone)]
pub struct VaultState {
    pub target: Address,
    pub owner: Address,
    pub balance: i128,
    pub tip: i128,
    pub interval: u32,
    pub last_claim: u32,
}
