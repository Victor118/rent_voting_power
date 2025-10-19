use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Uint128};

#[cw_serde]
pub struct InstantiateMsg {
    /// The proposal ID to vote on
    pub proposal_id: u64,
    /// The vote option (VoteOption enum value: 1=Yes, 2=Abstain, 3=No, 4=NoWithVeto, or custom option for multi-choice)
    pub vote_option: i32,
    /// The validator address that this contract will manage LSM shares for
    pub validator: String,
    /// The manager address (only address allowed to deposit and destroy)
    pub manager: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Deposit LSM shares to be redeemed and increase voting power
    /// Only callable by manager
    DepositLsmShares {},

    /// Destroy the contract after proposal is finished
    /// Claims rewards, tokenizes all delegations, and sends everything to manager
    /// Only callable by manager
    Destroy {},
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Get contract configuration
    #[returns(ConfigResponse)]
    Config {},

    /// Get total voting power (total staked amount)
    #[returns(TotalVotingPowerResponse)]
    TotalVotingPower {},
}

#[cw_serde]
pub struct ConfigResponse {
    pub proposal_id: u64,
    pub vote_option: i32,
    pub validator: String,
    pub manager: Addr,
    pub total_staked: Uint128,
    pub has_voted: bool,
}

#[cw_serde]
pub struct TotalVotingPowerResponse {
    pub total_staked: Uint128,
}

#[cw_serde]
pub struct Config {
    pub proposal_id: u64,
    pub vote_option: i32,
    pub validator: String,
    pub manager: Addr,
}

#[cw_serde]
pub struct State {
    /// Total amount staked (voting power)
    pub total_staked: Uint128,
    /// Whether the initial vote has been cast
    pub has_voted: bool,
}

impl State {
    pub fn new() -> Self {
        Self {
            total_staked: Uint128::zero(),
            has_voted: false,
        }
    }
}

/// Helper struct to hold LSM share information
#[cw_serde]
pub struct LsmShareInfo {
    pub validator: String,
    pub record_id: String,
}
