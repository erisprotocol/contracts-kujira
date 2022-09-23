#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;

use cosmwasm_std::{
    to_binary, Addr, Api, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
};
use cw2::set_contract_version;

use crate::execute::{
    accept_ownership, assert_minium_receive, execute_multi_swap_operation, execute_swap_operation,
    execute_swap_operations, register_pair, register_pairs, remove_pairs, transfer_ownership,
};
use crate::queries::{get_pair_by_denom, get_pairs, query_config, simulate_swap_operations};
use crate::state::State;

use eris::router::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg};

// version info for migration info
const CONTRACT_NAME: &str = "eris-router";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    mut deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let state = State::default();
    state
        .owner
        .save(deps.storage, &deps.api.addr_validate(&msg.owner)?)?;

    for pair in msg.pairs {
        register_pair(&mut deps, pair)?;
    }

    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(deps: DepsMut, env: Env, info: MessageInfo, msg: ExecuteMsg) -> StdResult<Response> {
    match msg {
        ExecuteMsg::ExecuteMultiSwapOperation {
            operation,
            allowed_intermediate_denom,
            minimum_receive,
            to,
        } => {
            let api = deps.api;
            execute_multi_swap_operation(
                deps,
                env,
                info.sender,
                operation,
                minimum_receive,
                allowed_intermediate_denom,
                optional_addr_validate(api, to)?,
            )
        }

        ExecuteMsg::ExecuteSwapOperations {
            operations,
            minimum_receive,
            to,
        } => {
            let api = deps.api;
            execute_swap_operations(
                deps,
                env,
                info.sender,
                operations,
                minimum_receive,
                optional_addr_validate(api, to)?,
            )
        }

        // Internal
        ExecuteMsg::ExecuteSwapOperation { operation, to } => {
            let api = deps.api;
            execute_swap_operation(
                deps,
                env,
                info,
                operation,
                optional_addr_validate(api, to)?.map(|v| v.to_string()),
            )
        }
        // Internal
        ExecuteMsg::AssertMinimumReceive {
            asset_info,
            prev_balance,
            minimum_receive,
            receiver,
        } => assert_minium_receive(
            deps.as_ref(),
            asset_info,
            prev_balance,
            minimum_receive,
            deps.api.addr_validate(&receiver)?,
        ),

        // Owner
        ExecuteMsg::RemovePairs { pairs } => remove_pairs(deps, info.sender, pairs),
        ExecuteMsg::RegisterPairs { pairs } => register_pairs(deps, info.sender, pairs),

        ExecuteMsg::TransferOwnership { new_owner } => {
            transfer_ownership(deps, info.sender, new_owner)
        }
        ExecuteMsg::AcceptOwnership {} => accept_ownership(deps, info.sender),
    }
}

fn optional_addr_validate(api: &dyn Api, addr: Option<String>) -> StdResult<Option<Addr>> {
    let addr = if let Some(addr) = addr {
        Some(api.addr_validate(&addr)?)
    } else {
        None
    };

    Ok(addr)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&query_config(deps)?),
        QueryMsg::SimulateSwapOperations {
            offer_amount,
            operations,
        } => to_binary(&simulate_swap_operations(deps, offer_amount, operations)?),
        QueryMsg::Pairs { start_after, limit } => to_binary(&get_pairs(deps, start_after, limit)?),
        QueryMsg::PairByDenoms { asset_infos } => to_binary(&get_pair_by_denom(deps, asset_infos)?),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, _msg: MigrateMsg) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(Response::default())
}
