use cosmwasm_std::{
    to_binary, Addr, BalanceResponse, BankQuery, QuerierWrapper, QueryRequest, StdResult, Uint128,
    WasmQuery,
};
use kujira::{
    asset::Asset,
    fin::{ConfigResponse as FinConfigResponse, QueryMsg as FinQueryMsg, SimulationResponse},
};

pub fn query_fin_pair(
    querier: &QuerierWrapper,
    contract_addr: &Addr,
) -> StdResult<FinConfigResponse> {
    let res: FinConfigResponse = querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: contract_addr.to_string(),
        msg: to_binary(&FinQueryMsg::Config {})?,
    }))?;

    Ok(res)
}

pub fn query_balance(
    querier: &QuerierWrapper,
    account_addr: Addr,
    denom: String,
) -> StdResult<Uint128> {
    // load price form the oracle
    let balance: BalanceResponse = querier.query(&QueryRequest::Bank(BankQuery::Balance {
        address: account_addr.to_string(),
        denom,
    }))?;
    Ok(balance.amount.amount)
}

pub fn simulate(
    querier: &QuerierWrapper,
    pair_contract: Addr,
    offer_asset: &Asset,
) -> StdResult<SimulationResponse> {
    querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
        contract_addr: pair_contract.to_string(),
        msg: to_binary(&FinQueryMsg::Simulation {
            offer_asset: offer_asset.clone(),
        })?,
    }))
}

// pub fn reverse_simulate(
//     querier: &QuerierWrapper,
//     pair_contract: Addr,
//     ask_asset: &Asset,
// ) -> StdResult<ReverseSimulationResponse> {
//     querier.query(&QueryRequest::Wasm(WasmQuery::Smart {
//         contract_addr: pair_contract.to_string(),
//         msg: to_binary(&FinQueryMsg::ReverseSimulation {
//             ask_asset: ask_asset.clone(),
//         })?,
//     }))
// }
