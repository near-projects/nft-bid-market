#![allow(clippy::too_many_arguments)]
use std::collections::HashMap;

use near_sdk::ext_contract;
use near_sdk::{promise_result_as_success, Gas};

use crate::auction::Auction;
use crate::*;
use common::*;

use bid::Bids;
pub type TokenSeriesId = String;

pub const GAS_FOR_FT_TRANSFER: Gas = Gas(5_000_000_000_000);
pub const GAS_FOR_ROYALTIES: Gas = Gas(115_000_000_000_000);
pub const GAS_FOR_NFT_TRANSFER: Gas = Gas(15_000_000_000_000);
pub const GAS_FOR_MINT: Gas = Gas(20_000_000_000_000);
pub const BID_HISTORY_LENGTH_DEFAULT: u8 = 10;
const NO_DEPOSIT: Balance = 0;
pub static DELIMETER: &str = "||";

pub type SaleConditions = HashMap<FungibleTokenId, U128>;

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Payout {
    pub payout: HashMap<AccountId, U128>,
}

pub type ContractAndTokenId = String;
pub type FungibleTokenId = AccountId;
pub type TokenType = Option<String>;
pub type ContractAndSeriesId = String;

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Sale {
    pub owner_id: AccountId,
    pub approval_id: u64,
    pub nft_contract_id: AccountId,
    pub token_id: String,
    pub sale_conditions: SaleConditions,
    pub bids: Bids,
    pub created_at: u64,
    pub token_type: TokenType,

    pub start: Option<u64>,
    pub end: Option<u64>,
}

#[derive(BorshDeserialize, BorshSerialize)]
pub struct SeriesSale {
    pub owner_id: AccountId,
    pub nft_contract_id: AccountId,
    pub series_id: String,
    pub sale_conditions: SaleConditions,
    pub created_at: u64,
    pub copies: u64,
}

impl Sale {
    pub fn in_limits(&self) -> bool {
        let mut res = true;
        let now = env::block_timestamp();
        if let Some(start) = self.start {
            res &= start < now;
        }
        if let Some(end) = self.end {
            res &= now < end;
        }
        res
    }

