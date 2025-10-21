use cosmwasm_std::{Addr, Uint128};
use cw_storage_plus::{Item, Map};
use lsm_types::{Config, Staker, State, VotingSession};
use serde::{Deserialize, Serialize};

/// Contract configuration
pub const CONFIG: Item<Config> = Item::new("config");

/// Global state (total staked, global reward index)
pub const STATE: Item<State> = Item::new("state");

/// Map of staker address to their staking info
pub const STAKERS: Map<&Addr, Staker> = Map::new("stakers");

/// Map of proposal_id to VotingSession
pub const VOTING_SESSIONS: Map<u64, VotingSession> = Map::new("voting_sessions");

/// Global pause flag - true when any voting session is active
pub const IS_PAUSED: Item<bool> = Item::new("is_paused");

/// Temporary state for active reward claim
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ActiveClaim {
    /// User who initiated the claim
    pub claimer: Addr,
    /// Contract balance before claiming rewards
    pub balance_before: Uint128,
    /// Global reward index before claiming
    pub global_index_before: cosmwasm_std::Decimal256,
    /// If this is part of a withdrawal (Some(amount)) or just a claim (None)
    pub withdraw_amount: Option<Uint128>,
}

pub const ACTIVE_CLAIM: Item<ActiveClaim> = Item::new("active_claim");

/// Temporary state for active voting power rental
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ActiveRental {
    /// Proposal ID for the rental
    pub proposal_id: u64,
    /// Vote option for the rental
    pub vote_option: i32,
}

pub const ACTIVE_RENTAL: Item<ActiveRental> = Item::new("active_rental");

/// Temporary state for active withdrawal
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct ActiveWithdraw {
    /// User who initiated the withdrawal
    pub withdrawer: Addr,
    /// Amount of tokens being withdrawn
    pub amount: Uint128,
}

pub const ACTIVE_WITHDRAW: Item<ActiveWithdraw> = Item::new("active_withdraw");
