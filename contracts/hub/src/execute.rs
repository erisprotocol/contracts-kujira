use cosmwasm_std::{
    attr, to_binary, Addr, BankMsg, Coin, CosmosMsg, Decimal, DepsMut, DistributionMsg, Env, Event,
    Order, Response, StdError, StdResult, Storage, Uint128, WasmMsg,
};
use cw2::set_contract_version;
use eris::adapters::bow_vault::BowVault;
use eris::{CustomResponse, DecimalCheckedOps};

use eris::adapters::bw_vault::BlackWhaleVault;
use eris::adapters::fin_multi::FinMulti;
use eris::hub::{
    Batch, CallbackMsg, DelegationStrategy, ExecuteMsg, FeeConfig, InstantiateMsg, PendingBatch,
    StakeToken, UnbondRequest, WithdrawType,
};
use kujira::denom::Denom;
use kujira::msg::{DenomMsg, KujiraMsg};

use crate::constants::{get_reward_fee_cap, CONTRACT_DENOM};
use crate::error::{ContractError, ContractResult};
use crate::helpers::{
    assert_validator_exists, assert_validators_exists, dedupe, get_wanted_delegations,
    query_all_delegations, query_delegation, query_delegations,
};
use crate::math::{
    compute_mint_amount, compute_redelegations_for_rebalancing, compute_redelegations_for_removal,
    compute_unbond_amount, compute_undelegations, mark_reconciled_batches, reconcile_batches,
};
use crate::state::State;
// use crate::types::gauges::TuneInfoGaugeLoader;
use crate::types::{Coins, Delegation, SendFee};

const CONTRACT_NAME: &str = "eris-hub";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

//--------------------------------------------------------------------------------------------------
// Instantiation
//--------------------------------------------------------------------------------------------------

pub fn instantiate(deps: DepsMut, env: Env, msg: InstantiateMsg) -> ContractResult {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let state = State::default();

    if msg.protocol_reward_fee.gt(&get_reward_fee_cap()) {
        return Err(ContractError::ProtocolRewardFeeTooHigh {});
    }

    if msg.epoch_period == 0 {
        return Err(ContractError::CantBeZero("epoch_period".into()));
    }

    if msg.unbond_period == 0 {
        return Err(ContractError::CantBeZero("unbond_period".into()));
    }

    state.owner.save(deps.storage, &deps.api.addr_validate(&msg.owner)?)?;
    state.operator.save(deps.storage, &deps.api.addr_validate(&msg.operator)?)?;
    state.epoch_period.save(deps.storage, &msg.epoch_period)?;
    state.unbond_period.save(deps.storage, &msg.unbond_period)?;

    if let Some(vote_operator) = msg.vote_operator {
        state.vote_operator.save(deps.storage, &deps.api.addr_validate(&vote_operator)?)?;
    }

    // by default donations are set to false
    state.allow_donations.save(deps.storage, &false)?;

    let mut validators = msg.validators;

    dedupe(&mut validators);
    assert_validators_exists(&deps.querier, &validators)?;

    state.validators.save(deps.storage, &validators)?;
    state.unlocked_coins.save(deps.storage, &vec![])?;
    state.fee_config.save(
        deps.storage,
        &FeeConfig {
            protocol_fee_contract: deps.api.addr_validate(&msg.protocol_fee_contract)?,
            protocol_reward_fee: msg.protocol_reward_fee,
        },
    )?;

    state.pending_batch.save(
        deps.storage,
        &PendingBatch {
            id: 1,
            ustake_to_burn: Uint128::zero(),
            est_unbond_start_time: env.block.time.seconds() + msg.epoch_period,
        },
    )?;

    let delegation_strategy = msg.delegation_strategy.unwrap_or(DelegationStrategy::Uniform);
    state
        .delegation_strategy
        .save(deps.storage, &delegation_strategy.validate(deps.api, &validators)?)?;

    state
        .fin_multi
        .save(deps.storage, &FinMulti(deps.api.addr_validate(&msg.fin_multi_contract)?))?;

    let addr = env.contract.address;
    let denom = format!("factory/{0}/{1}", addr, msg.denom);
    state.stake_token.save(
        deps.storage,
        &StakeToken {
            denom,
            total_supply: Uint128::zero(),
        },
    )?;

    // needs to be validated after stake token has been set
    validate_no_utoken_or_ustake_swap(&msg.stages_preset, &state, deps.storage)?;
    state.stages_preset.save(deps.storage, &msg.stages_preset.unwrap_or_default())?;

    Ok(Response::new().add_message(DenomMsg::Create {
        subdenom: msg.denom.into(),
    }))
}

