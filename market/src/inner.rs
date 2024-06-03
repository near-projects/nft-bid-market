use crate::bid::{Bid, BidId};
use crate::common::*;
use crate::sale::{Sale, DELIMETER};
use crate::Market;

impl Market {
    pub(crate) fn internal_remove_sale(
        &mut self,
        nft_contract_id: AccountId,
        token_id: TokenId,
    ) -> Sale {
        let contract_and_token_id = format!("{}{}{}", &nft_contract_id, DELIMETER, token_id);
        let sale = self
            .market
            .sales
            .remove(&contract_and_token_id)
            .expect("No sale");

        let mut by_owner_id = self
            .market
            .by_owner_id
            .get(&sale.owner_id)
            .expect("No sale by_owner_id");
        by_owner_id.remove(&contract_and_token_id);
        if by_owner_id.is_empty() {
            self.market.by_owner_id.remove(&sale.owner_id);
        } else {
            self.market.by_owner_id.insert(&sale.owner_id, &by_owner_id);
        }

        let mut by_nft_contract_id = self
            .market
            .by_nft_contract_id
            .get(&nft_contract_id)
            .expect("No sale by nft_contract_id");
        by_nft_contract_id.remove(&token_id);
        if by_nft_contract_id.is_empty() {
            self.market.by_nft_contract_id.remove(&nft_contract_id);
        } else {
            self.market
                .by_nft_contract_id
                .insert(&nft_contract_id, &by_nft_contract_id);
        }

        // here AccountId is used as "token type", idk why so (adsick)
        if let Some(token_type) = sale.token_type.to_owned() {
            let mut by_nft_token_type = self
                .market
                .by_nft_token_type
                .get(&token_type)
                .expect("No sale by nft_token_type");
            by_nft_token_type.remove(&contract_and_token_id);
            if by_nft_token_type.is_empty() {
                self.market.by_nft_token_type.remove(&token_type);
            } else {
                self.market
                    .by_nft_token_type
                    .insert(&token_type, &by_nft_token_type);
            }
        }

        sale
    }

    pub(crate) fn internal_remove_bid(
        &mut self,
        nft_contract_id: AccountId,
        ft_token_id: &AccountId,
        token_id: TokenId,
        owner_id: &AccountId,
        price: U128,
        bid_id: BidId,
    ) -> Option<Bid> {
        let contract_and_token_id = format!("{}{}{}", &nft_contract_id, DELIMETER, token_id);
        let mut bids_by_ft = self
            .market
            .bids
            .get(&contract_and_token_id)
            .expect("No bid for this nft contract and ft token");
        let mut bids_tree = bids_by_ft.remove(ft_token_id).expect("No token");
        let mut equal_bids = bids_tree.get(&price.0).expect("No bid with this balance");
        assert!(equal_bids.remove(&bid_id), "No bid with this price and id");
        bids_tree.insert(&price.0, &equal_bids);
        bids_by_ft.insert(ft_token_id.clone(), bids_tree);
        self.market.bids.insert(&contract_and_token_id, &bids_by_ft);

        let mut bids_by_owner = self
            .market
            .bids_by_owner
            .get(owner_id)
            .expect("No bids for the owner");
        bids_by_owner
            .remove(&contract_and_token_id)
            .expect("No bid for owner, nft contract and token");
        self.market.bids_by_owner.insert(owner_id, &bids_by_owner);
        /*for (index, bid_from_vec) in bid_vec.iter().enumerate() {
            if &(bid_from_vec.owner_id) == owner_id && bid_from_vec.price == price {
                if bid_vec.len() == 1 {
                    //If the vector contained only one bid, should remove ft_token_id from the HashMap
                    bids.remove(ft_token_id);
                } else {
                    //If there are several bids for this ft_token_id, should remove one bid
                    bids.get_mut(ft_token_id).expect("No token").remove(index);
                };
                self.market.bids.insert(&contract_and_token_id, &bids);
                //break; // shouldn't allow bids with equal price
                return Some((bid_from_vec).clone());
            };
        }*/
        self.market.bids_by_index.remove(&bid_id)
    }
}
