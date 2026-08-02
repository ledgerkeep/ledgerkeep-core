//! LedgerKeep rent vault.
//!
//! Lets a protocol pre-fund maintenance and pay whoever performs it.
//!
//! # The verification limit
//!
//! The vault cannot confirm an extension was *necessary*, because no contract
//! can read TTL at runtime. It confirms only that maintenance *occurred* and
//! *who did it*, by cross-calling `lk_state()` on the target.
//!
//! Three conditions together bound the exploit surface:
//!
//! 1. `state.last_maintained > vault.last_claim` — work happened since the last
//!    payout.
//! 2. `state.last_keeper == keeper` — the claimant did the work. Closes the
//!    front-running window where a bystander claims after someone else's
//!    `extend_all`.
//! 3. `current_ledger - vault.last_claim >= vault.interval` — at most one tip per
//!    window.
//!
//! Worst case: a keeper burns its own fees calling `extend_all` more often than
//! needed and collects one tip per interval. The owner sets `tip` and
//! `interval`, caps exposure by vault balance, and can withdraw at any time. It
//! is a rate limit, not a proof of necessity.

#![no_std]

mod errors;
mod events;
mod storage;
mod types;

#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, token::TokenClient, Address, Env};

use maintainable::MaintainableClient;

pub use errors::VaultError;
pub use types::{DataKey, VaultState};

#[contract]
pub struct RentVault;

#[contractimpl]
impl RentVault {
    /// Set the tip asset. Callable once; the token is immutable afterward. No
    /// admin is stored.
    ///
    /// Errors `AlreadyInitialized`.
    pub fn initialize(env: Env, token: Address) -> Result<(), VaultError> {
        if storage::has_token(&env) {
            return Err(VaultError::AlreadyInitialized);
        }
        storage::set_token(&env, &token);
        Ok(())
    }

    /// Open a vault for a maintenance target.
    ///
    /// Callable by the owner. `last_claim` is set to the current ledger so the
    /// first claim still waits a full interval.
    ///
    /// Errors `NotInitialized`, `VaultExists`, `InvalidTerms` (`tip <= 0` or
    /// `interval == 0`).
    pub fn open(
        env: Env,
        target: Address,
        owner: Address,
        tip: i128,
        interval: u32,
    ) -> Result<(), VaultError> {
        owner.require_auth();

        if !storage::has_token(&env) {
            return Err(VaultError::NotInitialized);
        }
        if storage::has_vault(&env, &target) {
            return Err(VaultError::VaultExists);
        }
        if tip <= 0 || interval == 0 {
            return Err(VaultError::InvalidTerms);
        }

        let last_claim = env.ledger().sequence();
        storage::set_vault(
            &env,
            &target,
            &VaultState {
                target: target.clone(),
                owner: owner.clone(),
                balance: 0,
                tip,
                interval,
                last_claim,
            },
        );
        events::Opened {
            target,
            owner,
            tip,
            interval,
        }
        .publish(&env);
        Ok(())
    }

    /// Add funds to a vault. Anyone may fund a vault they do not own.
    ///
    /// The token is transferred in first, then the balance is credited — the
    /// funds must arrive before the vault records them.
    ///
    /// Errors `VaultMissing`, `InvalidAmount` (`amount <= 0`).
    pub fn fund(env: Env, target: Address, from: Address, amount: i128) -> Result<(), VaultError> {
        from.require_auth();

        let mut vault = storage::get_vault(&env, &target).ok_or(VaultError::VaultMissing)?;
        if amount <= 0 {
            return Err(VaultError::InvalidAmount);
        }
        let token = storage::get_token(&env).ok_or(VaultError::NotInitialized)?;

        let this = env.current_contract_address();
        TokenClient::new(&env, &token).transfer(&from, &this, &amount);
        vault.balance += amount;
        storage::set_vault(&env, &target, &vault);

        events::Funded {
            target,
            from,
            amount,
            balance: vault.balance,
        }
        .publish(&env);
        Ok(())
    }