//--------------------------------------------------------------------------------------------------
// Bonding and harvesting logics
//--------------------------------------------------------------------------------------------------

/// NOTE: In a previous implementation, we split up the deposited Token over all validators, so that
/// they all have the same amount of delegation. This is however quite gas-expensive: $1.5 cost in
/// the case of 15 validators.
///
/// To save gas for users, now we simply delegate all deposited Token to the validator with the
/// smallest amount of delegation. If delegations become severely unbalance as a result of this
/// (e.g. when a single user makes a very big deposit), anyone can invoke `ExecuteMsg::Rebalance`
/// to balance the delegations.
pub fn bond(
    deps: DepsMut,
    env: Env,
    receiver: Addr,
    token_to_bond: Uint128,
    donate: bool,
) -> ContractResult {
    let state = State::default();
    let mut stake = state.stake_token.load(deps.storage)?;
    let (new_delegation, delegations) = find_new_delegation(&state, &deps, &env, token_to_bond)?;

    // Query the current supply of Staking Token and compute the amount to mint
    let ustake_supply = stake.total_supply;
    let ustake_to_mint = if donate {
        match state.allow_donations.may_load(deps.storage)? {
            Some(false) => Err(ContractError::DonationsDisabled {})?,
            Some(true) | None => {
                // if it is not set (backward compatibility) or set to true, donations are allowed
            },
        }
        Uint128::zero()
    } else {
        compute_mint_amount(ustake_supply, token_to_bond, &delegations)
    };

    let event = Event::new("erishub/bonded")
        .add_attribute("receiver", receiver.clone())
        .add_attribute("token_bonded", token_to_bond)
        .add_attribute("ustake_minted", ustake_to_mint);

    let mint_msg: Option<CosmosMsg<KujiraMsg>> = if donate {
        None
    } else {
        // create mint message and add to stored total supply
        stake.total_supply = stake.total_supply.checked_add(ustake_to_mint)?;
        state.stake_token.save(deps.storage, &stake)?;

        Some(
            DenomMsg::Mint {
                denom: stake.denom.clone().into(),
                amount: ustake_to_mint,
                recipient: receiver,
            }
            .into(),
        )
    };

    Ok(Response::new()
        .add_message(new_delegation.to_cosmos_msg())
        .add_optional_message(mint_msg)
        .add_message(check_received_coin_msg(&deps, &env, stake, Some(token_to_bond))?)
        .add_event(event)
        .add_attribute("action", "erishub/bond"))
}

pub fn harvest(
    deps: DepsMut,
    env: Env,
    withdrawals: Option<Vec<(WithdrawType, Addr, Denom)>>,
    stages: Option<Vec<Vec<(Addr, Denom)>>>,
    sender: Addr,
) -> ContractResult {
    let state = State::default();

    // 1. withdraw delegation rewards
    let withdraw_submsgs: Vec<CosmosMsg<KujiraMsg>> =
        query_all_delegations(&deps.querier, &env.contract.address)?
            .into_iter()
            .map(|d| {
                CosmosMsg::Distribution(DistributionMsg::WithdrawDelegatorReward {
                    validator: d.validator,
                })
            })
            .collect::<Vec<_>>();

    let claim_funds_msg = withdrawals.map(|w| CallbackMsg::ClaimFunds {
        withdrawals: Some(w),
    });

    let swap_msg = stages.map(|s| CallbackMsg::Swap {
        stages: Some(s),
        sender,
    });

    Ok(Response::new()
        // 1. withdraw delegation rewards
        .add_messages(withdraw_submsgs)
        // 2. claim funds
        .add_optional_callback(&env, claim_funds_msg)?
        // 3. swap
        .add_optional_callback(&env, swap_msg)?
        // 4. apply received total ukuji to unlocked_coins
        .add_message(check_received_coin_msg(
            &deps,
            &env,
            state.stake_token.load(deps.storage)?,
            None,
        )?)
        // 5. restake unlocked_coins
        .add_callback(&env, CallbackMsg::Reinvest {})?
        .add_attribute("action", "erishub/harvest"))
}

