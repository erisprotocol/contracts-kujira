use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use cosmwasm_std::{Addr, Uint128};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    pub owner: String,

    pub pairs: Vec<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SwapOperation {
    Swap {
        offer_asset_info: String,
        ask_asset_info: String,
    },
}

impl SwapOperation {
    pub fn get_target_asset_info(&self) -> String {
        match self {
            SwapOperation::Swap {
                ask_asset_info,
                ..
            } => ask_asset_info.clone(),
        }
    }

    pub fn get_source_asset_info(&self) -> String {
        match self {
            SwapOperation::Swap {
                offer_asset_info,
                ..
            } => offer_asset_info.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Execute multiple BuyOperation
    ExecuteSwapOperations {
        operations: Vec<SwapOperation>,
        minimum_receive: Option<Uint128>,
        to: Option<String>,
    },

    /// Execute multiple Operations, by taking the first path found
    /// Will check if direct connection is available between provided assets.
    /// If not, will try to find swaps using the intermediate denoms
    ExecuteMultiSwapOperation {
        operation: SwapOperation,
        allowed_intermediate_denom: Vec<String>,
        minimum_receive: Option<Uint128>,
        to: Option<String>,
    },

    /// Internal use
    /// Swap all offer tokens to ask token
    ExecuteSwapOperation {
        operation: SwapOperation,
        to: Option<String>,
    },
    /// Internal use
    /// Check the swap amount is exceed minimum_receive
    AssertMinimumReceive {
        asset_info: String,
        prev_balance: Uint128,
        minimum_receive: Uint128,
        receiver: String,
    },

    /// Addes fin market pairs to the registry
    RegisterPairs {
        pairs: Vec<String>,
    },

    /// Remove fin market pairs from the registry
    RemovePairs {
        pairs: Vec<String>,
    },

    // Transfer ownership to another account; will not take effect unless the new owner accepts
    TransferOwnership {
        new_owner: String,
    },
    /// Accept an ownership transfer
    AcceptOwnership {},
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    Config {},
    SimulateSwapOperations {
        offer_amount: Uint128,
        operations: Vec<SwapOperation>,
    },
    Pairs {
        start_after: Option<String>,
        limit: Option<u32>,
    },
    PairByDenoms {
        asset_infos: [String; 2],
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PairsResponse {
    pub pairs: Vec<PairInfo>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PairByDenomsResponse {
    pub pair: PairInfo,
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    /// Account who can call certain privileged functions
    pub owner: String,
    /// Pending ownership transfer, awaiting acceptance by the new owner
    pub new_owner: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PairInfo {
    pub asset_infos: [String; 2],
    pub contract_addr: Addr,
}

impl PairInfo {
    pub fn create_key(&self) -> String {
        create_key_for_assets(&self.asset_infos)
    }
}

pub fn create_key_for_assets(asset_infos: &[String; 2]) -> String {
    let first = asset_infos[0].clone();
    let second = asset_infos[1].clone();

    if first < second {
        return format!("{0}{1}", first, second);
    } else {
        return format!("{1}{0}", first, second);
    }
}

// We define a custom struct for each query response
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct SimulateSwapOperationsResponse {
    pub amount: Uint128,
}

/// We currently take no arguments for migrations
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct MigrateMsg {}
