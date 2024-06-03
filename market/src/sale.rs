#![allow(clippy::too_many_arguments)]
use std::collections::HashMap;

use near_sdk::ext_contract;
use near_sdk::serde_json::json;
use near_sdk::{promise_result_as_success, Gas};

use crate::fee::calculate_price_with_fees;
use crate::market_core::SaleArgs;
use crate::*;
use common::*;

use bid::Origins;
pub type TokenSeriesId = String;

pub const GAS_FOR_FT_TRANSFER: Gas = Gas(5_000_000_000_000);
pub const GAS_FOR_ROYALTIES: Gas = Gas(115_000_000_000_000);
pub const GAS_FOR_NFT_TRANSFER: Gas = Gas(30_000_000_000_000);
// pub const GAS_FOR_MINT: Gas = Gas(20_000_000_000_000);
pub const BID_HISTORY_LENGTH_DEFAULT: u8 = 5;
pub(crate) const NO_DEPOSIT: Balance = 0;
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

#[derive(BorshDeserialize, BorshSerialize, Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct Sale {
    pub owner_id: AccountId,
    pub approval_id: u64,
    pub nft_contract_id: AccountId,
    pub token_id: String,
    pub sale_conditions: SaleConditions,
    //pub bids: Bids,
    pub created_at: u64,
    pub token_type: TokenType,

    pub start: Option<u64>,
    pub end: Option<u64>,

    pub origins: Origins,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "near_sdk::serde")]
pub struct SaleJson {
    pub owner_id: AccountId,
    pub nft_contract_id: AccountId,
    pub token_id: String,
    pub sale_conditions: SaleConditions,
    pub created_at: U64,
    pub token_type: TokenType,

    pub start: Option<U64>,
    pub end: Option<U64>,
    pub origins: Origins,
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

#[near_bindgen]
impl Market {
    pub(crate) fn start_sale(
        &mut self,
        args: SaleArgs,
        token_id: TokenId,
        owner_id: AccountId,
        approval_id: u64,
        nft_contract_id: AccountId,
    ) -> SaleJson {
        let SaleArgs {
            sale_conditions,
            token_type,
            start,
            end,
            origins,
        } = args;

        // check that the offered ft token is supported

        for ft_token_id in sale_conditions.keys() {
            if !self.market.ft_token_ids.contains(ft_token_id) {
                env::panic_str(&format!(
                    "Token {} not supported by this market",
                    ft_token_id
                ));
            }
            //*price = U128::from(calculate_price_with_fees(*price, None));
        }

        // Create a new sale with given arguments and empty list of bids

        let contract_and_token_id = format!("{}{}{}", nft_contract_id, DELIMETER, token_id);
        let start = start.map(|s| s.into()).unwrap_or_else(env::block_timestamp);
        let sale = Sale {
            owner_id: owner_id.clone(),
            approval_id,
            nft_contract_id: nft_contract_id.clone(),
            token_id: token_id.clone(),
            sale_conditions,
            created_at: env::block_timestamp(),
            token_type: token_type.clone(),
            start: Some(start),
            end: end.map(|e| e.into()),
            origins: origins.unwrap_or_default(),
        };
        self.market.sales.insert(&contract_and_token_id, &sale);

        // extra for views

        let mut by_owner_id = self.market.by_owner_id.get(&owner_id).unwrap_or_else(|| {
            UnorderedSet::new(
                StorageKey::ByOwnerIdInner {
                    account_id_hash: hash_account_id(&owner_id),
                }
                .try_to_vec()
                .unwrap(),
            )
        });

        // Check that the paid storage amount is enough
        let owner_paid_storage = self
            .market
            .storage_deposits
            .get(&env::signer_account_id())
            .unwrap_or(0);
        let owner_occupied_storage = u128::from(by_owner_id.len()) * STORAGE_PER_SALE;
        assert!(
            owner_paid_storage > owner_occupied_storage,
            "User has more sales than storage paid"
        );
        by_owner_id.insert(&contract_and_token_id);
        self.market.by_owner_id.insert(&owner_id, &by_owner_id);

        let mut by_nft_contract_id = self
            .market
            .by_nft_contract_id
            .get(&nft_contract_id)
            .unwrap_or_else(|| {
                UnorderedSet::new(
                    StorageKey::ByNFTContractIdInner {
                        account_id_hash: hash_account_id(&nft_contract_id),
                    }
                    .try_to_vec()
                    .unwrap(),
                )
            });
        by_nft_contract_id.insert(&token_id);
        self.market
            .by_nft_contract_id
            .insert(&nft_contract_id, &by_nft_contract_id);

        if let Some(token_type) = token_type {
            assert!(
                token_id.contains(token_type.as_str()),
                "TokenType should be substr of TokenId"
            );
            let mut by_nft_token_type = self
                .market
                .by_nft_token_type
                .get(&token_type)
                .unwrap_or_else(|| {
                    UnorderedSet::new(
                        StorageKey::ByNFTTokenTypeInner {
                            token_type_hash: hash_account_id(&AccountId::new_unchecked(
                                token_type.clone(),
                            )),
                        }
                        .try_to_vec()
                        .unwrap(),
                    )
                });
            by_nft_token_type.insert(&contract_and_token_id);
            self.market
                .by_nft_token_type
                .insert(&token_type, &by_nft_token_type);
        }

        self.json_from_sale(sale)
    }

