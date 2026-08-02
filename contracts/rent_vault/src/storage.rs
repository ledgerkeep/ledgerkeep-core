//! Storage accessors.
//!
//! The tip token address lives in instance storage under `LK_TOKEN`. Each
//! vault's state lives in persistent storage keyed by its target address.

use soroban_sdk::{symbol_short, Address, Env, Symbol};

use crate::types::{DataKey, VaultState};

/// The tip token address.
const TOKEN: Symbol = symbol_short!("LK_TOKEN");

pub fn has_token(env: &Env) -> bool {
    env.storage().instance().has(&TOKEN)
}

pub fn get_token(env: &Env) -> Option<Address> {
    env.storage().instance().get(&TOKEN)
}

pub fn set_token(env: &Env, token: &Address) {
    env.storage().instance().set(&TOKEN, token);
}

pub fn has_vault(env: &Env, target: &Address) -> bool {
    env.storage()
        .persistent()
        .has(&DataKey::Vault(target.clone()))
}

pub fn get_vault(env: &Env, target: &Address) -> Option<VaultState> {
    env.storage()
        .persistent()
        .get(&DataKey::Vault(target.clone()))
}

pub fn set_vault(env: &Env, target: &Address, vault: &VaultState) {
    env.storage()
        .persistent()
        .set(&DataKey::Vault(target.clone()), vault);
}
