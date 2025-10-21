use cosmwasm_std::{
    coins, entry_point, to_json_binary, BalanceResponse, BankMsg, BankQuery, Binary, Coin,
    CosmosMsg, Deps, DepsMut, DistributionMsg, Env, MessageInfo, Order, QuerierWrapper, Reply,
    Response, StakingMsg, StdResult, SubMsg, Uint128,
};
use cw2::set_contract_version;
use cw_storage_plus::Bound;
use lsm_types::{
    Config, ConfigResponse, ExecuteMsg, InstantiateMsg, LsmShareInfo, QueryMsg,
    RewardIndexResponse, Staker, StakerInfoResponse, StakersResponse, State, TotalStakedResponse,
};

use crate::error::ContractError;
use crate::state::{
    ActiveClaim, ActiveRental, ActiveWithdraw, ACTIVE_CLAIM, ACTIVE_RENTAL, ACTIVE_WITHDRAW,
    CONFIG, IS_PAUSED, STAKERS, STATE, VOTING_SESSIONS,
};

const CONTRACT_NAME: &str = "crates.io:lsm-staking";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;

// Reply IDs
const REPLY_CLAIM_REWARDS: u64 = 1;
const REPLY_TOKENIZE_SHARES_RENTAL: u64 = 2;
const REPLY_TOKENIZE_SHARES_WITHDRAW: u64 = 3;

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let owner = deps.api.addr_validate(&msg.owner)?;

    // Verify that the validator exists on chain
    verify_validator_exists(&deps.querier, &msg.validator)?;

    let config = Config {
        owner: owner.clone(),
        staking_denom: msg.staking_denom,
        validator: msg.validator.clone(),
        max_cap: msg.max_cap,
        locker_code_id: msg.locker_code_id,
    };

    CONFIG.save(deps.storage, &config)?;
    STATE.save(deps.storage, &State::new())?;
    IS_PAUSED.save(deps.storage, &false)?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("owner", owner)
        .add_attribute("validator", msg.validator))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::DepositLsmShares {} => execute_deposit_lsm_shares(deps, env, info),
        ExecuteMsg::ClaimRewards {} => execute_claim_rewards(deps, env, info),
        ExecuteMsg::DepositRewards {} => execute_deposit_rewards(deps, info),
        ExecuteMsg::Withdraw { amount, validator } => {
            execute_withdraw(deps, env, info, amount, validator)
        }
        ExecuteMsg::UpdateConfig { owner, max_cap } => {
            execute_update_config(deps, info, owner, max_cap)
        }
        ExecuteMsg::CreateVotingLockers { proposal_id } => {
            execute_create_voting_lockers(deps, env, info, proposal_id)
        }
        ExecuteMsg::DestroyVotingLockers { proposal_id } => {
            execute_destroy_voting_lockers(deps, info, proposal_id)
        }
        ExecuteMsg::ReturnLsmShares {
            proposal_id,
            vote_option,
        } => execute_return_lsm_shares(deps, env, info, proposal_id, vote_option),
        ExecuteMsg::RentVotingPower {
            proposal_id,
            vote_option,
        } => execute_rent_voting_power(deps, env, info, proposal_id, vote_option),
    }
}

/// Deposit LSM shares which will be redeemed and staked
pub fn execute_deposit_lsm_shares(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    // Check if contract is paused
    let is_paused = IS_PAUSED.load(deps.storage)?;
    if is_paused {
        return Err(ContractError::ContractPaused {});
    }

    let mut state = STATE.load(deps.storage)?;
    let config = CONFIG.load(deps.storage)?;
    // Verify exactly one token is sent
    if info.funds.len() != 1 {
        return Err(ContractError::InvalidLsmShares {
            reason: "Must send exactly one token".to_string(),
        });
    }

    let lsm_share = &info.funds[0];

    if lsm_share.amount.is_zero() {
        return Err(ContractError::ZeroAmount {});
    }

    // Parse and validate LSM denom
    let lsm_info = parse_lsm_denom(&lsm_share.denom)?;

    // Verify validator exists
    if lsm_info.validator != config.validator {
        return Err(ContractError::InvalidValidator {
            validator: lsm_info.validator,
            expected: config.validator,
        });
    }

    // Check if adding this amount would exceed max_cap
    if let Some(max_cap) = config.max_cap {
        let new_total = state
            .total_staked
            .checked_add(lsm_share.amount)
            .map_err(|e| ContractError::Std(e.into()))?;
        if new_total > max_cap {
            return Err(ContractError::MaxCapReached {
                cap: max_cap,
                current: state.total_staked,
                attempting: lsm_share.amount,
            });
        }
    }

    // Update staker info
    let mut staker = STAKERS
        .may_load(deps.storage, &info.sender)?
        .unwrap_or_else(Staker::new);

    // Update reward index before changing staked amount
    staker.update_index(state.global_reward_index);

    // Add the LSM share amount to the staker's staked amount
    staker.staked_amount += lsm_share.amount;

    // Update total staked
    state.total_staked += lsm_share.amount;

    STAKERS.save(deps.storage, &info.sender, &staker)?;
    STATE.save(deps.storage, &state)?;

    // Create MsgRedeemTokensForShares message from liquid staking module
    // This converts LSM shares back to a native delegation
    let redeem_msg = create_redeem_tokens_msg(
        env.contract.address.to_string(),
        lsm_share.denom.clone(),
        lsm_share.amount,
    )?;

    Ok(Response::new()
        .add_message(redeem_msg)
        .add_attribute("method", "deposit_lsm_shares")
        .add_attribute("sender", info.sender)
        .add_attribute("validator", lsm_info.validator)
        .add_attribute("record_id", lsm_info.record_id)
        .add_attribute("amount", lsm_share.amount))
}