    /// TODO remove without redirect to wallet? panic reverts
    #[payable]
    pub fn remove_sale(&mut self, nft_contract_id: AccountId, token_id: String) {
        assert_one_yocto();
        let sale = self.internal_remove_sale(nft_contract_id, token_id);
        let owner_id = env::predecessor_account_id();
        if sale.in_limits() {
            assert_eq!(
                owner_id, sale.owner_id,
                "Until the sale is finished, it can only be removed by the sale owner"
            );
        };
        //self.refund_all_bids(&sale.bids); TODO: refactor the bids so that funds are not taken before the sale is made
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

    // Offer to buy the nft
    // Buy nft if the attached deposit equal to the price, otherwise adds a bid
    #[payable]
    pub fn offer(
        &mut self,
        nft_contract_id: AccountId,
        token_id: String,
        ft_token_id: AccountId,
        offered_price: U128,
        start: Option<U64>,
        duration: Option<U64>,
        origins: Option<Origins>,
    ) -> Option<U128> {
        assert_one_yocto();
        let contract_id: AccountId = nft_contract_id;
        let contract_and_token_id = format!("{}{}{}", contract_id, DELIMETER, token_id);
        require!(offered_price.0 > 0, "Offered price must be greater than 0");
        let start = start.unwrap_or(env::block_timestamp().into());
        let end = duration.map(|d| U64(d.0 + start.0));
        let buyer_id = env::predecessor_account_id();
        let sale = self.market.sales.get(&contract_and_token_id);
        // Check that the sale is in progress
        // require!(
        //     sale.in_limits(),
        //     "Either the sale is finished or it hasn't started yet"
        // );
        if let Some(sale) = sale {
            require!(sale.owner_id != buyer_id, "Cannot bid on your own sale.");
            let price = *sale
                .sale_conditions
                .get(&ft_token_id)
                .unwrap_or_else(|| env::panic_str("Not supported ft"));

            let balance = self
                .get_bid_balance(&buyer_id, &ft_token_id)
                .unwrap_or_default();

            if offered_price.0 == calculate_price_with_fees(price, origins.as_ref())
                && balance >= offered_price.0
            {
                self.process_purchase(
                    contract_id,
                    token_id,
                    ft_token_id,
                    offered_price,
                    buyer_id,
                    origins.unwrap_or_default(),
                );
                None
            } else {
                Some(
                    self.add_bid(
                        contract_id,
                        token_id,
                        offered_price.0,
                        ft_token_id,
                        buyer_id,
                        start,
                        end,
                        origins,
                    )
                    .into(),
                )
            }
        } else {
            Some(
                self.add_bid(
                    contract_id,
                    token_id,
                    offered_price.0,
                    ft_token_id,
                    buyer_id,
                    start,
                    end,
                    origins,
                )
                .into(),
            )
        }
    }

    // Accepts the highest active bid
    pub fn accept_bid(
        &mut self,
        nft_contract_id: AccountId,
        token_id: String,
        ft_token_id: AccountId,
    ) {
        let contract_id: AccountId = nft_contract_id;
        let contract_and_token_id = format!("{}{}{}", contract_id, DELIMETER, token_id);
        // Check that the sale is in progress and remove bid before proceeding to process purchase
        let sale = self
            .market
            .sales
            .get(&contract_and_token_id)
            .expect("No sale");
        require!(
            sale.in_limits(),
            "Either the sale is finished or it hasn't started yet"
        );
        //let bids_for_token_id = self.market.bids.remove()
        // let bids_for_token_id = self
        //     .market
        //     .bids
        //     .get(&contract_and_token_id)
        //     .unwrap()
        //     .remove(&ft_token_id)
        //     .expect("No bids");
        // let mut bid = &bids_for_token_id.get(bids_for_token_id.len() - 1).unwrap();

        //let bids = self.market.bids;
        let bids_for_contract_and_token_id = self
            .market
            .bids
            .get(&contract_and_token_id)
            .expect("No bids for this contract and token id");
        let bids_tree = bids_for_contract_and_token_id
            .get(&ft_token_id)
            .expect("No token");

        let mut biggest_bid = u128::MAX;
        let mut price = 0;
        for (balance, equal_bids) in bids_tree.iter_rev() {
            let mut earliest_bid_id = equal_bids
                .as_vector()
                .get(0)
                .expect("No bids with this balance");
            // let bid = self
            //     .market
            //     .bids_by_index
            //     .get(&bid_id)
            //     .expect("No bid with this id");
            let mut min_start_time = u64::MAX;

            for bid_id in equal_bids.iter() {
                let bid = self
                    .market
                    .bids_by_index
                    .get(&bid_id)
                    .expect("No bid with this id");
                if bid.in_limits()
                    && bid.start.0 < min_start_time
                    && self.is_active(bid_id, ft_token_id.clone())
                {
                    min_start_time = bid.start.0;
                    earliest_bid_id = bid_id;
                }
            }
            if min_start_time < u64::MAX {
                biggest_bid = earliest_bid_id;
                price = balance;
                break;
            }
        }
        require!(price > 0, "There are no active non-finished bids");
        let bid = self
            .market
            .bids_by_index
            .get(&biggest_bid)
            .expect("No bid with this id");
        //require!(bid.in_limits(), "Out of time limit of the bid");
        // self.market.sales.insert(&contract_and_token_id, &sale);
        // panics at `self.internal_remove_sale` and reverts above if predecessor is not sale.owner_id
        self.process_purchase(
            contract_id,
            token_id,
            ft_token_id,
            bid.price,
            bid.owner_id.clone(),
            bid.origins,
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
        origins: Origins,
    ) -> Promise {
        // TODO: better to remove this sale at callback, so we don't pass this huge struct
        let sale = self.internal_remove_sale(nft_contract_id.clone(), token_id.clone());

        // Decrease account balance
        let mut buyer_bid_account = self
            .market
            .bid_accounts
            .get(&buyer_id)
            .expect("No bid account");
        let mut balance = buyer_bid_account
            .total_balance
            .get(&ft_token_id)
            .expect("No ft_token_id");
        assert!(balance >= price.0, "Not enough funds");
        balance -= price.0;
        buyer_bid_account
            .total_balance
            .insert(&ft_token_id, &balance);
        self.market
            .bid_accounts
            .insert(&buyer_id, &buyer_bid_account);

        let mut buyer = origins;
        buyer.insert(env::current_account_id(), PROTOCOL_FEE as u32);
        let mut seller_fee = HashMap::with_capacity(sale.origins.len() + 1);
        seller_fee.extend(sale.origins.clone()); // TODO: dodge this clone
        seller_fee.insert(env::current_account_id(), PROTOCOL_FEE as u32);
        let fees = fee::Fees {
            buyer,
            seller: seller_fee,
        };
        ext_contract::nft_transfer_payout(
            buyer_id.clone(),
            token_id,
            sale.approval_id,
            Some(near_sdk::serde_json::to_string(&fees).expect("Failed to sereailize")),
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

    // self callback
    // If transfer of token succeded - count fees and transfer payouts
    // If failed - refund price to buyer
    #[private]
    pub fn resolve_purchase(
        &mut self,
        ft_token_id: AccountId,
        buyer_id: AccountId,
        sale: Sale,
        price: U128,
    ) -> U128 {
        //TODO: should take ContractAndTokenId instead of Sale
        let contract_and_token_id: ContractAndTokenId =
            format!("{}{}{}", &sale.nft_contract_id, DELIMETER, sale.token_id);
        // checking for payout information
        let payout_option = promise_result_as_success().and_then(|value| {
            // None means a bad payout from bad NFT contract
            near_sdk::serde_json::from_slice::<Payout>(&value)
                .ok()
                .and_then(|payout| {
                    // gas to do 10 FT transfers (and definitely 10 NEAR transfers)
                    if payout.payout.len() > 10 || payout.payout.is_empty() {
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
        let payout = if let Some(payout_option) = payout_option {
            payout_option
        } else {
            if ft_token_id == "near".parse().unwrap() {
                Promise::new(buyer_id.clone()).transfer(u128::from(price));
            }
            // leave function and return all FTs in ft_resolve_transfer
            env::log_str(
                &json!({
                    "type": "resolve_purchase_fail",
                    "params": {
                        "owner_id": sale.owner_id,
                        "nft_contract_id": sale.nft_contract_id,
                        "token_id": sale.token_id,
                        "ft_token_id": ft_token_id,
                        "price": price,
                        "buyer_id": buyer_id,
                    }
                })
                .to_string(),
            );
            return price;
        };
        // Going to payout everyone, first return all outstanding bids (accepted offer bid was already removed)
        //self.refund_all_bids(&bids_for_contract_and_token_id); // TODO: maybe should do this outside of this call, to lower gas for this call

        // NEAR payouts
        if ft_token_id == "near".parse().unwrap() {
            for (receiver_id, amount) in payout.payout {
                Promise::new(receiver_id).transfer(amount.0);
            }
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

    // For lazy-mint situations easier resolver
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

    fn get_bid_balance(&self, owner_id: &AccountId, ft: &AccountId) -> Option<Balance> {
        let bid_account = self.market.bid_accounts.get(&owner_id);
        if let Some(bid_account) = bid_account {
            let ft_balance = bid_account.total_balance.get(&ft);
            if let Some(ft_balance) = ft_balance {
                return Some(ft_balance);
            }
        }
        None
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

    fn resolve_finish_auction(&mut self, ft_token_id: AccountId, buyer_id: AccountId, price: U128);

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
    ) -> Promise;
    fn ft_transfer(&mut self, receiver_id: AccountId, amount: U128, memo: Option<String>);
    fn nft_mint(&mut self, token_series_id: TokenSeriesId, receiver_id: AccountId);
    fn nft_payout(&self, token_id: String, balance: U128, max_len_payout: u32) -> Payout;
}
