//! Rent vault error codes.
//!
//! Codes are in the `2xx` range, fixed by the project specification. They are
//! permanent once committed and must never be renumbered.

use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum VaultError {
    /// `initialize` was called after the token was already set.
    AlreadyInitialized = 201,
    /// An operation needs the tip token, but `initialize` has not run.
    NotInitialized = 202,
    /// `open` was called for a target that already has a vault.
    VaultExists = 203,
    /// The referenced vault does not exist.
    VaultMissing = 204,
    /// `tip <= 0` or `interval == 0`.
    InvalidTerms = 205,
    /// A funding or withdrawal amount was not positive.
    InvalidAmount = 206,
    /// The vault balance is below the requested or required amount.
    InsufficientBalance = 207,
    /// The claimant is not the address recorded as the last keeper.
    NotTheKeeper = 208,
    /// No maintenance has occurred since the last claim.
    NoMaintenance = 209,
    /// The interval since the last claim has not elapsed.
    TooSoon = 210,
}