    /// Withdraw funds from a vault to any address.
    ///
    /// Callable by the vault owner. The balance is decremented and written
    /// before the token transfer.
    ///
    /// Errors `VaultMissing`, `InvalidAmount`, `InsufficientBalance`.
    pub fn withdraw(
        env: Env,
        target: Address,
        to: Address,
        amount: i128,
    ) -> Result<(), VaultError> {
        let mut vault = storage::get_vault(&env, &target).ok_or(VaultError::VaultMissing)?;
        vault.owner.require_auth();

        if amount <= 0 {
            return Err(VaultError::InvalidAmount);
        }
        if vault.balance < amount {
            return Err(VaultError::InsufficientBalance);
        }
        let token = storage::get_token(&env).ok_or(VaultError::NotInitialized)?;

        vault.balance -= amount;
        storage::set_vault(&env, &target, &vault);
        let this = env.current_contract_address();
        TokenClient::new(&env, &token).transfer(&this, &to, &amount);

        events::Withdrawn {
            target,
            to,
            amount,
            balance: vault.balance,
        }
        .publish(&env);
        Ok(())
    }

    /// Change a vault's tip and interval.
    ///
    /// Callable by the vault owner.
    ///
    /// Errors `VaultMissing`, `InvalidTerms`.
    pub fn set_terms(
        env: Env,
        target: Address,
        tip: i128,
        interval: u32,
    ) -> Result<(), VaultError> {
        let mut vault = storage::get_vault(&env, &target).ok_or(VaultError::VaultMissing)?;
        vault.owner.require_auth();

        if tip <= 0 || interval == 0 {
            return Err(VaultError::InvalidTerms);
        }

        vault.tip = tip;
        vault.interval = interval;
        storage::set_vault(&env, &target, &vault);

        events::TermsSet {
            target,
            tip,
            interval,
        }
        .publish(&env);
        Ok(())
    }

    /// Pay a keeper for maintaining the target.
    ///
    /// Callable by the keeper. Verifies, in order, that the keeper is the last
    /// recorded maintainer, that maintenance happened since the last claim, that
    /// the interval has elapsed, and that the balance covers the tip.
    ///
    /// Errors `VaultMissing`, `NotTheKeeper`, `NoMaintenance`, `TooSoon`,
    /// `InsufficientBalance`.
    pub fn claim(env: Env, target: Address, keeper: Address) -> Result<i128, VaultError> {
        keeper.require_auth();

        let mut vault = storage::get_vault(&env, &target).ok_or(VaultError::VaultMissing)?;
        let state = MaintainableClient::new(&env, &target).lk_state();

        if state.last_keeper != keeper {
            return Err(VaultError::NotTheKeeper);
        }
        if state.last_maintained <= vault.last_claim {
            return Err(VaultError::NoMaintenance);
        }
        if env.ledger().sequence() - vault.last_claim < vault.interval {
            return Err(VaultError::TooSoon);
        }
        if vault.balance < vault.tip {
            return Err(VaultError::InsufficientBalance);
        }
        let token = storage::get_token(&env).ok_or(VaultError::NotInitialized)?;

        let tip = vault.tip;
        vault.balance -= tip;
        // The interval clock runs from payout (the current ledger), not from when
        // the work was done, so a keeper cannot batch maintenance and drain
        // several tips at once.
        vault.last_claim = env.ledger().sequence();
        storage::set_vault(&env, &target, &vault);
        let this = env.current_contract_address();
        TokenClient::new(&env, &token).transfer(&this, &keeper, &tip);

        events::Claimed {
            target,
            keeper,
            tip,
            ledger: vault.last_claim,
        }
        .publish(&env);
        Ok(tip)
    }

    /// Return a vault's state, or `None` if it does not exist.
    ///
    /// Read-only. Callable by anyone.
    pub fn get_vault(env: Env, target: Address) -> Option<VaultState> {
        storage::get_vault(&env, &target)
    }
}
