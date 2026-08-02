//! Rent vault events.
//!
//! Each `#[contractevent]` prepends a fixed topic equal to the struct name in
//! lower snake case (`opened`, `funded`, `withdrawn`, `terms_set`, `claimed`),
//! followed by the marked `#[topic]` fields.

use soroban_sdk::{contractevent, Address};

/// Emitted when a vault is opened.
///
/// Topics: `(opened, target, owner)`. Data: `(tip, interval)`.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Opened {
    #[topic]
    pub target: Address,
    #[topic]
    pub owner: Address,
    pub tip: i128,
    pub interval: u32,
}

/// Emitted when a vault is funded.
///
/// Topics: `(funded, target, from)`. Data: `(amount, balance)`.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Funded {
    #[topic]
    pub target: Address,
    #[topic]
    pub from: Address,
    pub amount: i128,
    pub balance: i128,
}

/// Emitted when funds are withdrawn from a vault.
///
/// Topics: `(withdrawn, target, to)`. Data: `(amount, balance)`.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Withdrawn {
    #[topic]
    pub target: Address,
    #[topic]
    pub to: Address,
    pub amount: i128,
    pub balance: i128,
}

/// Emitted when a vault's terms change.
///
/// Topics: `(terms_set, target)`. Data: `(tip, interval)`.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TermsSet {
    #[topic]
    pub target: Address,
    pub tip: i128,
    pub interval: u32,
}

/// Emitted when a keeper claims a tip.
///
/// Topics: `(claimed, target, keeper)`. Data: `(tip, ledger)`.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Claimed {
    #[topic]
    pub target: Address,
    #[topic]
    pub keeper: Address,
    pub tip: i128,
    pub ledger: u32,
}
