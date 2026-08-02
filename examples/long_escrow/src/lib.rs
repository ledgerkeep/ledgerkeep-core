//! Example only — a milestone escrow that adopts the LedgerKeep maintenance
//! standard.
//!
//! **This is an example, not a product.** The Stellar ecosystem already has
//! mature escrow infrastructure and this does not compete with it. It exists to
//! show the standard on a contract whose state genuinely must outlive default
//! TTL: funds held across a multi-month term. There is no dispute resolution, no
//! oracle, and no roles beyond buyer, seller, and approver. It is a fixture.
//!
//! Lifecycle: the buyer `initialize`s the escrow with a list of milestone
//! amounts, `deposit`s funds, the approver `approve_milestone`s each stage, and
//! the seller `release`s approved milestones. The escrow's persistent balance
//! and milestone list are declared to the maintenance standard so a keeper can
//! keep them alive across the term.

#![no_std]

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, token::TokenClient,
    Address, Env, Vec,
};

use maintainable::{MaintainableError, MaintenanceState};

#[cfg(test)]
mod test;

/// Fixed configuration set once at initialization.
#[contracttype]
#[derive(Clone)]
pub struct EscrowConfig {
    pub buyer: Address,
    pub seller: Address,
    pub approver: Address,
    pub token: Address,
}

/// One milestone: an amount and its approval/release flags.
#[contracttype]
#[derive(Clone)]
pub struct Milestone {
    pub amount: i128,
    pub approved: bool,
    pub released: bool,
}

/// Storage keys. `Config` is instance data; `Balance` and `Milestones` are
/// persistent and are the keys declared to the maintenance standard.
#[contracttype]
pub enum DataKey {
    Config,
    Balance,
    Milestones,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum EscrowError {
    AlreadyInitialized = 401,
    NotInitialized = 402,
    NoMilestones = 403,
    InvalidAmount = 404,
    BadIndex = 405,
    AlreadyApproved = 406,
    NotApproved = 407,
    AlreadyReleased = 408,
    InsufficientBalance = 409,
}

/// Emitted once at initialization.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Initialized {
    #[topic]
    pub buyer: Address,
    pub milestone_count: u32,
}

/// Emitted when the buyer deposits funds.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Deposited {
    #[topic]
    pub from: Address,
    pub amount: i128,
    pub balance: i128,
}

/// Emitted when the approver approves a milestone.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MilestoneApproved {
    #[topic]
    pub approver: Address,
    pub index: u32,
}

/// Emitted when the seller releases an approved milestone.
#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MilestoneReleased {
    #[topic]
    pub seller: Address,
    pub index: u32,
    pub amount: i128,
}

// Adopt the maintenance standard: keep the persistent balance and milestone list
// alive. The instance entry (holding Config) is extended automatically.
maintainable::impl_maintainable!(
    threshold: 100_000,
    extend_to: 500_000,
    persistent: [DataKey::Balance, DataKey::Milestones],
);

fn get_config(env: &Env) -> Option<EscrowConfig> {
    env.storage().instance().get(&DataKey::Config)
}

fn get_balance(env: &Env) -> i128 {
    env.storage()
        .persistent()
        .get(&DataKey::Balance)
        .unwrap_or(0)
}

fn get_milestones(env: &Env) -> Vec<Milestone> {
    env.storage()
        .persistent()
        .get(&DataKey::Milestones)
        .unwrap_or_else(|| Vec::new(env))
}

#[contract]
pub struct LongEscrow;

#[contractimpl]
impl LongEscrow {
    /// Set up the escrow with its roles, tip token, and milestone amounts.
    ///
    /// Callable once. Errors `AlreadyInitialized`, `NoMilestones`,
    /// `InvalidAmount` (any amount not positive).
    pub fn initialize(
        env: Env,
        buyer: Address,
        seller: Address,
        approver: Address,
        token: Address,
        amounts: Vec<i128>,
    ) -> Result<(), EscrowError> {
        if get_config(&env).is_some() {
            return Err(EscrowError::AlreadyInitialized);
        }
        if amounts.is_empty() {
            return Err(EscrowError::NoMilestones);
        }

        let mut milestones = Vec::new(&env);
        for amount in amounts.iter() {
            if amount <= 0 {
                return Err(EscrowError::InvalidAmount);
            }
            milestones.push_back(Milestone {
                amount,
                approved: false,
                released: false,
            });
        }

        let count = milestones.len();
        env.storage().instance().set(
            &DataKey::Config,
            &EscrowConfig {
                buyer: buyer.clone(),
                seller,
                approver,
                token,
            },
        );
        env.storage().persistent().set(&DataKey::Balance, &0i128);
        env.storage()
            .persistent()
            .set(&DataKey::Milestones, &milestones);

        Initialized {
            buyer,
            milestone_count: count,
        }
        .publish(&env);
        Ok(())
    }

