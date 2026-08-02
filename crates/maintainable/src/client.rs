//! Cross-contract client for maintainable contracts.
//!
//! A rent vault, or any other consumer, uses `MaintainableClient` to call a
//! maintainable contract without duplicating its interface:
//!
//! ```ignore
//! let state = maintainable::client::MaintainableClient::new(&env, &target).lk_state();
//! ```
//!
//! Each method returns the value of a successful call and traps if the target
//! returns an error; the generated `try_*` variants return the error instead.

use soroban_sdk::{contractclient, Address, Env};

use crate::types::MaintenanceState;

#[contractclient(name = "MaintainableClient")]
pub trait Maintainable {
    fn extend_all(env: Env, keeper: Address) -> u32;
    fn lk_state(env: Env) -> MaintenanceState;
}