/// Claim accumulated rewards
/// This will:
/// 1. Verify user has staked tokens
/// 2. Query current balance
/// 3. Withdraw rewards from the single validator
/// 4. In the reply, update global index, calculate user rewards, and distribute to user
pub fn execute_claim_rewards(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;

    // Verify user has staked tokens (we'll calculate rewards in the reply)
    let _staker = STAKERS
        .load(deps.storage, &info.sender)
        .map_err(|_| ContractError::NoRewards {})?;

    // Query current balance before claiming
    let balance_query: BalanceResponse = deps.querier.query(
        &BankQuery::Balance {
            address: env.contract.address.to_string(),
            denom: config.staking_denom.clone(),
        }
        .into(),
    )?;

    // Store active claim state with current global index
    ACTIVE_CLAIM.save(
        deps.storage,
        &ActiveClaim {
            claimer: info.sender.clone(),
            balance_before: balance_query.amount.amount,
            global_index_before: state.global_reward_index,
        },
    )?;

    // Create withdraw reward message for the single validator
    let withdraw_msg = SubMsg::reply_on_success(
        CosmosMsg::Distribution(DistributionMsg::WithdrawDelegatorReward {
            validator: config.validator.clone(),
        }),
        REPLY_CLAIM_REWARDS,
    );

    Ok(Response::new()
        .add_submessage(withdraw_msg)
        .add_attribute("method", "claim_rewards")
        .add_attribute("sender", info.sender)
        .add_attribute("validator", config.validator))
}

/// Deposit additional rewards to be distributed among stakers
pub fn execute_deposit_rewards(
    deps: DepsMut,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // Find the staking token in the sent funds
    let reward = info
        .funds
        .iter()
        .find(|coin| coin.denom == config.staking_denom)
        .ok_or(ContractError::InvalidFunds {
            expected: config.staking_denom.clone(),
        })?;

    if reward.amount.is_zero() {
        return Err(ContractError::ZeroAmount {});
    }

    // Update global reward index using the cumulative reward algorithm
    state.add_rewards(reward.amount);
    STATE.save(deps.storage, &state)?;

    Ok(Response::new()
        .add_attribute("method", "deposit_rewards")
        .add_attribute("sender", info.sender)
        .add_attribute("amount", reward.amount))
}

/// Withdraw staked tokens (initiate unstaking)
/// This will automatically claim any pending rewards before unstaking
/// The validator is automatically set to the configured validator
pub fn execute_withdraw(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount: Uint128,
    _validator: String,
) -> Result<Response, ContractError> {
    // Check if contract is paused
    let is_paused = IS_PAUSED.load(deps.storage)?;
    if is_paused {
        return Err(ContractError::ContractPaused {});
    }

    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    if amount.is_zero() {
        return Err(ContractError::ZeroAmount {});
    }

    let mut staker = STAKERS
        .load(deps.storage, &info.sender)
        .map_err(|_| ContractError::InsufficientStakedAmount {})?;

    // Query the current delegation to get the actual token amount
    let delegation_response = deps
        .querier
        .query_delegation(env.contract.address.clone(), config.validator.clone())?;

    let delegated_tokens = delegation_response
        .map(|d| d.amount.amount)
        .unwrap_or(Uint128::zero());

    // Calculate user's share of tokens based on their shares proportion
    // user_tokens = (delegated_tokens * user_shares) / total_shares
    // Using Decimal256 for precision
    let user_available_tokens = if state.total_staked.is_zero() {
        Uint128::zero()
    } else {
        let delegated_decimal = cosmwasm_std::Decimal256::from_ratio(delegated_tokens, 1u128);
        let user_shares_decimal = cosmwasm_std::Decimal256::from_ratio(staker.staked_amount, 1u128);
        let total_shares_decimal = cosmwasm_std::Decimal256::from_ratio(state.total_staked, 1u128);

        let user_tokens_decimal = delegated_decimal
            .checked_mul(user_shares_decimal)
            .map_err(|e| {
                ContractError::Std(cosmwasm_std::StdError::generic_err(format!(
                    "Decimal multiplication error: {}",
                    e
                )))
            })?
            .checked_div(total_shares_decimal)
            .map_err(|e| {
                ContractError::Std(cosmwasm_std::StdError::generic_err(format!(
                    "Decimal division error: {}",
                    e
                )))
            })?;

        // Convert back to Uint128, rounding down
        user_tokens_decimal
            .atomics()
            .checked_div(cosmwasm_std::Uint256::from(1_000_000_000_000_000_000u128))
            .map_err(|e| ContractError::Std(e.into()))?
            .try_into()
            .map_err(|_| {
                ContractError::Std(cosmwasm_std::StdError::generic_err(
                    "Overflow converting to Uint128",
                ))
            })?
    };

    // Check if user has enough tokens available
    if user_available_tokens < amount {
        return Err(ContractError::InsufficientStakedAmount {});
    }

    // Calculate how many shares to deduct based on the token amount requested
    // shares_to_deduct = (amount * total_shares) / delegated_tokens
    // Using Decimal256 for precision
    let shares_to_deduct = if delegated_tokens.is_zero() {
        staker.staked_amount // If no delegation, deduct all shares
    } else {
        let amount_decimal = cosmwasm_std::Decimal256::from_ratio(amount, 1u128);
        let total_shares_decimal = cosmwasm_std::Decimal256::from_ratio(state.total_staked, 1u128);
        let delegated_decimal = cosmwasm_std::Decimal256::from_ratio(delegated_tokens, 1u128);

        let shares_decimal = amount_decimal
            .checked_mul(total_shares_decimal)
            .map_err(|e| {
                ContractError::Std(cosmwasm_std::StdError::generic_err(format!(
                    "Decimal multiplication error: {}",
                    e
                )))
            })?
            .checked_div(delegated_decimal)
            .map_err(|e| {
                ContractError::Std(cosmwasm_std::StdError::generic_err(format!(
                    "Decimal division error: {}",
                    e
                )))
            })?;

        // Convert back to Uint128, rounding up to ensure we deduct enough shares
        let shares_atomics = shares_decimal.atomics();
        let divisor = cosmwasm_std::Uint256::from(1_000_000_000_000_000_000u128);
        let quotient = shares_atomics
            .checked_div(divisor)
            .map_err(|e| ContractError::Std(e.into()))?;
        let remainder = shares_atomics
            .checked_rem(divisor)
            .map_err(|e| ContractError::Std(e.into()))?;

        // Round up if there's a remainder
        let shares_u256 = if remainder.is_zero() {
            quotient
        } else {
            quotient
                .checked_add(cosmwasm_std::Uint256::from(1u128))
                .map_err(|e| ContractError::Std(e.into()))?
        };

        shares_u256.try_into().map_err(|_| {
            ContractError::Std(cosmwasm_std::StdError::generic_err(
                "Overflow converting to Uint128",
            ))
        })?
    };

    // Calculate pending rewards BEFORE changing staked amount
    let user_rewards = staker.calculate_pending_rewards(state.global_reward_index);

    // Update staker and state
    staker.staked_amount = staker.staked_amount.saturating_sub(shares_to_deduct);
    staker.update_index(state.global_reward_index);
    state.total_staked = state.total_staked.saturating_sub(shares_to_deduct);

    STAKERS.save(deps.storage, &info.sender, &staker)?;
    STATE.save(deps.storage, &state)?;

    let mut messages = vec![];
    let mut response = Response::new()
        .add_attribute("method", "withdraw")
        .add_attribute("sender", info.sender.to_string())
        .add_attribute("amount", amount)
        .add_attribute("shares_deducted", shares_to_deduct)
        .add_attribute("validator", config.validator.clone());

    // If user has rewards, send them
    if !user_rewards.is_zero() {
        let send_rewards_msg = CosmosMsg::Bank(BankMsg::Send {
            to_address: info.sender.to_string(),
            amount: coins(user_rewards.u128(), config.staking_denom.clone()),
        });
        messages.push(send_rewards_msg);
        response = response.add_attribute("rewards_claimed", user_rewards);
    }

    // Store active withdraw info for the reply handler
    ACTIVE_WITHDRAW.save(
        deps.storage,
        &ActiveWithdraw {
            withdrawer: info.sender.clone(),
            amount,
        },
    )?;

    // Create tokenize shares message to convert delegation to LSM shares
    // The reply handler will send the LSM shares to the user
    let tokenize_msg = create_tokenize_shares_msg(
        env.contract.address.to_string(),
        config.validator,
        amount,
        env.contract.address.to_string(), // Send to self first, then forward in reply
    )?;

    Ok(response
        .add_messages(messages)
        .add_submessage(SubMsg::reply_on_success(
            tokenize_msg,
            REPLY_TOKENIZE_SHARES_WITHDRAW,
        )))
}

