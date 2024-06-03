use anyhow::Result;
use serde_json::json;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

//use crate::utils::{init_market, init_nft, mint_token, check_outcome_success, check_outcome_fail};
use crate::utils::{create_series, create_subaccount, deposit, init_market, init_nft, mint_token};
use near_units::{parse_gas, parse_near};
use nft_bid_market::{ArgsKind, AuctionArgs, AuctionJson};
//use workspaces::{Contract, Account, Worker};

use crate::transaction_status::StatusCheck;
pub use workspaces::result::CallExecutionDetails;

const THIRTY_SECONDS: Duration = Duration::from_secs(30);
const FIFTEEN_MINUTES: Duration = Duration::from_secs(60 * 15);

#[tokio::test]
async fn nft_on_approve_auction_positive() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;
    let market = init_market(&worker, worker.root_account().id(), vec![nft.id()]).await?;

    let user1 = create_subaccount(&worker, &owner, "user1").await?;

    let series = create_series(&worker, nft.id(), &user1, owner.id()).await?;
    let token1 = mint_token(&worker, nft.id(), &user1, user1.id(), &series).await?;

    deposit(&worker, market.id(), &user1).await?;
    let outcome = user1
        .call(&worker, nft.id(), "nft_approve")
        .args_json(json!({
            "token_id": token1,
            "account_id": market.id(),
            "msg": json!(ArgsKind::Auction(AuctionArgs {
                token_type: None,
                minimal_step: 100.into(),
                start_price: 10000.into(),
                start: None,
                duration: 900000000000.into(),
                buy_out_price: Some(10000000000.into()),
                origins: None,
            })).to_string()
        }))?
        .deposit(parse_near!("1 N"))
        .gas(parse_gas!("200 Tgas") as u64)
        .transact()
        .await;
    //check_outcome_success(outcome.status).await;
    assert!(
        outcome.is_ok(),
        "Failed with error {}",
        outcome.err().unwrap()
    );
    Ok(())
}

/*
    - Should panic if `ft_token_id` is not supported
    - TODO: Should panic if the auction is not in progress
    - Panics if auction is not active
    - Should panic if the owner tries to bid on his own auction
    - Should panic if the bid is smaller than the minimal deposit
    - Should panic if the bid is smaller than the previous one + minimal step + fees
*/
#[tokio::test]
async fn auction_add_bid_negative() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;
    let market = init_market(&worker, worker.root_account().id(), vec![nft.id()]).await?;

    let user1 = create_subaccount(&worker, &owner, "user1").await?;
    let user2 = create_subaccount(&worker, &owner, "user2").await?;

    let series = create_series(&worker, nft.id(), &user1, owner.id()).await?;
    let token1 = mint_token(&worker, nft.id(), &user1, user1.id(), &series).await?;

    deposit(&worker, market.id(), &user1).await?;
    user1
        .call(&worker, nft.id(), "nft_approve")
        .args_json(json!({
            "token_id": token1,
            "account_id": market.id(),
            "msg": json!(ArgsKind::Auction(AuctionArgs {
                token_type: None,
                minimal_step: 100.into(),
                start_price: 10000.into(),
                start: None,
                duration: 900000000000.into(),
                buy_out_price: Some(10000000000.into()),
                origins: None,
            })).to_string()
        }))?
        .deposit(parse_near!("1 N"))
        .gas(parse_gas!("200 Tgas") as u64)
        .transact()
        .await?;

    // Should panic if `ft_token_id` is not supported
    let outcome = user2
        .call(&worker, market.id(), "auction_add_bid")
        .args_json(json!({
            "auction_id": "0".to_string(),
            "token_type": "not_near".to_string(),
        }))?
        .deposit(10300)
        .transact()
        .await;
    outcome.assert_err("token not supported").unwrap();

    // Panics if auction is not active
    let outcome = user2
        .call(&worker, market.id(), "auction_add_bid")
        .args_json(json!({
            "auction_id": "1".to_string(),
        }))?
        .deposit(10300)
        .transact()
        .await;
    outcome.assert_err("Auction does not exist").unwrap();

    // Should panic if the owner tries to bid on his own auction
    let outcome = user1
        .call(&worker, market.id(), "auction_add_bid")
        .args_json(json!({
            "auction_id": "0".to_string(),
        }))?
        .deposit(10300)
        .transact()
        .await;
    outcome
        .assert_err("Cannot bid on your own auction")
        .unwrap();

    // Should panic if the bid is smaller than the minimal deposit
    let outcome = user2
        .call(&worker, market.id(), "auction_add_bid")
        .args_json(json!({
            "auction_id": "0".to_string(),
        }))?
        .deposit(10200)
        .transact()
        .await;
    outcome.assert_err("Should bid at least 10300").unwrap();

    // Should panic if the bid is smaller than the previous one
    user2
        .call(&worker, market.id(), "auction_add_bid")
        .args_json(json!({
            "auction_id": "0".to_string(),
        }))?
        .deposit(10300)
        .transact()
        .await?;
    let outcome = user2
        .call(&worker, market.id(), "auction_add_bid")
        .args_json(json!({
            "auction_id": "0".to_string(),
        }))?
        .deposit(10350)
        .transact()
        .await;
    outcome.assert_err("Should bid at least 10403").unwrap();

    Ok(())
}

