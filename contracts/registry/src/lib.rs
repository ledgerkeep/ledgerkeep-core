//! LedgerKeep registry.
//!
//! A permissionless directory mapping a contract address to its published
//! maintenance manifest, so external tooling can discover which contracts opt
//! into the standard and which keys they consider critical.
//!
//! There is **no admin, no upgrade authority, and no pause function**. This is a
//! public good; a privileged role would undermine the reason to trust it.
//!
//! # Manifest is advisory
//!
//! A [`RegistryEntry`]'s `keys_xdr` is off-chain metadata only, never decoded
//! on-chain. It can drift from the keys a contract's compiled
//! `impl_maintainable!` actually extends. `ledgerkeep-cli` detects drift by
//! simulating `extend_all` and diffing observed TTLs.
//!
//! # Bounded maintenance scope
//!
//! The registry adopts the standard for its own state, but maintains only its
//! instance entry and `LK_COUNT`. It does **not** maintain the `Entry`/`Slot`/
//! `Index` entries of every registered contract — that would be an unbounded
//! loop that exceeds resource limits in production. Per-entry maintenance is
//! each registered contract's own responsibility via its own vault.

#![no_std]

mod errors;
mod events;
mod storage;
mod types;

#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, Address, Bytes, Env, Vec};

use maintainable::{MaintainableError, MaintenanceState};

pub use errors::RegistryError;
pub use types::{DataKey, RegistryEntry};

// Adopt the maintenance standard. The registry's own critical state (`LK_COUNT`
// and the maintenance record) lives in instance storage, so an empty persistent
// key list is correct: extending the instance entry covers it. This deliberately
// does not extend the per-registration persistent entries; see the module docs.
maintainable::impl_maintainable!(
    threshold: 100_000,
    extend_to: 500_000,
    persistent: [],
);

#[contract]
pub struct Registry;

/// Write a new manifest entry and append it to the index. Shared by `register`
/// and the constructor. Does not perform authorization; callers are responsible
/// for that.
fn add_entry(
    env: &Env,
    contract: Address,
    keys_xdr: Vec<Bytes>,
    threshold: u32,
    extend_to: u32,
) -> Result<(), RegistryError> {
    if keys_xdr.is_empty() {
        return Err(RegistryError::EmptyManifest);
    }
    if threshold >= extend_to {
        return Err(RegistryError::InvalidParams);
    }
    if storage::has_entry(env, &contract) {
        return Err(RegistryError::AlreadyRegistered);
    }

    let ledger = env.ledger().sequence();
    let key_count = keys_xdr.len();
    storage::set_entry(
        env,
        &contract,
        &RegistryEntry {
            contract: contract.clone(),
            keys_xdr,
            threshold,
            extend_to,
            registered: ledger,
            updated: ledger,
        },
    );

    // Index append.
    let count = storage::get_count(env);
    storage::set_index(env, count, &contract);
    storage::set_slot(env, &contract, count);
    storage::set_count(env, count + 1);

    events::Registered {
        contract,
        key_count,
        ledger,
    }
    .publish(env);
    Ok(())
}

#[contractimpl]
impl Registry {
    /// Self-register at deployment. Writes the registry's own manifest entry
    /// directly. This needs no authorization: the contract is registering itself
    /// during its own construction, which no other party can do.
    ///
    /// Errors `EmptyManifest` or `InvalidParams` on bad arguments, which aborts
    /// deployment.
    pub fn __constructor(
        env: Env,
        keys_xdr: Vec<Bytes>,
        threshold: u32,
        extend_to: u32,
    ) -> Result<(), RegistryError> {
        let me = env.current_contract_address();
        add_entry(&env, me, keys_xdr, threshold, extend_to)
    }

    /// Register a contract's maintenance manifest.
    ///
    /// Callable by the contract itself only. `contract.require_auth()` is the
    /// entire security model: only the contract can satisfy auth for its own
    /// address, by invoking the registry from within its own code. No private key
    /// exists for a contract address, so manifest spoofing is structurally
    /// impossible.
    ///
    /// Errors `AlreadyRegistered`, `EmptyManifest`, `InvalidParams`.
    pub fn register(
        env: Env,
        contract: Address,
        keys_xdr: Vec<Bytes>,
        threshold: u32,
        extend_to: u32,
    ) -> Result<(), RegistryError> {
        contract.require_auth();
        add_entry(&env, contract, keys_xdr, threshold, extend_to)
    }

