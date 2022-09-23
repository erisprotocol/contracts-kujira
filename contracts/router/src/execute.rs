use std::{collections::HashMap, vec};

use cosmwasm_std::{
    to_binary, Addr, Coin, CosmosMsg, Deps, DepsMut, Env, Event, MessageInfo, Response, StdError,
    StdResult, Uint128, WasmMsg,
};
use eris::{
    querier::{query_balance, query_fin_pair},
    router::{ExecuteMsg, PairInfo, SwapOperation},
};
use kujira::fin::{ConfigResponse as FinConfigResponse, ExecuteMsg as FinExecuteMsg};

use crate::state::State;
// use terraswap::asset::{Asset, AssetInfo, PairInfo};
// use terraswap::pair::ExecuteMsg as PairExecuteMsg;
// use terraswap::querier::{query_balance, query_pair_info, query_token_balance};

pub fn assert_operations(operations: &[SwapOperation]) -> StdResult<()> {
    let mut ask_asset_map: HashMap<String, bool> = HashMap::new();
    for operation in operations.iter() {
        let (offer_asset, ask_asset) = match operation {
            SwapOperation::Swap {
                offer_asset_info,
                ask_asset_info,
            } => (offer_asset_info.clone(), ask_asset_info.clone()),
        };

        ask_asset_map.remove(&offer_asset.to_string());
        ask_asset_map.insert(ask_asset.to_string(), true);
    }

    if ask_asset_map.keys().len() != 1 {
        return Err(StdError::generic_err(
            "invalid operations; multiple output token",
        ));
    }

    Ok(())
}

pub fn execute_multi_swap_operation(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    operation: SwapOperation,
    minimum_receive: Option<Uint128>,
    allowed_intermediate_denom: Vec<String>,
    to: Option<Addr>,
) -> StdResult<Response> {
    let state = State::default();
    let offer = operation.get_source_asset_info();
    let ask = operation.get_target_asset_info();
    let pair = state.get_pair(deps.storage, offer.clone(), ask.clone());
    if pair.is_ok() {
        // if we find the pair we can
        return execute_swap_operations(deps, env, sender, vec![operation], minimum_receive, to);
    }

    // could be indexed when adding pairs
    let pairs =
        state.try_find_pair(deps.storage, offer.clone(), ask, allowed_intermediate_denom)?;

    let mut from = offer;
    let mut operations: Vec<SwapOperation> = vec![];

    // execute in the right order, as pairs are stored in any order.
    for pair in pairs {
        if pair.asset_infos[0] == from {
            operations.push(SwapOperation::Swap {
                offer_asset_info: pair.asset_infos[0].clone(),
                ask_asset_info: pair.asset_infos[1].clone(),
            });
            from = pair.asset_infos[1].clone();
        } else {
            operations.push(SwapOperation::Swap {
                offer_asset_info: pair.asset_infos[1].clone(),
                ask_asset_info: pair.asset_infos[0].clone(),
            });
            from = pair.asset_infos[0].clone();
        }
    }

    execute_swap_operations(deps, env, sender, operations, minimum_receive, to)
}

pub fn execute_swap_operations(
    deps: DepsMut,
    env: Env,
    sender: Addr,
    operations: Vec<SwapOperation>,
    minimum_receive: Option<Uint128>,
    to: Option<Addr>,
) -> StdResult<Response> {
    let operations_len = operations.len();
    if operations_len == 0 {
        return Err(StdError::generic_err("must provide operations"));
    }

    // Assert the operations are properly set
    assert_operations(&operations)?;

    let to = if let Some(to) = to { to } else { sender };
    let target_asset_info = operations.last().unwrap().get_target_asset_info();

    let mut operation_index = 0;
    let mut messages: Vec<CosmosMsg> = operations
        .into_iter()
        .map(|op| {
            operation_index += 1;
            Ok(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: env.contract.address.to_string(),
                funds: vec![],
                msg: to_binary(&ExecuteMsg::ExecuteSwapOperation {
                    operation: op,
                    to: if operation_index == operations_len {
                        Some(to.to_string())
                    } else {
                        None
                    },
                })?,
            }))
        })
        .collect::<StdResult<Vec<CosmosMsg>>>()?;

    // Execute minimum amount assertion
    if let Some(minimum_receive) = minimum_receive {
        let receiver_balance =
            query_balance(&deps.querier, to.clone(), target_asset_info.to_string())?;

        messages.push(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            funds: vec![],
            msg: to_binary(&ExecuteMsg::AssertMinimumReceive {
                asset_info: target_asset_info,
                prev_balance: receiver_balance,
                minimum_receive,
                receiver: to.to_string(),
            })?,
        }))
    }

    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "erisrouter/execute_swap_operations"))
}

