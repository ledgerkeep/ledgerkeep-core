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
    contract, contractclient, contracterror, contractevent, contractimpl, contracttype,
    token::TokenClient, Address, Bytes, Env, Vec,
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

/// The one registry entry point this escrow calls, declared as a client
/// interface.
///
/// This is deliberately not a dependency on the `registry` crate. The registry
/// is a contract, and linking a contract crate into another contract copies its
/// exported functions and contract spec into the resulting wasm: the escrow
/// would export `register`, `deregister`, `update` and a second
/// `__constructor`, roughly tripling in size. Integrating against the interface
/// instead is also what an outside adopter would do, since they will not have
/// the registry's source.
///
/// The cost is that nothing checks this signature against the deployed
/// registry. A mismatch compiles and fails at invocation. It must stay in step
/// with `Registry::register`. Errors are declared away here — the generated
/// `try_register` returns them; plain `register` reverts on them.
#[contractclient(name = "RegistryClient")]
pub trait Registrar {
    fn register(env: Env, contract: Address, keys_xdr: Vec<Bytes>, threshold: u32, extend_to: u32);
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

    /// Publish this escrow's maintenance manifest to a LedgerKeep registry.
    ///
    /// Callable by the buyer. The buyer's funds are the ones held here, so the
    /// buyer is who loses if this contract's state is archived, and is who
    /// decides where it is advertised for maintenance. Without that gate anyone
    /// could make the escrow register itself under a manifest of their choosing.
    ///
    /// This has to be a function on the escrow rather than a CLI call.
    /// `Registry::register` calls `require_auth()` on the address being
    /// registered, and no private key exists for a contract address, so no
    /// account can register a contract on its behalf. The cross-call below
    /// originates inside the escrow, which makes
    /// `env.current_contract_address()` the invoker and satisfies that check.
    /// Every protocol adopting the standard needs a function shaped like this
    /// one.
    ///
    /// `keys_xdr` is not checked here or by the registry. Nothing on-chain
    /// verifies that it describes the keys the `impl_maintainable!` above
    /// actually extends, so a caller can publish a manifest that has drifted
    /// from the contract. `ledgerkeep-cli` is what catches that, by simulating
    /// `extend_all` and diffing the TTLs it observes against the manifest.
    ///
    /// Errors `NotInitialized`. Registry errors (`AlreadyRegistered`,
    /// `EmptyManifest`, `InvalidParams`) propagate from the cross-call and
    /// revert this invocation.
    pub fn register_with(
        env: Env,
        registry: Address,
        keys_xdr: Vec<Bytes>,
        threshold: u32,
        extend_to: u32,
    ) -> Result<(), EscrowError> {
        let config = get_config(&env).ok_or(EscrowError::NotInitialized)?;
        config.buyer.require_auth();

        let this = env.current_contract_address();
        RegistryClient::new(&env, &registry).register(&this, &keys_xdr, &threshold, &extend_to);
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