/// Update contract configuration (owner only)
pub fn execute_update_config(
    deps: DepsMut,
    info: MessageInfo,
    owner: Option<String>,
    max_cap: Option<Uint128>,
) -> Result<Response, ContractError> {
    let mut config = CONFIG.load(deps.storage)?;

    // Check if sender is owner
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    let mut response = Response::new().add_attribute("method", "update_config");

    if let Some(owner) = owner {
        let new_owner = deps.api.addr_validate(&owner)?;
        config.owner = new_owner.clone();
        response = response.add_attribute("new_owner", new_owner);
    }

    if let Some(new_max_cap) = max_cap {
        config.max_cap = Some(new_max_cap);
        response = response.add_attribute("new_max_cap", new_max_cap.to_string());
    }

    CONFIG.save(deps.storage, &config)?;

    Ok(response)
}

/// Create voting lockers for a governance proposal (owner only)
/// This queries the proposal to get vote options and creates a locker for each
pub fn execute_create_voting_lockers(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    proposal_id: u64,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Only owner can create voting lockers
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    // Check if voting session already exists for this proposal
    if VOTING_SESSIONS.has(deps.storage, proposal_id) {
        return Err(ContractError::VotingSessionExists { proposal_id });
    }

    // Query the governance proposal to get vote options
    // For Cosmos SDK governance, standard options are: 1=Yes, 2=Abstain, 3=No, 4=NoWithVeto
    // We'll create a locker for each option
    let vote_options = vec![1i32, 2i32, 3i32, 4i32]; // Yes, Abstain, No, NoWithVeto

    use cosmwasm_std::WasmMsg;
    use proposal_locker_types::InstantiateMsg as LockerInstantiateMsg;

    let mut locker_addresses: Vec<(i32, cosmwasm_std::Addr)> = Vec::new();
    let mut messages: Vec<CosmosMsg> = Vec::new();

    // Create a locker for each vote option
    for vote_option in &vote_options {
        let locker_init_msg = LockerInstantiateMsg {
            proposal_id,
            vote_option: *vote_option,
            validator: config.validator.clone(),
            manager: env.contract.address.to_string(),
        };

        // Calculate the locker address deterministically
        // We'll use the contract address as label with the vote option
        let label = format!("proposal_{}_option_{}", proposal_id, vote_option);

        let instantiate_msg = WasmMsg::Instantiate {
            admin: Some(env.contract.address.to_string()),
            code_id: config.locker_code_id,
            msg: to_json_binary(&locker_init_msg)?,
            funds: vec![],
            label: label.clone(),
        };

        messages.push(CosmosMsg::Wasm(instantiate_msg));

        // For now, we'll store a placeholder address. The actual address will be set
        // in a reply handler or we can calculate it deterministically
        // In production, you'd want to use a reply to get the actual instantiated address
        let locker_addr = deps
            .api
            .addr_validate(&format!("locker_{}_{}", proposal_id, vote_option))?;
        locker_addresses.push((*vote_option, locker_addr));
    }

    // Create and save the voting session
    let voting_session = lsm_types::VotingSession {
        proposal_id,
        locker_addresses,
        is_active: true,
    };

    VOTING_SESSIONS.save(deps.storage, proposal_id, &voting_session)?;

    // Set contract to paused
    IS_PAUSED.save(deps.storage, &true)?;

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("method", "create_voting_lockers")
        .add_attribute("proposal_id", proposal_id.to_string())
        .add_attribute("num_lockers", vote_options.len().to_string()))
}

