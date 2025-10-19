use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, Decimal256, Uint128, Uint256};

#[cw_serde]
pub struct InstantiateMsg {
    /// The base staking denom (e.g., "uatom")
    pub staking_denom: String,
    /// Contract owner/admin
    pub owner: String,
    /// The validator address that this contract will manage LSM shares for
    pub validator: String,
    /// Optional maximum cap for total staked amount
    pub max_cap: Option<Uint128>,
    /// Code ID of the ProposalOptionLocker contract
    pub locker_code_id: u64,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Deposit LSM shares to the contract
    /// The shares will be redeemed and staked
    DepositLsmShares {},

    /// Claim accumulated rewards for the caller
    ClaimRewards {},

    /// Deposit additional rewards to be distributed
    /// This increases the reward pool
    DepositRewards {},

    /// Withdraw staked tokens (unstake from validators)
    /// This initiates the unbonding period
    Withdraw { amount: Uint128, validator: String },

    /// Update contract configuration (owner only)
    UpdateConfig {
        owner: Option<String>,
        max_cap: Option<Uint128>,
    },

    /// Create voting lockers for a governance proposal (owner only)
    /// This will pause deposits and withdrawals
    CreateVotingLockers { proposal_id: u64 },

    /// Destroy voting lockers for a governance proposal (owner only)
    /// This will unpause if no other active voting sessions exist
    DestroyVotingLockers { proposal_id: u64 },

    /// Return LSM shares from a voting locker after destroy
    /// This redeems the shares without modifying total_staked or global_reward_index
    /// Only callable by registered voting lockers
    ReturnLsmShares {
        proposal_id: u64,
        vote_option: i32,
    },

    /// Rent voting power for a governance proposal
    /// Receives ATOM in funds and tokenizes shares to deposit to the specified locker
    RentVotingPower {
        proposal_id: u64,
        vote_option: i32,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Get contract configuration
    #[returns(ConfigResponse)]
    Config {},

    /// Get staker information including pending rewards
    #[returns(StakerInfoResponse)]
    StakerInfo { address: String },

    /// Get total staked amount
    #[returns(TotalStakedResponse)]
    TotalStaked {},

    /// Get global reward index
    #[returns(RewardIndexResponse)]
    RewardIndex {},

    /// Get list of stakers with pagination
    #[returns(StakersResponse)]
    Stakers {
        start_after: Option<String>,
        limit: Option<u32>,
    },
}

#[cw_serde]
pub struct ConfigResponse {
    pub owner: Addr,
    pub staking_denom: String,
    pub validator: String,
    pub max_cap: Option<Uint128>,
    pub locker_code_id: u64,
    pub total_staked: Uint128,
    pub global_reward_index: Decimal256,
    pub is_paused: bool,
}

/// Helper struct to hold LSM share information
#[cw_serde]
pub struct LsmShareInfo {
    pub validator: String,
    pub record_id: String,
}

/// Voting session for a governance proposal
#[cw_serde]
pub struct VotingSession {
    pub proposal_id: u64,
    /// List of (vote_option, locker_address) pairs
    pub locker_addresses: Vec<(i32, Addr)>,
    pub is_active: bool,
}

#[cw_serde]
pub struct StakerInfoResponse {
    pub address: Addr,
    pub staked_amount: Uint128,
    pub reward_index: Decimal256,
    pub pending_rewards: Uint128,
}

#[cw_serde]
pub struct TotalStakedResponse {
    pub total_staked: Uint128,
}

#[cw_serde]
pub struct RewardIndexResponse {
    pub global_reward_index: Decimal256,
}

#[cw_serde]
pub struct StakersResponse {
    pub stakers: Vec<StakerInfoResponse>,
}

/// State stored for each staker
#[cw_serde]
pub struct Staker {
    /// Amount of tokens staked by this user
    pub staked_amount: Uint128,
    /// Reward index at the last update for this user
    pub reward_index: Decimal256,
}

impl Staker {
    pub fn new() -> Self {
        Self {
            staked_amount: Uint128::zero(),
            reward_index: Decimal256::zero(),
        }
    }

    /// Calculate pending rewards based on current global index
    pub fn calculate_pending_rewards(&self, global_index: Decimal256) -> Uint128 {
        if self.staked_amount.is_zero() {
            return Uint128::zero();
        }

        // rewards = staked_amount * (global_index - user_index)
        let index_diff = global_index
            .checked_sub(self.reward_index)
            .unwrap_or_default();
        let new_rewards = Uint256::from(self.staked_amount)
            .checked_mul(index_diff.atomics())
            .unwrap_or_default()
            / Uint256::from(10u128.pow(18)); // Decimal256 has 18 decimals

        Uint128::try_from(new_rewards).unwrap_or_default()
    }

    /// Update user's reward index (called after claiming or when staked amount changes)
    pub fn update_index(&mut self, global_index: Decimal256) {
        self.reward_index = global_index;
    }
}

#[cw_serde]
pub struct Config {
    pub owner: Addr,
    pub staking_denom: String,
    pub validator: String,
    pub max_cap: Option<Uint128>,
    pub locker_code_id: u64,
}

#[cw_serde]
pub struct State {
    /// Total amount staked in the contract
    pub total_staked: Uint128,
    /// Global reward index (cumulative rewards per token)
    pub global_reward_index: Decimal256,
}

impl State {
    pub fn new() -> Self {
        Self {
            total_staked: Uint128::zero(),
            global_reward_index: Decimal256::zero(),
        }
    }

    /// Update global reward index when new rewards are added
    pub fn add_rewards(&mut self, reward_amount: Uint128) {
        if self.total_staked.is_zero() {
            return;
        }

        // new_index = old_index + (reward_amount / total_staked)
        let reward_per_token = Decimal256::from_ratio(
            Uint256::from(reward_amount),
            Uint256::from(self.total_staked),
        );

        self.global_reward_index = self
            .global_reward_index
            .checked_add(reward_per_token)
            .unwrap_or(self.global_reward_index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_staker_calculate_rewards() {
        let mut staker = Staker::new();
        staker.staked_amount = Uint128::new(1000);

        // Global index increased by 0.1 (meaning 0.1 tokens reward per staked token)
        let global_index = Decimal256::from_ratio(1u128, 10u128);
        let rewards = staker.calculate_pending_rewards(global_index);

        // Expected: 1000 * 0.1 = 100
        assert_eq!(rewards, Uint128::new(100));

        // After updating index, pending rewards should be 0
        staker.update_index(global_index);
        let rewards_after = staker.calculate_pending_rewards(global_index);
        assert_eq!(rewards_after, Uint128::zero());
    }

    #[test]
    fn test_state_add_rewards() {
        let mut state = State::new();
        state.total_staked = Uint128::new(1000);

        // Add 100 tokens as rewards
        state.add_rewards(Uint128::new(100));

        // Expected: 100 / 1000 = 0.1
        let expected = Decimal256::from_ratio(1u128, 10u128);
        assert_eq!(state.global_reward_index, expected);
    }
}
