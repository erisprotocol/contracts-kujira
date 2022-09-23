use cosmwasm_std::testing::{mock_env, mock_info, MockApi, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    from_binary, to_binary, Addr, Coin, CosmosMsg, OwnedDeps, StdError, SubMsg, Uint128, WasmMsg,
};
use kujira::fin::ExecuteMsg as FinExecuteMsg;

use crate::contract::{execute, instantiate, query};
use crate::testing::helpers::{mock_dependencies, query_helper};

use eris::router::{
    ConfigResponse, ExecuteMsg, InstantiateMsg, PairByDenomsResponse, PairInfo, PairsResponse,
    QueryMsg, SimulateSwapOperationsResponse, SwapOperation,
};

use super::custom_querier::CustomQuerier;

fn setup_test() -> OwnedDeps<MockStorage, MockApi, CustomQuerier> {
    let mut deps = mock_dependencies();

    let msg = InstantiateMsg {
        owner: "owner".to_string(),
        pairs: vec![
            "pair1".to_string(),
            "pair2".to_string(),
            "pair3".to_string(),
        ],
    };

    let info = mock_info("owner", &[]);
    let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();
    deps
}

#[test]
fn proper_initialization() {
    let deps = setup_test();

    // it worked, let's query the state
    let config: ConfigResponse =
        from_binary(&query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap()).unwrap();
    assert_eq!(
        config,
        ConfigResponse {
            new_owner: None,
            owner: "owner".to_string(),
        }
    );
}

#[test]
fn check_pairs() {
    let deps = setup_test();

    // ALL
    let pairs: PairsResponse = query_helper(
        deps.as_ref(),
        QueryMsg::Pairs {
            start_after: None,
            limit: None,
        },
    );
    assert_eq!(
        pairs,
        PairsResponse {
            pairs: vec![
                PairInfo {
                    asset_infos: ["from1".to_string(), "to1".to_string()],
                    contract_addr: Addr::unchecked("pair1")
                },
                PairInfo {
                    asset_infos: ["to1".to_string(), "to2".to_string()],
                    contract_addr: Addr::unchecked("pair2")
                },
                PairInfo {
                    asset_infos: ["to2".to_string(), "to3".to_string()],
                    contract_addr: Addr::unchecked("pair3")
                }
            ]
        }
    );

    // LIMIT
    let pairs: PairsResponse = query_helper(
        deps.as_ref(),
        QueryMsg::Pairs {
            start_after: None,
            limit: Some(1),
        },
    );
    assert_eq!(
        pairs,
        PairsResponse {
            pairs: vec![PairInfo {
                asset_infos: ["from1".to_string(), "to1".to_string()],
                contract_addr: Addr::unchecked("pair1")
            }]
        }
    );

    // START_AFTER
    let pairs: PairsResponse = query_helper(
        deps.as_ref(),
        QueryMsg::Pairs {
            start_after: Some("pair1".to_string()),
            limit: None,
        },
    );
    assert_eq!(
        pairs,
        PairsResponse {
            pairs: vec![
                PairInfo {
                    asset_infos: ["to1".to_string(), "to2".to_string()],
                    contract_addr: Addr::unchecked("pair2")
                },
                PairInfo {
                    asset_infos: ["to2".to_string(), "to3".to_string()],
                    contract_addr: Addr::unchecked("pair3")
                }
            ]
        }
    );

    // BY DENOM
    let pair: PairByDenomsResponse = query_helper(
        deps.as_ref(),
        QueryMsg::PairByDenoms {
            asset_infos: ["from1".to_string(), "to1".to_string()],
        },
    );
    assert_eq!(
        pair,
        PairByDenomsResponse {
            pair: PairInfo {
                asset_infos: ["from1".to_string(), "to1".to_string()],
                contract_addr: Addr::unchecked("pair1")
            }
        }
    );

    // BY DENOM REVERSE
    let pair: PairByDenomsResponse = query_helper(
        deps.as_ref(),
        QueryMsg::PairByDenoms {
            asset_infos: ["to1".to_string(), "from1".to_string()],
        },
    );
    assert_eq!(
        pair,
        PairByDenomsResponse {
            pair: PairInfo {
                asset_infos: ["from1".to_string(), "to1".to_string()],
                contract_addr: Addr::unchecked("pair1")
            }
        }
    );

    // BY DENOM REVERSE
    let pair: PairByDenomsResponse = query_helper(
        deps.as_ref(),
        QueryMsg::PairByDenoms {
            asset_infos: ["to2".to_string(), "to1".to_string()],
        },
    );
    assert_eq!(
        pair,
        PairByDenomsResponse {
            pair: PairInfo {
                asset_infos: ["to1".to_string(), "to2".to_string()],
                contract_addr: Addr::unchecked("pair2")
            }
        }
    );
}

