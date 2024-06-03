use near_contract_standards::non_fungible_token::Token;
use near_units::parse_gas;
use near_units::parse_near;
use nft_bid_market::Fees;
use nft_contract::common::TokenMetadata;
use nft_contract::common::{U128, U64};
use nft_contract::Payout;
use serde_json::json;
use std::collections::HashMap;
use workspaces::network::Sandbox;
use workspaces::prelude::*;
use workspaces::result::CallExecutionDetails;
use workspaces::{Account, AccountId, Contract, Worker};

use anyhow::Result;

const NFT_WASM_FILEPATH: &str = "../res/nft_contract.wasm";
const MARKET_WASM_FILEPATH: &str = "../res/nft_bid_market.wasm";

pub async fn init_nft(worker: &Worker<Sandbox>, root_id: &AccountId) -> Result<Contract> {
    let wasm = std::fs::read(NFT_WASM_FILEPATH)?;
    let contract = worker.dev_deploy(&wasm).await?;
    let outcome = contract
        .call(worker, "new_default_meta")
        .args_json(json!({
            "owner_id": root_id,
        }))?
        .gas(parse_gas!("150 Tgas") as u64)
        .transact()
        .await;
    assert!(
        outcome.is_ok(),
        "Failed with error {}",
        outcome.err().unwrap()
    );
    Ok(contract)
}

pub async fn init_market(
    worker: &Worker<Sandbox>,
    root_id: &AccountId,
    nft_ids: Vec<&AccountId>,
) -> Result<workspaces::Contract> {
    let wasm = std::fs::read(MARKET_WASM_FILEPATH)?;
    let contract = worker.dev_deploy(&wasm).await?;
    let outcome = contract
        .call(worker, "new")
        .args_json(json!({
            "nft_ids": nft_ids,
            "owner_id": root_id,
        }))?
        .gas(parse_gas!("150 Tgas") as u64)
        .transact()
        .await;
    // match outcome.status {
    //     near_primitives::views::FinalExecutionStatus::SuccessValue(_) => (),
    //     _ => panic!(),
    // };
    assert!(
        outcome.is_ok(),
        "Failed with error {}",
        outcome.err().unwrap()
    );
    Ok(contract)
}

pub async fn mint_token(
    worker: &Worker<Sandbox>,
    nft_id: &AccountId,
    minter_id: &Account,
    receiver_id: &AccountId,
    series: &str,
) -> Result<String> {
    let token_id = minter_id
        .call(worker, &nft_id, "nft_mint")
        .args_json(json!({
            "token_series_id": series,
            "receiver_id": receiver_id.as_ref()
        }))?
        .deposit(parse_near!("0.01 N"))
        .transact()
        .await?
        .json()?;
    Ok(token_id)
}

pub async fn create_subaccount(
    worker: &Worker<Sandbox>,
    owner: &Account,
    user_id: &str,
) -> Result<Account> {
    let user = owner
        .create_subaccount(worker, user_id)
        .initial_balance(parse_near!("10 N"))
        .transact()
        .await?
        .unwrap();
    Ok(user)
}

pub async fn create_series(
    worker: &Worker<Sandbox>,
    nft: &AccountId,
    user: &Account,
    owner: &AccountId,
) -> Result<String> {
    let series: String = user
        .call(worker, &nft, "nft_create_series")
        .args_json(json!({
        "token_metadata":
        {
            "title": "some title",
            "media": "ipfs://QmTqZsmhZLLbi8vxZwm21wjKRFRBUQFzMFtTiyh3DJ2CCz",
            "copies": 10
        },
        "royalty":
        {
            owner.as_ref(): 1000
        }}))?
        .deposit(parse_near!("0.005 N"))
        .transact()
        .await?
        .json()?;
    Ok(series)
}

pub async fn deposit(
    worker: &Worker<Sandbox>,
    market: &AccountId,
    user: &Account,
) -> Result<CallExecutionDetails> {
    user.call(worker, &market, "storage_deposit")
        .deposit(parse_near!("1 N"))
        .args_json(json!({}))
        .unwrap()
        .transact()
        .await
}

