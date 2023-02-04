use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{to_binary, Addr, Coin, CosmosMsg, Decimal, Empty, StdResult, Uint128, WasmMsg};
use kujira::{denom::Denom, msg::KujiraMsg};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct InstantiateMsg {
    /// fin multi contract addr
    pub fin_multi_contract: String,
    /// Account who can call certain privileged functions
    pub owner: String,

    /// Account who can call harvest
    pub operator: String,
    /// Stages that should be used in the permissionless harvest function
    pub stages_preset: Option<Vec<Vec<(Addr, Denom)>>>,

    /// Name of the liquid staking token
    pub denom: String,
    /// How often the unbonding queue is to be executed, in seconds
    pub epoch_period: u64,
    /// The staking module's unbonding time, in seconds
    pub unbond_period: u64,
    /// Initial set of validators who will receive the delegations
    pub validators: Vec<String>,

    /// Contract address where fees are sent
    pub protocol_fee_contract: String,
    /// Fees that are being applied during reinvest of staking rewards
    pub protocol_reward_fee: Decimal, // "1 is 100%, 0.05 is 5%"
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Bond specified amount of Token
    Bond {
        receiver: Option<String>,
    },
    /// Donates specified amount of Token to pool
    Donate {},
    /// Withdraw Token that have finished unbonding in previous batches
    WithdrawUnbonded {
        receiver: Option<String>,
    },
    /// Add a validator to the whitelist; callable by the owner
    AddValidator {
        validator: String,
    },
    /// Remove a validator from the whitelist; callable by the owner
    RemoveValidator {
        validator: String,
    },
    /// Transfer ownership to another account; will not take effect unless the new owner accepts
    TransferOwnership {
        new_owner: String,
    },
    /// Accept an ownership transfer
    AcceptOwnership {},
    /// Claim staking rewards, swap all for Token, and restake
    Harvest {
        withdrawals: Option<Vec<(WithdrawType, Addr, Denom)>>,
        stages: Option<Vec<Vec<(Addr, Denom)>>>,
    },
    /// Use redelegations to balance the amounts of Token delegated to validators
    Rebalance {},
    /// Update Token amounts in unbonding batches to reflect any slashing or rounding errors
    Reconcile {},
    /// Submit the current pending batch of unbonding requests to be unbonded
    SubmitBatch {},
    /// Callbacks; can only be invoked by the contract itself
    Callback(CallbackMsg),

    /// Updates the fee config,
    UpdateConfig {
        /// Contract address where fees are sent
        protocol_fee_contract: Option<String>,
        /// Fees that are being applied during reinvest of staking rewards
        protocol_reward_fee: Option<Decimal>, // "1 is 100%, 0.05 is 5%"
        /// Sets a new operator
        operator: Option<String>,
        /// Sets the stages preset
        stages_preset: Option<Vec<Vec<(Addr, Denom)>>>,
    },

    /// Submit an unbonding request to the current unbonding queue; automatically invokes `unbond`
    /// if `epoch_time` has elapsed since when the last unbonding queue was executed.
    QueueUnbond {
        receiver: Option<String>,
    },
}

#[cw_serde]
pub enum WithdrawType {
    BlackWhale,
    Bow,
}

#[cw_serde]
pub enum CallbackMsg {
    ClaimFunds {
        withdrawals: Option<Vec<(WithdrawType, Addr, Denom)>>,
    },
    /// Swap remaining tokens held by the contract to Token
    Swap {
        sender: Addr,
        stages: Option<Vec<Vec<(Addr, Denom)>>>,
    },
    /// Following the swaps, stake the Token acquired to the whitelisted validators
    Reinvest {},

    CheckReceivedCoin {
        snapshot: Coin,
        snapshot_stake: Coin,
    },
}

