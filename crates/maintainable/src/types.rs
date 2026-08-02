//! Data types for the maintenance standard.

use soroban_sdk::{contracttype, Address};

/// Record of the most recent maintenance run.
///
/// Written by the generated maintenance function and read on-chain by a rent
/// vault (via cross-contract call) to decide whether a keeper has earned a tip.
///
/// This is the only stored type. Thresholds and the key list are compile-time
/// macro parameters, not stored state.
#[contracttype]
#[derive(Clone)]
pub struct MaintenanceState {
    pub last_maintained: u32,
    pub last_keeper: Address,
}