    /// Deposit funds into the escrow. Callable by the buyer.
    ///
    /// Errors `NotInitialized`, `InvalidAmount` (`amount <= 0`).
    pub fn deposit(env: Env, amount: i128) -> Result<(), EscrowError> {
        let config = get_config(&env).ok_or(EscrowError::NotInitialized)?;
        config.buyer.require_auth();

        if amount <= 0 {
            return Err(EscrowError::InvalidAmount);
        }

        let this = env.current_contract_address();
        TokenClient::new(&env, &config.token).transfer(&config.buyer, &this, &amount);
        let balance = get_balance(&env) + amount;
        env.storage().persistent().set(&DataKey::Balance, &balance);

        Deposited {
            from: config.buyer,
            amount,
            balance,
        }
        .publish(&env);
        Ok(())
    }

    /// Approve a milestone. Callable by the approver.
    ///
    /// Errors `NotInitialized`, `BadIndex`, `AlreadyApproved`.
    pub fn approve_milestone(env: Env, index: u32) -> Result<(), EscrowError> {
        let config = get_config(&env).ok_or(EscrowError::NotInitialized)?;
        config.approver.require_auth();

        let mut milestones = get_milestones(&env);
        let mut milestone = milestones.get(index).ok_or(EscrowError::BadIndex)?;
        if milestone.approved {
            return Err(EscrowError::AlreadyApproved);
        }
        milestone.approved = true;
        milestones.set(index, milestone);
        env.storage()
            .persistent()
            .set(&DataKey::Milestones, &milestones);

        MilestoneApproved {
            approver: config.approver,
            index,
        }
        .publish(&env);
        Ok(())
    }

    /// Release an approved milestone's amount to the seller. Callable by the
    /// seller.
    ///
    /// The balance is decremented and written before the token transfer.
    ///
    /// Errors `NotInitialized`, `BadIndex`, `NotApproved`, `AlreadyReleased`,
    /// `InsufficientBalance`.
    pub fn release(env: Env, index: u32) -> Result<(), EscrowError> {
        let config = get_config(&env).ok_or(EscrowError::NotInitialized)?;
        config.seller.require_auth();

        let mut milestones = get_milestones(&env);
        let mut milestone = milestones.get(index).ok_or(EscrowError::BadIndex)?;
        if !milestone.approved {
            return Err(EscrowError::NotApproved);
        }
        if milestone.released {
            return Err(EscrowError::AlreadyReleased);
        }
        let balance = get_balance(&env);
        if balance < milestone.amount {
            return Err(EscrowError::InsufficientBalance);
        }

        let amount = milestone.amount;
        env.storage()
            .persistent()
            .set(&DataKey::Balance, &(balance - amount));
        milestone.released = true;
        milestones.set(index, milestone);
        env.storage()
            .persistent()
            .set(&DataKey::Milestones, &milestones);

        let this = env.current_contract_address();
        TokenClient::new(&env, &config.token).transfer(&this, &config.seller, &amount);

        MilestoneReleased {
            seller: config.seller,
            index,
            amount,
        }
        .publish(&env);
        Ok(())
    }

    /// Extend the TTL of the escrow's instance entry, balance, and milestone
    /// list, and record the keeper. Permissionless.
    pub fn extend_all(env: Env, keeper: Address) -> Result<u32, MaintainableError> {
        __lk_extend_all(&env, keeper)
    }

    /// Return the last maintenance record. Errors `NotMaintained` if none.
    pub fn lk_state(env: Env) -> Result<MaintenanceState, MaintainableError> {
        maintainable::lk_state(&env)
    }
}
