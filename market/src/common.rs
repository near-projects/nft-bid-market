pub use near_sdk::{
    assert_one_yocto,
    borsh::{self, BorshDeserialize, BorshSerialize},
    collections::{LazyOption, LookupMap, LookupSet, TreeMap, UnorderedMap, UnorderedSet, Vector},
    env::{self, STORAGE_PRICE_PER_BYTE},
    json_types::{U128, U64},
    near_bindgen, require,
    serde::{Deserialize, Serialize},
    AccountId, Balance, BorshStorageKey, CryptoHash, PanicOnDefault, Promise,
};

pub use near_contract_standards::non_fungible_token::{
    metadata::{NFTContractMetadata, TokenMetadata, NFT_METADATA_SPEC},
    refund_deposit, NonFungibleToken, Token, TokenId,
};

pub const NANOS_PER_SEC: u64 = 1_000_000_000;

pub fn hash_string(string: &str) -> CryptoHash {
    let mut hash = CryptoHash::default();
    hash.copy_from_slice(&env::sha256(string.as_bytes()));
    hash
}