pub fn claim_funds(
    deps: DepsMut,
    env: Env,
    withdrawals: Option<Vec<(WithdrawType, Addr, Denom)>>,
) -> ContractResult {
    let mut withdraw_msgs: Vec<CosmosMsg<KujiraMsg>> = vec![];
    if let Some(withdrawals) = withdrawals {
        let balances = deps.querier.query_all_balances(env.contract.address)?;

        for (withdraw_type, addr, denom) in withdrawals {
            let balance = balances.iter().find(|b| b.denom == denom.to_string());

            if let Some(coin) = balance {
                if !coin.amount.is_zero() {
                    match withdraw_type {
                        WithdrawType::BlackWhale => {
                            withdraw_msgs
                                .push(BlackWhaleVault(addr).withdraw_msg(denom, coin.amount)?);
                        },
                        WithdrawType::Bow => {
                            withdraw_msgs.push(BowVault(addr).withdraw_msg(denom, coin.amount)?);
                        },
                    }
                }
            }
        }
    }

    Ok(Response::new().add_messages(withdraw_msgs).add_attribute("action", "erishub/claim_funds"))
}

/// swaps all unlocked coins to token
pub fn swap(
    deps: DepsMut,
    env: Env,
    mut stages: Option<Vec<Vec<(Addr, Denom)>>>,
    sender: Addr,
) -> ContractResult {
    let state = State::default();

    if stages.is_some() {
        state.assert_operator(deps.storage, &sender)?
    } else {
        stages = Some(state.stages_preset.load(deps.storage)?);
    }

    validate_no_utoken_or_ustake_swap(&stages, &state, deps.storage)?;

    let fin_multi = if let Some(stages) = stages {
        let balances = deps.querier.query_all_balances(env.contract.address)?;
        Some(state.fin_multi.load(deps.storage)?.swap_msg(stages, balances)?)
    } else {
        None
    };

    Ok(Response::new().add_optional_message(fin_multi).add_attribute("action", "erishub/swap"))
}

fn validate_no_utoken_or_ustake_swap(
    stages: &Option<Vec<Vec<(Addr, Denom)>>>,
    state: &State,
    storage: &dyn Storage,
) -> Result<(), ContractError> {
    if let Some(stages) = stages {
        let stake_token_denom = state.stake_token.load(storage)?.denom;

        for stage in stages {
            for (_addr, denom) in stage {
                if denom.to_string() == CONTRACT_DENOM || denom.to_string() == stake_token_denom {
                    return Err(ContractError::SwapFromNotAllowed(denom.to_string()));
                }
            }
        }
    }
    Ok(())
}

/// This callback is used to take a current snapshot of the balance and add the received balance to the unlocked_coins state after the execution
fn check_received_coin_msg(
    deps: &DepsMut,
    env: &Env,
    stake: StakeToken,
    // offset to account for funds being sent that should be ignored
    negative_offset: Option<Uint128>,
) -> StdResult<CosmosMsg<KujiraMsg>> {
    let mut amount =
        deps.querier.query_balance(env.contract.address.to_string(), CONTRACT_DENOM)?.amount;

    if let Some(negative_offset) = negative_offset {
        amount = amount.checked_sub(negative_offset)?;
    }

    let amount_stake =
        deps.querier.query_balance(env.contract.address.to_string(), stake.denom.clone())?.amount;

    CallbackMsg::CheckReceivedCoin {
        // 0. take current balance - offset
        snapshot: Coin {
            denom: CONTRACT_DENOM.to_string(),
            amount,
        },
        snapshot_stake: Coin {
            denom: stake.denom,
            amount: amount_stake,
        },
    }
    .into_cosmos_msg(&env.contract.address)
}

/// NOTE:
/// 1. When delegation Token here, we don't need to use a `SubMsg` to handle the received coins,
/// because we have already withdrawn all claimable staking rewards previously in the same atomic
/// execution.
/// 2. Same as with `bond`, in the latest implementation we only delegate staking rewards with the
/// validator that has the smallest delegation amount.
pub fn reinvest(deps: DepsMut, env: Env) -> ContractResult {
    let state = State::default();
    let mut unlocked_coins = state.unlocked_coins.load(deps.storage)?;
    let fee_config = state.fee_config.load(deps.storage)?;

    let utoken_available = unlocked_coins
        .iter()
        .find(|coin| coin.denom == CONTRACT_DENOM)
        .ok_or_else(|| ContractError::NoTokensAvailable(CONTRACT_DENOM.into()))?
        .amount;

    let protocol_fee_amount = fee_config.protocol_reward_fee.checked_mul_uint(utoken_available)?;
    let utoken_to_bond = utoken_available.saturating_sub(protocol_fee_amount);

    let (new_delegation, _) = find_new_delegation(&state, &deps, &env, utoken_to_bond)?;

    unlocked_coins.retain(|coin| coin.denom != CONTRACT_DENOM);
    state.unlocked_coins.save(deps.storage, &unlocked_coins)?;

    let event = Event::new("erishub/harvested")
        .add_attribute("utoken_bonded", utoken_to_bond)
        .add_attribute("utoken_protocol_fee", protocol_fee_amount);

    let mut msgs = vec![new_delegation.to_cosmos_msg()];

    if !protocol_fee_amount.is_zero() {
        let send_fee = SendFee::new(fee_config.protocol_fee_contract, protocol_fee_amount.u128());
        msgs.push(send_fee.to_cosmos_msg());
    }

    Ok(Response::new()
        .add_messages(msgs)
        .add_event(event)
        .add_attribute("action", "erishub/reinvest"))
}

