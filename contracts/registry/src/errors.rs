//! Registry error codes.
//!
//! Codes are in the `1xx` range, fixed by the project specification. They are
//! permanent once committed and must never be renumbered.

use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum RegistryError {
    /// `register` was called for an address that already has an entry.
    AlreadyRegistered = 101,
    /// `update` or `deregister` was called for an address with no entry.
    NotRegistered = 102,
    /// A manifest with no keys was supplied.
    EmptyManifest = 103,
    /// `threshold` was not below `extend_to`.
    InvalidParams = 104,
    /// `page` was called with `limit` above 50.
    LimitTooLarge = 105,
}
