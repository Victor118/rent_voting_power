use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Coin, CosmosMsg, Deps, DepsMut, DistributionMsg, Env,
    MessageInfo, QuerierWrapper, Response, StakingMsg, StdResult, SubMsg, Uint128, WasmMsg,
};
use cw2::set_contract_version;
use proposal_locker_types::{
    Config, ConfigResponse, ExecuteMsg, InstantiateMsg, LsmShareInfo, QueryMsg, State,
    TotalVotingPowerResponse,
};

use crate::error::ContractError;
use crate::state::{CONFIG, STATE};

const CONTRACT_NAME: &str = "crates.io:proposal-option-locker";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

// Reply IDs
const REPLY_CLAIM_REWARDS: u64 = 1;
const REPLY_TOKENIZE_SHARES: u64 = 2;

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    // Validate manager address
    let manager = deps.api.addr_validate(&msg.manager)?;

    // Validate validator address format
    deps.api.addr_validate(&msg.validator)?;

    // Verify that the validator exists on chain
    verify_validator_exists(&deps.querier, &msg.validator)?;

    // Verify that the proposal is in VOTING_PERIOD before voting
    verify_proposal_in_voting(&deps.querier, msg.proposal_id)?;

    let config = Config {
        proposal_id: msg.proposal_id,
        vote_option: msg.vote_option,
        validator: msg.validator.clone(),
        manager,
    };

    CONFIG.save(deps.storage, &config)?;
    STATE.save(deps.storage, &State::new())?;

    // Cast initial vote
    // The vote will be weighted as more LSM shares are deposited
    let vote_msg = create_vote_msg(
        env.contract.address.to_string(),
        msg.proposal_id,
        msg.vote_option,
    )?;

    Ok(Response::new()
        .add_message(vote_msg)
        .add_attribute("method", "instantiate")
        .add_attribute("proposal_id", msg.proposal_id.to_string())
        .add_attribute("vote_option", msg.vote_option.to_string())
        .add_attribute("validator", msg.validator)
        .add_attribute("manager", config.manager))
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
        ExecuteMsg::Destroy {} => execute_destroy(deps, env, info),
    }
}

/// Deposit LSM shares which will be redeemed to increase voting power
/// Only callable by manager
pub fn execute_deposit_lsm_shares(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut state = STATE.load(deps.storage)?;

    // Only manager can deposit
    if info.sender != config.manager {
        return Err(ContractError::Unauthorized {});
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

    // Verify that the LSM share is from the configured validator
    if lsm_info.validator != config.validator {
        return Err(ContractError::InvalidValidator {
            expected: config.validator.clone(),
            validator: lsm_info.validator.clone(),
        });
    }

    // Update total staked (voting power)
    state.total_staked += lsm_share.amount;
    STATE.save(deps.storage, &state)?;

    // Create MsgRedeemTokensForShares message from liquid staking module
    // This converts LSM shares back to a native delegation, increasing voting power
    let redeem_msg = create_redeem_tokens_msg(
        env.contract.address.to_string(),
        lsm_share.denom.clone(),
        lsm_share.amount,
    )?;

    Ok(Response::new()
        .add_message(redeem_msg)
        .add_attribute("method", "deposit_lsm_shares")
        .add_attribute("manager", config.manager)
        .add_attribute("validator", lsm_info.validator)
        .add_attribute("record_id", lsm_info.record_id)
        .add_attribute("amount", lsm_share.amount)
        .add_attribute("total_voting_power", state.total_staked))
}

/// Destroy the contract after proposal is finished
/// Claims rewards, tokenizes all delegations, sends everything to manager
/// Only callable by manager
pub fn execute_destroy(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;

    // Only manager can destroy
    if info.sender != config.manager {
        return Err(ContractError::Unauthorized {});
    }

    // TODO: Verify that proposal is finished
    // This requires querying the gov module to check proposal status
    // For now, we allow destruction at any time

    let mut submessages: Vec<SubMsg> = Vec::new();

    // 1. Claim all delegation rewards with reply
    // The reply will call DepositRewards on the manager
    if !state.total_staked.is_zero() {
        let claim_msg = SubMsg::reply_on_success(
            CosmosMsg::Distribution(DistributionMsg::WithdrawDelegatorReward {
                validator: config.validator.clone(),
            }),
            REPLY_CLAIM_REWARDS,
        );
        submessages.push(claim_msg);
    }

    // 2. Tokenize all delegations to create LSM shares with reply
    // The reply will send the LSM shares to the manager via ReturnLsmShares
    if !state.total_staked.is_zero() {
        let tokenize_msg = create_tokenize_shares_msg(
            env.contract.address.to_string(),
            config.validator.clone(),
            state.total_staked,
            env.contract.address.to_string(), // Send to self first
        )?;
        submessages.push(SubMsg::reply_on_success(tokenize_msg, REPLY_TOKENIZE_SHARES));
    }

    Ok(Response::new()
        .add_submessages(submessages)
        .add_attribute("method", "destroy")
        .add_attribute("manager", config.manager)
        .add_attribute("total_staked", state.total_staked)
        .add_attribute("rewards_claimed", "true"))
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_json_binary(&query_config(deps)?),
        QueryMsg::TotalVotingPower {} => to_json_binary(&query_total_voting_power(deps)?),
    }
}

fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let config = CONFIG.load(deps.storage)?;
    let state = STATE.load(deps.storage)?;

    Ok(ConfigResponse {
        proposal_id: config.proposal_id,
        vote_option: config.vote_option,
        validator: config.validator,
        manager: config.manager,
        total_staked: state.total_staked,
        has_voted: state.has_voted,
    })
}

fn query_total_voting_power(deps: Deps) -> StdResult<TotalVotingPowerResponse> {
    let state = STATE.load(deps.storage)?;
    Ok(TotalVotingPowerResponse {
        total_staked: state.total_staked,
    })
}

/// Parse LSM denom and validate format
/// LSM denom format: {validator_address}/{record_id}
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

/// Verify that a proposal is in VOTING_PERIOD (status = 2)
/// This ensures we can vote on the proposal
fn verify_proposal_in_voting(querier: &QuerierWrapper, proposal_id: u64) -> Result<(), ContractError> {
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
    request.encode(&mut query_data).map_err(|e| {
        ContractError::InvalidLsmShares {
            reason: format!("Failed to encode proposal query: {}", e),
        }
    })?;

    // Query the gov module using Stargate
    let stargate_response: Binary = querier.query(&QueryRequest::Stargate {
        path: "/cosmos.gov.v1beta1.Query/Proposal".to_string(),
        data: Binary::from(query_data),
    }).map_err(|_| ContractError::InvalidLsmShares {
        reason: format!("Failed to query proposal {}", proposal_id),
    })?;

    // Decode the response
    let response = QueryProposalResponse::decode(stargate_response.as_slice()).map_err(|e| {
        ContractError::InvalidLsmShares {
            reason: format!("Failed to decode proposal query response: {}", e),
        }
    })?;

    // Check proposal status
    if let Some(proposal) = response.proposal {
        // Status codes:
        // 0 = UNSPECIFIED
        // 1 = DEPOSIT_PERIOD
        // 2 = VOTING_PERIOD
        // 3 = PASSED
        // 4 = REJECTED
        // 5 = FAILED
        if proposal.status == 2 {
            // Proposal is in VOTING_PERIOD - OK to vote
            Ok(())
        } else {
            // Proposal is not in voting period
            Err(ContractError::ProposalNotInVoting {
                proposal_id,
                status: match proposal.status {
                    0 => "UNSPECIFIED".to_string(),
                    1 => "DEPOSIT_PERIOD".to_string(),
                    2 => "VOTING_PERIOD".to_string(),
                    3 => "PASSED".to_string(),
                    4 => "REJECTED".to_string(),
                    5 => "FAILED".to_string(),
                    _ => format!("UNKNOWN({})", proposal.status),
                },
            })
        }
    } else {
        // Proposal not found
        Err(ContractError::InvalidLsmShares {
            reason: format!("Proposal {} not found", proposal_id),
        })
    }
}

