use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token::{StellarAssetClient, TokenClient},
    Address, Env, Vec,
};

use crate::{EscrowError, LongEscrow, LongEscrowClient};

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
