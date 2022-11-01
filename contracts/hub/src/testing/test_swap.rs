use std::vec;

use cosmwasm_std::testing::{mock_env, mock_info, MockApi, MockStorage, MOCK_CONTRACT_ADDR};
use cosmwasm_std::{
    coin, to_binary, Addr, Coin, CosmosMsg, Decimal, DistributionMsg, OwnedDeps, StdError,
    StdResult, SubMsg, Uint128, WasmMsg,
};

use eris::adapters::bw_vault::BlackwhaleExecuteMsg;
use eris::adapters::fin_multi::FinMultiExecuteMsg;
use eris::hub::{
    CallbackMsg, ConfigResponse, ExecuteMsg, FeeConfig, InstantiateMsg, PendingBatch, QueryMsg,
    StateResponse,
};
use kujira::msg::{DenomMsg, KujiraMsg};

use crate::constants::CONTRACT_DENOM;
use crate::contract::{execute, instantiate};
use crate::testing::helpers::check_received_coin;
use crate::types::Delegation;

use super::custom_querier::CustomQuerier;
use super::helpers::{mock_dependencies, mock_env_at_timestamp, query_helper};

//--------------------------------------------------------------------------------------------------
// Test setup
//--------------------------------------------------------------------------------------------------

pub const STAKE_DENOM: &str = "factory/cosmos2contract/stake";
pub const BW_DENOM: &str = "factory/anycontract/btoken";
pub const BW_DENOM2: &str = "factory/anycontract/btoken2";

fn setup_test() -> OwnedDeps<MockStorage, MockApi, CustomQuerier> {
    let mut deps = mock_dependencies();

    let res = instantiate(
        deps.as_mut(),
        mock_env_at_timestamp(10000),
        mock_info("deployer", &[]),
        InstantiateMsg {
            fin_multi_contract: "fin_multi".to_string(),
            owner: "owner".to_string(),
            denom: "stake".to_string(),
            epoch_period: 259200,   // 3 * 24 * 60 * 60 = 3 days
            unbond_period: 1814400, // 21 * 24 * 60 * 60 = 21 days
            validators: vec!["alice".to_string(), "bob".to_string(), "charlie".to_string()],
            protocol_fee_contract: "fee".to_string(),
            protocol_reward_fee: Decimal::from_ratio(1u128, 100u128),
            operator: "operator".to_string(),
            stages_preset: Some(vec![vec![(Addr::unchecked("fin1"), "test".into())]]),
        },
    )
    .unwrap();

    assert_eq!(res.messages.len(), 1);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Custom(KujiraMsg::Denom(DenomMsg::Create {
            subdenom: "stake".into(),
        })))
    );

    deps
}

//--------------------------------------------------------------------------------------------------
// Execution
//--------------------------------------------------------------------------------------------------

#[test]
fn proper_instantiation() {
    let deps = setup_test();

    let res: ConfigResponse = query_helper(deps.as_ref(), QueryMsg::Config {});
    assert_eq!(
        res,
        ConfigResponse {
            owner: "owner".to_string(),
            new_owner: None,
            stake_token: STAKE_DENOM.to_string(),
            epoch_period: 259200,
            unbond_period: 1814400,
            validators: vec!["alice".to_string(), "bob".to_string(), "charlie".to_string()],
            fee_config: FeeConfig {
                protocol_fee_contract: Addr::unchecked("fee"),
                protocol_reward_fee: Decimal::from_ratio(1u128, 100u128)
            },
            operator: "operator".to_string(),
            stages_preset: vec![vec![(Addr::unchecked("fin1"), "test".into())]],
        }
    );

    let res: StateResponse = query_helper(deps.as_ref(), QueryMsg::State {});
    assert_eq!(
        res,
        StateResponse {
            total_ustake: Uint128::zero(),
            total_utoken: Uint128::zero(),
            exchange_rate: Decimal::one(),
            unlocked_coins: vec![],
            unbonding: Uint128::zero(),
            available: Uint128::zero(),
            tvl_utoken: Uint128::zero(),
        },
    );

    let res: PendingBatch = query_helper(deps.as_ref(), QueryMsg::PendingBatch {});
    assert_eq!(
        res,
        PendingBatch {
            id: 1,
            ustake_to_burn: Uint128::zero(),
            est_unbond_start_time: 269200, // 10,000 + 259,200
        },
    );
}