pub fn callback_received_coins(
    deps: DepsMut,
    env: Env,
    snapshot: Coin,
    snapshot_stake: Coin,
) -> ContractResult {
    let state = State::default();
    // in some cosmwasm versions the events are not received in the callback
    // so each time the contract can receive some coins from rewards we also need to check after receiving some and add them to the unlocked_coins

    let mut received_coins = Coins(vec![]);
    let mut event = Event::new("erishub/received");
    let current_balance =
        deps.querier.query_balance(&env.contract.address, snapshot.denom.to_string())?.amount;

    if current_balance > snapshot.amount {
        let amount = current_balance.checked_sub(snapshot.amount)?;

        event = event.add_attribute("received_coin", amount.to_string() + snapshot.denom.as_str());

        received_coins.add(&Coin::new(amount.u128(), snapshot.denom))?;

        state.unlocked_coins.update(deps.storage, |coins| -> StdResult<_> {
            let mut coins = Coins(coins);
            coins.add_many(&received_coins)?;
            Ok(coins.0)
        })?;
    }

    let current_balance_stake =
        deps.querier.query_balance(&env.contract.address, snapshot_stake.denom.to_string())?.amount;

    let mut burn_msg: Option<CosmosMsg<KujiraMsg>> = None;
    if current_balance_stake > snapshot_stake.amount {
        // if we have received ampKuji as staking rewards we burn it and increase exchange rate by it.
        let ustake_to_burn = current_balance_stake.checked_sub(snapshot_stake.amount)?;

        state.stake_token.update(deps.storage, |mut stake| -> StdResult<_> {
            stake.total_supply = stake.total_supply.checked_sub(ustake_to_burn)?;
            Ok(stake)
        })?;

        burn_msg = Some(
            DenomMsg::Burn {
                denom: snapshot_stake.denom.into(),
                amount: ustake_to_burn,
            }
            .into(),
        );
    }

    Ok(Response::new()
        .add_optional_message(burn_msg)
        .add_event(event)
        .add_attribute("action", "erishub/received"))
}

/// searches for the validator with the least amount of delegations
/// For Uniform mode, searches through the validators list
/// For Gauge mode, searches for all delegations, and if nothing found, use the first validator from the list.
fn find_new_delegation(
    state: &State,
    deps: &DepsMut,
    env: &Env,
    uluna_to_bond: Uint128,
) -> Result<(Delegation, Vec<Delegation>), StdError> {
    let delegation_strategy =
        state.delegation_strategy.may_load(deps.storage)?.unwrap_or(DelegationStrategy::Uniform {});

    let delegations = match delegation_strategy {
        DelegationStrategy::Uniform {} => {
            let validators = state.validators.load(deps.storage)?;
            query_delegations(&deps.querier, &validators, &env.contract.address)?
        },
        // DelegationStrategy::Gauges {
        //     ..
        // }
        // |
        DelegationStrategy::Defined {
            ..
        } => {
            // if we have gauges, only delegate to validators that have delegations, all others are "inactive"
            let mut delegations = query_all_delegations(&deps.querier, &env.contract.address)?;
            if delegations.is_empty() {
                let validators = state.validators.load(deps.storage)?;

                delegations = vec![Delegation {
                    amount: 0,
                    validator: validators.first().unwrap().to_string(),
                }]
            }
            delegations
        },
    };

    // Query the current delegations made to validators, and find the validator with the smallest
    // delegated amount through a linear search
    // The code for linear search is a bit uglier than using `sort_by` but cheaper: O(n) vs O(n * log(n))
    let mut validator = &delegations[0].validator;
    let mut amount = delegations[0].amount;

    for d in &delegations[1..] {
        if d.amount < amount {
            validator = &d.validator;
            amount = d.amount;
        }
    }
    let new_delegation = Delegation::new(validator, uluna_to_bond.u128());

    Ok((new_delegation, delegations))
}

