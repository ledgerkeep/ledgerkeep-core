//! Storage accessors and index helpers.
//!
//! `Entry`, `Slot`, and `Index` live in persistent storage. `LK_COUNT` — the
//! number of registered contracts — lives in instance storage, so it shares the
//! instance entry's TTL and is covered by the registry's own maintenance.

use soroban_sdk::{symbol_short, Address, Env, Symbol};

use crate::types::{DataKey, RegistryEntry};

/// Number of registered contracts.
const COUNT: Symbol = symbol_short!("LK_COUNT");

pub fn get_count(env: &Env) -> u32 {
    env.storage().instance().get(&COUNT).unwrap_or(0)
}

pub fn set_count(env: &Env, count: u32) {
    env.storage().instance().set(&COUNT, &count);
}

pub fn has_entry(env: &Env, contract: &Address) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::Entry(contract.clone()))
}

pub fn get_entry(env: &Env, contract: &Address) -> Option<RegistryEntry> {
    env.storage()
        .persistent()
        .get(&DataKey::Entry(contract.clone()))
}

pub fn set_entry(env: &Env, contract: &Address, entry: &RegistryEntry) {
    env.storage()
        .persistent()
        .set(&DataKey::Entry(contract.clone()), entry);
}

pub fn remove_entry(env: &Env, contract: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::Entry(contract.clone()));
}

pub fn get_slot(env: &Env, contract: &Address) -> Option<u32> {
    env.storage()
        .persistent()
        .get(&DataKey::Slot(contract.clone()))
}

pub fn set_slot(env: &Env, contract: &Address, slot: u32) {
    env.storage()
        .persistent()
        .set(&DataKey::Slot(contract.clone()), &slot);
}

pub fn remove_slot(env: &Env, contract: &Address) {
    env.storage()
        .persistent()
        .remove(&DataKey::Slot(contract.clone()));
}

pub fn get_index(env: &Env, index: u32) -> Option<Address> {
    env.storage().persistent().get(&DataKey::Index(index))
}

pub fn set_index(env: &Env, index: u32, contract: &Address) {
    env.storage()
        .persistent()
        .set(&DataKey::Index(index), contract);
}

pub fn remove_index(env: &Env, index: u32) {
    env.storage().persistent().remove(&DataKey::Index(index));
}
