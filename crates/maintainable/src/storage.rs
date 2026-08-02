//! Reserved instance-storage key and typed accessors.
//!
//! State lives in the **host contract's** instance storage under one reserved
//! symbol. A host contract must not reuse this symbol for its own data.

use soroban_sdk::{symbol_short, Env, Symbol};

use crate::types::MaintenanceState;

/// Last maintenance record.
const STATE: Symbol = symbol_short!("LK_STATE");

pub fn has_state(env: &Env) -> bool {
    env.storage().instance().has(&STATE)
}

pub fn get_state(env: &Env) -> Option<MaintenanceState> {
    env.storage().instance().get(&STATE)
}

pub fn set_state(env: &Env, state: &MaintenanceState) {
    env.storage().instance().set(&STATE, state);
}