//--------------------------------------------------------------------------------------------------
// Unbonding logics
//--------------------------------------------------------------------------------------------------

pub fn queue_unbond(
    deps: DepsMut,
    env: Env,
    receiver: Addr,
    ustake_to_burn: Uint128,
) -> ContractResult {
    let state = State::default();

    let mut pending_batch = state.pending_batch.load(deps.storage)?;
    pending_batch.ustake_to_burn += ustake_to_burn;
    state.pending_batch.save(deps.storage, &pending_batch)?;

    state.unbond_requests.update(
        deps.storage,
        (pending_batch.id, &receiver),
        |x| -> StdResult<_> {
            let mut request = x.unwrap_or_else(|| UnbondRequest {
                id: pending_batch.id,
                user: receiver.clone(),
                shares: Uint128::zero(),
            });
            request.shares += ustake_to_burn;
            Ok(request)
        },
    )?;

    let mut msgs: Vec<CosmosMsg<KujiraMsg>> = vec![];
    let mut start_time = pending_batch.est_unbond_start_time.to_string();
    if env.block.time.seconds() >= pending_batch.est_unbond_start_time {
        start_time = "immediate".to_string();
        msgs.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.into(),
            msg: to_binary(&ExecuteMsg::SubmitBatch {})?,
            funds: vec![],
        }));
    }

    let event = Event::new("erishub/unbond_queued")
        .add_attribute("est_unbond_start_time", start_time)
        .add_attribute("id", pending_batch.id.to_string())
        .add_attribute("receiver", receiver)
        .add_attribute("ustake_to_burn", ustake_to_burn);

    Ok(Response::new()
        .add_messages(msgs)
        .add_event(event)
        .add_attribute("action", "erishub/queue_unbond"))
}

pub fn submit_batch(deps: DepsMut, env: Env) -> ContractResult {
    let state = State::default();
    let mut stake = state.stake_token.load(deps.storage)?;
    let validators = state.validators.load(deps.storage)?;
    let unbond_period = state.unbond_period.load(deps.storage)?;
    let pending_batch = state.pending_batch.load(deps.storage)?;

    let current_time = env.block.time.seconds();
    if current_time < pending_batch.est_unbond_start_time {
        return Err(ContractError::SubmitBatchAfter(pending_batch.est_unbond_start_time));
    }

    let delegations = query_all_delegations(&deps.querier, &env.contract.address)?;
    let ustake_supply = stake.total_supply;

    let utoken_to_unbond =
        compute_unbond_amount(ustake_supply, pending_batch.ustake_to_burn, &delegations);
    let new_undelegations =
        compute_undelegations(&state, deps.storage, utoken_to_unbond, &delegations, validators)?;

    state.previous_batches.save(
        deps.storage,
        pending_batch.id,
        &Batch {
            id: pending_batch.id,
            reconciled: false,
            total_shares: pending_batch.ustake_to_burn,
            utoken_unclaimed: utoken_to_unbond,
            est_unbond_end_time: current_time + unbond_period,
        },
    )?;

    let epoch_period = state.epoch_period.load(deps.storage)?;
    state.pending_batch.save(
        deps.storage,
        &PendingBatch {
            id: pending_batch.id + 1,
            ustake_to_burn: Uint128::zero(),
            est_unbond_start_time: current_time + epoch_period,
        },
    )?;

    let undelegate_msgs = new_undelegations.iter().map(|d| d.to_cosmos_msg()).collect::<Vec<_>>();

    // apply burn to the stored total supply and save state
    stake.total_supply = stake.total_supply.checked_sub(pending_batch.ustake_to_burn)?;
    state.stake_token.save(deps.storage, &stake)?;
    let burn_msg: CosmosMsg<KujiraMsg> = DenomMsg::Burn {
        denom: stake.denom.clone().into(),
        amount: pending_batch.ustake_to_burn,
    }
    .into();

    let event = Event::new("erishub/unbond_submitted")
        .add_attribute("id", pending_batch.id.to_string())
        .add_attribute("utoken_unbonded", utoken_to_unbond)
        .add_attribute("ustake_burned", pending_batch.ustake_to_burn);

    Ok(Response::new()
        .add_messages(undelegate_msgs)
        .add_message(burn_msg)
        .add_message(check_received_coin_msg(&deps, &env, stake, None)?)
        .add_event(event)
        .add_attribute("action", "erishub/unbond"))
}

