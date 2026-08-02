//! LedgerKeep maintenance standard.
//!
//! A library a Soroban contract adopts to expose permissionless self-maintenance:
//! extend the TTL of a fixed set of critical keys and record who did it, so a
//! rent vault can pay them.
//!
//! # The core constraint
//!
//! A Soroban contract cannot read the TTL of its own entries at runtime.
//! `get_ttl` is available only under the `testutils` feature, for tests. So this
//! crate can *extend* TTL and *record* that it did, but it cannot *observe* TTL.
//! All observation happens off-chain via RPC. There is deliberately no function
//! here that reads TTL in contract logic, because none can exist.
//!
//! # Adopting it in a host contract
//!
//! Call [`impl_maintainable!`] at module level with the host's own storage keys
//! and TTL terms. It generates `__lk_extend_all`. The host then writes two thin
//! wrappers inside its own `#[contractimpl]`:
//!
//! ```ignore
//! pub fn extend_all(env: Env, keeper: Address) -> Result<u32, MaintainableError> {
//!     __lk_extend_all(&env, keeper)
//! }
//! pub fn lk_state(env: Env) -> Result<MaintenanceState, MaintainableError> {
//!     maintainable::lk_state(&env)
//! }
//! ```

#![no_std]

pub mod client;
pub mod errors;
pub mod events;
pub mod macros;
pub mod storage;
pub mod types;

pub use client::MaintainableClient;
pub use errors::MaintainableError;
pub use events::Maintained;
pub use types::MaintenanceState;

use soroban_sdk::Env;

/// Return the last maintenance record.
///
/// Callable by anyone (read-only). Errors [`MaintainableError::NotMaintained`]
/// if no maintenance run has recorded state yet.
///
/// A host contract exposes this through a thin wrapper in its own
/// `#[contractimpl]`; see the crate-level docs.
pub fn lk_state(env: &Env) -> Result<MaintenanceState, MaintainableError> {
    storage::get_state(env).ok_or(MaintainableError::NotMaintained)
}

#[cfg(test)]
mod test;
