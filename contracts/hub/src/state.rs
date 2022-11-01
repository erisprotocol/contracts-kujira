use cosmwasm_std::{Addr, Coin, StdError, StdResult, Storage};
use cw_storage_plus::{Index, IndexList, IndexedMap, Item, MultiIndex};

use eris::{
    adapters::fin_multi::FinMulti,
    hub::{Batch, FeeConfig, PendingBatch, StakeToken, UnbondRequest},
};
use kujira::denom::Denom;

use crate::types::BooleanKey;

pub(crate) struct State<'a> {
    /// Fin Multi Contract
    pub fin_multi: Item<'a, FinMulti>,
    /// Account who can call certain privileged functions
    pub owner: Item<'a, Addr>,
    /// Account who can call harvest
    pub operator: Item<'a, Addr>,
    /// Stages that must be used by permissionless users
    pub stages_preset: Item<'a, Vec<Vec<(Addr, Denom)>>>,
    /// Pending ownership transfer, awaiting acceptance by the new owner
    pub new_owner: Item<'a, Addr>,
    /// Denom and supply of the Liquid Staking token
    pub stake_token: Item<'a, StakeToken>,
    /// How often the unbonding queue is to be executed
    pub epoch_period: Item<'a, u64>,
    /// The staking module's unbonding time, in seconds
    pub unbond_period: Item<'a, u64>,
    /// Validators who will receive the delegations
    pub validators: Item<'a, Vec<String>>,
    /// Coins that can be reinvested
    pub unlocked_coins: Item<'a, Vec<Coin>>,
    /// The current batch of unbonding requests queded to be executed
    pub pending_batch: Item<'a, PendingBatch>,
    /// Previous batches that have started unbonding but not yet finished
    pub previous_batches: IndexedMap<'a, u64, Batch, PreviousBatchesIndexes<'a>>,
    /// Users' shares in unbonding batches
    pub unbond_requests: IndexedMap<'a, (u64, &'a Addr), UnbondRequest, UnbondRequestsIndexes<'a>>,
    /// Fee Config
    pub fee_config: Item<'a, FeeConfig>,
}

impl Default for State<'static> {
    fn default() -> Self {
        let pb_indexes = PreviousBatchesIndexes {
            reconciled: MultiIndex::new(
                |d: &Batch| d.reconciled.into(),
                "previous_batches",
                "previous_batches__reconciled",
            ),
        };
        let ubr_indexes = UnbondRequestsIndexes {
            user: MultiIndex::new(
                |d: &UnbondRequest| d.user.clone().into(),
                "unbond_requests",
                "unbond_requests__user",
            ),
        };
        Self {
            fin_multi: Item::new("fin_multi"),
            owner: Item::new("owner"),
            new_owner: Item::new("new_owner"),
            operator: Item::new("operator"),
            stages_preset: Item::new("stages_preset"),
            stake_token: Item::new("stake_token"),
            epoch_period: Item::new("epoch_period"),
            unbond_period: Item::new("unbond_period"),
            validators: Item::new("validators"),
            unlocked_coins: Item::new("unlocked_coins"),
            pending_batch: Item::new("pending_batch"),
            previous_batches: IndexedMap::new("previous_batches", pb_indexes),
            unbond_requests: IndexedMap::new("unbond_requests", ubr_indexes),
            fee_config: Item::new("fee_config"),
        }
    }
}

impl<'a> State<'a> {
    pub fn assert_owner(&self, storage: &dyn Storage, sender: &Addr) -> StdResult<()> {
        let owner = self.owner.load(storage)?;
        if *sender == owner {
            Ok(())
        } else {
            Err(StdError::generic_err("unauthorized: sender is not owner"))
        }
    }

    pub fn assert_operator(&self, storage: &dyn Storage, sender: &Addr) -> StdResult<()> {
        let operator = self.operator.load(storage)?;
        if *sender == operator {
            Ok(())
        } else {
            Err(StdError::generic_err("unauthorized: sender is not operator"))
        }
    }
}

pub(crate) struct PreviousBatchesIndexes<'a> {
    // pk goes to second tuple element
    pub reconciled: MultiIndex<'a, BooleanKey, Batch, Vec<u8>>,
}

impl<'a> IndexList<Batch> for PreviousBatchesIndexes<'a> {
    fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<Batch>> + '_> {
        let v: Vec<&dyn Index<Batch>> = vec![&self.reconciled];
        Box::new(v.into_iter())
    }
}

pub(crate) struct UnbondRequestsIndexes<'a> {
    // pk goes to second tuple element
    pub user: MultiIndex<'a, String, UnbondRequest, (u64, &'a Addr)>,
}

impl<'a> IndexList<UnbondRequest> for UnbondRequestsIndexes<'a> {
    fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<UnbondRequest>> + '_> {
        let v: Vec<&dyn Index<UnbondRequest>> = vec![&self.user];

        Box::new(v.into_iter())
    }
}
