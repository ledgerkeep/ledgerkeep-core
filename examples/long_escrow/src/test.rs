use soroban_sdk::{
    testutils::{Address as _, Ledger as _, MockAuth, MockAuthInvoke},
    token::{StellarAssetClient, TokenClient},
    Address, Bytes, Env, IntoVal, Vec,
};

use crate::{EscrowError, LongEscrow, LongEscrowClient};

/// A stand-in for the registry that records the arguments it was called with.
///
/// The escrow calls the registry through a client interface rather than a crate
/// dependency, so there is no way to link the real registry contract into this
/// test. `register` here keeps the real one's signature and its
/// `contract.require_auth()`, which is the part these tests exercise: that the
/// cross-call presents the escrow's own address as the registering party.
mod fake_registry {
    use soroban_sdk::{contract, contractimpl, contracttype, Address, Bytes, Env, Vec};

    #[contracttype]
    #[derive(Clone, Debug, Eq, PartialEq)]
    pub struct Call {
        pub contract: Address,
        pub keys_xdr: Vec<Bytes>,
        pub threshold: u32,
        pub extend_to: u32,
    }

    #[contracttype]
    pub enum Key {
        Last,
    }

    #[contract]
    pub struct FakeRegistry;

    #[contractimpl]
    impl FakeRegistry {
        pub fn register(
            env: Env,
            contract: Address,
            keys_xdr: Vec<Bytes>,
            threshold: u32,
            extend_to: u32,
        ) {
            contract.require_auth();
            env.storage().instance().set(
                &Key::Last,
                &Call {
                    contract,
                    keys_xdr,
                    threshold,
                    extend_to,
                },
            );
        }

        pub fn last(env: Env) -> Option<Call> {
            env.storage().instance().get(&Key::Last)
        }
    }
}

struct Fixture {
    env: Env,
    escrow_id: Address,
    token: Address,
    buyer: Address,
    seller: Address,
    approver: Address,
}

fn setup() -> Fixture {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(admin);
    let token = sac.address();

    let buyer = Address::generate(&env);
    let seller = Address::generate(&env);
    let approver = Address::generate(&env);
    StellarAssetClient::new(&env, &token).mint(&buyer, &1_000);

    let escrow_id = env.register(LongEscrow, ());

    Fixture {
        env,
        escrow_id,
        token,
        buyer,
        seller,
        approver,
    }
}

fn amounts(env: &Env) -> Vec<i128> {
    let mut v = Vec::new(env);
    v.push_back(100);
    v.push_back(200);
    v
}

#[test]
fn escrow_lifecycle_with_maintenance() {
    let f = setup();
    let escrow = LongEscrowClient::new(&f.env, &f.escrow_id);
    let token = TokenClient::new(&f.env, &f.token);

    escrow.initialize(&f.buyer, &f.seller, &f.approver, &f.token, &amounts(&f.env));

    escrow.deposit(&300);
    assert_eq!(token.balance(&f.escrow_id), 300);
    assert_eq!(token.balance(&f.buyer), 700);

    // Cannot release before approval.
    assert_eq!(
        escrow.try_release(&0).err(),
        Some(Ok(EscrowError::NotApproved))
    );

    escrow.approve_milestone(&0);
    escrow.release(&0);
    assert_eq!(token.balance(&f.seller), 100);
    assert_eq!(token.balance(&f.escrow_id), 200);

    // A released milestone cannot be released again.
    assert_eq!(
        escrow.try_release(&0).err(),
        Some(Ok(EscrowError::AlreadyReleased))
    );

    escrow.approve_milestone(&1);
    escrow.release(&1);
    assert_eq!(token.balance(&f.seller), 300);
    assert_eq!(token.balance(&f.escrow_id), 0);

    // Maintenance across advanced ledgers: the persistent state survives and the
    // keeper is recorded each run.
    let keeper = Address::generate(&f.env);
    escrow.extend_all(&keeper);
    let first = escrow.lk_state();
    assert_eq!(first.last_keeper, keeper);

    f.env.ledger().with_mut(|li| li.sequence_number += 5_000);
    escrow.extend_all(&keeper);
    let second = escrow.lk_state();
    assert!(second.last_maintained > first.last_maintained);
}

