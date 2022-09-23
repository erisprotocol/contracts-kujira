use cosmwasm_std::{Addr, Order, StdError, StdResult, Storage};
use cw_storage_plus::{Index, IndexList, IndexedMap, Item, MultiIndex};

use eris::router::{create_key_for_assets, PairInfo};

pub(crate) struct State<'a> {
    /// Account who can call certain privileged functions
    pub owner: Item<'a, Addr>,
    /// Pending ownership transfer, awaiting acceptance by the new owner
    pub new_owner: Item<'a, Addr>,
    /// Market pairs available
    pub pairs: IndexedMap<'a, Addr, PairInfo, PairIndexes<'a>>,
}

impl Default for State<'static> {
    fn default() -> Self {
        let pair_indexes = PairIndexes {
            denom: MultiIndex::new(|d: &PairInfo| d.create_key(), "pairs", "pairs__by_denom"),
        };

        Self {
            owner: Item::new("owner"),
            new_owner: Item::new("new_owner"),
            pairs: IndexedMap::new("pairs", pair_indexes),
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

    pub(crate) fn get_pair(
        &self,
        storage: &dyn Storage,
        offer_asset_info: String,
        ask_asset_info: String,
    ) -> StdResult<PairInfo> {
        let pair_key = create_key_for_assets(&[offer_asset_info, ask_asset_info]);
        let results: StdResult<Vec<PairInfo>> = self
            .pairs
            .idx
            .denom
            .prefix(pair_key)
            .range(storage, None, None, Order::Ascending)
            .take(1)
            .map(|item| {
                let (_, v) = item?;

                Ok(v)
            })
            .collect();

        let results = results?;

        if results.is_empty() {
            return Err(StdError::generic_err("pair not found"));
        }

        Ok(results[0].clone())
    }

    /// tries to find a pair using a single hop through allowed intermediate denoms
    pub(crate) fn try_find_pair(
        &self,
        storage: &dyn Storage,
        offer_asset_info: String,
        ask_asset_info: String,
        allowed_intermediate_denom: Vec<String>,
    ) -> StdResult<Vec<PairInfo>> {
        for allowed in allowed_intermediate_denom {
            let pairs = self.try_find_pair_single_hop(
                storage,
                offer_asset_info.clone(),
                ask_asset_info.clone(),
                allowed,
            );

            if let Ok(pairs) = pairs {
                return Ok(pairs);
            }
        }

        Err(StdError::generic_err("pair not found"))
    }

    /// tries to find a pair using a single hop
    pub(crate) fn try_find_pair_single_hop(
        &self,
        storage: &dyn Storage,
        offer_asset_info: String,
        ask_asset_info: String,
        intermediate_denom: String,
    ) -> StdResult<Vec<PairInfo>> {
        let pair = self.get_pair(storage, offer_asset_info, intermediate_denom.clone())?;
        let pair2 = self.get_pair(storage, intermediate_denom, ask_asset_info)?;

        Ok(vec![pair, pair2])
    }
}

pub(crate) struct PairIndexes<'a> {
    // pk goes to second tuple element
    pub denom: MultiIndex<'a, String, PairInfo, Vec<u8>>,
}

impl<'a> IndexList<PairInfo> for PairIndexes<'a> {
    fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<PairInfo>> + '_> {
        let v: Vec<&dyn Index<PairInfo>> = vec![&self.denom];
        Box::new(v.into_iter())
    }
}