pub async fn nft_approve(
    worker: &Worker<Sandbox>,
    nft: &AccountId,
    market: &AccountId,
    user: &Account,
    token: &str,
    sale_conditions: &HashMap<AccountId, U128>,
    series: &str,
) -> Result<CallExecutionDetails> {
    let msg = json!({"Sale": {
        "sale_conditions": sale_conditions,
        "token_type": Some(series),
    }});

    user.call(worker, nft, "nft_approve")
        .args_json(json!({
            "token_id": token,
            "account_id": market,
            "msg": msg.to_string()
        }))
        .unwrap()
        .deposit(parse_near!("1 N"))
        .gas(parse_gas!("200 Tgas") as u64)
        .transact()
        .await
}

pub async fn offer(
    worker: &Worker<Sandbox>,
    nft: &AccountId,
    market: &AccountId,
    user: &Account,
    token: &str,
    price: U128,
) -> Result<CallExecutionDetails> {
    user.call(worker, &market.clone(), "offer")
        .args_json(json!({
            "nft_contract_id": nft,
            "token_id": token,
            "ft_token_id": "near",
            "offered_price": price
        }))
        .unwrap()
        .deposit(1)
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await
}

pub async fn create_series_raw(
    worker: &Worker<Sandbox>,
    nft: &AccountId,
    owner: &Account,
    copies: Option<u64>,
    royalty: HashMap<&AccountId, u64>,
) -> Result<String> {
    let token_metadata = TokenMetadata {
        title: Some("some title".to_string()),
        description: None,
        media: Some("ipfs://QmTqZsmhZLLbi8vxZwm21wjKRFRBUQFzMFtTiyh3DJ2CCz".to_string()),
        media_hash: None,
        copies,
        issued_at: None,
        expires_at: None,
        starts_at: None,
        updated_at: None,
        extra: None,
        reference: None,
        reference_hash: None,
    };
    Ok(owner
        .call(worker, &nft, "nft_create_series")
        .args_json(json!({
                "token_metadata": token_metadata,
                "royalty": royalty
        }))?
        .deposit(parse_near!("0.005 N"))
        .transact()
        .await?
        .json()?)
}

pub async fn offer_with_duration(
    worker: &Worker<Sandbox>,
    nft: &AccountId,
    market: &AccountId,
    user: &Account,
    token: &str,
    price: U128,
    duration: U64,
) -> Result<CallExecutionDetails> {
    user.call(worker, &market.clone(), "offer")
        .args_json(json!({
            "nft_contract_id": nft,
            "token_id": token,
            "ft_token_id": "near",
            "offered_price": price,
            "duration": duration
        }))
        .unwrap()
        .deposit(1)
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await
}

pub async fn nft_transfer_payout_helper(
    worker: &Worker<Sandbox>,
    nft: &Contract,
    user1: &Account,
    user2: &Account,
    user3: &Account,
    royalty: HashMap<&AccountId, u64>,
    fees: Fees,
    balance: U128,
) -> Payout {
    let series = create_series_raw(worker, nft.id(), user1, Some(4), royalty)
        .await
        .unwrap();

    let token_id = mint_token(worker, nft.id(), user1, user1.id(), &series)
        .await
        .unwrap();
    user1
        .call(worker, nft.id(), "nft_approve")
        .args_json(json!({
            "token_id": token_id,
            "account_id": user2.id(),
        }))
        .unwrap()
        .deposit(parse_near!("1 N"))
        .gas(parse_gas!("200 Tgas") as u64)
        .transact()
        .await
        .unwrap();
    let approval_id: u64 = {
        let token: Token = nft
            .view(
                worker,
                "nft_token",
                json!({ "token_id": token_id }).to_string().into_bytes(),
            )
            .await
            .unwrap()
            .json()
            .unwrap();
        let approval_account_ids = token.approved_account_ids.unwrap();
        *approval_account_ids
            .get(&user2.id().as_ref().parse().unwrap())
            .unwrap()
    };
    user2
        .call(worker, nft.id(), "nft_transfer_payout")
        .args_json(json!({
            "receiver_id": user3.id(),
            "token_id": token_id,
            "approval_id": approval_id,
            "memo": json!(fees).to_string(),
            "balance": balance,
            "max_len_payout": 10,
        }))
        .unwrap()
        .deposit(1)
        .transact()
        .await
        .unwrap()
        .json()
        .unwrap()
}