/// Create MsgVote message for governance
fn create_vote_msg(
    voter: String,
    proposal_id: u64,
    vote_option: i32,
) -> Result<CosmosMsg, ContractError> {
    use prost::Message;

    // Proto definition for MsgVote
    #[derive(Clone, PartialEq, Message)]
    struct MsgVote {
        #[prost(uint64, tag = "1")]
        pub proposal_id: u64,
        #[prost(string, tag = "2")]
        pub voter: String,
        #[prost(int32, tag = "3")]
        pub option: i32,
    }

    let msg = MsgVote {
        proposal_id,
        voter,
        option: vote_option,
    };

    // Encode the message
    let mut buf = Vec::new();
    msg.encode(&mut buf)
        .map_err(|e| ContractError::InvalidLsmShares {
            reason: format!("Failed to encode MsgVote: {}", e),
        })?;

    Ok(CosmosMsg::Any(cosmwasm_std::AnyMsg {
        type_url: "/cosmos.gov.v1beta1.MsgVote".to_string(),
        value: Binary::from(buf),
    }))
}

/// Create MsgRedeemTokensForShares message for redeeming LSM shares
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

    Ok(CosmosMsg::Any(cosmwasm_std::AnyMsg {
        type_url: "/gaia.liquid.v1beta1.MsgRedeemTokensForShares".to_string(),
        value: Binary::from(buf),
    }))
}

/// Create MsgTokenizeShares message to convert delegation to LSM shares
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

    Ok(CosmosMsg::Any(cosmwasm_std::AnyMsg {
        type_url: "/gaia.liquid.v1beta1.MsgTokenizeShares".to_string(),
        value: Binary::from(buf),
    }))
}

#[entry_point]
pub fn reply(deps: DepsMut, env: Env, msg: cosmwasm_std::Reply) -> Result<Response, ContractError> {
    match msg.id {
        REPLY_CLAIM_REWARDS => reply_claim_rewards(deps, env),
        REPLY_TOKENIZE_SHARES => reply_tokenize_shares(deps, env),
        _ => Err(ContractError::InvalidLsmShares {
            reason: format!("Unknown reply ID: {}", msg.id),
        }),
    }
}

/// Reply handler after claiming rewards
/// Deposits the rewards to the manager contract
fn reply_claim_rewards(deps: DepsMut, env: Env) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Query balance to see how much rewards we received
    let balance = deps.querier.query_balance(env.contract.address, "uatom")?; // TODO: make denom configurable

    if balance.amount.is_zero() {
        return Ok(Response::new()
            .add_attribute("action", "claim_rewards_reply")
            .add_attribute("rewards", "0"));
    }

    // Call DepositRewards on the manager with the rewards
    use lsm_types::ExecuteMsg as ManagerExecuteMsg;

    let deposit_msg = WasmMsg::Execute {
        contract_addr: config.manager.to_string(),
        msg: to_json_binary(&ManagerExecuteMsg::DepositRewards {})?,
        funds: vec![balance.clone()],
    };

    Ok(Response::new()
        .add_message(deposit_msg)
        .add_attribute("action", "claim_rewards_reply")
        .add_attribute("rewards", balance.amount))
}

/// Reply handler after tokenizing shares
/// Sends the LSM shares back to the manager via ReturnLsmShares
fn reply_tokenize_shares(deps: DepsMut, env: Env) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    // Query all token balances to find the LSM share
    // LSM shares have format: {validator}/{record_id}
    use cosmwasm_std::{AllBalanceResponse, BankQuery, QueryRequest};
    let all_balances_response: AllBalanceResponse = deps.querier.query(&QueryRequest::Bank(BankQuery::AllBalances {
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

    // Call ReturnLsmShares on the manager with the LSM shares
    use lsm_types::ExecuteMsg as ManagerExecuteMsg;

    let return_msg = WasmMsg::Execute {
        contract_addr: config.manager.to_string(),
        msg: to_json_binary(&ManagerExecuteMsg::ReturnLsmShares {
            proposal_id: config.proposal_id,
            vote_option: config.vote_option,
        })?,
        funds: vec![lsm_share.clone()],
    };

    Ok(Response::new()
        .add_message(return_msg)
        .add_attribute("action", "tokenize_shares_reply")
        .add_attribute("lsm_denom", &lsm_share.denom)
        .add_attribute("amount", lsm_share.amount))
}