/// Destroy voting lockers for a governance proposal (owner only)
/// This will call destroy on each locker and unpause if no other voting sessions are active
/// The proposal must be finished (PASSED, REJECTED, FAILED) or no longer exist on-chain
pub fn execute_destroy_voting_lockers(
    deps: DepsMut,
    info: MessageInfo,
    proposal_id: u64,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Only owner can destroy voting lockers
    if info.sender != config.owner {
        return Err(ContractError::Unauthorized {});
    }

    // Verify proposal is finished or doesn't exist anymore
    verify_proposal_finished(&deps.querier, proposal_id)?;

    // Load the voting session
    let mut voting_session = VOTING_SESSIONS
        .load(deps.storage, proposal_id)
        .map_err(|_| ContractError::VotingSessionNotFound { proposal_id })?;

    use cosmwasm_std::WasmMsg;
    use proposal_locker_types::ExecuteMsg as LockerExecuteMsg;

    let mut messages: Vec<CosmosMsg> = Vec::new();

    // Call Destroy on each locker
    for (_vote_option, locker_addr) in &voting_session.locker_addresses {
        let destroy_msg = WasmMsg::Execute {
            contract_addr: locker_addr.to_string(),
            msg: to_json_binary(&LockerExecuteMsg::Destroy {})?,
            funds: vec![],
        };
        messages.push(CosmosMsg::Wasm(destroy_msg));
    }

    // Mark voting session as inactive
    voting_session.is_active = false;
    VOTING_SESSIONS.save(deps.storage, proposal_id, &voting_session)?;

    // Check if there are any other active voting sessions
    let has_active_sessions = VOTING_SESSIONS
        .range(deps.storage, None, None, Order::Ascending)
        .any(|result| {
            if let Ok((_id, session)) = result {
                session.is_active
            } else {
                false
            }
        });

    // Only unpause if no other active sessions exist
    if !has_active_sessions {
        IS_PAUSED.save(deps.storage, &false)?;
    } else {
        // Count active sessions for error reporting
        let active_count = VOTING_SESSIONS
            .range(deps.storage, None, None, Order::Ascending)
            .filter(|result| {
                if let Ok((_id, session)) = result {
                    session.is_active
                } else {
                    false
                }
            })
            .count() as u64;

        return Err(ContractError::CannotUnpause { active_count });
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("method", "destroy_voting_lockers")
        .add_attribute("proposal_id", proposal_id.to_string())
        .add_attribute(
            "num_lockers",
            voting_session.locker_addresses.len().to_string(),
        )
        .add_attribute("unpaused", "true"))
}

/// Return LSM shares from a voting locker
/// This redeems the shares WITHOUT modifying total_staked or global_reward_index
/// because these shares were already counted when the locker was created
pub fn execute_return_lsm_shares(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    proposal_id: u64,
    vote_option: i32,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Load the voting session
    let voting_session = VOTING_SESSIONS
        .load(deps.storage, proposal_id)
        .map_err(|_| ContractError::VotingSessionNotFound { proposal_id })?;

    // Verify that the sender is the registered locker for this proposal and vote option
    let expected_locker = voting_session
        .locker_addresses
        .iter()
        .find(|(option, _)| *option == vote_option)
        .map(|(_, addr)| addr)
        .ok_or(ContractError::InvalidLocker {
            sender: info.sender.to_string(),
            proposal_id,
            vote_option,
        })?;

    if info.sender != *expected_locker {
        return Err(ContractError::InvalidLocker {
            sender: info.sender.to_string(),
            proposal_id,
            vote_option,
        });
    }

    // Verify exactly one token is sent
    if info.funds.len() != 1 {
        return Err(ContractError::InvalidLsmShares {
            reason: "Must send exactly one token".to_string(),
        });
    }

    let lsm_share = &info.funds[0];

    if lsm_share.amount.is_zero() {
        return Err(ContractError::ZeroAmount {});
    }

    // Parse and validate LSM denom
    let lsm_info = parse_lsm_denom(&lsm_share.denom)?;

    // Verify validator matches
    if lsm_info.validator != config.validator {
        return Err(ContractError::InvalidValidator {
            validator: lsm_info.validator,
            expected: config.validator,
        });
    }

    // Create MsgRedeemTokensForShares message
    // IMPORTANT: We do NOT update total_staked or any state because these shares
    // were already counted in the contract's total before being sent to the locker
    let redeem_msg = create_redeem_tokens_msg(
        env.contract.address.to_string(),
        lsm_share.denom.clone(),
        lsm_share.amount,
    )?;

    Ok(Response::new()
        .add_message(redeem_msg)
        .add_attribute("method", "return_lsm_shares")
        .add_attribute("locker", info.sender)
        .add_attribute("proposal_id", proposal_id.to_string())
        .add_attribute("vote_option", vote_option.to_string())
        .add_attribute("amount", lsm_share.amount))
}

/// Rent voting power for a governance proposal
/// Receives ATOM in funds, calculates VP amount, tokenizes shares, and deposits to locker
/// VP_PRICE: 1 VP = 0.1 ATOM (hardcoded for now)
pub fn execute_rent_voting_power(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    proposal_id: u64,
    vote_option: i32,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Verify voting session exists for this proposal
    let voting_session = VOTING_SESSIONS
        .load(deps.storage, proposal_id)
        .map_err(|_| ContractError::NoVotingSession { proposal_id })?;

    // Verify the vote option exists in the voting session
    let locker_addr = voting_session
        .locker_addresses
        .iter()
        .find(|(option, _)| *option == vote_option)
        .map(|(_, addr)| addr)
        .ok_or(ContractError::LockerNotFound {
            proposal_id,
            vote_option,
        })?;

    // Verify exactly one coin is sent and it's the staking denom
    if info.funds.len() != 1 {
        return Err(ContractError::InvalidFunds {
            expected: config.staking_denom.clone(),
        });
    }

    let payment = &info.funds[0];
    if payment.denom != config.staking_denom {
        return Err(ContractError::InvalidFunds {
            expected: config.staking_denom.clone(),
        });
    }

    if payment.amount.is_zero() {
        return Err(ContractError::ZeroAmount {});
    }

    // Calculate VP amount: 1 VP = 0.1 ATOM
    // So VP_amount = ATOM_amount / 0.1 = ATOM_amount * 10
    let vp_amount = payment.amount.checked_mul(Uint128::new(10)).map_err(|_| {
        ContractError::InsufficientBalance {
            available: payment.amount,
            required: Uint128::new(1),
        }
    })?;

    // Query the delegation to get our shares and calculate available tokens
    // We need to account for the shares→tokens ratio which can be < 1 if validator was slashed
    let delegation_response = deps
        .querier
        .query_delegation(env.contract.address.clone(), config.validator.clone())?;

    // The delegation response already contains the token amount (not shares)
    // This is because CosmWasm's query_delegation returns the Coin amount which represents tokens
    let available_tokens = delegation_response
        .map(|d| d.amount.amount)
        .unwrap_or(Uint128::zero());

    // Verify we have enough tokens available to tokenize
    if vp_amount > available_tokens {
        return Err(ContractError::InsufficientStakedTokens {
            available: available_tokens,
            required: vp_amount,
        });
    }

    // Add the rental payment to the global reward index
    // The payment goes into the contract balance and should be distributed as rewards
    let mut state = STATE.load(deps.storage)?;
    state.add_rewards(payment.amount);
    STATE.save(deps.storage, &state)?;

    // Store rental info for the reply handler
    ACTIVE_RENTAL.save(
        deps.storage,
        &ActiveRental {
            proposal_id,
            vote_option,
        },
    )?;

    // Create MsgTokenizeShares to convert delegation to LSM shares
    let tokenize_msg = create_tokenize_shares_msg(
        env.contract.address.to_string(),
        config.validator.clone(),
        vp_amount,
        env.contract.address.to_string(), // Send to self first, then forward in reply
    )?;

    Ok(Response::new()
        .add_submessage(SubMsg::reply_on_success(
            tokenize_msg,
            REPLY_TOKENIZE_SHARES_RENTAL,
        ))
        .add_attribute("method", "rent_voting_power")
        .add_attribute("renter", info.sender)
        .add_attribute("proposal_id", proposal_id.to_string())
        .add_attribute("vote_option", vote_option.to_string())
        .add_attribute("payment", payment.amount)
        .add_attribute("vp_amount", vp_amount)
        .add_attribute("locker", locker_addr))
}