/*
    - TODO: Refunds a previous bid (if it exists)
    - Extends an auction if the bid is added less than 15 minutes before the end
    - The auction ends if the `attached_deposit` is bigger than the `buy_out_price` (plus fees)
*/
#[tokio::test]
async fn auction_add_bid_positive() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;
    let market = init_market(&worker, worker.root_account().id(), vec![nft.id()]).await?;

    let user1 = create_subaccount(&worker, &owner, "user1").await?;
    let user2 = create_subaccount(&worker, &owner, "user2").await?;

    let series = create_series(&worker, nft.id(), &user1, owner.id()).await?;
    let token1 = mint_token(&worker, nft.id(), &user1, user1.id(), &series).await?;

    deposit(&worker, market.id(), &user1).await?;

    user1
        .call(&worker, nft.id(), "nft_approve")
        .args_json(json!({
            "token_id": token1,
            "account_id": market.id(),
            "msg": json!(ArgsKind::Auction(AuctionArgs {
                token_type: None,
                minimal_step: 100.into(),
                start_price: 10000.into(),
                start: None,
                duration: 900000000000.into(),
                buy_out_price: Some(10000000000.into()),
                origins: None,
            })).to_string()
        }))?
        .deposit(parse_near!("1 N"))
        .gas(parse_gas!("200 Tgas") as u64)
        .transact()
        .await?;

    // Extends an auction if the bid is added less than 15 minutes before the end
    let auction: AuctionJson = market
        .view(
            &worker,
            "get_auction",
            json!({ "auction_id": "0".to_string() })
                .to_string()
                .into_bytes(),
        )
        .await?
        .json()?;
    let right_before_bid = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    user2
        .call(&worker, market.id(), "auction_add_bid")
        .args_json(json!({
            "auction_id": "0".to_string(),
        }))?
        .deposit(10300)
        .transact()
        .await?;
    let auction_bought_out: AuctionJson = market
        .view(
            &worker,
            "get_auction",
            json!({ "auction_id": "0".to_string() })
                .to_string()
                .into_bytes(),
        )
        .await?
        .json()?;
    assert!(
        auction.end.0 < auction_bought_out.end.0,
        "The auction wasn't extended"
    );
    assert!(
        Duration::from_nanos(auction_bought_out.end.0) - (right_before_bid + FIFTEEN_MINUTES)
            < THIRTY_SECONDS
    );

    // The auction ends if the `attached_deposit` is bigger than the `buy_out_price` (plus fees)
    let auction: AuctionJson = market
        .view(
            &worker,
            "get_auction",
            json!({ "auction_id": "0".to_string() })
                .to_string()
                .into_bytes(),
        )
        .await?
        .json()?;
    let right_before_bid = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    user2
        .call(&worker, market.id(), "auction_add_bid")
        .args_json(json!({
            "auction_id": "0".to_string(),
        }))?
        .deposit(10300000000)
        .transact()
        .await?;
    let auction_bought_out: AuctionJson = market
        .view(
            &worker,
            "get_auction",
            json!({ "auction_id": "0".to_string() })
                .to_string()
                .into_bytes(),
        )
        .await?
        .json()?;
    let in_progress: bool = market
        .view(
            &worker,
            "check_auction_in_progress",
            json!({ "auction_id": "0".to_string() })
                .to_string()
                .into_bytes(),
        )
        .await?
        .json()?;
    assert!(
        auction.end.0 > auction_bought_out.end.0,
        "The end time wasn't decreased"
    );
    assert!(!in_progress, "The auction didn't end");
    assert!(Duration::from_nanos(auction_bought_out.end.0) - right_before_bid < THIRTY_SECONDS);
    Ok(())
}

