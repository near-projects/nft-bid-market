use crate::sale::DELIMETER;
use crate::*;

#[near_bindgen]
impl Market {
    pub fn hack_finish_sale(&mut self, nft_contract_id: AccountId, token_id: TokenId) {
        let nft_contract_token = format!("{}{}{}", nft_contract_id, DELIMETER, token_id);
        let mut sale = self.market.sales.get(&nft_contract_token).expect("no sale");
        sale.end = Some(env::block_timestamp());
        self.market.sales.insert(&nft_contract_token, &sale);
    }

    // Don't use it in quick-ref
    // pub fn hack_finish_bid(&mut self, nft_contract_token: ContractAndTokenId) {
    //     //TODO: do we need it? or should remove
    //     let mut bids = self
    //         .market
    //         .bids
    //         .get(&nft_contract_token)
    //         .expect(format!("no bids for {}", nft_contract_token.clone()).as_str());
    //     let vec_bid = bids
    //         .get_mut(&("near".parse().unwrap()))
    //         .expect("no bids");
    //     let mut bid = vec_bid
    //         .get(vec_bid.len() - 1);
    //     if let Some(mut bid) = bid {
    //         bid.end = Some(U64(env::block_timestamp()))
    //     }
    //     self.market.bids.insert(&nft_contract_token, &bids);
    // }

    pub fn hack_finish_auction(&mut self, auction_id: U128) {
        let mut auction = self
            .market
            .auctions
            .get(&auction_id.into())
            .expect("no auction");
        auction.end = env::block_timestamp();
        self.market.auctions.insert(&auction_id.into(), &auction);
    }
}