    /// Replace a registered contract's manifest and terms.
    ///
    /// Callable by the contract itself only. Does not touch the index.
    ///
    /// Errors `NotRegistered`, `EmptyManifest`, `InvalidParams`.
    pub fn update(
        env: Env,
        contract: Address,
        keys_xdr: Vec<Bytes>,
        threshold: u32,
        extend_to: u32,
    ) -> Result<(), RegistryError> {
        contract.require_auth();

        let mut entry = storage::get_entry(&env, &contract).ok_or(RegistryError::NotRegistered)?;
        if keys_xdr.is_empty() {
            return Err(RegistryError::EmptyManifest);
        }
        if threshold >= extend_to {
            return Err(RegistryError::InvalidParams);
        }

        let ledger = env.ledger().sequence();
        let key_count = keys_xdr.len();
        entry.keys_xdr = keys_xdr;
        entry.threshold = threshold;
        entry.extend_to = extend_to;
        entry.updated = ledger;
        storage::set_entry(&env, &contract, &entry);

        events::Updated {
            contract,
            key_count,
            ledger,
        }
        .publish(&env);
        Ok(())
    }

    /// Remove a registered contract's manifest.
    ///
    /// Callable by the contract itself only. Uses swap-remove to keep the index
    /// contiguous: the last entry is moved into the freed slot.
    ///
    /// Errors `NotRegistered`.
    pub fn deregister(env: Env, contract: Address) -> Result<(), RegistryError> {
        contract.require_auth();

        let slot = storage::get_slot(&env, &contract).ok_or(RegistryError::NotRegistered)?;
        // An entry exists, so at least one is registered and count >= 1.
        let last = storage::get_count(&env) - 1;

        if slot != last {
            if let Some(last_addr) = storage::get_index(&env, last) {
                storage::set_index(&env, slot, &last_addr);
                storage::set_slot(&env, &last_addr, slot);
            }
        }

        storage::remove_index(&env, last);
        storage::remove_slot(&env, &contract);
        storage::remove_entry(&env, &contract);
        storage::set_count(&env, last);

        let ledger = env.ledger().sequence();
        events::Deregistered { contract, ledger }.publish(&env);
        Ok(())
    }

    /// Return a contract's manifest entry, or `None` if not registered.
    ///
    /// Read-only. Callable by anyone.
    pub fn get(env: Env, contract: Address) -> Option<RegistryEntry> {
        storage::get_entry(&env, &contract)
    }

    /// Return the number of registered contracts.
    ///
    /// Read-only. Callable by anyone.
    pub fn count(env: Env) -> u32 {
        storage::get_count(&env)
    }

    /// Return a page of manifest entries in index order.
    ///
    /// Read-only. Callable by anyone. Returns fewer than `limit` when the page
    /// runs past the end, and an empty vec when `start` is at or past the count.
    ///
    /// Errors `LimitTooLarge` if `limit` is above 50.
    pub fn page(env: Env, start: u32, limit: u32) -> Result<Vec<RegistryEntry>, RegistryError> {
        if limit > 50 {
            return Err(RegistryError::LimitTooLarge);
        }

        let count = storage::get_count(&env);
        let mut out = Vec::new(&env);
        if start >= count {
            return Ok(out);
        }

        let end = core::cmp::min(start.saturating_add(limit), count);
        let mut i = start;
        while i < end {
            if let Some(addr) = storage::get_index(&env, i) {
                if let Some(entry) = storage::get_entry(&env, &addr) {
                    out.push_back(entry);
                }
            }
            i += 1;
        }
        Ok(out)
    }

    /// Extend the TTL of the registry's instance entry and record the keeper.
    ///
    /// Permissionless: anyone may call it. See the module docs for why the scope
    /// is bounded to instance storage.
    pub fn extend_all(env: Env, keeper: Address) -> Result<u32, MaintainableError> {
        __lk_extend_all(&env, keeper)
    }

    /// Return the registry's last maintenance record.
    ///
    /// Read-only. Callable by anyone. Errors `NotMaintained` if no maintenance
    /// has run yet.
    pub fn lk_state(env: Env) -> Result<MaintenanceState, MaintainableError> {
        maintainable::lk_state(&env)
    }
}
