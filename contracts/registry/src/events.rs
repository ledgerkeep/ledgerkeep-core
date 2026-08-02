//! Registry events.
//!
//! Each `#[contractevent]` prepends a fixed topic equal to the struct name in
//! lower snake case (`registered`, `updated`, `deregistered`), followed by the
//! `contract` topic. Remaining fields form the event data.

use soroban_sdk::{contractevent, Address};

/// Emitted when a contract is registered.
///
/// Topics: `(registered, contract)`. Data: `(key_count, ledger)`.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Registered {
    #[topic]
    pub contract: Address,
    pub key_count: u32,
    pub ledger: u32,
}

/// Emitted when a contract's manifest is updated.
///
/// Topics: `(updated, contract)`. Data: `(key_count, ledger)`.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Updated {
    #[topic]
    pub contract: Address,
    pub key_count: u32,
    pub ledger: u32,
}

/// Emitted when a contract is deregistered.
///
/// Topics: `(deregistered, contract)`. Data: `(ledger)`.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Deregistered {
    #[topic]
    pub contract: Address,
    pub ledger: u32,
}