impl CallbackMsg {
    pub fn into_cosmos_msg(&self, contract_addr: &Addr) -> StdResult<CosmosMsg<KujiraMsg>> {
        Ok(CosmosMsg::Wasm(WasmMsg::Execute {
            contract_addr: contract_addr.to_string(),
            msg: to_binary(&ExecuteMsg::Callback(self.clone()))?,
            funds: vec![],
        }))
    }
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// The contract's configurations. Response: `ConfigResponse`
    #[returns(ConfigResponse)]
    Config {},
    /// The contract's current state. Response: `StateResponse`
    #[returns(StateResponse)]
    State {},
    /// The current batch on unbonding requests pending submission. Response: `PendingBatch`
    #[returns(PendingBatch)]
    PendingBatch {},
    /// Query an individual batch that has previously been submitted for unbonding but have not yet
    /// fully withdrawn. Response: `Batch`
    #[returns(Batch)]
    PreviousBatch(u64),
    /// Enumerate all previous batches that have previously been submitted for unbonding but have not
    /// yet fully withdrawn. Response: `Vec<Batch>`
    #[returns(Vec<Batch>)]
    PreviousBatches {
        start_after: Option<u64>,
        limit: Option<u32>,
    },
    /// Enumerate all outstanding unbonding requests in a given batch. Response: `Vec<UnbondRequestsByBatchResponseItem>`
    #[returns(Vec<UnbondRequestsByBatchResponseItem>)]
    UnbondRequestsByBatch {
        id: u64,
        start_after: Option<String>,
        limit: Option<u32>,
    },
    /// Enumreate all outstanding unbonding requests from given a user. Response: `Vec<UnbondRequestsByUserResponseItem>`
    #[returns(Vec<UnbondRequestsByUserResponseItem>)]
    UnbondRequestsByUser {
        user: String,
        start_after: Option<u64>,
        limit: Option<u32>,
    },
    /// Enumreate all outstanding unbonding requests from given a user. Response: `Vec<UnbondRequestsByUserResponseItemDetails>`
    #[returns(Vec<UnbondRequestsByUserResponseItemDetails>)]
    UnbondRequestsByUserDetails {
        user: String,
        start_after: Option<u64>,
        limit: Option<u32>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct ConfigResponse {
    /// Account who can call certain privileged functions
    pub owner: String,
    /// Pending ownership transfer, awaiting acceptance by the new owner
    pub new_owner: Option<String>,
    /// Address of the Stake token
    pub stake_token: String,

    /// How often the unbonding queue is to be executed, in seconds
    pub epoch_period: u64,
    /// The staking module's unbonding time, in seconds
    pub unbond_period: u64,
    /// Initial set of validators who will receive the delegations
    pub validators: Vec<String>,

    /// Information about applied fees
    pub fee_config: FeeConfig,

    /// Account who can call harvest
    pub operator: String,
    /// Stages that must be used by permissionless users
    pub stages_preset: Vec<Vec<(Addr, Denom)>>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct StateResponse {
    /// Total supply to the Stake token
    pub total_ustake: Uint128,
    /// Total amount of utoken staked (bonded)
    pub total_utoken: Uint128,
    /// The exchange rate between ustake and utoken, in terms of utoken per ustake
    pub exchange_rate: Decimal,
    /// Staking rewards currently held by the contract that are ready to be reinvested
    pub unlocked_coins: Vec<Coin>,
    // Amount of utoken currently unbonding
    pub unbonding: Uint128,
    // Amount of utoken currently available as balance of the contract
    pub available: Uint128,
    // Total amount of utoken within the contract (bonded + unbonding + available)
    pub tvl_utoken: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct PendingBatch {
    /// ID of this batch
    pub id: u64,
    /// Total amount of `ustake` to be burned in this batch
    pub ustake_to_burn: Uint128,
    /// Estimated time when this batch will be submitted for unbonding
    pub est_unbond_start_time: u64,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug, PartialEq, JsonSchema)]
pub struct StakeToken {
    // denom of the stake token
    pub denom: String,
    // supply of the stake token
    pub total_supply: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct FeeConfig {
    /// Contract address where fees are sent
    pub protocol_fee_contract: Addr,
    /// Fees that are being applied during reinvest of staking rewards
    pub protocol_reward_fee: Decimal, // "1 is 100%, 0.05 is 5%"
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct Batch {
    /// ID of this batch
    pub id: u64,
    /// Whether this batch has already been reconciled
    pub reconciled: bool,
    /// Total amount of shares remaining this batch. Each `ustake` burned = 1 share
    pub total_shares: Uint128,
    /// Amount of `utoken` in this batch that have not been claimed
    pub utoken_unclaimed: Uint128,
    /// Estimated time when this batch will finish unbonding
    pub est_unbond_end_time: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UnbondRequest {
    /// ID of the batch
    pub id: u64,
    /// The user's address
    pub user: Addr,
    /// The user's share in the batch
    pub shares: Uint128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UnbondRequestsByBatchResponseItem {
    /// The user's address
    pub user: String,
    /// The user's share in the batch
    pub shares: Uint128,
}

impl From<UnbondRequest> for UnbondRequestsByBatchResponseItem {
    fn from(s: UnbondRequest) -> Self {
        Self {
            user: s.user.into(),
            shares: s.shares,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UnbondRequestsByUserResponseItem {
    /// ID of the batch
    pub id: u64,
    /// The user's share in the batch
    pub shares: Uint128,
}

impl From<UnbondRequest> for UnbondRequestsByUserResponseItem {
    fn from(s: UnbondRequest) -> Self {
        Self {
            id: s.id,
            shares: s.shares,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, JsonSchema)]
pub struct UnbondRequestsByUserResponseItemDetails {
    /// ID of the batch
    pub id: u64,
    /// The user's share in the batch
    pub shares: Uint128,

    // state of pending, unbonding or completed
    pub state: String,

    // The details of the unbonding batch
    pub batch: Option<Batch>,

    // Is set if the unbonding request is still pending
    pub pending: Option<PendingBatch>,
}

pub type MigrateMsg = Empty;
