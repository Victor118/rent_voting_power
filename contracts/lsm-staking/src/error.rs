use cosmwasm_std::{StdError, Uint128};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Invalid LSM shares: {reason}")]
    InvalidLsmShares { reason: String },

    #[error("Validator not found: {validator}")]
    ValidatorNotFound { validator: String },

    #[error("Invalid validator: {validator}, expected: {expected}")]
    InvalidValidator { validator: String, expected: String },

    #[error("Invalid funds: expected {expected} denom")]
    InvalidFunds { expected: String },

    #[error("Insufficient staked amount")]
    InsufficientStakedAmount {},

    #[error("No rewards to claim")]
    NoRewards {},

    #[error("Amount cannot be zero")]
    ZeroAmount {},

    #[error("Unexpected rewards amount: expected {expected}, got {actual}")]
    UnexpectedRewardsAmount { expected: String, actual: String },

    #[error("Insufficient balance: available {available}, required {required}")]
    InsufficientBalance {
        available: Uint128,
        required: Uint128,
    },

    #[error("Maximum cap reached: cap {cap}, current {current}, attempting to add {attempting}")]
    MaxCapReached {
        cap: Uint128,
        current: Uint128,
        attempting: Uint128,
    },

    #[error("Contract is paused")]
    ContractPaused {},

    #[error("Voting session already exists for proposal {proposal_id}")]
    VotingSessionExists { proposal_id: u64 },

    #[error("Voting session not found for proposal {proposal_id}")]
    VotingSessionNotFound { proposal_id: u64 },

    #[error("Cannot unpause: {active_count} voting sessions still active")]
    CannotUnpause { active_count: u64 },

    #[error("Invalid locker: sender {sender} is not registered for proposal {proposal_id} option {vote_option}")]
    InvalidLocker {
        sender: String,
        proposal_id: u64,
        vote_option: i32,
    },

    #[error("Proposal {proposal_id} is still active (status: {status})")]
    ProposalStillActive { proposal_id: u64, status: String },

    #[error("Insufficient staked tokens: available {available}, required {required}")]
    InsufficientStakedTokens {
        available: Uint128,
        required: Uint128,
    },

    #[error("No voting session found for proposal {proposal_id}")]
    NoVotingSession { proposal_id: u64 },

    #[error("Locker not found for proposal {proposal_id} and vote option {vote_option}")]
    LockerNotFound { proposal_id: u64, vote_option: i32 },
}
