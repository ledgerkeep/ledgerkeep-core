//! Events emitted by the maintenance standard.
//!
//! Defined with `#[contractevent]`; `env.events().publish` is deprecated in
//! 27.x and fails under `-D warnings`. The macro prepends a fixed topic equal to
//! the struct name in lower snake case (`maintained`), followed by the `keeper`
//! topic. Remaining fields form the event data.

use soroban_sdk::{contractevent, Address};

/// Emitted after a maintenance run.
///
/// Topics: `(maintained, keeper)`. Data: `(ledger, key_count)`.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Maintained {
    #[topic]
    pub keeper: Address,
    pub ledger: u32,
    pub key_count: u32,
}
