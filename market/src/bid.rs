use std::collections::HashMap;

use near_sdk::assert_one_yocto;

use crate::fee::calculate_origins;
use crate::sale::{ext_contract, FungibleTokenId, DELIMETER, GAS_FOR_FT_TRANSFER};
use crate::*;

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize, Clone)]
#[cfg_attr(not(target_arch = "wasm32"), derive(Debug, PartialEq))]
#[serde(crate = "near_sdk::serde")]
pub struct Bid {
    pub bid_id: BidId,

    pub contract_and_token_id: ContractAndTokenId,
    pub owner_id: AccountId,
    pub fungible_token: FungibleTokenId,
    pub price: U128,

    pub start: U64,
    pub end: Option<U64>,

    pub origins: Origins,
}

impl Bid {
    pub fn in_limits(&self) -> bool {
        let mut res_start = true;
        let mut res_end = true;
        let now = env::block_timestamp();
        res_start &= self.start.0 < now;
        if let Some(end) = self.end {
            res_end &= now < end.0;
        }
        res_end && res_start
    }
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct BidAccount {
    pub total_balance: LookupMap<FungibleTokenId, Balance>,
    //pub availible_balance: LookupMap<FungibleTokenId, Balance>,
}

//pub type Bids = HashMap<FungibleTokenId, Vector<Bid>>;
pub type Origins = HashMap<AccountId, u32>;
pub type BidId = u128;
pub type BidsForContractAndTokenId =
    HashMap<FungibleTokenId, TreeMap<Balance, UnorderedSet<BidId>>>;

#[near_bindgen]
impl Market {
    // Adds a bid if it is higher than the last bid of this ft_token_id
    // Refunds the previous bid (of this ft_token_id)
    #[allow(clippy::too_many_arguments)]
    #[private]
    pub(crate) fn add_bid(
        &mut self,
        //contract_and_token_id: ContractAndTokenId,
        nft_contract_id: AccountId,
        token_id: TokenId,
        amount: Balance,
        ft_token_id: AccountId,
        buyer_id: AccountId,
        start: U64,
        end: Option<U64>,
        origins: Option<Origins>,
    ) -> BidId {
        let contract_and_token_id = format!("{}{}{}", &nft_contract_id, DELIMETER, token_id);
        require!(
            self.market.ft_token_ids.contains(&ft_token_id),
            format!("Token {} not supported by this market", ft_token_id)
        );
        let total_origins = if let Some(ref origins) = origins {
            calculate_origins(origins)
        } else {
            0
        };

        require!(total_origins < 4_700, "Max origins exceeded"); // TODO: FINDOUT MAX ORIGINS
                                                                 //let actual_amount = calculate_actual_amount(amount, total_origins);

        // create a bid
        let new_bid = Bid {
            bid_id: self.market.next_bid_id,
            contract_and_token_id: contract_and_token_id.clone(),
            owner_id: buyer_id.clone(),
            fungible_token: ft_token_id.clone(),
            price: U128(amount),
            start,
            end,
            origins: origins.unwrap_or_default(),
        };

        // if the buyer_id has a bid on already has a bid on this contract_and_token_id,
        // it should be removed
        let bids_for_buyer = self.market.bids_by_owner.get(&buyer_id);
        if let Some(bids_for_buyer) = bids_for_buyer {
            let bids_for_buyer_contract_and_token = bids_for_buyer.get(&contract_and_token_id);
            if let Some(bid_for_buyer_contract_and_token) = bids_for_buyer_contract_and_token {
                self.internal_remove_bid(
                    nft_contract_id,
                    &bid_for_buyer_contract_and_token.0,
                    token_id,
                    &buyer_id,
                    bid_for_buyer_contract_and_token.1.into(),
                    bid_for_buyer_contract_and_token.2,
                );
            }
        }

        // increase id of a next bid
        self.market.next_bid_id += 1;

        // add the bid to bids_by_index
        self.market.bids_by_index.insert(&new_bid.bid_id, &new_bid);

        // add the bid to bids
        let mut bids_for_contract_and_token_id = self
            .market
            .bids
            .get(&contract_and_token_id)
            .unwrap_or_default();
        let mut bids_tree = bids_for_contract_and_token_id
            .remove(&ft_token_id)
            .unwrap_or_else(|| {
                TreeMap::new(StorageKey::BidsForContractAndOwner {
                    contract_and_token_hash: hash_string(&contract_and_token_id),
                })
            });
        let mut equal_bids = bids_tree.get(&amount).unwrap_or_else(|| {
            UnorderedSet::new(StorageKey::BidsForContractAndOwnerInner {
                contract_and_token_hash: hash_string(&contract_and_token_id),
                balance: amount.to_le_bytes(),
            })
        });
        equal_bids.insert(&new_bid.bid_id);
        bids_tree.insert(&new_bid.price.into(), &equal_bids);
        bids_for_contract_and_token_id.insert(ft_token_id.clone(), bids_tree);
        self.market
            .bids
            .insert(&contract_and_token_id, &bids_for_contract_and_token_id);

        // add the bid to bids_by_owner
        let mut bids_by_owner = self.market.bids_by_owner.get(&buyer_id).unwrap_or_else(|| {
            UnorderedMap::new(StorageKey::BidsByOwnerInner {
                account_id_hash: hash_account_id(&buyer_id),
            })
        });
        let bid_data = (ft_token_id, amount, new_bid.bid_id);
        bids_by_owner.insert(&contract_and_token_id, &bid_data);
        self.market.bids_by_owner.insert(&buyer_id, &bids_by_owner);

        /*let mut bids = self.market.bids.get(&contract_and_token_id).unwrap();
        let bids_for_token_id = bids.entry(ft_token_id.clone()).or_insert_with(||Vector::new(b"v"));
        if let Some(current_bid) = bids_for_token_id.get(bids_for_token_id.len() - 1) {
            let current_origins = calculate_origins(&current_bid.origins);
            let current_amount = calculate_actual_amount(current_bid.price.0, current_origins);
            require!(
                actual_amount > current_amount,
                format!(
                    "Can't pay less than or equal to current bid price: {}",
                    current_bid.price.0
                )
            );
        }

        bids_for_token_id.push(&new_bid);
        if bids_for_token_id.len() > self.market.bid_history_length as usize {
            // Need to refund the earliest bid before removing it
            let early_bid = &bids_for_token_id[0];
            self.refund_bid(ft_token_id, early_bid.owner_id.clone(), early_bid.price);
            bids_for_token_id.remove(0);
        }*/

        new_bid.bid_id
    }

