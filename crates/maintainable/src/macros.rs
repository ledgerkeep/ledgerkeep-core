//! The `impl_maintainable!` macro.
//!
//! A host contract knows its own storage keys when it is written, so the key
//! list is fixed at compile time rather than stored on-chain. This macro inlines
//! the keys into a generated maintenance function. It removes the need for a raw
//! `Val` in storage (undeployable, see the crate spec) and the earlier
//! ungated-declare vulnerability, and it skips a per-call storage read.

/// Generate `__lk_extend_all`, the maintenance function, with the key list and
/// TTL terms inlined.
///
/// # Example
///
/// ```ignore
/// impl_maintainable!(
///     threshold: 100_000,
///     extend_to: 500_000,
///     persistent: [DataKey::Balance, DataKey::Milestones],
/// );
/// ```
///
/// The generated function signature is:
///
/// ```ignore
/// pub fn __lk_extend_all(env: &Env, keeper: Address) -> Result<u32, MaintainableError>
/// ```
///
/// It does not emit a `#[contractimpl]` block. The host contract writes two thin
/// wrappers in its own `#[contractimpl]` that forward to `__lk_extend_all` and
/// to [`crate::lk_state`].
///
/// Behaviour, in order:
/// 1. `keeper.require_auth()` — first statement.
/// 2. Error `ExtendTooLarge` if `extend_to` exceeds the network maximum TTL.
/// 3. Extend the instance entry.
/// 4. Extend each inlined persistent key.
/// 5. Write `MaintenanceState`.
/// 6. Emit `Maintained`.
/// 7. Return the current ledger sequence.
///
/// `__lk_extend_all` is permissionless by design. Anyone may call it. Worst case
/// a caller pays a fee to extend TTL that did not need extending, which harms
/// only that caller.
#[macro_export]
macro_rules! impl_maintainable {
    (
        threshold: $threshold:expr,
        extend_to: $extend_to:expr,
        persistent: [ $($key:expr),* $(,)? ] $(,)?
    ) => {
        pub fn __lk_extend_all(
            env: &::soroban_sdk::Env,
            keeper: ::soroban_sdk::Address,
        ) -> ::core::result::Result<u32, $crate::errors::MaintainableError> {
            // First statement: without this a caller could record an arbitrary
            // address as keeper and let it claim tips from a rent vault.
            keeper.require_auth();

            let threshold: u32 = $threshold;
            let extend_to: u32 = $extend_to;
            if extend_to > env.storage().max_ttl() {
                return ::core::result::Result::Err(
                    $crate::errors::MaintainableError::ExtendTooLarge,
                );
            }

            env.storage().instance().extend_ttl(threshold, extend_to);

            let mut key_count: u32 = 0;
            $(
                env.storage()
                    .persistent()
                    .extend_ttl(&$key, threshold, extend_to);
                key_count += 1;
            )*

            let ledger = env.ledger().sequence();
            $crate::storage::set_state(
                env,
                &$crate::types::MaintenanceState {
                    last_maintained: ledger,
                    last_keeper: keeper.clone(),
                },
            );
            $crate::events::Maintained {
                keeper,
                ledger,
                key_count,
            }
            .publish(env);
            ::core::result::Result::Ok(ledger)
        }
    };
}
