use super::helpers::err_unsupported_query;
use cosmwasm_std::testing::BankQuerier;
use cosmwasm_std::{
    from_binary, from_slice, to_binary, Addr, Coin, Empty, Querier, QuerierResult, QueryRequest,
    SystemError, Uint128, Uint256, WasmQuery,
};

#[derive(Default)]
pub(super) struct CustomQuerier {
    pub bank_querier: BankQuerier,
}

impl Querier for CustomQuerier {
    fn raw_query(&self, bin_request: &[u8]) -> QuerierResult {
        let request: QueryRequest<_> = match from_slice(bin_request) {
            Ok(v) => v,
            Err(e) => {
                return Err(SystemError::InvalidRequest {
                    error: format!("Parsing query request: {}", e),
                    request: bin_request.into(),
                })
                .into()
            }
        };
        self.handle_query(&request)
    }
}

impl CustomQuerier {
    // pub fn with_balance(&mut self, balances: &[(&str, &[Coin])]) {
    //     self.bank_querier = BankQuerier::new(balances);
    // }

    // pub fn set_bank_balances(&mut self, balances: &[Coin]) {
    //     self.bank_querier = BankQuerier::new(&[(MOCK_CONTRACT_ADDR, balances)])
    // }

    pub fn set_bank_balances_addr(&mut self, addr: &str, balances: &[Coin]) {
        self.bank_querier = BankQuerier::new(&[(addr, balances)])
    }

    pub fn handle_query(&self, request: &QueryRequest<Empty>) -> QuerierResult {
        match request {
            QueryRequest::Wasm(WasmQuery::Smart { contract_addr, msg }) => {
                if contract_addr == "pair1" {
                    if let Ok(query) = from_binary::<kujira::fin::QueryMsg>(msg) {
                        return match &query {
                            kujira::fin::QueryMsg::Config {} => {
                                return Ok(to_binary(&kujira::fin::ConfigResponse {
                                    decimal_delta: 9,
                                    denoms: ["from1".into(), "to1".into()],
                                    is_bootstrapping: false,
                                    owner: Addr::unchecked("owner"),
                                    price_precision: kujira::precision::Precision::DecimalPlaces(3),
                                })
                                .into())
                                .into();
                            }
                            kujira::fin::QueryMsg::Simulation { offer_asset } => {
                                return Ok(to_binary(&kujira::fin::SimulationResponse {
                                    commission_amount: Uint256::from(Uint128::new(0)),
                                    return_amount: offer_asset
                                        .amount
                                        .checked_div(Uint128::new(2))
                                        .unwrap()
                                        .into(),
                                    spread_amount: Uint256::from(Uint128::new(0)),
                                })
                                .into())
                                .into();
                            }
                            _ => err_unsupported_query(msg),
                        };
                    }
                } else if contract_addr == "pair2" {
                    if let Ok(query) = from_binary::<kujira::fin::QueryMsg>(msg) {
                        return match &query {
                            kujira::fin::QueryMsg::Config {} => {
                                return Ok(to_binary(&kujira::fin::ConfigResponse {
                                    decimal_delta: 9,
                                    denoms: ["to1".into(), "to2".into()],
                                    is_bootstrapping: false,
                                    owner: Addr::unchecked("owner"),
                                    price_precision: kujira::precision::Precision::DecimalPlaces(3),
                                })
                                .into())
                                .into();
                            }
                            kujira::fin::QueryMsg::Simulation { offer_asset } => {
                                return Ok(to_binary(&kujira::fin::SimulationResponse {
                                    commission_amount: Uint256::from(Uint128::new(0)),
                                    return_amount: offer_asset.amount.into(),
                                    spread_amount: Uint256::from(Uint128::new(0)),
                                })
                                .into())
                                .into();
                            }
                            _ => err_unsupported_query(msg),
                        };
                    }
                } else if contract_addr == "pair3" {
                    if let Ok(query) = from_binary::<kujira::fin::QueryMsg>(msg) {
                        return match &query {
                            kujira::fin::QueryMsg::Config {} => {
                                return Ok(to_binary(&kujira::fin::ConfigResponse {
                                    decimal_delta: 9,
                                    denoms: ["to2".into(), "to3".into()],
                                    is_bootstrapping: false,
                                    owner: Addr::unchecked("owner"),
                                    price_precision: kujira::precision::Precision::DecimalPlaces(3),
                                })
                                .into())
                                .into();
                            }
                            kujira::fin::QueryMsg::Simulation { offer_asset } => {
                                return Ok(to_binary(&kujira::fin::SimulationResponse {
                                    commission_amount: Uint256::from(Uint128::new(0)),
                                    return_amount: offer_asset.amount.into(),
                                    spread_amount: Uint256::from(Uint128::new(0)),
                                })
                                .into())
                                .into();
                            }
                            _ => err_unsupported_query(msg),
                        };
                    }
                }

                // if let Ok(query) = from_binary::<Cw20QueryMsg>(msg) {
                //     return self.cw20_querier.handle_query(contract_addr, query);
                // }

                err_unsupported_query(msg)
            }

            QueryRequest::Bank(query) => self.bank_querier.query(query),

            _ => err_unsupported_query(request),
        }
    }
}