#[test]
fn execute_swap_operations() {
    let mut deps = setup_test();

    let msg = ExecuteMsg::ExecuteSwapOperations {
        operations: vec![],
        minimum_receive: None,
        to: None,
    };

    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "must provide operations"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let msg = ExecuteMsg::ExecuteSwapOperations {
        operations: vec![
            SwapOperation::Swap {
                offer_asset_info: "from1".to_string(),
                ask_asset_info: "to1".to_string(),
            },
            SwapOperation::Swap {
                offer_asset_info: "to1".to_string(),
                ask_asset_info: "to2".to_string(),
            },
            SwapOperation::Swap {
                offer_asset_info: "to2".to_string(),
                ask_asset_info: "to3".to_string(),
            },
        ],
        minimum_receive: Some(Uint128::from(1000000u128)),
        to: None,
    };

    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                funds: vec![],
                msg: to_binary(&ExecuteMsg::ExecuteSwapOperation {
                    operation: SwapOperation::Swap {
                        offer_asset_info: "from1".to_string(),
                        ask_asset_info: "to1".to_string(),
                    },
                    to: None,
                })
                .unwrap(),
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                funds: vec![],
                msg: to_binary(&ExecuteMsg::ExecuteSwapOperation {
                    operation: SwapOperation::Swap {
                        offer_asset_info: "to1".to_string(),
                        ask_asset_info: "to2".to_string(),
                    },
                    to: None,
                })
                .unwrap(),
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                funds: vec![],
                msg: to_binary(&ExecuteMsg::ExecuteSwapOperation {
                    operation: SwapOperation::Swap {
                        offer_asset_info: "to2".to_string(),
                        ask_asset_info: "to3".to_string(),
                    },
                    to: Some("addr0000".to_string()),
                })
                .unwrap(),
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                funds: vec![],
                msg: to_binary(&ExecuteMsg::AssertMinimumReceive {
                    asset_info: "to3".to_string(),
                    prev_balance: Uint128::zero(),
                    minimum_receive: Uint128::from(1000000u128),
                    receiver: "addr0000".to_string(),
                })
                .unwrap(),
            })),
        ]
    );
}

#[test]
fn execute_swap_operation() {
    let mut deps = setup_test();

    deps.querier.set_bank_balances_addr(
        MOCK_CONTRACT_ADDR,
        &[Coin {
            amount: Uint128::from(1000000u128),
            denom: "to1".to_string(),
        }]
        .to_vec(),
    );

    let msg = ExecuteMsg::ExecuteSwapOperation {
        operation: SwapOperation::Swap {
            offer_asset_info: "to1".to_string(),
            ask_asset_info: "to2".to_string(),
        },
        to: None,
    };
    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg.clone());
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "unauthorized"),
        _ => panic!("DO NOT ENTER HERE"),
    }

    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "pair2".to_string(),
            funds: vec![Coin {
                denom: "to1".to_string(),
                amount: Uint128::from(1000000u128),
            }],
            msg: to_binary(&FinExecuteMsg::Swap {
                offer_asset: None,
                belief_price: None,
                max_spread: None,
                to: None
            })
            .unwrap(),
        }))],
    );

    // optional to address
    // swap_send
    let msg = ExecuteMsg::ExecuteSwapOperation {
        operation: SwapOperation::Swap {
            offer_asset_info: "to1".to_string(),
            ask_asset_info: "to2".to_string(),
        },
        to: Some("addr0000".to_string()),
    };
    let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();
    assert_eq!(
        res.messages,
        vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "pair2".to_string(),
            funds: vec![Coin {
                denom: "to1".to_string(),
                amount: Uint128::from(1000000u128),
            }],
            msg: to_binary(&FinExecuteMsg::Swap {
                offer_asset: None,
                belief_price: None,
                max_spread: None,
                to: Some(Addr::unchecked("addr0000"))
            })
            .unwrap(),
        }))],
    );
}