pub fn reconcile(deps: DepsMut, env: Env) -> ContractResult {
    let state = State::default();
    let current_time = env.block.time.seconds();

    // Load batches that have not been reconciled
    let all_batches = state
        .previous_batches
        .idx
        .reconciled
        .prefix(false.into())
        .range(deps.storage, None, None, Order::Ascending)
        .map(|item| {
            let (_, v) = item?;
            Ok(v)
        })
        .collect::<StdResult<Vec<_>>>()?;

    let mut batches = all_batches
        .into_iter()
        .filter(|b| current_time > b.est_unbond_end_time)
        .collect::<Vec<_>>();

    let utoken_expected_received: Uint128 = batches.iter().map(|b| b.utoken_unclaimed).sum();

    if utoken_expected_received.is_zero() {
        return Ok(Response::new());
    }

    let unlocked_coins = state.unlocked_coins.load(deps.storage)?;
    let utoken_expected_unlocked = Coins(unlocked_coins).find(CONTRACT_DENOM).amount;

    let utoken_expected = utoken_expected_received + utoken_expected_unlocked;
    let utoken_actual = deps.querier.query_balance(&env.contract.address, CONTRACT_DENOM)?.amount;

    if utoken_actual >= utoken_expected {
        mark_reconciled_batches(&mut batches);
        for batch in &batches {
            state.previous_batches.save(deps.storage, batch.id, batch)?;
        }
        let ids = batches.iter().map(|b| b.id.to_string()).collect::<Vec<_>>().join(",");
        let event = Event::new("erishub/reconciled")
            .add_attribute("ids", ids)
            .add_attribute("utoken_deducted", "0");
        return Ok(Response::new().add_event(event).add_attribute("action", "erishub/reconcile"));
    }

    let utoken_to_deduct = utoken_expected - utoken_actual;

    reconcile_batches(&mut batches, utoken_to_deduct);

    for batch in &batches {
        state.previous_batches.save(deps.storage, batch.id, batch)?;
    }

    let ids = batches.iter().map(|b| b.id.to_string()).collect::<Vec<_>>().join(",");

    let event = Event::new("erishub/reconciled")
        .add_attribute("ids", ids)
        .add_attribute("utoken_deducted", utoken_to_deduct.to_string());

    Ok(Response::new().add_event(event).add_attribute("action", "erishub/reconcile"))
}

pub fn withdraw_unbonded(deps: DepsMut, env: Env, user: Addr, receiver: Addr) -> ContractResult {
    let state = State::default();
    let current_time = env.block.time.seconds();

    // NOTE: If the user has too many unclaimed requests, this may not fit in the WASM memory...
    // However, this is practically never going to happen. Who would create hundreds of unbonding
    // requests and never claim them?
    let requests = state
        .unbond_requests
        .idx
        .user
        .prefix(user.to_string())
        .range(deps.storage, None, None, Order::Ascending)
        .map(|item| {
            let (_, v) = item?;
            Ok(v)
        })
        .collect::<StdResult<Vec<_>>>()?;

    // NOTE: Token in the following batches are withdrawn it the batch:
    // - is a _previous_ batch, not a _pending_ batch
    // - is reconciled
    // - has finished unbonding
    // If not sure whether the batches have been reconciled, the user should first invoke `ExecuteMsg::Reconcile`
    // before withdrawing.
    let mut total_utoken_to_refund = Uint128::zero();
    let mut ids: Vec<String> = vec![];
    for request in &requests {
        if let Ok(mut batch) = state.previous_batches.load(deps.storage, request.id) {
            if batch.reconciled && batch.est_unbond_end_time < current_time {
                let utoken_to_refund =
                    batch.utoken_unclaimed.multiply_ratio(request.shares, batch.total_shares);

                ids.push(request.id.to_string());

                total_utoken_to_refund += utoken_to_refund;
                batch.total_shares -= request.shares;
                batch.utoken_unclaimed -= utoken_to_refund;

                if batch.total_shares.is_zero() {
                    state.previous_batches.remove(deps.storage, request.id)?;
                } else {
                    state.previous_batches.save(deps.storage, batch.id, &batch)?;
                }

                state.unbond_requests.remove(deps.storage, (request.id, &user))?;
            }
        }
    }

    if total_utoken_to_refund.is_zero() {
        return Err(ContractError::CantBeZero("withdrawable amount".into()));
    }

    let refund_msg = CosmosMsg::Bank(BankMsg::Send {
        to_address: receiver.clone().into(),
        amount: vec![Coin::new(total_utoken_to_refund.u128(), CONTRACT_DENOM)],
    });

    let event = Event::new("erishub/unbonded_withdrawn")
        .add_attribute("ids", ids.join(","))
        .add_attribute("user", user)
        .add_attribute("receiver", receiver)
        .add_attribute("utoken_refunded", total_utoken_to_refund);

    Ok(Response::new()
        .add_message(refund_msg)
        .add_event(event)
        .add_attribute("action", "erishub/withdraw_unbonded"))
}

