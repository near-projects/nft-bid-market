use std::collections::HashMap;

use near_sdk::assert_one_yocto;

use crate::sale::{ext_contract, ContractAndTokenId, FungibleTokenId, Sale, GAS_FOR_FT_TRANSFER};
use crate::*;

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Bid {
    pub owner_id: AccountId,
    pub price: U128,

    pub start: Option<U64>,
    pub end: Option<U64>,
}

impl Bid {
    pub fn in_limits(&self) -> bool {
        let mut res = true;
        let now = env::block_timestamp();
        if let Some(start) = self.start {
            res &= start.0 < now;
        }
        if let Some(end) = self.end {
            res &= now < end.0;
        }
        res
    }
}

pub type Bids = HashMap<FungibleTokenId, Vec<Bid>>;

#[near_bindgen]
impl Market {
    // Adds a bid if it is higher than the last bid of this ft_token_id
    // Refunds the previous bid (of this ft_token_id)
    #[allow(clippy::too_many_arguments)]
    #[private]
    pub fn add_bid(
        &mut self,
        contract_and_token_id: ContractAndTokenId,
        amount: Balance,
        ft_token_id: AccountId,
        buyer_id: AccountId,
        sale: &mut Sale,
        start: Option<U64>,
        end: Option<U64>,
    ) {
        // store a bid and refund any current bid lower
        let new_bid = Bid {
            owner_id: buyer_id,
            price: U128(amount),
            start,
            end,
        };

        let bids_for_token_id = sale
            .bids
            .entry(ft_token_id.clone())
            .or_insert_with(Vec::new);

        if !bids_for_token_id.is_empty() {
            let current_bid = &bids_for_token_id.last().unwrap();
            assert!(
                amount > current_bid.price.0,
                "Can't pay less than or equal to current bid price: {}",
                current_bid.price.0
            );
            if ft_token_id == "near".to_string().parse().unwrap() {
                Promise::new(current_bid.owner_id.clone()).transfer(u128::from(current_bid.price));
            } else {
                ext_contract::ft_transfer(
                    current_bid.owner_id.clone(),
                    current_bid.price,
                    None,
                    ft_token_id,
                    1,
                    GAS_FOR_FT_TRANSFER,
                );
            }
        }

        bids_for_token_id.push(new_bid);
        if bids_for_token_id.len() > self.market.bid_history_length as usize {
            bids_for_token_id.remove(0);
        }

        self.market.sales.insert(&contract_and_token_id, sale);
    }

    #[payable]
    pub fn remove_bid(&mut self, nft_contract_id: AccountId, token_id: TokenId, bid: Bid) {
        assert_one_yocto();
        assert_eq!(
            env::predecessor_account_id(),
            bid.owner_id,
            "Must be bid owner"
        );
        let ft_token_id = AccountId::new_unchecked("near".to_owned()); // Should be argument, if support of ft needed
        self.internal_remove_bid(nft_contract_id, &ft_token_id, token_id, &bid);
        self.refund_bid(ft_token_id, &bid);
    }
}

impl Market {
    pub(crate) fn refund_all_bids(&mut self, bids_map: &Bids) {
        for (ft, bids) in bids_map {
            for bid in bids {
                self.refund_bid((*ft).clone(), bid);
            }
        }
    }

    pub(crate) fn refund_bid(&mut self, bid_ft: FungibleTokenId, bid: &Bid) {
        if bid_ft.as_str() == "near" {
            Promise::new(bid.owner_id.clone()).transfer(u128::from(bid.price));
        } else {
            ext_contract::ft_transfer(
                bid.owner_id.clone(),
                bid.price,
                None,
                bid_ft,
                1,
                GAS_FOR_FT_TRANSFER,
            );
        }
    }
}
