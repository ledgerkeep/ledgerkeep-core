use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    Address, Bytes, Env, Vec,
};

use crate::{Registry, RegistryClient, RegistryError};

fn manifest(env: &Env) -> Vec<Bytes> {
    let mut m = Vec::new(env);
    m.push_back(Bytes::from_array(env, &[1u8, 2, 3, 4]));
    m
}

/// Deploy the registry. Its constructor self-registers, so the returned address
/// occupies index 0 and the count starts at 1.
fn deploy(env: &Env) -> Address {
    env.register(Registry, (manifest(env), 100u32, 1000u32))
}

#[test]
fn constructor_self_registers() {
    let env = Env::default();
    let id = deploy(&env);
    let client = RegistryClient::new(&env, &id);

    assert_eq!(client.count(), 1);
    let entry = client.get(&id).unwrap();
    assert_eq!(entry.contract, id);
}

#[test]
fn register_update_deregister_round_trip() {
    let env = Env::default();
    env.mock_all_auths();
    let id = deploy(&env);
    let client = RegistryClient::new(&env, &id);

    let a = Address::generate(&env);
    client.register(&a, &manifest(&env), &100, &1000);
    assert!(client.get(&a).is_some());
    assert_eq!(client.count(), 2);

    let registered = client.get(&a).unwrap().registered;

    // Advance so `updated` differs from `registered`.
    env.ledger().with_mut(|li| li.sequence_number += 10);
    let mut m2 = Vec::new(&env);
    m2.push_back(Bytes::from_array(&env, &[9u8]));
    m2.push_back(Bytes::from_array(&env, &[8u8]));
    client.update(&a, &m2, &200, &2000);

    let entry = client.get(&a).unwrap();
    assert_eq!(entry.keys_xdr.len(), 2);
    assert_eq!(entry.threshold, 200);
    assert_eq!(entry.extend_to, 2000);
    assert_eq!(entry.registered, registered);
    assert!(entry.updated > registered);

    client.deregister(&a);
    assert!(client.get(&a).is_none());
    assert_eq!(client.count(), 1);
}

#[test]
fn rejects_third_party_registration() {
    let env = Env::default();
    // No mock_all_auths: nobody has authorized anything.
    let id = deploy(&env);
    let client = RegistryClient::new(&env, &id);

    // A caller cannot register an address it does not control: `require_auth`
    // on that address fails.
    let other = Address::generate(&env);
    let result = client.try_register(&other, &manifest(&env), &100, &1000);
    assert!(result.is_err());
}

#[test]
fn swap_remove_of_middle_entry() {
    let env = Env::default();
    env.mock_all_auths();
    let id = deploy(&env);
    let client = RegistryClient::new(&env, &id);

    // Index becomes: 0=registry, 1=a, 2=b, 3=c.
    let a = Address::generate(&env);
    let b = Address::generate(&env);
    let c = Address::generate(&env);
    client.register(&a, &manifest(&env), &100, &1000);
    client.register(&b, &manifest(&env), &100, &1000);
    client.register(&c, &manifest(&env), &100, &1000);
    assert_eq!(client.count(), 4);

    // Remove the middle entry b. The last entry c swaps into b's slot.
    client.deregister(&b);
    assert_eq!(client.count(), 3);
    assert!(client.get(&b).is_none());
    assert!(client.get(&a).is_some());
    assert!(client.get(&c).is_some());

    // Order is now 0=registry, 1=a, 2=c.
    let page = client.page(&0, &50);
    assert_eq!(page.len(), 3);
    assert_eq!(page.get(0).unwrap().contract, id);
    assert_eq!(page.get(1).unwrap().contract, a);
    assert_eq!(page.get(2).unwrap().contract, c);
}

#[test]
fn pagination_boundaries_and_limit() {
    let env = Env::default();
    env.mock_all_auths();
    let id = deploy(&env);
    let client = RegistryClient::new(&env, &id);

    // registry + a + b => count 3.
    let a = Address::generate(&env);
    let b = Address::generate(&env);
    client.register(&a, &manifest(&env), &100, &1000);
    client.register(&b, &manifest(&env), &100, &1000);
    assert_eq!(client.count(), 3);

    // Over the cap.
    assert_eq!(
        client.try_page(&0, &51).err(),
        Some(Ok(RegistryError::LimitTooLarge))
    );

    // Full range.
    assert_eq!(client.page(&0, &50).len(), 3);
    // Start past the end.
    assert_eq!(client.page(&5, &10).len(), 0);
    // Page that runs off the end returns fewer than `limit`.
    assert_eq!(client.page(&2, &50).len(), 1);
}

#[test]
fn self_maintenance_records_keeper() {
    let env = Env::default();
    env.mock_all_auths();
    let id = deploy(&env);
    let client = RegistryClient::new(&env, &id);
    let keeper = Address::generate(&env);

    let seq = client.extend_all(&keeper);
    assert_eq!(seq, env.ledger().sequence());

    let state = client.lk_state();
    assert_eq!(state.last_keeper, keeper);
}