pub fn tune_delegations(deps: DepsMut, env: Env, sender: Addr) -> ContractResult {
    let state = State::default();
    state.assert_owner(deps.storage, &sender)?;
    let (wanted_delegations, save) = get_wanted_delegations(
        &state,
        &env,
        deps.storage,
        &deps.querier, //, TuneInfoGaugeLoader {}
    )?;
    let attributes = if save {
        state.delegation_goal.save(deps.storage, &wanted_delegations)?;
        wanted_delegations
            .shares
            .iter()
            .map(|a| attr("goal_delegation", format!("{0}={1}", a.0, a.1)))
            .collect()
    } else {
        state.delegation_goal.remove(deps.storage);
        // these would be boring, as all are the same
        vec![]
    };
    Ok(Response::new()
        .add_attribute("action", "erishub/tune_delegations")
        .add_attributes(attributes))
}

//--------------------------------------------------------------------------------------------------
// Ownership and management logics
//--------------------------------------------------------------------------------------------------

pub fn rebalance(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    min_redelegation: Option<Uint128>,
) -> ContractResult {
    let delegations = query_all_delegations(&deps.querier, &env.contract.address)?;

    let state = State::default();
    state.assert_owner(deps.storage, &sender)?;
    let validators = state.validators.load(deps.storage)?;

    let min_redelegation = min_redelegation.unwrap_or_default();

    let new_redelegations =
        compute_redelegations_for_rebalancing(&state, deps.storage, &delegations, validators)?
            .into_iter()
            .filter(|redelegation| redelegation.amount >= min_redelegation.u128())
            .collect::<Vec<_>>();

    let redelegate_msgs = new_redelegations.iter().map(|rd| rd.to_cosmos_msg()).collect::<Vec<_>>();

    let amount: u128 = new_redelegations.iter().map(|rd| rd.amount).sum();

    let event = Event::new("erishub/rebalanced").add_attribute("utoken_moved", amount.to_string());

    let check_msg = if !redelegate_msgs.is_empty() {
        // only check coins if a redelegation is happening
        Some(check_received_coin_msg(&deps, &env, state.stake_token.load(deps.storage)?, None)?)
    } else {
        None
    };

    Ok(Response::new()
        .add_messages(redelegate_msgs)
        .add_optional_message(check_msg)
        .add_event(event)
        .add_attribute("action", "erishub/rebalance"))
}

pub fn add_validator(deps: DepsMut, sender: Addr, validator: String) -> ContractResult {
    let state = State::default();

    state.assert_owner(deps.storage, &sender)?;
    assert_validator_exists(&deps.querier, &validator)?;

    state.validators.update(deps.storage, |mut validators| {
        if validators.contains(&validator) {
            return Err(ContractError::ValidatorAlreadyWhitelisted(validator.clone()));
        }
        validators.push(validator.clone());
        Ok(validators)
    })?;

    let event = Event::new("erishub/validator_added").add_attribute("validator", validator);

    Ok(Response::new().add_event(event).add_attribute("action", "erishub/add_validator"))
}

pub fn remove_validator(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    validator: String,
) -> ContractResult {
    let state = State::default();

    state.assert_owner(deps.storage, &sender)?;

    let validators = state.validators.update(deps.storage, |mut validators| {
        if !validators.contains(&validator) {
            return Err(ContractError::ValidatorNotWhitelisted(validator.clone()));
        }
        validators.retain(|v| *v != validator);
        Ok(validators)
    })?;

    let delegation_strategy =
        state.delegation_strategy.may_load(deps.storage)?.unwrap_or(DelegationStrategy::Uniform);

    let redelegate_msgs = match delegation_strategy {
        DelegationStrategy::Uniform => {
            // only redelegate when old strategy
            let delegations = query_delegations(&deps.querier, &validators, &env.contract.address)?;
            let delegation_to_remove =
                query_delegation(&deps.querier, &validator, &env.contract.address)?;
            let new_redelegations = compute_redelegations_for_removal(
                &state,
                deps.storage,
                &delegation_to_remove,
                &delegations,
                validators,
            )?;

            new_redelegations.iter().map(|d| d.to_cosmos_msg()).collect::<Vec<_>>()
        },
        // DelegationStrategy::Gauges {
        //     ..
        // }
        // |
        DelegationStrategy::Defined {
            ..
        } => {
            // removed validators can have a delegation until the next tune, to keep undelegations in sync.
            vec![]
        },
    };

    let event = Event::new("erishub/validator_removed").add_attribute("validator", validator);

    let check_msg = if !redelegate_msgs.is_empty() {
        // only check coins if a redelegation is happening
        Some(check_received_coin_msg(&deps, &env, state.stake_token.load(deps.storage)?, None)?)
    } else {
        None
    };

    Ok(Response::new()
        .add_messages(redelegate_msgs)
        .add_optional_message(check_msg)
        .add_event(event)
        .add_attribute("action", "erishub/remove_validator"))
}