pub fn assert_minium_receive(
    deps: Deps,
    asset_info: String,
    prev_balance: Uint128,
    minium_receive: Uint128,
    receiver: Addr,
) -> StdResult<Response> {
    let receiver_balance = query_balance(&deps.querier, receiver, asset_info)?;
    let swap_amount = receiver_balance.checked_sub(prev_balance)?;

    if swap_amount < minium_receive {
        return Err(StdError::generic_err(format!(
            "assertion failed; minimum receive amount: {}, swap amount: {}",
            minium_receive, swap_amount
        )));
    }

    Ok(Response::default())
}

/// Execute swap operation
/// swap all offer asset to ask asset
pub fn execute_swap_operation(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    operation: SwapOperation,
    to: Option<String>,
) -> StdResult<Response> {
    if env.contract.address != info.sender {
        return Err(StdError::generic_err("unauthorized"));
    }

    let message: CosmosMsg = match operation {
        SwapOperation::Swap {
            offer_asset_info,
            ask_asset_info,
        } => {
            let state = State::default();
            let pair_info: PairInfo =
                state.get_pair(deps.storage, offer_asset_info.clone(), ask_asset_info)?;

            let amount = query_balance(
                &deps.querier,
                env.contract.address,
                offer_asset_info.clone(),
            )?;

            let denom = offer_asset_info;

            let mut to_addr: Option<Addr> = None;
            if let Some(to) = to {
                to_addr = Some(deps.api.addr_validate(to.as_str())?);
            }

            CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: pair_info.contract_addr.to_string(),
                funds: vec![Coin { denom, amount }],
                msg: to_binary(&FinExecuteMsg::Swap {
                    offer_asset: None, // Coin::new(amount, denom),
                    belief_price: None,
                    max_spread: None,
                    to: to_addr,
                })?,
            })
        }
    };

    Ok(Response::new()
        .add_message(message)
        .add_attribute("action", "erisrouter/execute_swap_operation"))
}

// pub fn asset_into_swap_msg(
//     _deps: Deps,
//     pair_contract: Addr,
//     offer_asset: Asset,
//     max_spread: Option<Decimal>,
//     to: Option<String>,
// ) -> StdResult<CosmosMsg> {
//     match offer_asset.info.clone() {
//         AssetInfo::NativeToken { denom } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
//             contract_addr: pair_contract.to_string(),
//             funds: vec![Coin {
//                 denom,
//                 amount: offer_asset.amount,
//             }],
//             msg: to_binary(&PairExecuteMsg::Swap {
//                 offer_asset,
//                 belief_price: None,
//                 max_spread,
//                 to,
//             })?,
//         })),
//         AssetInfo::Token { contract_addr } => Ok(CosmosMsg::Wasm(WasmMsg::Execute {
//             contract_addr,
//             funds: vec![],
//             msg: to_binary(&Cw20ExecuteMsg::Send {
//                 contract: pair_contract.to_string(),
//                 amount: offer_asset.amount,
//                 msg: to_binary(&PairExecuteMsg::Swap {
//                     offer_asset,
//                     belief_price: None,
//                     max_spread,
//                     to,
//                 })?,
//             })?,
//         })),
//     }
// }