#[test]
fn harvesting_with_options() {
    let mut deps = setup_test();

    // Assume users have bonded a total of 1,000,000 utoken and minted the same amount of ustake
    deps.querier.set_staking_delegations(&[
        Delegation::new("alice", 341667),
        Delegation::new("bob", 341667),
        Delegation::new("charlie", 341666),
    ]);
    // deps.querier.set_cw20_total_supply("stake_token", 1000000);

    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("worker", &[]),
        ExecuteMsg::Harvest {
            stages: Some(vec![vec![(Addr::unchecked("fin1"), "test".into())]]),
            withdrawals: Some(vec![(Addr::unchecked("bw1"), BW_DENOM.into())]),
        },
    )
    .unwrap();

    assert_eq!(res.messages.len(), 7);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Distribution(DistributionMsg::WithdrawDelegatorReward {
            validator: "alice".to_string(),
        }))
    );
    assert_eq!(
        res.messages[1],
        SubMsg::new(CosmosMsg::Distribution(DistributionMsg::WithdrawDelegatorReward {
            validator: "bob".to_string(),
        }))
    );
    assert_eq!(
        res.messages[2],
        SubMsg::new(CosmosMsg::Distribution(DistributionMsg::WithdrawDelegatorReward {
            validator: "charlie".to_string(),
        }))
    );

    assert_eq!(
        res.messages[3],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: MOCK_CONTRACT_ADDR.to_string(),
            msg: to_binary(&ExecuteMsg::Callback(CallbackMsg::ClaimFunds {
                withdrawals: Some(vec![(Addr::unchecked("bw1"), BW_DENOM.into())]),
            }))
            .unwrap(),
            funds: vec![]
        }))
    );

    assert_eq!(
        res.messages[4],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: MOCK_CONTRACT_ADDR.to_string(),
            msg: to_binary(&ExecuteMsg::Callback(CallbackMsg::Swap {
                stages: Some(vec![vec![(Addr::unchecked("fin1"), "test".into())]]),
                sender: Addr::unchecked("worker")
            }))
            .unwrap(),
            funds: vec![]
        }))
    );

    assert_eq!(res.messages[5], check_received_coin(0, 0));

    assert_eq!(
        res.messages[6],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: MOCK_CONTRACT_ADDR.to_string(),
            msg: to_binary(&ExecuteMsg::Callback(CallbackMsg::Reinvest {})).unwrap(),
            funds: vec![]
        }))
    );
}

#[test]
fn claim_funds() -> StdResult<()> {
    let mut deps = setup_test();
    deps.querier.set_bank_balances(&[coin(100, BW_DENOM)]);

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("worker", &[]),
        ExecuteMsg::Callback(CallbackMsg::ClaimFunds {
            withdrawals: Some(vec![(Addr::unchecked("bw1"), BW_DENOM.into())]),
        }),
    )
    .unwrap_err();
    assert_eq!(err, StdError::generic_err("callbacks can only be invoked by the contract itself"));

    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(MOCK_CONTRACT_ADDR, &[]),
        ExecuteMsg::Callback(CallbackMsg::ClaimFunds {
            withdrawals: Some(vec![
                (Addr::unchecked("bw1"), BW_DENOM.into()),
                (Addr::unchecked("bw2"), BW_DENOM2.into()),
            ]),
        }),
    )
    .unwrap();

    assert_eq!(res.messages.len(), 1);

    let contract = "bw1";
    let amount = Uint128::new(100);
    let denom = BW_DENOM;

    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: contract.to_string(),
            funds: vec![Coin {
                amount,
                denom: denom.to_string(),
            }],
            msg: to_binary(&BlackwhaleExecuteMsg::WithdrawLiquidity {
                amount,
            })?,
        }))
    );

    Ok(())
}