pub fn transfer_ownership(deps: DepsMut, sender: Addr, new_owner: String) -> ContractResult {
    let state = State::default();

    state.assert_owner(deps.storage, &sender)?;
    state.new_owner.save(deps.storage, &deps.api.addr_validate(&new_owner)?)?;

    Ok(Response::new().add_attribute("action", "erishub/transfer_ownership"))
}

pub fn drop_ownership_proposal(deps: DepsMut, sender: Addr) -> ContractResult {
    let state = State::default();

    state.assert_owner(deps.storage, &sender)?;
    state.new_owner.remove(deps.storage);

    Ok(Response::new().add_attribute("action", "erishub/drop_ownership_proposal"))
}

pub fn accept_ownership(deps: DepsMut, sender: Addr) -> ContractResult {
    let state = State::default();

    let previous_owner = state.owner.load(deps.storage)?;
    let new_owner = state.new_owner.load(deps.storage)?;

    if sender != new_owner {
        return Err(ContractError::UnauthorizedSenderNotNewOwner {});
    }

    state.owner.save(deps.storage, &sender)?;
    state.new_owner.remove(deps.storage);

    let event = Event::new("erishub/ownership_transferred")
        .add_attribute("new_owner", new_owner)
        .add_attribute("previous_owner", previous_owner);

    Ok(Response::new().add_event(event).add_attribute("action", "erishub/transfer_ownership"))
}

#[allow(clippy::too_many_arguments)]
pub fn update_config(
    deps: DepsMut,
    sender: Addr,
    protocol_fee_contract: Option<String>,
    protocol_reward_fee: Option<Decimal>,
    operator: Option<String>,
    stages_preset: Option<Vec<Vec<(Addr, Denom)>>>,
    allow_donations: Option<bool>,
    delegation_strategy: Option<DelegationStrategy>,
    vote_operator: Option<String>,
) -> ContractResult {
    let state = State::default();

    state.assert_owner(deps.storage, &sender)?;

    if protocol_fee_contract.is_some() || protocol_reward_fee.is_some() {
        let mut fee_config = state.fee_config.load(deps.storage)?;

        if let Some(protocol_fee_contract) = protocol_fee_contract {
            fee_config.protocol_fee_contract = deps.api.addr_validate(&protocol_fee_contract)?;
        }

        if let Some(protocol_reward_fee) = protocol_reward_fee {
            if protocol_reward_fee.gt(&get_reward_fee_cap()) {
                return Err(ContractError::ProtocolRewardFeeTooHigh {});
            }
            fee_config.protocol_reward_fee = protocol_reward_fee;
        }

        state.fee_config.save(deps.storage, &fee_config)?;
    }

    if let Some(operator) = operator {
        state.operator.save(deps.storage, &deps.api.addr_validate(operator.as_str())?)?;
    }

    if stages_preset.is_some() {
        validate_no_utoken_or_ustake_swap(&stages_preset, &state, deps.storage)?;
    }

    if let Some(stages_preset) = stages_preset {
        state.stages_preset.save(deps.storage, &stages_preset)?;
    }

    if let Some(delegation_strategy) = delegation_strategy {
        let validators = state.validators.load(deps.storage)?;
        state
            .delegation_strategy
            .save(deps.storage, &delegation_strategy.validate(deps.api, &validators)?)?;
    }

    if let Some(allow_donations) = allow_donations {
        state.allow_donations.save(deps.storage, &allow_donations)?;
    }

    if let Some(vote_operator) = vote_operator {
        state.vote_operator.save(deps.storage, &deps.api.addr_validate(&vote_operator)?)?;
    }

    Ok(Response::new().add_attribute("action", "erishub/update_config"))
}