#[entry_point]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_json_binary(&query_config(deps)?),
        QueryMsg::StakerInfo { address } => {
            to_json_binary(&query_staker_info(deps, env.clone(), address)?)
        }
        QueryMsg::TotalStaked {} => to_json_binary(&query_total_staked(deps)?),
        QueryMsg::RewardIndex {} => to_json_binary(&query_reward_index(deps)?),
        QueryMsg::Stakers { start_after, limit } => {
            to_json_binary(&query_stakers(deps, env, start_after, limit)?)
        }
    }
}

/// Calculate the simulated global reward index by querying pending staking rewards
/// This is used in queries to show accurate pending rewards without modifying state
fn calculate_simulated_global_index(
    deps: Deps,
    env: &Env,
    state: &State,
    config: &Config,
) -> StdResult<cosmwasm_std::Decimal256> {
    if state.total_staked.is_zero() {
        return Ok(state.global_reward_index);
    }

    // Query pending staking rewards from the validator
    let pending_rewards = deps
        .querier
        .query_delegation(env.contract.address.clone(), config.validator.clone())?
        .and_then(|delegation| Some(delegation.accumulated_rewards))
        .and_then(|rewards| {
            rewards
                .iter()
                .find(|coin| coin.denom == config.staking_denom)
                .map(|coin| coin.amount)
        })
        .unwrap_or(Uint128::zero());

    // If there are pending rewards, calculate the simulated global index
    if !pending_rewards.is_zero() {
        let reward_per_token = cosmwasm_std::Decimal256::from_ratio(
            cosmwasm_std::Uint256::from(pending_rewards),
            cosmwasm_std::Uint256::from(state.total_staked),
        );

        state
            .global_reward_index
            .checked_add(reward_per_token)
            .or(Ok(state.global_reward_index))
    } else {
        Ok(state.global_reward_index)
    }
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;
    let is_paused = IS_PAUSED.load(deps.storage)?;

    Ok(ConfigResponse {
        owner: config.owner,
        staking_denom: config.staking_denom,
        validator: config.validator,
        max_cap: config.max_cap,
        locker_code_id: config.locker_code_id,
        total_staked: state.total_staked,
        global_reward_index: state.global_reward_index,
        is_paused,
    })
}

fn query_staker_info(deps: Deps, env: Env, address: String) -> StdResult<StakerInfoResponse> {
    let addr = deps.api.addr_validate(&address)?;
    let state = STATE.load(deps.storage)?;
    let config = CONFIG.load(deps.storage)?;

    let staker = STAKERS.load(deps.storage, &addr)?;

    // Calculate simulated global index including pending staking rewards
    let simulated_global_index = calculate_simulated_global_index(deps, &env, &state, &config)?;

    // Calculate pending rewards using the simulated index
    let pending_rewards = staker.calculate_pending_rewards(simulated_global_index);

    Ok(StakerInfoResponse {
        address: addr,
        staked_amount: staker.staked_amount,
        reward_index: staker.reward_index,
        pending_rewards,
    })
}

fn query_total_staked(deps: Deps) -> StdResult<TotalStakedResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(TotalStakedResponse {
        total_staked: state.total_staked,
    })
}

fn query_reward_index(deps: Deps) -> StdResult<RewardIndexResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(RewardIndexResponse {
        global_reward_index: state.global_reward_index,
    })
}