    pub fn extend(&mut self, time: u64) -> bool {
        if let Some(end) = self.end {
            self.end = Some(end + time);
            true
        } else {
            false
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct PurchaseArgs {
    pub nft_contract_id: AccountId,
    pub token_id: TokenId,
}

#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct MarketSales {
    pub owner_id: AccountId,
    pub sales: UnorderedMap<ContractAndTokenId, Sale>,
    pub series_sales: UnorderedMap<ContractAndSeriesId, SeriesSale>,
    pub by_owner_id: LookupMap<AccountId, UnorderedSet<ContractAndTokenId>>,
    pub by_nft_contract_id: LookupMap<AccountId, UnorderedSet<TokenId>>,
    pub by_nft_token_type: LookupMap<AccountId, UnorderedSet<ContractAndTokenId>>,
    pub ft_token_ids: UnorderedSet<FungibleTokenId>,
    pub storage_deposits: LookupMap<AccountId, Balance>,
    pub bid_history_length: u8,

    pub auctions: UnorderedMap<u128, Auction>,
    pub next_auction_id: u128,
}

#[near_bindgen]
impl Market {
    /// TODO remove without redirect to wallet? panic reverts
    #[payable]
    pub fn remove_sale(&mut self, nft_contract_id: AccountId, token_id: String) {
        assert_one_yocto();
        let sale = self.internal_remove_sale(nft_contract_id, token_id);
        let owner_id = env::predecessor_account_id();
        assert_eq!(owner_id, sale.owner_id, "Must be sale owner");
        self.refund_all_bids(&sale.bids);
    }

    #[payable]
    pub fn update_price(
        &mut self,
        nft_contract_id: AccountId,
        token_id: String,
        ft_token_id: FungibleTokenId,
        price: U128,
    ) {
        assert_one_yocto();
        let contract_id: AccountId = nft_contract_id;
        let contract_and_token_id = format!("{}{}{}", contract_id, DELIMETER, token_id);
        let mut sale = self
            .market
            .sales
            .get(&contract_and_token_id)
            .expect("No sale");
        assert_eq!(
            env::predecessor_account_id(),
            sale.owner_id,
            "Must be sale owner"
        );
        if !self.market.ft_token_ids.contains(&ft_token_id) {
            env::panic_str(&format!(
                "Token '{}' is not supported by this market",
                ft_token_id
            ));
        }
        sale.sale_conditions.insert(ft_token_id, price);
        self.market.sales.insert(&contract_and_token_id, &sale);
    }

    #[payable]
    pub fn offer(
        &mut self,
        nft_contract_id: AccountId,
        token_id: String,
        start: Option<U64>,
        end: Option<U64>,
    ) {
        let contract_id: AccountId = nft_contract_id;
        let contract_and_token_id = format!("{}{}{}", contract_id, DELIMETER, token_id);
        let mut sale = self
            .market
            .sales
            .get(&contract_and_token_id)
            .expect("No sale");
        // Check that the sale is in progress
        require!(
            sale.in_limits(),
            "Either the sale is finished or it hasn't started yet"
        );

        let buyer_id = env::predecessor_account_id();
        assert_ne!(sale.owner_id, buyer_id, "Cannot bid on your own sale.");
        let ft_token_id = "near".to_string(); // Should be argument, if support of ft needed
        let price = *sale
            .sale_conditions
            .get(&ft_token_id.parse().unwrap())
            .expect("Not for sale in NEAR");

        let deposit = env::attached_deposit();
        assert!(deposit > 0, "Attached deposit must be greater than 0");

        if deposit == price.0 {
            self.process_purchase(
                contract_id,
                token_id,
                ft_token_id.parse().unwrap(),
                U128(deposit),
                buyer_id,
            );
        } else {
            self.add_bid(
                contract_and_token_id,
                deposit,
                ft_token_id.parse().unwrap(),
                buyer_id,
                &mut sale,
                start,
                end,
            );
        }
    }

    pub fn accept_offer(
        &mut self,
        nft_contract_id: AccountId,
        token_id: String,
        ft_token_id: AccountId,
    ) {
        let contract_id: AccountId = nft_contract_id;
        let contract_and_token_id = format!("{}{}{}", contract_id, DELIMETER, token_id);
        // Check that the sale is in progress and remove bid before proceeding to process purchase
        let mut sale = self
            .market
            .sales
            .get(&contract_and_token_id)
            .expect("No sale");
        require!(
            sale.in_limits(),
            "Either the sale is finished or it hasn't started yet"
        );
        let bids_for_token_id = sale.bids.remove(&ft_token_id).expect("No bids");
        let bid = &bids_for_token_id[bids_for_token_id.len() - 1];
        require!(bid.in_limits(), "Out of time limit of the bid");
        self.market.sales.insert(&contract_and_token_id, &sale);
        // panics at `self.internal_remove_sale` and reverts above if predecessor is not sale.owner_id
        self.process_purchase(
            contract_id,
            token_id,
            ft_token_id,
            bid.price,
            bid.owner_id.clone(),
        );
    }

    #[private]
    pub fn process_purchase(
        &mut self,
        nft_contract_id: AccountId,
        token_id: String,
        ft_token_id: AccountId,
        price: U128,
        buyer_id: AccountId,
    ) -> Promise {
        let sale = self.internal_remove_sale(nft_contract_id.clone(), token_id.clone());

        ext_contract::nft_transfer_payout(
            buyer_id.clone(),
            token_id,
            sale.approval_id,
            None, // need to check here if series
            price,
            10,
            nft_contract_id,
            1,
            GAS_FOR_NFT_TRANSFER,
        )
        .then(ext_self::resolve_purchase(
            ft_token_id,
            buyer_id,
            sale,
            price,
            env::current_account_id(),
            NO_DEPOSIT,
            GAS_FOR_ROYALTIES,
        ))
    }

    /// self callback

    #[private]
    pub fn resolve_purchase(
        &mut self,
        ft_token_id: AccountId,
        buyer_id: AccountId,
        sale: Sale,
        price: U128,
    ) -> U128 {
        // checking for payout information
        let payout_option = promise_result_as_success().and_then(|value| {
            // None means a bad payout from bad NFT contract
            near_sdk::serde_json::from_slice::<Payout>(&value)
                .ok()
                .and_then(|payout| {
                    // gas to do 10 FT transfers (and definitely 10 NEAR transfers)
                    if payout.payout.len() + sale.bids.len() > 10 || payout.payout.is_empty() {
                        env::log_str("Cannot have more than 10 royalties and sale.bids refunds");
                        None
                    } else {
                        let mut remainder = price.0;
                        for &value in payout.payout.values() {
                            remainder = remainder.checked_sub(value.0)?;
                        }
                        if remainder <= 1 {
                            Some(payout)
                        } else {
                            None
                        }
                    }
                })
        });
        // is payout option valid?
        let mut payout = if let Some(payout_option) = payout_option {
            payout_option
        } else {
            if ft_token_id == "near".parse().unwrap() {
                Promise::new(buyer_id).transfer(u128::from(price));
            }
            // leave function and return all FTs in ft_resolve_transfer
            return price;
        };
        // Going to payout everyone, first return all outstanding bids (accepted offer bid was already removed)
        self.refund_all_bids(&sale.bids);

        // Protocol fees
        let protocol_fee = price.0 * PROTOCOL_FEE / 10_000u128;

        let mut owner_payout:u128 = payout
            .payout
            .remove(&sale.owner_id)
            .unwrap_or_else(|| unreachable!()).into();
        owner_payout -= protocol_fee;
        // NEAR payouts
        if ft_token_id == "near".parse().unwrap() {
            // Royalties
            for (receiver_id, amount) in payout.payout {
                Promise::new(receiver_id).transfer(amount.0);
                owner_payout -= amount.0;
            }
            // Payouts
            Promise::new(sale.owner_id).transfer(owner_payout);
            // refund all FTs (won't be any)
            price
        } else {
            // FT payouts
            for (receiver_id, amount) in payout.payout {
                ext_contract::ft_transfer(
                    receiver_id,
                    amount,
                    None,
                    ft_token_id.clone(),
                    1,
                    GAS_FOR_FT_TRANSFER,
                );
            }
            // keep all FTs (already transferred for payouts)
            U128(0)
        }
    }

    // #[payable]
    // pub fn buy_token_copy(
    //     &mut self,
    //     nft_contract_id: AccountId,
    //     series_id: TokenSeriesId,
    //     reciever_id: AccountId,
    // ) -> Promise {
    //     let contract_and_series: ContractAndSeriesId =
    //         format!("{}{}{}", nft_contract_id, DELIMETER, series_id);
    //     let price = self
    //         .market
    //         .token_series
    //         .get(&contract_and_series)
    //         .expect("Token series not found");
    //     let balance = env::attached_deposit() - price;
    //     ext_contract::nft_mint(
    //         series_id,
    //         reciever_id,
    //         nft_contract_id.clone(),
    //         balance,
    //         GAS_FOR_MINT,
    //     )
    //     .then(ext_self::resolve_mint(
    //         nft_contract_id,
    //         env::predecessor_account_id(),
    //         env::attached_deposit().into(),
    //         price.into(),
    //         env::current_account_id(),
    //         0,
    //         GAS_FOR_MINT,
    //     ))
    // }

    // #[private]
    // pub fn resolve_mint(
    //     &mut self,
    //     nft_contract_id: AccountId,
    //     buyer_id: AccountId,
    //     deposit: U128,
    //     price: U128,
    // ) {
    //     require!(
    //         env::promise_results_count() == 1,
    //         "Contract expected a result on the callback"
    //     );
    //     match env::promise_result(0) {
    //         PromiseResult::Successful(token_id) => ext_contract::nft_payout(
    //             near_sdk::serde_json::from_slice::<TokenId>(&token_id)
    //                 .unwrap_or_else(|_| env::panic_str("Should be unreachable")),
    //             price,
    //             10,
    //             nft_contract_id.clone(),
    //             0,
    //             GAS_FOR_NFT_TRANSFER,
    //         )
    //         .then(ext_self::resolve_token_buy(
    //             buyer_id,
    //             deposit,
    //             price,
    //             nft_contract_id,
    //             0,
    //             GAS_FOR_ROYALTIES,
    //         )),
    //         _ => Promise::new(buyer_id).transfer(deposit.into()),
    //     };
    // }

    #[private]
    pub fn resolve_token_buy(&mut self, buyer_id: AccountId, deposit: U128, price: U128) -> U128 {
        let payout_option = promise_result_as_success().and_then(|value| {
            // None means a bad payout from bad NFT contract
            near_sdk::serde_json::from_slice::<Payout>(&value)
                .ok()
                .and_then(|payout| {
                    let mut remainder = price.0;
                    for &value in payout.payout.values() {
                        remainder = remainder.checked_sub(value.0)?;
                    }
                    if remainder <= 1 {
                        Some(payout)
                    } else {
                        None
                    }
                })
        });
        let payout = if let Some(payout_option) = payout_option {
            payout_option
        } else {
            Promise::new(buyer_id).transfer(u128::from(deposit));
            return price;
        };
        for (receiver_id, amount) in payout.payout {
            Promise::new(receiver_id).transfer(amount.0);
        }
        price
    }
}

/// self call

#[ext_contract(ext_self)]
trait ExtSelf {
    fn resolve_purchase(
        &mut self,
        ft_token_id: AccountId,
        buyer_id: AccountId,
        sale: Sale,
        price: U128,
    ) -> Promise;

    fn resolve_mint(
        &mut self,
        nft_contract_id: AccountId,
        buyer_id: AccountId,
        deposit: U128,
        price: U128,
    ) -> Promise;

    fn resolve_token_buy(&mut self, buyer_id: AccountId, deposit: U128, price: U128) -> Promise;
}

/// external contract calls

#[ext_contract(ext_contract)]
trait ExtContract {
    fn nft_transfer_payout(
        &mut self,
        receiver_id: AccountId,
        token_id: TokenId,
        approval_id: u64,
        memo: Option<String>,
        balance: U128,
        max_len_payout: u32,
    );
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>);
    fn nft_mint(&mut self, token_series_id: TokenSeriesId, reciever_id: AccountId);
    fn nft_payout(&self, token_id: String, balance: U128, max_len_payout: u32) -> Payout;
}
