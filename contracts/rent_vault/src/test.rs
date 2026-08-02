use soroban_sdk::{
    testutils::{Address as _, Ledger as _},
    token::{StellarAssetClient, TokenClient},
    Address, Env,
};

use crate::{RentVault, RentVaultClient, VaultError};

/// A maintainable target the vault cross-calls `lk_state()` on.
mod target {
    use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};

    use maintainable::{MaintainableError, MaintenanceState};

    #[contracttype]
    #[derive(Clone)]
    pub enum DataKey {
        Balance,
    }

    maintainable::impl_maintainable!(
        threshold: 100,
        extend_to: 10_000,
        persistent: [DataKey::Balance],
    );

    #[contract]
    pub struct Target;

    #[contractimpl]
    impl Target {
        pub fn seed(env: Env) {
            env.storage().persistent().set(&DataKey::Balance, &0u32);
        }

        pub fn extend_all(env: Env, keeper: Address) -> Result<u32, MaintainableError> {
            __lk_extend_all(&env, keeper)
        }

        pub fn lk_state(env: Env) -> Result<MaintenanceState, MaintainableError> {
            maintainable::lk_state(&env)
        }
    }
}

struct Fixture {
    env: Env,
    vault_id: Address,
    token: Address,
    target_id: Address,
}

fn setup() -> Fixture {
    let env = Env::default();
    env.mock_all_auths();

    let admin = Address::generate(&env);
    let sac = env.register_stellar_asset_contract_v2(admin);
    let token = sac.address();

    let vault_id = env.register(RentVault, ());
    RentVaultClient::new(&env, &vault_id).initialize(&token);

    let target_id = env.register(target::Target, ());
    target::TargetClient::new(&env, &target_id).seed();

    Fixture {
        env,
        vault_id,
        token,
        target_id,
    }
}

#[test]
fn initialize_sets_token_once() {
    let f = setup();
    let vault = RentVaultClient::new(&f.env, &f.vault_id);
    // Already initialized in setup; a second call fails.
    let other = Address::generate(&f.env);
    assert_eq!(
        vault.try_initialize(&other).err(),
        Some(Ok(VaultError::AlreadyInitialized))
    );
}

#[test]
fn open_requires_initialization() {
    let env = Env::default();
    env.mock_all_auths();
    let vault_id = env.register(RentVault, ());
    let vault = RentVaultClient::new(&env, &vault_id);

    let target = Address::generate(&env);
    let owner = Address::generate(&env);
    assert_eq!(
        vault.try_open(&target, &owner, &100, &10).err(),
        Some(Ok(VaultError::NotInitialized))
    );
}

#[test]
fn open_rejects_bad_terms_and_duplicates() {
    let f = setup();
    let vault = RentVaultClient::new(&f.env, &f.vault_id);
    let owner = Address::generate(&f.env);

    assert_eq!(
        vault.try_open(&f.target_id, &owner, &0, &10).err(),
        Some(Ok(VaultError::InvalidTerms))
    );
    assert_eq!(
        vault.try_open(&f.target_id, &owner, &100, &0).err(),
        Some(Ok(VaultError::InvalidTerms))
    );

    vault.open(&f.target_id, &owner, &100, &10);
    assert_eq!(
        vault.try_open(&f.target_id, &owner, &100, &10).err(),
        Some(Ok(VaultError::VaultExists))
    );
}

#[test]
fn full_claim_path() {
    let f = setup();
    let vault = RentVaultClient::new(&f.env, &f.vault_id);
    let token = TokenClient::new(&f.env, &f.token);
    let sac = StellarAssetClient::new(&f.env, &f.token);

    let owner = Address::generate(&f.env);
    let keeper = Address::generate(&f.env);
    sac.mint(&owner, &1_000);

    vault.open(&f.target_id, &owner, &100, &10);
    vault.fund(&f.target_id, &owner, &500);
    assert_eq!(vault.get_vault(&f.target_id).unwrap().balance, 500);

    // Advance a full interval, then the keeper maintains the target.
    f.env.ledger().with_mut(|li| li.sequence_number += 10);
    let target = target::TargetClient::new(&f.env, &f.target_id);
    target.extend_all(&keeper);

    let paid = vault.claim(&f.target_id, &keeper);
    assert_eq!(paid, 100);
    assert_eq!(token.balance(&keeper), 100);

    let v = vault.get_vault(&f.target_id).unwrap();
    assert_eq!(v.balance, 400);
    assert_eq!(v.last_claim, f.env.ledger().sequence());
}