fn query_stakers(
    deps: Deps,
    env: Env,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<StakersResponse> {
    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;
    let state = STATE.load(deps.storage)?;
    let config = CONFIG.load(deps.storage)?;

    // Calculate simulated global index including pending staking rewards
    let simulated_global_index = calculate_simulated_global_index(deps, &env, &state, &config)?;

    let stakers: Vec<StakerInfoResponse> = if let Some(s) = start_after {
        let addr = deps.api.addr_validate(&s)?;
        STAKERS
            .range(
                deps.storage,
                Some(Bound::exclusive(&addr)),
                None,
                Order::Ascending,
            )
            .take(limit)
            .map(|item| {
                let (addr, staker) = item?;
                let pending_rewards = staker.calculate_pending_rewards(simulated_global_index);

                Ok(StakerInfoResponse {
                    address: addr,
                    staked_amount: staker.staked_amount,
                    reward_index: staker.reward_index,
                    pending_rewards,
                })
            })
            .collect::<StdResult<Vec<_>>>()?
    } else {
        STAKERS
            .range(deps.storage, None, None, Order::Ascending)
            .take(limit)
            .map(|item| {
                let (addr, staker) = item?;
                let pending_rewards = staker.calculate_pending_rewards(simulated_global_index);

                Ok(StakerInfoResponse {
                    address: addr,
                    staked_amount: staker.staked_amount,
                    reward_index: staker.reward_index,
                    pending_rewards,
                })
            })
            .collect::<StdResult<Vec<_>>>()?
    };

    Ok(StakersResponse { stakers })
}

/// Parse LSM denom and validate format
/// LSM denom format: {validator_address}/{record_id}
/// Example: cosmosvaloper1abc.../123
fn parse_lsm_denom(lsm_denom: &str) -> Result<LsmShareInfo, ContractError> {
    let parts: Vec<&str> = lsm_denom.split('/').collect();

    if parts.len() != 2 {
        return Err(ContractError::InvalidLsmShares {
            reason: format!(
                "Invalid LSM denom format. Expected 'validator/record_id', got '{}'",
                lsm_denom
            ),
        });
    }

    let validator = parts[0].to_string();
    let record_id = parts[1].to_string();

    // Validate validator address format (should start with valoper prefix)
    if !validator.starts_with("cosmosvaloper") && !validator.starts_with("osmosisvaloper") {
        return Err(ContractError::InvalidLsmShares {
            reason: format!(
                "Invalid validator address format. Expected valoper address, got '{}'",
                validator
            ),
        });
    }

    // Validate record_id is numeric
    if record_id.parse::<u64>().is_err() {
        return Err(ContractError::InvalidLsmShares {
            reason: format!(
                "Invalid record_id. Expected numeric value, got '{}'",
                record_id
            ),
        });
    }

    Ok(LsmShareInfo {
        validator,
        record_id,
    })
}

/// Verify that the validator exists on chain
fn verify_validator_exists(querier: &QuerierWrapper, validator: &str) -> Result<(), ContractError> {
    querier
        .query_validator(validator)
        .map_err(|_| ContractError::ValidatorNotFound {
            validator: validator.to_string(),
        })?;

    Ok(())
}

/// Verify that a proposal is finished or doesn't exist anymore
/// Finished means status is PASSED (3), REJECTED (4), or FAILED (5)
/// If the proposal doesn't exist (query fails), we allow the destroy (proposal was purged)
fn verify_proposal_finished(
    querier: &QuerierWrapper,
    proposal_id: u64,
) -> Result<(), ContractError> {
    use cosmwasm_std::QueryRequest;
    use prost::Message;

    // Proto definition for QueryProposalRequest
    #[derive(Clone, PartialEq, Message)]
    struct QueryProposalRequest {
        #[prost(uint64, tag = "1")]
        pub proposal_id: u64,
    }

    // Proto definition for QueryProposalResponse
    #[derive(Clone, PartialEq, Message)]
    struct QueryProposalResponse {
        #[prost(message, optional, tag = "1")]
        pub proposal: Option<Proposal>,
    }

    // Proto definition for Proposal (simplified, only fields we need)
    #[derive(Clone, PartialEq, Message)]
    struct Proposal {
        #[prost(uint64, tag = "1")]
        pub proposal_id: u64,
        #[prost(int32, tag = "3")]
        pub status: i32,
        // We skip other fields we don't need
    }

    // Encode the query request
    let request = QueryProposalRequest { proposal_id };
    let mut query_data = Vec::new();
    request
        .encode(&mut query_data)
        .map_err(|e| ContractError::InvalidLsmShares {
            reason: format!("Failed to encode proposal query: {}", e),
        })?;

    // Query the gov module using Stargate
    let stargate_result: Result<Binary, cosmwasm_std::StdError> =
        querier.query(&QueryRequest::Stargate {
            path: "/cosmos.gov.v1beta1.Query/Proposal".to_string(),
            data: Binary::from(query_data),
        });

    match stargate_result {
        Ok(stargate_response) => {
            // Decode the response
            let proposal_response = QueryProposalResponse::decode(stargate_response.as_slice())
                .map_err(|e| ContractError::InvalidLsmShares {
                    reason: format!("Failed to decode proposal query response: {}", e),
                })?;
            // Proposal exists, check its status
            if let Some(proposal) = proposal_response.proposal {
                // Status codes:
                // 0 = UNSPECIFIED
                // 1 = DEPOSIT_PERIOD
                // 2 = VOTING_PERIOD
                // 3 = PASSED
                // 4 = REJECTED
                // 5 = FAILED
                if proposal.status >= 3 && proposal.status <= 5 {
                    // Proposal is finished (PASSED, REJECTED, or FAILED)
                    Ok(())
                } else {
                    // Proposal is still active (DEPOSIT_PERIOD or VOTING_PERIOD)
                    Err(ContractError::ProposalStillActive {
                        proposal_id,
                        status: match proposal.status {
                            0 => "UNSPECIFIED".to_string(),
                            1 => "DEPOSIT_PERIOD".to_string(),
                            2 => "VOTING_PERIOD".to_string(),
                            _ => format!("UNKNOWN({})", proposal.status),
                        },
                    })
                }
            } else {
                // Proposal exists but has no data - treat as finished
                Ok(())
            }
        }
        Err(_) => {
            // Query failed - proposal doesn't exist (was purged)
            // This is OK, we can destroy the lockers
            Ok(())
        }
    }
}

/// Create MsgRedeemTokensForShares message for redeeming LSM shares
/// This uses the gaia.liquid.v1beta1.MsgRedeemTokensForShares proto
fn create_redeem_tokens_msg(
    delegator_address: String,
    denom: String,
    amount: Uint128,
) -> Result<CosmosMsg, ContractError> {
    use prost::Message;

    // Proto definition for MsgRedeemTokensForShares
    #[derive(Clone, PartialEq, Message)]
    struct MsgRedeemTokensForShares {
        #[prost(string, tag = "1")]
        pub delegator_address: String,
        #[prost(message, required, tag = "2")]
        pub amount: ProtoCoin,
    }

    #[derive(Clone, PartialEq, Message)]
    struct ProtoCoin {
        #[prost(string, tag = "1")]
        pub denom: String,
        #[prost(string, tag = "2")]
        pub amount: String,
    }

    let msg = MsgRedeemTokensForShares {
        delegator_address,
        amount: ProtoCoin {
            denom,
            amount: amount.to_string(),
        },
    };

    // Encode the message
    let mut buf = Vec::new();
    msg.encode(&mut buf)
        .map_err(|e| ContractError::InvalidLsmShares {
            reason: format!("Failed to encode MsgRedeemTokensForShares: {}", e),
        })?;

    // Create Any message with the correct type URL
    // The package is gaia.liquid.v1beta1 as defined in the proto file
    Ok(CosmosMsg::Any(cosmwasm_std::AnyMsg {
        type_url: "/gaia.liquid.v1beta1.MsgRedeemTokensForShares".to_string(),
        value: Binary::from(buf),
    }))
}

/// Create MsgTokenizeShares message to convert delegation to LSM shares
/// This uses the gaia.liquid.v1beta1.MsgTokenizeShares proto
fn create_tokenize_shares_msg(
    delegator_address: String,
    validator_address: String,
    amount: Uint128,
    tokenized_share_owner: String,
) -> Result<CosmosMsg, ContractError> {
    use prost::Message;

    // Proto definition for MsgTokenizeShares
    #[derive(Clone, PartialEq, Message)]
    struct MsgTokenizeShares {
        #[prost(string, tag = "1")]
        pub delegator_address: String,
        #[prost(string, tag = "2")]
        pub validator_address: String,
        #[prost(message, required, tag = "3")]
        pub amount: ProtoCoin,
        #[prost(string, tag = "4")]
        pub tokenized_share_owner: String,
    }

    #[derive(Clone, PartialEq, Message)]
    struct ProtoCoin {
        #[prost(string, tag = "1")]
        pub denom: String,
        #[prost(string, tag = "2")]
        pub amount: String,
    }

    let msg = MsgTokenizeShares {
        delegator_address,
        validator_address,
        amount: ProtoCoin {
            denom: "uatom".to_string(), // TODO: make configurable
            amount: amount.to_string(),
        },
        tokenized_share_owner,
    };

    // Encode the message
    let mut buf = Vec::new();
    msg.encode(&mut buf)
        .map_err(|e| ContractError::InvalidLsmShares {
            reason: format!("Failed to encode MsgTokenizeShares: {}", e),
        })?;

    // Create Any message with the correct type URL
    Ok(CosmosMsg::Any(cosmwasm_std::AnyMsg {
        type_url: "/gaia.liquid.v1beta1.MsgTokenizeShares".to_string(),
        value: Binary::from(buf),
    }))
}

#[entry_point]
pub fn reply(deps: DepsMut, env: Env, msg: Reply) -> Result<Response, ContractError> {
    match msg.id {
        REPLY_CLAIM_REWARDS => reply_claim_rewards(deps, env),
        REPLY_TOKENIZE_SHARES_RENTAL => reply_tokenize_shares_rental(deps, env),
        REPLY_TOKENIZE_SHARES_WITHDRAW => reply_tokenize_shares_withdraw(deps, env),
        _ => Err(ContractError::InvalidLsmShares {
            reason: format!("Unknown reply ID: {}", msg.id),
        }),
    }
}

/// Reply handler after withdrawing rewards from the validator
/// This:
/// 1. Calculates the rewards received from the validator
/// 2. Updates the global reward index with these rewards
/// 3. Calculates the user's pending rewards with the new index
/// 4. Updates user state and sends rewards
fn reply_claim_rewards(deps: DepsMut, env: Env) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let active_claim = ACTIVE_CLAIM.load(deps.storage)?;

    // Query balance after rewards withdrawal
    let balance_query: BalanceResponse = deps.querier.query(
        &BankQuery::Balance {
            address: env.contract.address.to_string(),
            denom: config.staking_denom.clone(),
        }
        .into(),
    )?;
    let balance_after = balance_query.amount.amount;

    // Calculate actual rewards received from the validator
    let rewards_received = balance_after.saturating_sub(active_claim.balance_before);

    // Update global reward index with the rewards received
    let mut state = STATE.load(deps.storage)?;
    state.add_rewards(rewards_received);
    STATE.save(deps.storage, &state)?;

    // NOW calculate the user's pending rewards with the updated global index
    // This includes both:
    // 1. Rewards that were pending before (from global_index_before)
    // 2. Rewards from this claim (from rewards_received)
    let staker = STAKERS.load(deps.storage, &active_claim.claimer)?;
    let user_rewards = staker.calculate_pending_rewards(state.global_reward_index);

    // If no rewards after updating, return error
    if user_rewards.is_zero() {
        ACTIVE_CLAIM.remove(deps.storage);
        return Err(ContractError::NoRewards {});
    }

    // Update staker state - update their reward index to the new global index
    let mut staker = staker;
    staker.update_index(state.global_reward_index);
    STAKERS.save(deps.storage, &active_claim.claimer, &staker)?;

    // Send rewards to user
    let send_msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: active_claim.claimer.to_string(),
        amount: coins(user_rewards.u128(), config.staking_denom),
    });

    // Clean up active claim
    ACTIVE_CLAIM.remove(deps.storage);

    Ok(Response::new()
        .add_message(send_msg)
        .add_attribute("action", "rewards_claimed")
        .add_attribute("user", active_claim.claimer.to_string())
        .add_attribute("rewards_received", rewards_received.to_string())
        .add_attribute("user_amount", user_rewards.to_string()))
}

