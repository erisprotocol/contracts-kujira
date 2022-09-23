use std::convert::TryInto;

use cosmwasm_std::{Addr, Deps, Order, StdError, StdResult, Uint128};
use cw_storage_plus::Bound;
use eris::{
    querier::simulate,
    router::{
        ConfigResponse, PairByDenomsResponse, PairInfo, PairsResponse,
        SimulateSwapOperationsResponse, SwapOperation,
    },
};
use kujira::asset::Asset;

use crate::{execute::assert_operations, state::State};

const MAX_LIMIT: u32 = 30;
const DEFAULT_LIMIT: u32 = 10;

pub fn query_config(deps: Deps) -> StdResult<ConfigResponse> {
    let state = State::default();
    Ok(ConfigResponse {
        owner: state.owner.load(deps.storage)?.into(),
        new_owner: state
            .new_owner
            .may_load(deps.storage)?
            .map(|addr| addr.into()),
    })
}

pub fn get_pairs(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<PairsResponse> {
    let state = State::default();

    let limit = limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT) as usize;

    let mut start: Option<Bound<Addr>> = None;
    let addr: Addr;
    if let Some(start_after) = start_after {
        if let Ok(start_after_addr) = deps.api.addr_validate(&start_after) {
            addr = start_after_addr;
            start = Some(Bound::exclusive(addr));
        }
    }

    let pairs: StdResult<Vec<PairInfo>> = state
        .pairs
        .range(deps.storage, start, None, Order::Ascending)
        .take(limit)
        .map(|item| {
            let (_, v) = item?;

            Ok(v)
        })
        .collect();

    Ok(PairsResponse { pairs: pairs? })
}

pub fn get_pair_by_denom(deps: Deps, asset_infos: [String; 2]) -> StdResult<PairByDenomsResponse> {
    let state = State::default();

    Ok(PairByDenomsResponse {
        pair: state.get_pair(deps.storage, asset_infos[0].clone(), asset_infos[1].clone())?,
    })
}

pub fn simulate_swap_operations(
    deps: Deps,
    offer_amount: Uint128,
    operations: Vec<SwapOperation>,
) -> StdResult<SimulateSwapOperationsResponse> {
    let operations_len = operations.len();
    if operations_len == 0 {
        return Err(StdError::generic_err("must provide operations"));
    }

    assert_operations(&operations)?;

    let state = State::default();
    let mut offer_amount: Uint128 = offer_amount;
    for operation in operations.into_iter() {
        match operation {
            SwapOperation::Swap {
                offer_asset_info,
                ask_asset_info,
            } => {
                let pair_info =
                    state.get_pair(deps.storage, offer_asset_info.clone(), ask_asset_info)?;

                let res = simulate(
                    &deps.querier,
                    Addr::unchecked(pair_info.contract_addr),
                    &Asset {
                        info: kujira::asset::AssetInfo::NativeToken {
                            denom: offer_asset_info.into(),
                        },
                        amount: offer_amount,
                    },
                )?;

                offer_amount = res.return_amount.try_into()?;
            }
        }
    }

    Ok(SimulateSwapOperationsResponse {
        amount: offer_amount,
    })
}

// fn reverse_simulate_swap_operations(
//     deps: Deps,
//     ask_amount: Uint128,
//     operations: Vec<SwapOperation>,
// ) -> StdResult<SimulateSwapOperationsResponse> {
//     let operations_len = operations.len();
//     if operations_len == 0 {
//         return Err(StdError::generic_err("must provide operations"));
//     }

//     let state = State::default();
//     let mut ask_amount = ask_amount;
//     for operation in operations.into_iter().rev() {
//         ask_amount = match operation {
//             SwapOperation::Swap {
//                 offer_asset_info,
//                 ask_asset_info,
//             } => reverse_simulate_return_amount(
//                 deps,
//                 state,
//                 ask_amount,
//                 offer_asset_info,
//                 ask_asset_info,
//             )
//             .unwrap(),
//         }
//     }

//     Ok(SimulateSwapOperationsResponse { amount: ask_amount })
// }

// fn reverse_simulate_return_amount(
//     deps: Deps,
//     state: State,
//     ask_amount: Uint128,
//     offer_asset_info: AssetInfo,
//     ask_asset_info: AssetInfo,
// ) -> StdResult<Uint128> {
//     let pair_info = state.get_pair(deps.storage, offer_asset_info, ask_asset_info)?;

//     let res = reverse_simulate(
//         &deps.querier,
//         Addr::unchecked(pair_info.contract_addr),
//         &Asset {
//             amount: ask_amount,
//             info: ask_asset_info,
//         },
//     )?;

//     Ok(res.offer_amount)
// }