    #[payable]
    pub fn remove_bid(
        &mut self,
        nft_contract_id: AccountId,
        token_id: TokenId,
        ft_token_id: AccountId,
        price: U128,
        bid_id: U128,
    ) {
        assert_one_yocto();
        let owner_id = env::predecessor_account_id();
        self.internal_remove_bid(
            nft_contract_id,
            &ft_token_id,
            token_id,
            &owner_id,
            price,
            bid_id.0,
        );
        //self.refund_bid(ft_token_id, owner_id, price); //TODO: remove it and use bidding account
    }

    // Cancels the bid if it has ended
    // Refunds it
    pub fn cancel_bid(
        &mut self,
        nft_contract_id: AccountId,
        token_id: TokenId,
        ft_token_id: AccountId,
        owner_id: AccountId,
        price: U128,
        bid_id: U128,
    ) {
        let bid = self
            .internal_remove_bid(
                nft_contract_id,
                &ft_token_id,
                token_id,
                &owner_id,
                price,
                bid_id.0,
            )
            .expect("No such bid");
        if let Some(end) = bid.end {
            let is_finished = env::block_timestamp() >= end.0;
            require!(is_finished, "The bid hasn't ended yet");
            //self.refund_bid(ft_token_id, owner_id, price);
        } else {
            panic!("The bid doesn't have an end");
        }
    }