/// Reply handler after tokenizing shares for rental
/// This sends the LSM shares to the corresponding locker via DepositLsmShares
fn reply_tokenize_shares_rental(deps: DepsMut, env: Env) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let active_rental = ACTIVE_RENTAL.load(deps.storage)?;

    // Load voting session to get locker address
    let voting_session = VOTING_SESSIONS
        .load(deps.storage, active_rental.proposal_id)
        .map_err(|_| ContractError::NoVotingSession {
            proposal_id: active_rental.proposal_id,
        })?;

    // Find the locker address for this vote option
    let locker_addr = voting_session
        .locker_addresses
        .iter()
        .find(|(option, _)| *option == active_rental.vote_option)
        .map(|(_, addr)| addr)
        .ok_or(ContractError::LockerNotFound {
            proposal_id: active_rental.proposal_id,
            vote_option: active_rental.vote_option,
        })?;

    // Query all token balances to find the LSM share
    // LSM shares have format: {validator}/{record_id}
    use cosmwasm_std::{AllBalanceResponse, BankQuery, QueryRequest};
    let all_balances_response: AllBalanceResponse =
        deps.querier
            .query(&QueryRequest::Bank(BankQuery::AllBalances {
                address: env.contract.address.to_string(),
            }))?;
    let all_balances = all_balances_response.amount;

    // Find the LSM share token for our specific validator
    // The denom should start with the validator address followed by '/'
    let expected_prefix = format!("{}/", config.validator);
    let lsm_share = all_balances
        .iter()
        .find(|coin| coin.denom.starts_with(&expected_prefix))
        .ok_or(ContractError::InvalidLsmShares {
            reason: format!(
                "No LSM share found for validator {} after tokenization",
                config.validator
            ),
        })?;

    // Call DepositLsmShares on the locker with the LSM shares
    use cosmwasm_std::WasmMsg;
    use proposal_locker_types::ExecuteMsg as LockerExecuteMsg;

    let deposit_msg = WasmMsg::Execute {
        contract_addr: locker_addr.to_string(),
        msg: to_json_binary(&LockerExecuteMsg::DepositLsmShares {})?,
        funds: vec![lsm_share.clone()],
    };

    // Clean up active rental
    ACTIVE_RENTAL.remove(deps.storage);

    Ok(Response::new()
        .add_message(deposit_msg)
        .add_attribute("action", "tokenize_shares_rental_reply")
        .add_attribute("proposal_id", active_rental.proposal_id.to_string())
        .add_attribute("vote_option", active_rental.vote_option.to_string())
        .add_attribute("locker", locker_addr)
        .add_attribute("lsm_denom", &lsm_share.denom)
        .add_attribute("amount", lsm_share.amount))
}