#[test]
fn swap() -> StdResult<()> {
    let mut deps = setup_test();

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info("worker", &[]),
        ExecuteMsg::Callback(CallbackMsg::Swap {
            stages: Some(vec![vec![(Addr::unchecked("fin1"), "test".into())]]),
            sender: Addr::unchecked("worker"),
        }),
    )
    .unwrap_err();
    assert_eq!(err, StdError::generic_err("callbacks can only be invoked by the contract itself"));

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(MOCK_CONTRACT_ADDR, &[]),
        ExecuteMsg::Callback(CallbackMsg::Swap {
            stages: Some(vec![vec![(Addr::unchecked("fin1"), CONTRACT_DENOM.into())]]),
            sender: Addr::unchecked("worker"),
        }),
    )
    .unwrap_err();
    assert_eq!(err, StdError::generic_err("unauthorized: sender is not operator"));

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(MOCK_CONTRACT_ADDR, &[]),
        ExecuteMsg::Callback(CallbackMsg::Swap {
            stages: Some(vec![vec![(Addr::unchecked("fin1"), CONTRACT_DENOM.into())]]),
            sender: Addr::unchecked("operator"),
        }),
    )
    .unwrap_err();
    assert_eq!(err, StdError::generic_err(format!("swap from {} is not allowed", CONTRACT_DENOM)));

    let err = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(MOCK_CONTRACT_ADDR, &[]),
        ExecuteMsg::Callback(CallbackMsg::Swap {
            stages: Some(vec![vec![(Addr::unchecked("fin2"), STAKE_DENOM.into())]]),
            sender: Addr::unchecked("operator"),
        }),
    )
    .unwrap_err();
    assert_eq!(err, StdError::generic_err(format!("swap from {} is not allowed", STAKE_DENOM)));

    deps.querier.set_bank_balances(&[
        coin(100, "test"),
        coin(200, "abc"),
        coin(1000, "not_relevant"),
    ]);

    let stages = vec![
        vec![(Addr::unchecked("fin1"), "test".into())],
        vec![(Addr::unchecked("fin2"), "abc".into()), (Addr::unchecked("fin3"), "test2".into())],
        vec![(Addr::unchecked("fin4"), "abc".into())],
    ];

    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(MOCK_CONTRACT_ADDR, &[]),
        ExecuteMsg::Callback(CallbackMsg::Swap {
            stages: Some(stages.clone()),
            sender: Addr::unchecked("operator"),
        }),
    )
    .unwrap();

    assert_eq!(res.messages.len(), 1);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "fin_multi".to_string(),
            funds: vec![coin(100, "test"), coin(200, "abc")],
            msg: to_binary(&FinMultiExecuteMsg {
                stages,
                recipient: None,
            })?,
        }))
    );

    // SWAP WITHOUT STAGES USES DEFAULT

    let res = execute(
        deps.as_mut(),
        mock_env(),
        mock_info(MOCK_CONTRACT_ADDR, &[]),
        ExecuteMsg::Callback(CallbackMsg::Swap {
            stages: None,
            sender: Addr::unchecked("anyone"),
        }),
    )
    .unwrap();

    assert_eq!(res.messages.len(), 1);
    assert_eq!(
        res.messages[0],
        SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: "fin_multi".to_string(),
            funds: vec![coin(100, "test")],
            msg: to_binary(&FinMultiExecuteMsg {
                stages: vec![vec![(Addr::unchecked("fin1"), "test".into())]],
                recipient: None,
            })?,
        }))
    );

    Ok(())
}
