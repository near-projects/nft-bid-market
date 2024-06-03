mod auction;
mod auction_views;
mod bid;
mod common;
mod fee;
mod inner;
mod market_core;
mod sale;
mod sale_views;
mod token;

mod hack; // TODO: remove

use common::*;
pub use near_contract_standards::non_fungible_token::hash_account_id;

use crate::auction::Auction;
pub use crate::auction::{AuctionJson, EXTENSION_DURATION};
pub use crate::bid::{Bid, BidAccount, BidId, BidsForContractAndTokenId};
pub use crate::fee::{Fees, PAYOUT_TOTAL_VALUE, PROTOCOL_FEE};
pub use crate::market_core::{ArgsKind, AuctionArgs, SaleArgs};
use crate::sale::{ContractAndTokenId, FungibleTokenId, Sale, SaleConditions, TokenType};
pub use crate::sale::{SaleJson, BID_HISTORY_LENGTH_DEFAULT};
//use std::collections::HashMap;

const STORAGE_PER_SALE: u128 = 1000 * STORAGE_PRICE_PER_BYTE;

/// Helper structure to for keys of the persistent collections.
#[derive(BorshStorageKey, BorshSerialize)]
pub enum StorageKey {
    Sales,
    ByOwnerId,
    ByOwnerIdInner {
        account_id_hash: CryptoHash,
    },
    ByNFTContractId,
    ByNFTContractIdInner {
        account_id_hash: CryptoHash,
    },
    ByNFTTokenType,
    ByNFTTokenTypeInner {
        token_type_hash: CryptoHash,
    },
    FTTokenIds,
    StorageDeposits,
    BidsByIndex,
    Bids,
    BidsForContractAndOwner {
        contract_and_token_hash: CryptoHash,
    },
    BidsForContractAndOwnerInner {
        contract_and_token_hash: CryptoHash,
        balance: [u8; 16],
    },
    BidsByOwner,
    BidsByOwnerInner {
        account_id_hash: CryptoHash,
    },
    BidAccounts,
    BidAccountsInner {
        account_id_hash: CryptoHash,
    },
    Auctions,
    NFTTokenContracts,
}

#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct MarketSales {
    pub owner_id: AccountId,
    pub sales: UnorderedMap<ContractAndTokenId, Sale>,
    pub by_owner_id: LookupMap<AccountId, UnorderedSet<ContractAndTokenId>>,
    pub by_nft_contract_id: LookupMap<AccountId, UnorderedSet<TokenId>>,
    pub by_nft_token_type: LookupMap<String, UnorderedSet<ContractAndTokenId>>,
    pub ft_token_ids: UnorderedSet<FungibleTokenId>,
    pub storage_deposits: LookupMap<AccountId, Balance>,

    pub bids_by_index: LookupMap<BidId, Bid>,
    pub bids: LookupMap<ContractAndTokenId, BidsForContractAndTokenId>,
    pub bids_by_owner:
        LookupMap<AccountId, UnorderedMap<ContractAndTokenId, (FungibleTokenId, Balance, BidId)>>,
    pub next_bid_id: BidId,

    pub bid_accounts: LookupMap<AccountId, BidAccount>,
    pub auctions: UnorderedMap<u128, Auction>,
    pub next_auction_id: u128,
}

#[near_bindgen]
#[derive(BorshDeserialize, BorshSerialize, PanicOnDefault)]
pub struct Market {
    non_fungible_token_account_ids: LookupSet<AccountId>,
    market: MarketSales,
}

#[near_bindgen]
impl Market {
    #[init]
    pub fn new(nft_ids: Vec<AccountId>, owner_id: AccountId) -> Self {
        let mut non_fungible_token_account_ids = LookupSet::new(StorageKey::NFTTokenContracts);
        non_fungible_token_account_ids.extend(nft_ids);
        let mut tokens = UnorderedSet::new(StorageKey::FTTokenIds);
        tokens.insert(&AccountId::new_unchecked("near".to_owned()));
        let market = MarketSales {
            owner_id,
            sales: UnorderedMap::new(StorageKey::Sales),
            by_owner_id: LookupMap::new(StorageKey::ByOwnerId),
            by_nft_contract_id: LookupMap::new(StorageKey::ByNFTContractId),
            by_nft_token_type: LookupMap::new(StorageKey::ByNFTTokenType),
            ft_token_ids: tokens,
            storage_deposits: LookupMap::new(StorageKey::StorageDeposits),

            bids_by_index: LookupMap::new(StorageKey::BidsByIndex),
            bids: LookupMap::new(StorageKey::Bids),
            bids_by_owner: LookupMap::new(StorageKey::BidsByOwner),
            next_bid_id: 0,

            bid_accounts: LookupMap::new(StorageKey::BidAccounts),
            //bid_history_length: BID_HISTORY_LENGTH_DEFAULT,
            auctions: UnorderedMap::new(StorageKey::Auctions),
            next_auction_id: 0,
        };
        Self {
            non_fungible_token_account_ids,
            market,
        }
    }