/// Reply handler after tokenizing shares for withdrawal
/// This sends the LSM shares directly to the user
fn reply_tokenize_shares_withdraw(deps: DepsMut, env: Env) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let active_withdraw = ACTIVE_WITHDRAW.load(deps.storage)?;

    // Query all token balances to find the LSM share
    // LSM shares have format: {validator}/{record_id}
    use cosmwasm_std::{AllBalanceResponse, BankQuery, QueryRequest};
    let all_balances_response: AllBalanceResponse =
        deps.querier
            .query(&QueryRequest::Bank(BankQuery::AllBalances {
                address: env.contract.address.to_string(),
            }))?;
    let all_balances = all_balances_response.amount;

    // Find the LSM share token for our specific validator
    // The denom should start with the validator address followed by '/'
    let expected_prefix = format!("{}/", config.validator);
    let lsm_share = all_balances
        .iter()
        .find(|coin| coin.denom.starts_with(&expected_prefix))
        .ok_or(ContractError::InvalidLsmShares {
            reason: format!(
                "No LSM share found for validator {} after tokenization",
                config.validator
            ),
        })?;

    // Send the LSM shares directly to the withdrawer
    let send_msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: active_withdraw.withdrawer.to_string(),
        amount: vec![lsm_share.clone()],
    });

    // Clean up active withdraw
    ACTIVE_WITHDRAW.remove(deps.storage);

    Ok(Response::new()
        .add_message(send_msg)
        .add_attribute("action", "tokenize_shares_withdraw_reply")
        .add_attribute("withdrawer", active_withdraw.withdrawer)
        .add_attribute("lsm_denom", &lsm_share.denom)
        .add_attribute("amount", lsm_share.amount))
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmwasm_std::testing::{message_info, mock_dependencies, mock_env};
    use cosmwasm_std::{coins, Decimal256};

    #[test]
    fn proper_initialization() {
        let mut deps = mock_dependencies();
        let owner_addr = deps.api.addr_make("owner");
        let validator_addr = deps.api.addr_make("validator");
        let msg = InstantiateMsg {
            staking_denom: "uatom".to_string(),
            owner: owner_addr.to_string(),
            validator: validator_addr.to_string(),
            max_cap: None,
            locker_code_id: 1,
        };

        let info = message_info(&deps.api.addr_make("creator"), &[]);
        let res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
        assert_eq!(0, res.messages.len());

        // Check config
        let config = CONFIG.load(&deps.storage).unwrap();
        assert_eq!(config.owner, owner_addr);
        assert_eq!(config.staking_denom, "uatom");
        assert_eq!(config.validator, validator_addr.to_string());
        assert_eq!(config.max_cap, None);
        assert_eq!(config.locker_code_id, 1);

        // Check state
        let state = STATE.load(&deps.storage).unwrap();
        assert_eq!(state.total_staked, Uint128::zero());
        assert_eq!(state.global_reward_index, Decimal256::zero());

        // Check is_paused
        let is_paused = IS_PAUSED.load(&deps.storage).unwrap();
        assert_eq!(is_paused, false);
    }

    #[test]
    fn test_parse_lsm_denom_valid() {
        let valid_denom = "cosmosvaloper1abc123/456";
        let result = parse_lsm_denom(valid_denom).unwrap();
        assert_eq!(result.validator, "cosmosvaloper1abc123");
        assert_eq!(result.record_id, "456");
    }

    #[test]
    fn test_parse_lsm_denom_invalid_format() {
        let invalid_denom = "cosmosvaloper1abc123";
        let result = parse_lsm_denom(invalid_denom);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_lsm_denom_invalid_validator() {
        let invalid_denom = "invalidprefix1abc123/456";
        let result = parse_lsm_denom(invalid_denom);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_lsm_denom_invalid_record_id() {
        let invalid_denom = "cosmosvaloper1abc123/notanumber";
        let result = parse_lsm_denom(invalid_denom);
        assert!(result.is_err());
    }

    #[test]
    fn test_deposit_rewards_and_claim() {
        let mut deps = mock_dependencies();

        // Initialize
        let owner_addr = deps.api.addr_make("owner");
        let validator_addr = deps.api.addr_make("validator");
        let msg = InstantiateMsg {
            staking_denom: "uatom".to_string(),
            owner: owner_addr.to_string(),
            validator: validator_addr.to_string(),
            max_cap: None,
            locker_code_id: 1,
        };
        let info = message_info(&deps.api.addr_make("creator"), &[]);
        instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Simulate a user having staked tokens
        let staker_addr = deps.api.addr_make("staker");
        let mut staker = Staker::new();
        staker.staked_amount = Uint128::new(1000);
        STAKERS
            .save(&mut deps.storage, &staker_addr, &staker)
            .unwrap();

        let mut state = STATE.load(&deps.storage).unwrap();
        state.total_staked = Uint128::new(1000);
        STATE.save(&mut deps.storage, &state).unwrap();

        // Deposit rewards
        let info = message_info(&deps.api.addr_make("depositor"), &coins(100, "uatom"));
        let msg = ExecuteMsg::DepositRewards {};
        execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Check state updated
        let state = STATE.load(&deps.storage).unwrap();
        assert_eq!(
            state.global_reward_index,
            Decimal256::from_ratio(100u128, 1000u128)
        );

        // Claim rewards
        let info = message_info(&staker_addr, &[]);
        let msg = ExecuteMsg::ClaimRewards {};
        let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

        // Check that withdraw message was created
        assert_eq!(res.messages.len(), 1);
    }
}