#[test]
fn execute_multi_swap_operation() {
    let mut deps = setup_test();

    let msg = ExecuteMsg::ExecuteMultiSwapOperation {
        operation: SwapOperation::Swap {
            offer_asset_info: "from1".to_string(),
            ask_asset_info: "to2".to_string(),
        },
        allowed_intermediate_denom: vec!["to1".to_string()],
        minimum_receive: Some(Uint128::from(1000000u128)),
        to: None,
    };

    let info = mock_info("addr0000", &[]);
    let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

    // match res.messages[1].msg.clone() {
    //     CosmosMsg::Wasm(WasmMsg::Execute {
    //         contract_addr,
    //         funds,
    //         msg,
    //     }) => {
    //         assert_eq!(contract_addr, MOCK_CONTRACT_ADDR.to_string());
    //         assert_eq!(funds.len(), 0);

    //         let sub_msg: ExecuteMsg = from_binary(&msg).unwrap();

    //         assert_eq!(
    //             sub_msg,
    //             ExecuteMsg::ExecuteSwapOperation {
    //                 operation: SwapOperation::Swap {
    //                     offer_asset_info: "to1".to_string(),
    //                     ask_asset_info: "to2".to_string(),
    //                 },
    //                 to: Some("addr0000".to_string()),
    //             }
    //         );
    //     }

    //     _ => panic!("DO NOT ENTER HERE"),
    // }

    assert_eq!(
        res.messages,
        vec![
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                funds: vec![],
                msg: to_binary(&ExecuteMsg::ExecuteSwapOperation {
                    operation: SwapOperation::Swap {
                        offer_asset_info: "from1".to_string(),
                        ask_asset_info: "to1".to_string(),
                    },
                    to: None,
                })
                .unwrap(),
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                funds: vec![],
                msg: to_binary(&ExecuteMsg::ExecuteSwapOperation {
                    operation: SwapOperation::Swap {
                        offer_asset_info: "to1".to_string(),
                        ask_asset_info: "to2".to_string(),
                    },
                    to: Some("addr0000".to_string()),
                })
                .unwrap(),
            })),
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: MOCK_CONTRACT_ADDR.to_string(),
                funds: vec![],
                msg: to_binary(&ExecuteMsg::AssertMinimumReceive {
                    asset_info: "to2".to_string(),
                    prev_balance: Uint128::zero(),
                    minimum_receive: Uint128::from(1000000u128),
                    receiver: "addr0000".to_string(),
                })
                .unwrap(),
            })),
        ]
    );
}

#[test]
fn query_buy_with_routes() {
    let deps = setup_test();

    let msg = QueryMsg::SimulateSwapOperations {
        offer_amount: Uint128::from(1000000u128),
        operations: vec![
            SwapOperation::Swap {
                offer_asset_info: "from1".to_string(),
                ask_asset_info: "to1".to_string(),
            },
            SwapOperation::Swap {
                offer_asset_info: "to1".to_string(),
                ask_asset_info: "to2".to_string(),
            },
        ],
    };

    let res: SimulateSwapOperationsResponse = query_helper(deps.as_ref(), msg);
    assert_eq!(
        res,
        SimulateSwapOperationsResponse {
            amount: Uint128::from(500000u128)
        }
    );
}

