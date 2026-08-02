//! Error codes for the maintenance standard.
//!
//! Codes are in the `3xx` range, fixed by the project specification. They are
//! permanent once committed and must never be renumbered.

use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum MaintainableError {
    /// The macro's `extend_to` parameter is above the network maximum TTL.
    ExtendTooLarge = 301,
    /// `lk_state` was read before any maintenance run recorded state.
    NotMaintained = 302,
}