    // Cancel all expired bids
    pub fn cancel_expired_bids(
        //TODO: check that the iterator works correctly after deleting
        &mut self,
        nft_contract_id: AccountId,
        token_id: TokenId,
        ft_token_id: AccountId,
    ) {
        let contract_and_token_id = format!("{}{}{}", &nft_contract_id, DELIMETER, token_id);
        let mut bids_for_contract_and_token_id = self
            .market
            .bids
            .get(&contract_and_token_id)
            .expect("No contract or token id");
        let mut bids_tree: TreeMap<u128, UnorderedSet<BidId>> = bids_for_contract_and_token_id
            .remove(&ft_token_id)
            .expect("No ft_token_id");
        //let bids_tree = bids_tree.borrow_mut();
        for (balance, mut equal_bids) in bids_tree
            .iter()
            .collect::<HashMap<u128, UnorderedSet<BidId>>>()
        {
            for bid_id in equal_bids.iter().collect::<Vec<BidId>>() {
                // Find a bid by its id
                let bid = self.market.bids_by_index.get(&bid_id).expect("No bid_id");
                let mut not_finished = true;
                if let Some(end) = bid.end {
                    //is_finished &= env::block_timestamp() >= end.0;
                    if env::block_timestamp() >= end.0 {
                        not_finished = false;
                    };
                }
                if !not_finished {
                    self.market
                        .bids_by_index
                        .remove(&bid_id)
                        .expect("No bid_id");
                    equal_bids.remove(&bid_id);
                    let mut bids_by_owner = self
                        .market
                        .bids_by_owner
                        .get(&bid.owner_id)
                        .expect("No bids for account");
                    bids_by_owner.remove(&contract_and_token_id);
                    self.market
                        .bids_by_owner
                        .insert(&bid.owner_id, &bids_by_owner);
                }
            }
            bids_tree.insert(&balance, &equal_bids);
        }
        bids_for_contract_and_token_id.insert(ft_token_id, bids_tree);
        self.market
            .bids
            .insert(&contract_and_token_id, &bids_for_contract_and_token_id);
        // let mut bids = self.market.bids.get(&contract_and_token_id).unwrap();
        // let bid_vec = bids.get(&ft_token_id).expect("No token").clone();
        // bid_vec.to_vec().retain(|bid_from_vec| {
        //     let mut not_finished = true;
        //     if let Some(end) = bid_from_vec.end {
        //         //is_finished &= env::block_timestamp() >= end.0;
        //         if env::block_timestamp() >= end.0 {
        //             self.refund_bid(
        //                 ft_token_id.clone(),
        //                 bid_from_vec.owner_id.clone(),
        //                 bid_from_vec.price,
        //             );
        //             not_finished = false;
        //         };
        //     }
        //     not_finished
        // });
        // if bid_vec.is_empty() {
        //     // If there is no bids left, should remove ft_token_id from the HashMap
        //     bids.remove(&ft_token_id);
        // } else {
        //     // If there are some bids left, add a vector of valid bids
        //     bids.insert(ft_token_id, *bid_vec);
        // };
        // self.market.bids.insert(&contract_and_token_id, &bids);
    }
}

impl Market {
    /*pub(crate) fn refund_all_bids(&mut self, bids_map: &Bids) {
        for (ft, bids) in bids_map {
            for bid in bids.iter() {
                self.refund_bid((*ft).clone(), bid.owner_id.clone(), bid.price);
            }
        }
    }*/

    pub(crate) fn refund_bid(&mut self, bid_ft: FungibleTokenId, owner_id: AccountId, price: U128) {
        if bid_ft.as_str() == "near" {
            Promise::new(owner_id).transfer(u128::from(price));
        } else {
            ext_contract::ft_transfer(owner_id, price, None, bid_ft, 1, GAS_FOR_FT_TRANSFER);
        }
    }

    pub(crate) fn is_active(&self, bid_id: BidId, ft: AccountId) -> bool {
        let bid = self
            .market
            .bids_by_index
            .get(&bid_id)
            .expect("No bid with this id");
        let owner_id = bid.owner_id;

        let bid_account = self.market.bid_accounts.get(&owner_id);

        if let Some(bid_account) = bid_account {
            let bid_balance = bid_account.total_balance.get(&ft);
            if let Some(bid_balance) = bid_balance {
                bid.price.0 <= bid_balance
            } else {
                false
            }
        } else {
            false
        }
    }
}