#[test]
fn query_buy_with_routes_2() {
    let deps = setup_test();

    let msg = QueryMsg::SimulateSwapOperations {
        offer_amount: Uint128::from(1000000u128),
        operations: vec![
            SwapOperation::Swap {
                offer_asset_info: "to2".to_string(),
                ask_asset_info: "to1".to_string(),
            },
            SwapOperation::Swap {
                offer_asset_info: "to1".to_string(),
                ask_asset_info: "from1".to_string(),
            },
        ],
    };

    let res: SimulateSwapOperationsResponse = query_helper(deps.as_ref(), msg);
    assert_eq!(
        res,
        SimulateSwapOperationsResponse {
            amount: Uint128::from(500000u128)
        }
    );
}

#[test]
fn query_buy_with_routes_failed() {
    let deps = setup_test();

    let msg = QueryMsg::SimulateSwapOperations {
        offer_amount: Uint128::from(1000000u128),
        operations: vec![
            SwapOperation::Swap {
                offer_asset_info: "to1".to_string(),
                ask_asset_info: "to2".to_string(),
            },
            SwapOperation::Swap {
                offer_asset_info: "to1".to_string(),
                ask_asset_info: "from1".to_string(),
            },
        ],
    };

    let res = query(deps.as_ref(), mock_env(), msg).unwrap_err();
    assert_eq!(
        res,
        StdError::generic_err("invalid operations; multiple output token")
    );
}

// #[test]
// fn query_reverse_routes_with_from_native() {
//     let mut deps = mock_dependencies(&[]);

//     let msg = InstantiateMsg {
//         Swap_factory: "Swapfactory".to_string(),
//     };

//     let target_amount = 1000000u128;

//     let info = mock_info("addr0000", &[coin(10000000, "ukrw")]);

//     // we can just call .unwrap() to assert this was a success
//     let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

//     deps.querier.with_balance(&[(
//         &MOCK_CONTRACT_ADDR.to_string(),
//         [Coin {
//             amount: Uint128::from(1000000u128),
//             denom: "ukrw".to_string(),
//         }]
//         .to_vec(),
//     )]);

//     deps.querier.with_token_balances(&[(
//         &"asset0001".to_string(),
//         &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(1000000u128))],
//     )]);

//     let msg = QueryMsg::ReverseSimulateSwapOperations {
//         ask_amount: Uint128::from(target_amount),
//         operations: vec![SwapOperation::Swap {
//             offer_asset_info: AssetInfo::NativeToken {
//                 denom: "ukrw".to_string(),
//             },
//             ask_asset_info: AssetInfo::Token {
//                 contract_addr: "asset0000".to_string(),
//             },
//         }],
//     };

//     deps.querier.with_Swap_factory(
//         &[
//             (
//                 &"ukrwasset0000".to_string(),
//                 &PairInfo {
//                     contract_addr: "pair0000".to_string(),
//                     liquidity_token: "liquidity0000".to_string(),
//                     asset_infos: [
//                         AssetInfo::Token {
//                             contract_addr: "asset0000".to_string(),
//                         },
//                         AssetInfo::NativeToken {
//                             denom: "ukrw".to_string(),
//                         },
//                     ],
//                     asset_decimals: [8u8, 6u8],
//                 },
//             ),
//             (
//                 &"asset0000uluna".to_string(),
//                 &PairInfo {
//                     contract_addr: "pair0001".to_string(),
//                     liquidity_token: "liquidity0001".to_string(),
//                     asset_infos: [
//                         AssetInfo::Token {
//                             contract_addr: "asset0000".to_string(),
//                         },
//                         AssetInfo::NativeToken {
//                             denom: "uluna".to_string(),
//                         },
//                     ],
//                     asset_decimals: [8u8, 6u8],
//                 },
//             ),
//         ],
//         &[("ukrw".to_string(), 6u8), ("uluna".to_string(), 6u8)],
//     );

//     let res: SimulateSwapOperationsResponse =
//         from_binary(&query(deps.as_ref(), mock_env(), msg).unwrap()).unwrap();

//     assert_eq!(
//         res,
//         SimulateSwapOperationsResponse {
//             amount: Uint128::from(1000000u128),
//         }
//     );

//     let offer_amount = res.amount;