pub fn register_pairs(mut deps: DepsMut, sender: Addr, pairs: Vec<String>) -> StdResult<Response> {
    let state = State::default();
    state.assert_owner(deps.storage, &sender)?;

    for pair in pairs {
        register_pair(&mut deps, pair)?;
    }

    Ok(Response::new().add_attribute("action", "erisrouter/add_pairs"))
}

pub fn register_pair(deps: &mut DepsMut, contract_addr: String) -> StdResult<PairInfo> {
    let contract_addr = deps.api.addr_validate(contract_addr.as_str())?;

    let state = State::default();

    let fin_config: FinConfigResponse = query_fin_pair(&deps.querier, &contract_addr)?;

    if fin_config.is_bootstrapping {
        return Err(StdError::generic_err("pair needs to be active"));
    }

    let pair = PairInfo {
        asset_infos: fin_config.denoms.map(|d| d.to_string()),
        contract_addr,
    };

    state
        .pairs
        .save(deps.storage, pair.contract_addr.clone(), &pair)?;

    Ok(pair)
}

pub fn remove_pairs(mut deps: DepsMut, sender: Addr, pairs: Vec<String>) -> StdResult<Response> {
    let state = State::default();
    state.assert_owner(deps.storage, &sender)?;

    for pair in pairs {
        remove_pair(&mut deps, pair)?;
    }

    Ok(Response::new().add_attribute("action", "erisrouter/remove_pairs"))
}

pub fn remove_pair(deps: &mut DepsMut, contract_addr: String) -> StdResult<()> {
    let contract_addr = deps.api.addr_validate(contract_addr.as_str())?;

    let state = State::default();

    state.pairs.remove(deps.storage, contract_addr)?;

    Ok(())
}

pub fn transfer_ownership(deps: DepsMut, sender: Addr, new_owner: String) -> StdResult<Response> {
    let state = State::default();

    state.assert_owner(deps.storage, &sender)?;
    state
        .new_owner
        .save(deps.storage, &deps.api.addr_validate(&new_owner)?)?;

    Ok(Response::new().add_attribute("action", "erisrouter/transfer_ownership"))
}

pub fn accept_ownership(deps: DepsMut, sender: Addr) -> StdResult<Response> {
    let state = State::default();

    let previous_owner = state.owner.load(deps.storage)?;
    let new_owner = state.new_owner.load(deps.storage)?;

    if sender != new_owner {
        return Err(StdError::generic_err(
            "unauthorized: sender is not new owner",
        ));
    }

    state.owner.save(deps.storage, &sender)?;
    state.new_owner.remove(deps.storage);

    let event = Event::new("erisrouter/ownership_transferred")
        .add_attribute("new_owner", new_owner)
        .add_attribute("previous_owner", previous_owner);

    Ok(Response::new()
        .add_event(event)
        .add_attribute("action", "erisrouter/transfer_ownership"))
}

#[test]
fn test_invalid_operations() {
    // empty error
    assert!(assert_operations(&[]).is_err());

    // uluna output
    assert!(assert_operations(&[
        SwapOperation::Swap {
            offer_asset_info: "ukrw".to_string(),
            ask_asset_info: "asset0001".to_string(),
        },
        SwapOperation::Swap {
            offer_asset_info: "asset0001".to_string(),
            ask_asset_info: "uluna".to_string(),
        }
    ])
    .is_ok());

    // asset0002 output
    assert!(assert_operations(&[
        SwapOperation::Swap {
            offer_asset_info: "ukrw".to_string(),
            ask_asset_info: "asset0001".to_string(),
        },
        SwapOperation::Swap {
            offer_asset_info: "asset0001".to_string(),
            ask_asset_info: "uluna".to_string(),
        },
        SwapOperation::Swap {
            offer_asset_info: "uluna".to_string(),
            ask_asset_info: "asset0002".to_string(),
        },
    ])
    .is_ok());
}