/*
    - Should panic unless 1 yoctoNEAR is attached
    - Can only be called by the creator of the auction
    - Panics if auction is not active
    - Panics if the auction already has a bid
*/
#[tokio::test]
async fn cancel_auction_negative() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;
    let market = init_market(&worker, worker.root_account().id(), vec![nft.id()]).await?;

    let user1 = create_subaccount(&worker, &owner, "user1").await?;
    let user2 = create_subaccount(&worker, &owner, "user2").await?;

    let series = create_series(&worker, nft.id(), &user1, owner.id()).await?;
    let token1 = mint_token(&worker, nft.id(), &user1, user1.id(), &series).await?;

    deposit(&worker, market.id(), &user1).await?;

    user1
        .call(&worker, nft.id(), "nft_approve")
        .args_json(json!({
            "token_id": token1,
            "account_id": market.id(),
            "msg": json!(ArgsKind::Auction(AuctionArgs {
                token_type: None,
                minimal_step: 100.into(),
                start_price: 10000.into(),
                start: None,
                duration: 900000000000.into(),
                buy_out_price: Some(10000000000.into()),
                origins: None,
            })).to_string()
        }))?
        .deposit(parse_near!("1 N"))
        .gas(parse_gas!("200 Tgas") as u64)
        .transact()
        .await?;

    // Should panic unless 1 yoctoNEAR is attached
    let outcome = user1
        .call(&worker, market.id(), "cancel_auction")
        .args_json(json!({
            "auction_id": "0".to_string()
        }))?
        .deposit(2)
        .gas(parse_gas!("200 Tgas") as u64)
        .transact()
        .await;
    outcome
        .assert_err("Requires attached deposit of exactly 1 yoctoNEAR")
        .unwrap();

    // Panics if auction is not active
    let outcome = user1
        .call(&worker, market.id(), "cancel_auction")
        .args_json(json!({
            "auction_id": "1".to_string()
        }))?
        .deposit(1)
        .gas(parse_gas!("200 Tgas") as u64)
        .transact()
        .await;
    outcome.assert_err("Auction is not active").unwrap();

    // Can only be called by the creator of the auction
    let outcome = user2
        .call(&worker, market.id(), "cancel_auction")
        .args_json(json!({
            "auction_id": "0".to_string()
        }))?
        .deposit(1)
        .gas(parse_gas!("200 Tgas") as u64)
        .transact()
        .await;
    outcome
        .assert_err("Only the auction owner can cancel the auction")
        .unwrap();

    // Panics if the auction already has a bid
    user2
        .call(&worker, market.id(), "auction_add_bid")
        .args_json(json!({
            "auction_id": "0".to_string(),
        }))?
        .deposit(10300)
        .transact()
        .await?;
    let outcome = user1
        .call(&worker, market.id(), "cancel_auction")
        .args_json(json!({
            "auction_id": "0".to_string()
        }))?
        .deposit(1)
        .gas(parse_gas!("200 Tgas") as u64)
        .transact()
        .await;
    outcome
        .assert_err("Can't cancel the auction after the first bid is made")
        .unwrap();

    let vector_auctions: Vec<AuctionJson> = market
        .view(
            &worker,
            "get_auctions",
            json!({"from_index": null, "limit": null})
                .to_string()
                .into_bytes(),
        )
        .await?
        .json()?;
    assert!(!vector_auctions.is_empty(), "Deleted the auction");
    Ok(())
}

/*
    - Removes the auction
*/
#[tokio::test]
async fn cancel_auction_positive() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;
    let market = init_market(&worker, worker.root_account().id(), vec![nft.id()]).await?;

    let user1 = create_subaccount(&worker, &owner, "user1").await?;

    let series = create_series(&worker, nft.id(), &user1, owner.id()).await?;
    let token1 = mint_token(&worker, nft.id(), &user1, user1.id(), &series).await?;

    deposit(&worker, market.id(), &user1).await?;

    user1
        .call(&worker, nft.id(), "nft_approve")
        .args_json(json!({
            "token_id": token1,
            "account_id": market.id(),
            "msg": json!(ArgsKind::Auction(AuctionArgs {
                token_type: None,
                minimal_step: 100.into(),
                start_price: 10000.into(),
                start: None,
                duration: 900000000000.into(),
                buy_out_price: Some(10000000000.into()),
                origins: None,
            })).to_string()
        }))?
        .deposit(parse_near!("1 N"))
        .gas(parse_gas!("200 Tgas") as u64)
        .transact()
        .await?;

    let outcome = user1
        .call(&worker, market.id(), "cancel_auction")
        .args_json(json!({
            "auction_id": "0".to_string()
        }))?
        .deposit(1)
        .gas(parse_gas!("200 Tgas") as u64)
        .transact()
        .await;
    //check_outcome_success(outcome.status).await;
    assert!(
        outcome.is_ok(),
        "Failed with error {}",
        outcome.err().unwrap()
    );
    let vector_auctions: Vec<AuctionJson> = market
        .view(&worker, "get_auctions", json!({}).to_string().into_bytes())
        .await?
        .json()?;
    assert!(vector_auctions.is_empty(), "Did not delete the auction");
    Ok(())
}

