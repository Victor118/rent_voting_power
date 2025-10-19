use cosmwasm_std::StdError;
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

    #[error("Amount cannot be zero")]
    ZeroAmount {},

    #[error("Proposal not finished: {proposal_id}")]
    ProposalNotFinished { proposal_id: u64 },

    #[error("No delegations to tokenize")]
    NoDelegations {},

    #[error("Proposal {proposal_id} is not in voting period (status: {status})")]
    ProposalNotInVoting { proposal_id: u64, status: String },
}