//     let msg = ExecuteMsg::ExecuteSwapOperation {
//         operation: SwapOperation::Swap {
//             offer_asset_info: AssetInfo::NativeToken {
//                 denom: "ukrw".to_string(),
//             },
//             ask_asset_info: AssetInfo::Token {
//                 contract_addr: "asset0000".to_string(),
//             },
//         },
//         to: None,
//     };
//     let info = mock_info("addr0", &[coin(offer_amount.u128(), "ukrw")]);
//     let res = execute(deps.as_mut(), mock_env(), info, msg.clone());
//     match res {
//         Err(StdError::GenericErr { msg, .. }) => assert_eq!(msg, "unauthorized"),
//         _ => panic!("DO NOT ENTER HERE"),
//     }

//     let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
//     let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

//     assert_eq!(
//         res.messages,
//         vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
//             contract_addr: "pair0000".to_string(),
//             funds: vec![coin(target_amount, "ukrw")],
//             msg: to_binary(&PairExecuteMsg::Swap {
//                 offer_asset: Asset {
//                     info: AssetInfo::NativeToken {
//                         denom: "ukrw".to_string(),
//                     },
//                     amount: Uint128::from(target_amount),
//                 },
//                 belief_price: None,
//                 max_spread: None,
//                 to: None,
//             })
//             .unwrap(),
//         })),],
//     );
// }

// #[test]
// fn query_reverse_routes_with_to_native() {
//     let mut deps = mock_dependencies(&[]);

//     let msg = InstantiateMsg {
//         Swap_factory: "Swapfactory".to_string(),
//     };

//     let target_amount = 1000000u128;

//     let info = mock_info("addr0000", &[]);

//     // we can just call .unwrap() to assert this was a success
//     let _res = instantiate(deps.as_mut(), mock_env(), info, msg).unwrap();

//     deps.querier.with_token_balances(&[
//         (
//             &"asset0000".to_string(),
//             &[(&"pair0000".to_string(), &Uint128::from(1000000u128))],
//         ),
//         (
//             &"asset0000".to_string(),
//             &[(&MOCK_CONTRACT_ADDR.to_string(), &Uint128::from(1000000u128))],
//         ),
//     ]);

//     let msg = QueryMsg::ReverseSimulateSwapOperations {
//         ask_amount: Uint128::from(target_amount),
//         operations: vec![SwapOperation::Swap {
//             offer_asset_info: AssetInfo::Token {
//                 contract_addr: "asset0000".to_string(),
//             },
//             ask_asset_info: AssetInfo::NativeToken {
//                 denom: "ukrw".to_string(),
//             },
//         }],
//     };

//     deps.querier.with_Swap_factory(
//         &[
//             (
//                 &"ukrwasset0000".to_string(),
//                 &PairInfo {
//                     contract_addr: "pair0000".to_string(),
//                     liquidity_token: "liquidity0000".to_string(),
//                     asset_infos: [
//                         AssetInfo::Token {
//                             contract_addr: "asset0000".to_string(),
//                         },
//                         AssetInfo::NativeToken {
//                             denom: "ukrw".to_string(),
//                         },
//                     ],
//                     asset_decimals: [8u8, 6u8],
//                 },
//             ),
//             (
//                 &"asset0000uluna".to_string(),
//                 &PairInfo {
//                     contract_addr: "pair0001".to_string(),
//                     liquidity_token: "liquidity0001".to_string(),
//                     asset_infos: [
//                         AssetInfo::Token {
//                             contract_addr: "asset0000".to_string(),
//                         },
//                         AssetInfo::NativeToken {
//                             denom: "uluna".to_string(),
//                         },
//                     ],
//                     asset_decimals: [8u8, 6u8],
//                 },
//             ),
//         ],
//         &[("ukrw".to_string(), 6u8), ("uluna".to_string(), 6u8)],
//     );

//     let res: SimulateSwapOperationsResponse =
//         from_binary(&query(deps.as_ref(), mock_env(), msg).unwrap()).unwrap();

//     assert_eq!(
//         res,
//         SimulateSwapOperationsResponse {
//             amount: Uint128::from(target_amount),
//         }
//     );