/*
    -  TODO: NFT is transferred to the buyer
    -  TODO: ft transferred to the previous owner
    -  TODO: protocol and origins fees are paid
    -  TODO: the previous owner also pays royalty
    -  TODO: the auction is removed from list of auctions
*/
#[tokio::test]
async fn finish_auction_positive() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;
    let market = init_market(&worker, worker.root_account().id(), vec![nft.id()]).await?;

    let user1 = create_subaccount(&worker, &owner, "user1").await?;

    let series = create_series(&worker, nft.id(), &user1, owner.id()).await?;
    let token1 = mint_token(&worker, nft.id(), &user1, user1.id(), &series).await?;

    deposit(&worker, market.id(), &user1).await?;

    user1
        .call(&worker, nft.id(), "nft_approve")
        .args_json(json!({
            "token_id": token1,
            "account_id": market.id(),
            "msg": json!(ArgsKind::Auction(AuctionArgs {
                token_type: None,
                minimal_step: 100.into(),
                start_price: 10000.into(),
                start: None,
                duration: 900000000000.into(),
                buy_out_price: Some(10000000000.into()),
                origins: None,
            })).to_string()
        }))?
        .deposit(parse_near!("1 N"))
        .gas(parse_gas!("200 Tgas") as u64)
        .transact()
        .await?;

    /*let outcome = user1
        .call(&worker, market.id(), "finish_auction")
        .args_json(json!({
            "auction_id": "1".to_string()
        }))?
        .gas(parse_gas!("200 Tgas") as u64)
        .transact()
        .await;
    println!("{:?}", outcome.status);
    //check_outcome_success(outcome.status).await;
    assert!(outcome.is_ok(), "Failed with error {}", outcome.err().unwrap()); */

    Ok(())
}

/*
    - Panics if the auction is not active
    - Should panic if called before the auction ends
    - TODO: Panics if there is no bid
    - TODO: panic if number of payouts plus number of bids exceeds 10
*/
#[tokio::test]
async fn finish_auction_negative() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;
    let market = init_market(&worker, worker.root_account().id(), vec![nft.id()]).await?;

    let user1 = create_subaccount(&worker, &owner, "user1").await?;

    let series = create_series(&worker, nft.id(), &user1, owner.id()).await?;
    let token1 = mint_token(&worker, nft.id(), &user1, user1.id(), &series).await?;

    deposit(&worker, market.id(), &user1).await?;

    user1
        .call(&worker, nft.id(), "nft_approve")
        .args_json(json!({
            "token_id": token1,
            "account_id": market.id(),
            "msg": json!(ArgsKind::Auction(AuctionArgs {
                token_type: None,
                minimal_step: 100.into(),
                start_price: 10000.into(),
                start: None,
                duration: 900000000000.into(),
                buy_out_price: Some(10000000000.into()),
                origins: None,
            })).to_string()
        }))?
        .deposit(parse_near!("1 N"))
        .gas(parse_gas!("200 Tgas") as u64)
        .transact()
        .await?;

    // Panics if the auction is not active
    let outcome = user1
        .call(&worker, market.id(), "finish_auction")
        .args_json(json!({
            "auction_id": "1".to_string()
        }))?
        .gas(parse_gas!("200 Tgas") as u64)
        .transact()
        .await;
    outcome.assert_err("Auction is not active").unwrap();

    // Should panic if called before the auction ends
    let outcome = user1
        .call(&worker, market.id(), "finish_auction")
        .args_json(json!({
            "auction_id": "0".to_string()
        }))?
        .gas(parse_gas!("200 Tgas") as u64)
        .transact()
        .await;
    outcome
        .assert_err("Auction can be finalized only after the end time")
        .unwrap();

    // Panics if there is no bid

    Ok(())
}