    #[payable]
    pub fn storage_withdraw(&mut self) {
        assert_one_yocto();
        let owner_id = env::predecessor_account_id();
        let mut amount = self.market.storage_deposits.remove(&owner_id).unwrap_or(0);
        let sales = self.market.by_owner_id.get(&owner_id);
        let len = sales.map(|s| s.len()).unwrap_or_default();
        let diff = u128::from(len) * STORAGE_PER_SALE;
        amount -= diff;
        if amount > 0 {
            Promise::new(owner_id.clone()).transfer(amount);
        }
        if diff > 0 {
            self.market.storage_deposits.insert(&owner_id, &diff);
        }
    }

    #[payable]
    pub fn storage_deposit(&mut self, account_id: Option<AccountId>) {
        let storage_account_id = account_id.unwrap_or_else(env::predecessor_account_id);
        let deposit = env::attached_deposit();
        assert!(
            deposit >= STORAGE_PER_SALE,
            "Requires minimum deposit of {}",
            STORAGE_PER_SALE
        );
        let mut balance: u128 = self
            .market
            .storage_deposits
            .get(&storage_account_id)
            .unwrap_or(0);
        balance += deposit;
        self.market
            .storage_deposits
            .insert(&storage_account_id, &balance);
    }

    pub fn storage_amount(&self) -> U128 {
        U128(STORAGE_PER_SALE)
    }

    #[payable]
    pub fn bid_withdraw(&mut self, amount: Option<U128>, ft_token_id: Option<AccountId>) {
        assert_one_yocto();
        let owner_id = env::predecessor_account_id();
        let balance = self.view_deposit(ft_token_id.clone());
        let amount = amount.map(|b| b.0).unwrap_or(balance);
        assert!(amount <= balance, "Can't withdraw more than you have");
        let bid_ft = match ft_token_id {
            Some(ft) => ft,
            None => "near".parse().unwrap(),
        };
        self.refund_bid(bid_ft.clone(), owner_id.clone(), amount.into());
        self.market
            .bid_accounts
            .get(&owner_id)
            .expect("Bid account not found")
            .total_balance
            .insert(&bid_ft, &(balance - amount));
    }

    #[payable]
    pub fn bid_deposit(&mut self, account_id: Option<AccountId>, ft_token_id: Option<AccountId>) {
        let owner_id = account_id.unwrap_or_else(env::predecessor_account_id);
        match ft_token_id {
            None => {
                let bid_ft = AccountId::new_unchecked("near".to_string());
                let added_amount = env::attached_deposit();
                // let mut initial_map = LookupMap::new(b"b");
                // initial_map.insert(&bid_ft, &0).unwrap();
                // let initial_acc = BidAccount {
                //     total_balance: initial_map,
                // };
                let mut bid_account =
                    self.market
                        .bid_accounts
                        .get(&owner_id)
                        .unwrap_or_else(|| BidAccount {
                            total_balance: LookupMap::new(StorageKey::BidAccountsInner {
                                account_id_hash: hash_account_id(&owner_id),
                            }),
                        });
                let previous_balance = bid_account.total_balance.get(&bid_ft).unwrap_or_default();
                bid_account
                    .total_balance
                    .insert(&bid_ft, &(previous_balance + added_amount));
                self.market.bid_accounts.insert(&owner_id, &bid_account);
            }
            Some(_ft) => (),
        };
    }

    pub fn view_deposit(&self, ft_token_id: Option<AccountId>) -> Balance {
        let ft = ft_token_id.unwrap_or(AccountId::new_unchecked("near".to_string()));
        self.market
            .bid_accounts
            .get(&env::predecessor_account_id())
            .expect("Bid account not found")
            .total_balance
            .get(&ft)
            .expect("No token")
    }
}