#[test]
fn rejects_double_initialize_and_bad_amounts() {
    let f = setup();
    let escrow = LongEscrowClient::new(&f.env, &f.escrow_id);

    // Empty milestone list.
    let empty = Vec::new(&f.env);
    assert_eq!(
        escrow
            .try_initialize(&f.buyer, &f.seller, &f.approver, &f.token, &empty)
            .err(),
        Some(Ok(EscrowError::NoMilestones))
    );

    // A non-positive milestone amount.
    let mut bad = Vec::new(&f.env);
    bad.push_back(100);
    bad.push_back(0);
    assert_eq!(
        escrow
            .try_initialize(&f.buyer, &f.seller, &f.approver, &f.token, &bad)
            .err(),
        Some(Ok(EscrowError::InvalidAmount))
    );

    // A valid initialize, then a second one fails.
    escrow.initialize(&f.buyer, &f.seller, &f.approver, &f.token, &amounts(&f.env));
    assert_eq!(
        escrow
            .try_initialize(&f.buyer, &f.seller, &f.approver, &f.token, &amounts(&f.env))
            .err(),
        Some(Ok(EscrowError::AlreadyInitialized))
    );
}

/// The escrow's real manifest: the three ledger keys `impl_maintainable!`
/// extends, XDR-encoded. In order: `ScVal::LedgerKeyContractInstance` (the
/// instance entry holding `Config`), then `DataKey::Balance` and
/// `DataKey::Milestones`, each of which encodes as a one-element `ScVal::Vec`
/// holding a symbol. `scripts/init_testnet.sh` passes these same three values.
fn manifest(env: &Env) -> Vec<Bytes> {
    let mut keys = Vec::new(env);
    keys.push_back(Bytes::from_array(env, &[0x00, 0x00, 0x00, 0x14]));
    keys.push_back(Bytes::from_array(
        env,
        &[
            0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
            0x00, 0x0f, 0x00, 0x00, 0x00, 0x07, 0x42, 0x61, 0x6c, 0x61, 0x6e, 0x63, 0x65, 0x00,
        ],
    ));
    keys.push_back(Bytes::from_array(
        env,
        &[
            0x00, 0x00, 0x00, 0x10, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
            0x00, 0x0f, 0x00, 0x00, 0x00, 0x0a, 0x4d, 0x69, 0x6c, 0x65, 0x73, 0x74, 0x6f, 0x6e,
            0x65, 0x73, 0x00, 0x00,
        ],
    ));
    keys
}

#[test]
fn register_with_presents_the_escrows_own_address() {
    let f = setup();
    let escrow = LongEscrowClient::new(&f.env, &f.escrow_id);
    escrow.initialize(&f.buyer, &f.seller, &f.approver, &f.token, &amounts(&f.env));

    let registry_id = f.env.register(fake_registry::FakeRegistry, ());
    let registry = fake_registry::FakeRegistryClient::new(&f.env, &registry_id);

    let keys = manifest(&f.env);
    escrow.register_with(&registry_id, &keys, &100_000, &500_000);

    let call = registry.last().unwrap();
    // The registry is told the escrow is the contract being registered. That is
    // what makes its `require_auth()` satisfiable: the call came from inside the
    // escrow, so the escrow is the invoker.
    assert_eq!(call.contract, f.escrow_id);
    assert_eq!(call.keys_xdr, keys);
    assert_eq!(call.threshold, 100_000);
    assert_eq!(call.extend_to, 500_000);
}

