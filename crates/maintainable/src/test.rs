use soroban_sdk::{
    contract, contractimpl, contracttype,
    testutils::{storage::Persistent as _, Address as _, Ledger as _},
    Address, Env,
};

use crate::errors::MaintainableError;
use crate::types::MaintenanceState;

/// A host's own storage keys. In a real contract these are whatever the contract
/// already uses; the macro inlines them.
#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Balance,
    Milestones,
}

// Adopt the standard: threshold 100, target 10_000, two persistent keys.
crate::impl_maintainable!(
    threshold: 100,
    extend_to: 10_000,
    persistent: [DataKey::Balance, DataKey::Milestones],
);

#[contract]
struct Host;

#[contractimpl]
impl Host {
    /// Seed the persistent entries so `extend_all` has real keys to extend.
    pub fn seed(env: Env) {
        env.storage().persistent().set(&DataKey::Balance, &0u32);
        env.storage().persistent().set(&DataKey::Milestones, &0u32);
    }

    pub fn extend_all(env: Env, keeper: Address) -> Result<u32, MaintainableError> {
        __lk_extend_all(&env, keeper)
    }

    pub fn lk_state(env: Env) -> Result<MaintenanceState, MaintainableError> {
        crate::lk_state(&env)
    }
}

/// A separate host whose `extend_to` exceeds the network maximum TTL, for the
/// `ExtendTooLarge` path. It lives in its own module so its generated
/// `__lk_extend_all` does not collide with the one above.
mod toolarge {
    use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

    use crate::errors::MaintainableError;

    #[contracttype]
    #[derive(Clone)]
    pub enum DataKey {
        Balance,
    }

    crate::impl_maintainable!(
        threshold: 100,
        extend_to: u32::MAX,
        persistent: [DataKey::Balance],
    );

    #[contract]
    pub struct HostBig;

    #[contractimpl]
    impl HostBig {
        pub fn extend_all(env: Env, keeper: Address) -> Result<u32, MaintainableError> {
            __lk_extend_all(&env, keeper)
        }
    }
}

#[test]
fn extend_all_extends_ttl_and_records_keeper() {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register(Host, ());
    let client = HostClient::new(&env, &id);
    let keeper = Address::generate(&env);

    client.seed();
    let returned = client.extend_all(&keeper);
    assert_eq!(returned, env.ledger().sequence());

    let state = client.lk_state();
    assert_eq!(state.last_keeper, keeper);
    assert_eq!(state.last_maintained, env.ledger().sequence());

    env.as_contract(&id, || {
        let ttl = env.storage().persistent().get_ttl(&DataKey::Balance);
        assert!(ttl >= 100);
    });
}

#[test]
fn extend_all_is_noop_when_above_threshold() {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register(Host, ());
    let client = HostClient::new(&env, &id);
    let keeper = Address::generate(&env);

    client.seed();
    client.extend_all(&keeper);

    // Advance far less than extend_to, so remaining TTL stays above threshold.
    env.ledger().with_mut(|li| li.sequence_number += 50);

    let before = env.as_contract(&id, || {
        env.storage().persistent().get_ttl(&DataKey::Balance)
    });
    client.extend_all(&keeper);
    let after = env.as_contract(&id, || {
        env.storage().persistent().get_ttl(&DataKey::Balance)
    });

    // No extension: remaining was already above the threshold.
    assert_eq!(before, after);
}

#[test]
fn extend_all_requires_keeper_auth() {
    let env = Env::default();
    // No mock_all_auths: the keeper authorized nothing.
    let id = env.register(Host, ());
    let client = HostClient::new(&env, &id);
    let keeper = Address::generate(&env);

    let result = client.try_extend_all(&keeper);
    assert!(result.is_err());
}

#[test]
fn lk_state_errors_before_any_maintenance() {
    let env = Env::default();
    let id = env.register(Host, ());
    let client = HostClient::new(&env, &id);

    // Compare on the error arm only: MaintenanceState deliberately derives
    // neither Debug nor PartialEq, so assert_eq on the whole Result won't work.
    let result = client.try_lk_state();
    assert_eq!(result.err(), Some(Ok(MaintainableError::NotMaintained)));
}

#[test]
fn extend_all_errors_when_extend_to_above_max_ttl() {
    let env = Env::default();
    env.mock_all_auths();
    let id = env.register(toolarge::HostBig, ());
    let client = toolarge::HostBigClient::new(&env, &id);
    let keeper = Address::generate(&env);

    let result = client.try_extend_all(&keeper);
    assert_eq!(result, Err(Ok(MaintainableError::ExtendTooLarge)));
}