//     let offer_amount = res.amount;

//     let msg = ExecuteMsg::Receive(Cw20ReceiveMsg {
//         sender: "addr0".to_string(),
//         amount: offer_amount,
//         msg: to_binary(&Cw20HookMsg::ExecuteSwapOperations {
//             operations: vec![SwapOperation::Swap {
//                 offer_asset_info: AssetInfo::Token {
//                     contract_addr: "asset0000".to_string(),
//                 },
//                 ask_asset_info: AssetInfo::NativeToken {
//                     denom: "ukrw".to_string(),
//                 },
//             }],
//             minimum_receive: None,
//             to: None,
//         })
//         .unwrap(),
//     });
//     let info = mock_info("addr0", &[]);
//     let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

//     assert_eq!(
//         res.messages,
//         vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
//             contract_addr: MOCK_CONTRACT_ADDR.to_string(),
//             funds: vec![],
//             msg: to_binary(&ExecuteMsg::ExecuteSwapOperation {
//                 operation: SwapOperation::Swap {
//                     offer_asset_info: AssetInfo::Token {
//                         contract_addr: "asset0000".to_string(),
//                     },
//                     ask_asset_info: AssetInfo::NativeToken {
//                         denom: "ukrw".to_string(),
//                     },
//                 },
//                 to: Some("addr0".to_string()),
//             })
//             .unwrap(),
//         })),],
//     );

//     let msg = ExecuteMsg::ExecuteSwapOperation {
//         operation: SwapOperation::Swap {
//             offer_asset_info: AssetInfo::Token {
//                 contract_addr: "asset0000".to_string(),
//             },
//             ask_asset_info: AssetInfo::NativeToken {
//                 denom: "ukrw".to_string(),
//             },
//         },
//         to: None,
//     };

//     let info = mock_info(MOCK_CONTRACT_ADDR, &[]);
//     let res = execute(deps.as_mut(), mock_env(), info, msg).unwrap();

//     assert_eq!(
//         res.messages,
//         vec![SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
//             contract_addr: "asset0000".to_string(),
//             funds: vec![],
//             msg: to_binary(&Cw20ExecuteMsg::Send {
//                 contract: "pair0000".to_string(),
//                 amount: Uint128::from(target_amount),
//                 msg: to_binary(&PairExecuteMsg::Swap {
//                     offer_asset: Asset {
//                         info: AssetInfo::Token {
//                             contract_addr: "asset0000".to_string(),
//                         },
//                         amount: Uint128::from(target_amount),
//                     },
//                     belief_price: None,
//                     max_spread: None,
//                     to: None,
//                 })
//                 .unwrap(),
//             })
//             .unwrap(),
//         }))],
//     );
// }

#[test]
fn assert_minimum_receive_native_token() {
    let mut deps = mock_dependencies();
    deps.querier.set_bank_balances_addr(
        "addr0000",
        &[Coin {
            denom: "utoken".to_string(),
            amount: Uint128::from(1000000u128),
        }]
        .to_vec(),
    );

    let info = mock_info("addr0000", &[]);
    // success
    let msg = ExecuteMsg::AssertMinimumReceive {
        asset_info: "utoken".to_string(),
        prev_balance: Uint128::zero(),
        minimum_receive: Uint128::from(1000000u128),
        receiver: "addr0000".to_string(),
    };
    let _res = execute(deps.as_mut(), mock_env(), info.clone(), msg).unwrap();

    // assertion failed; native token
    let msg = ExecuteMsg::AssertMinimumReceive {
        asset_info: "utoken".to_string(),
        prev_balance: Uint128::zero(),
        minimum_receive: Uint128::from(1000001u128),
        receiver: "addr0000".to_string(),
    };
    let res = execute(deps.as_mut(), mock_env(), info, msg);
    match res {
        Err(StdError::GenericErr { msg, .. }) => assert_eq!(
            msg,
            "assertion failed; minimum receive amount: 1000001, swap amount: 1000000"
        ),
        _ => panic!("DO NOT ENTER HERE"),
    }
}