#[test]
fn register_with_needs_the_buyers_authorization() {
    let f = setup();
    let escrow = LongEscrowClient::new(&f.env, &f.escrow_id);
    escrow.initialize(&f.buyer, &f.seller, &f.approver, &f.token, &amounts(&f.env));
    let registry_id = f.env.register(fake_registry::FakeRegistry, ());
    let registry = fake_registry::FakeRegistryClient::new(&f.env, &registry_id);

    // Withdraw the blanket authorization the fixture installs. The buyer's
    // `require_auth()` now has nothing to satisfy it.
    f.env.set_auths(&[]);
    assert!(escrow
        .try_register_with(&registry_id, &manifest(&f.env), &100_000, &500_000)
        .is_err());
    assert_eq!(registry.last(), None);
}

/// The invocation `register_with` authorizes: the escrow calling itself with
/// the registry's arguments. `buyer.require_auth()` (no args) authorizes the
/// current frame, so the mocked entry has to name the escrow, the function and
/// exactly the arguments the call passes.
fn register_with_invocation<'a>(
    f: &'a Fixture,
    registry_id: &Address,
    keys: &Vec<Bytes>,
) -> MockAuthInvoke<'a> {
    MockAuthInvoke {
        contract: &f.escrow_id,
        fn_name: "register_with",
        args: soroban_sdk::vec![
            &f.env,
            registry_id.clone().into_val(&f.env),
            keys.clone().into_val(&f.env),
            100_000_u32.into_val(&f.env),
            500_000_u32.into_val(&f.env),
        ],
        sub_invokes: &[],
    }
}

#[test]
fn register_with_rejects_non_buyer_authorizations() {
    let f = setup();
    let escrow = LongEscrowClient::new(&f.env, &f.escrow_id);
    escrow.initialize(&f.buyer, &f.seller, &f.approver, &f.token, &amounts(&f.env));

    let registry_id = f.env.register(fake_registry::FakeRegistry, ());
    let registry = fake_registry::FakeRegistryClient::new(&f.env, &registry_id);

    // Authorize the seller and a wholly unrelated account, but not the buyer.
    // The buyer's `require_auth()` has nothing to satisfy it, so the call fails
    // before the escrow ever reaches the registry.
    let unrelated = Address::generate(&f.env);
    let keys = manifest(&f.env);
    let invoke = register_with_invocation(&f, &registry_id, &keys);
    assert!(escrow
        .mock_auths(&[
            MockAuth {
                address: &f.seller,
                invoke: &invoke,
            },
            MockAuth {
                address: &unrelated,
                invoke: &invoke,
            },
        ])
        .try_register_with(&registry_id, &keys, &100_000, &500_000)
        .is_err());
    assert_eq!(registry.last(), None);
}

#[test]
fn register_with_succeeds_with_only_the_buyers_authorization() {
    let f = setup();
    let escrow = LongEscrowClient::new(&f.env, &f.escrow_id);
    escrow.initialize(&f.buyer, &f.seller, &f.approver, &f.token, &amounts(&f.env));

    let registry_id = f.env.register(fake_registry::FakeRegistry, ());
    let registry = fake_registry::FakeRegistryClient::new(&f.env, &registry_id);

    // Drop the blanket mock the fixture installs and authorize the buyer alone.
    // The buyer is the address the escrow's `require_auth()` names, so this is
    // the only authorization the call needs.
    let keys = manifest(&f.env);
    escrow
        .mock_auths(&[MockAuth {
            address: &f.buyer,
            invoke: &register_with_invocation(&f, &registry_id, &keys),
        }])
        .register_with(&registry_id, &keys, &100_000, &500_000);

    let call = registry.last().unwrap();
    assert_eq!(call.contract, f.escrow_id);
    assert_eq!(call.keys_xdr, keys);
    assert_eq!(call.threshold, 100_000);
    assert_eq!(call.extend_to, 500_000);
}

#[test]
fn register_with_rejects_an_uninitialized_escrow() {
    let f = setup();
    let escrow = LongEscrowClient::new(&f.env, &f.escrow_id);
    let registry_id = f.env.register(fake_registry::FakeRegistry, ());

    assert_eq!(
        escrow
            .try_register_with(&registry_id, &manifest(&f.env), &100_000, &500_000)
            .err(),
        Some(Ok(EscrowError::NotInitialized))
    );
}