#[test]
fn claim_rejects_not_the_keeper() {
    let f = setup();
    let vault = RentVaultClient::new(&f.env, &f.vault_id);
    let sac = StellarAssetClient::new(&f.env, &f.token);

    let owner = Address::generate(&f.env);
    let maintainer = Address::generate(&f.env);
    let bystander = Address::generate(&f.env);
    sac.mint(&owner, &1_000);

    vault.open(&f.target_id, &owner, &100, &10);
    vault.fund(&f.target_id, &owner, &500);

    f.env.ledger().with_mut(|li| li.sequence_number += 10);
    let target = target::TargetClient::new(&f.env, &f.target_id);
    target.extend_all(&maintainer);

    // A bystander who did not maintain cannot claim.
    assert_eq!(
        vault.try_claim(&f.target_id, &bystander).err(),
        Some(Ok(VaultError::NotTheKeeper))
    );
}

#[test]
fn claim_rejects_too_soon() {
    let f = setup();
    let vault = RentVaultClient::new(&f.env, &f.vault_id);
    let sac = StellarAssetClient::new(&f.env, &f.token);

    let owner = Address::generate(&f.env);
    let keeper = Address::generate(&f.env);
    sac.mint(&owner, &1_000);

    // Large interval; advance far less than it.
    vault.open(&f.target_id, &owner, &100, &1_000);
    vault.fund(&f.target_id, &owner, &500);

    f.env.ledger().with_mut(|li| li.sequence_number += 5);
    let target = target::TargetClient::new(&f.env, &f.target_id);
    target.extend_all(&keeper);

    assert_eq!(
        vault.try_claim(&f.target_id, &keeper).err(),
        Some(Ok(VaultError::TooSoon))
    );
}

#[test]
fn claim_rejects_no_maintenance() {
    let f = setup();
    let vault = RentVaultClient::new(&f.env, &f.vault_id);
    let sac = StellarAssetClient::new(&f.env, &f.token);

    let owner = Address::generate(&f.env);
    let keeper = Address::generate(&f.env);
    sac.mint(&owner, &1_000);

    // Maintain first, then open the vault so last_claim is after last_maintained.
    let target = target::TargetClient::new(&f.env, &f.target_id);
    target.extend_all(&keeper);

    f.env.ledger().with_mut(|li| li.sequence_number += 10);
    vault.open(&f.target_id, &owner, &100, &10);
    vault.fund(&f.target_id, &owner, &500);

    assert_eq!(
        vault.try_claim(&f.target_id, &keeper).err(),
        Some(Ok(VaultError::NoMaintenance))
    );
}

#[test]
fn balance_accounting_across_fund_claim_withdraw() {
    let f = setup();
    let vault = RentVaultClient::new(&f.env, &f.vault_id);
    let token = TokenClient::new(&f.env, &f.token);
    let sac = StellarAssetClient::new(&f.env, &f.token);

    let owner = Address::generate(&f.env);
    let keeper = Address::generate(&f.env);
    sac.mint(&owner, &1_000);

    vault.open(&f.target_id, &owner, &100, &10);
    vault.fund(&f.target_id, &owner, &500);
    assert_eq!(token.balance(&owner), 500);
    assert_eq!(token.balance(&f.vault_id), 500);

    f.env.ledger().with_mut(|li| li.sequence_number += 10);
    let target = target::TargetClient::new(&f.env, &f.target_id);
    target.extend_all(&keeper);
    vault.claim(&f.target_id, &keeper);
    assert_eq!(vault.get_vault(&f.target_id).unwrap().balance, 400);
    assert_eq!(token.balance(&f.vault_id), 400);
    assert_eq!(token.balance(&keeper), 100);

    // Withdraw the rest back to the owner.
    vault.withdraw(&f.target_id, &owner, &400);
    assert_eq!(vault.get_vault(&f.target_id).unwrap().balance, 0);
    assert_eq!(token.balance(&f.vault_id), 0);
    assert_eq!(token.balance(&owner), 900);

    // Nothing left to withdraw.
    assert_eq!(
        vault.try_withdraw(&f.target_id, &owner, &1).err(),
        Some(Ok(VaultError::InsufficientBalance))
    );
}
