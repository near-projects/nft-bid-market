use std::{
    collections::HashMap,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::utils::{init_market, init_nft, create_subaccount, create_series, deposit,
    mint_token, nft_approve, price_with_fees, offer, offer_with_duration,
    check_outcome_success, check_outcome_fail
};
use near_units::{parse_gas, parse_near};
use nft_bid_market::{ArgsKind, SaleArgs, SaleJson};
use nft_contract::common::{AccountId, U128, U64};

/*
- TODO: Refunds a bid, removes it from the list
*/
#[tokio::test]
async fn remove_bid_positive() -> anyhow::Result<()> {
    let worker = workspaces::sandbox();
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;
    let market = init_market(&worker, worker.root_account().id(), vec![nft.id()]).await?;

    let user1 = create_subaccount(&worker, &owner, "user1").await?;
    let user2 = create_subaccount(&worker, &owner, "user2").await?;

    let series = create_series(&worker, nft.id().clone(), &user1, owner.id().clone()).await?;
    let token1 = mint_token(&worker, nft.id().clone(), &user1, user1.id(), &series).await?;
    deposit(&worker, market.id().clone(), &user1).await;
    let sale_conditions = HashMap::from([("near".parse().unwrap(), 10000.into())]);
    nft_approve(&worker, nft.id().clone(), market.id().clone(), &user1, token1.clone(), sale_conditions.clone(), series.clone()).await;
    let price: U128 = 900.into();
    offer(&worker, nft.id().clone(), market.id().clone(), &user2, token1.clone(), price.clone()).await;

    let outcome = user2
        .call(&worker, market.id().clone(), "remove_bid")
        .args_json(serde_json::json!({
            "nft_contract_id": nft.id().clone(),
            "token_id": token1.clone(),
            "ft_token_id": "near",
            "price": price.clone(),
        }))?
        .deposit(1)
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await?;
    check_outcome_success(outcome.status).await;
    Ok(())
}

/*
- Should panic unless 1 yoctoNEAR is attached
- Should panic if there is no sale with the given `nft_contract_id` and `token_id`
- Should panic if there is no bids with `ft_token_id`
*/
#[tokio::test]
async fn remove_bid_negative() -> anyhow::Result<()> {
    let worker = workspaces::sandbox();
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;
    let market = init_market(&worker, worker.root_account().id(), vec![nft.id()]).await?;

    let user1 = create_subaccount(&worker, &owner, "user1").await?;
    let user2 = create_subaccount(&worker, &owner, "user2").await?;

    let series = create_series(&worker, nft.id().clone(), &user1, owner.id().clone()).await?;
    let token1 = mint_token(&worker, nft.id().clone(), &user1, user1.id(), &series).await?;
    deposit(&worker, market.id().clone(), &user1).await;
    let sale_conditions = HashMap::from([("near".parse().unwrap(), 10000.into())]);
    nft_approve(&worker, nft.id().clone(), market.id().clone(), &user1, token1.clone(), sale_conditions.clone(), series.clone()).await;
    let price: U128 = 900.into();
    offer(&worker, nft.id().clone(), market.id().clone(), &user2, token1.clone(), price.clone()).await;

    // Should panic unless 1 yoctoNEAR is attached
    let outcome = user2
        .call(&worker, market.id().clone(), "remove_bid")
        .args_json(serde_json::json!({
            "nft_contract_id": nft.id().clone(),
            "token_id": token1.clone(),
            "ft_token_id": "near",
            "price": price.clone(),
        }))?
        .deposit(2)
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await?;
    println!("nft_contract_id: {}, token_id: {}", nft.id().clone(), token1.clone());
    println!("{:?}", outcome.status);
    check_outcome_fail(outcome.status, "Requires attached deposit of exactly 1 yoctoNEAR").await;

    // Should panic if there is no sale with the given `nft_contract_id` and `token_id`
    let outcome = user2
        .call(&worker, market.id().clone(), "remove_bid")
        .args_json(serde_json::json!({
            "nft_contract_id": "some_other_nft_contract".to_string(),
            "token_id": token1.clone(),
            "ft_token_id": "near",
            "price": price.clone(),
        }))?
        .deposit(1)
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await?;
    check_outcome_fail(outcome.status, "No sale").await;

    let outcome = user2
        .call(&worker, market.id().clone(), "remove_bid")
        .args_json(serde_json::json!({
            "nft_contract_id": nft.id().clone(),
            "token_id": "1:10",
            "ft_token_id": "near",
            "price": price.clone(),
        }))?
        .deposit(1)
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await?;
    check_outcome_fail(outcome.status, "No sale").await;

    // Should panic if there is no bids with `ft_token_id`
    let outcome = user2
        .call(&worker, market.id().clone(), "remove_bid")
        .args_json(serde_json::json!({
            "nft_contract_id": nft.id().clone(),
            "token_id": token1.clone(),
            "ft_token_id": "not_near",
            "price": price.clone(),
        }))?
        .deposit(1)
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await?;
    check_outcome_fail(outcome.status, "No token").await;

    Ok(())
}

/*
TODO: Refunds a bid, removes it from the list
*/
#[tokio::test]
async fn cancel_bid_positive() -> anyhow::Result<()> {
    let worker = workspaces::sandbox();
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;
    let market = init_market(&worker, worker.root_account().id(), vec![nft.id()]).await?;

    let user1 = create_subaccount(&worker, &owner, "user1").await?;
    let user2 = create_subaccount(&worker, &owner, "user2").await?;
    let user3 = create_subaccount(&worker, &owner, "user3").await?;

    let series = create_series(&worker, nft.id().clone(), &user1, owner.id().clone()).await?;
    let token1 = mint_token(&worker, nft.id().clone(), &user1, user1.id(), &series).await?;
    deposit(&worker, market.id().clone(), &user1).await;
    let sale_conditions = HashMap::from([("near".parse().unwrap(), 10000.into())]);
    nft_approve(&worker, nft.id().clone(), market.id().clone(), &user1, token1.clone(), sale_conditions.clone(), series.clone()).await;
    let price: U128 = 900.into();
    offer_with_duration(&worker, nft.id().clone(), market.id().clone(), &user2, token1.clone(), price.clone(), U64(100000000)).await;

    let outcome = user3
        .call(&worker, market.id().clone(), "cancel_bid")
        .args_json(serde_json::json!({
            "nft_contract_id": nft.id().clone(),
            "token_id": token1.clone(),
            "ft_token_id": "near",
            "owner_id": user2.id(),
            "price": price.clone(),
        }))?
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await?;
    check_outcome_success(outcome.status).await;

    Ok(())
}

/*
- Should panic if the bid isn't finished yet
- Should panic if the bid doesn't have end time
- Should panic if there is no sale with the given `nft_contract_id` and `token_id`
- Should panic if there is no bids with `ft_token_id`
- Should panic if there is no bid with given `owner_id` and `price`
*/
#[tokio::test]
async fn cancel_bid_negative() -> anyhow::Result<()> {
    let worker = workspaces::sandbox();
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;
    let market = init_market(&worker, worker.root_account().id(), vec![nft.id()]).await?;

    let user1 = create_subaccount(&worker, &owner, "user1").await?;
    let user2 = create_subaccount(&worker, &owner, "user2").await?;
    let user3 = create_subaccount(&worker, &owner, "user3").await?;

    let series = create_series(&worker, nft.id().clone(), &user1, owner.id().clone()).await?;
    let token1 = mint_token(&worker, nft.id().clone(), &user1, user1.id(), &series).await?;
    deposit(&worker, market.id().clone(), &user1).await;
    let sale_conditions = HashMap::from([("near".parse().unwrap(), 10000.into())]);
    nft_approve(&worker, nft.id().clone(), market.id().clone(), &user1, token1.clone(), sale_conditions.clone(), series.clone()).await;
    
    // Should panic if the bid isn't finished yet
    let price: U128 = 900.into();
    offer_with_duration(&worker, nft.id().clone(), market.id().clone(), &user2, token1.clone(), price.clone(), U64(1000000000000)).await;

    let outcome = user3
        .call(&worker, market.id().clone(), "cancel_bid")
        .args_json(serde_json::json!({
            "nft_contract_id": nft.id().clone(),
            "token_id": token1.clone(),
            "ft_token_id": "near",
            "owner_id": user2.id(),
            "price": price.clone(),
        }))?
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await?;
    check_outcome_fail(outcome.status, "The bid hasn't ended yet").await;

    // Should panic if the bid doesn't have end time
    let price: U128 = 950.into();
    offer(&worker, nft.id().clone(), market.id().clone(), &user2, token1.clone(), price.clone()).await;

    let outcome = user3
        .call(&worker, market.id().clone(), "cancel_bid")
        .args_json(serde_json::json!({
            "nft_contract_id": nft.id().clone(),
            "token_id": token1.clone(),
            "ft_token_id": "near",
            "owner_id": user2.id(),
            "price": price.clone(),
        }))?
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await?;
    check_outcome_fail(outcome.status, "The bid doesn't have an end").await;

    // Should panic if the bid isn't finished yet
    let price: U128 = 900.into();
    offer_with_duration(&worker, nft.id().clone(), market.id().clone(), &user2, token1.clone(), price.clone(), U64(1000000000000)).await;

    let outcome = user3
        .call(&worker, market.id().clone(), "cancel_bid")
        .args_json(serde_json::json!({
            "nft_contract_id": nft.id().clone(),
            "token_id": token1.clone(),
            "ft_token_id": "near",
            "owner_id": user2.id(),
            "price": price.clone(),
        }))?
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await?;
    check_outcome_fail(outcome.status, "The bid hasn't ended yet").await;

    // Should panic if there is no sale with the given `nft_contract_id` and `token_id`
    let price: U128 = 1000.into();
    offer_with_duration(&worker, nft.id().clone(), market.id().clone(), &user2, token1.clone(), price.clone(), U64(100000000)).await;

    let outcome = user3
        .call(&worker, market.id().clone(), "cancel_bid")
        .args_json(serde_json::json!({
            "nft_contract_id": "another_nft_contract_id".to_string(),
            "token_id": token1.clone(),
            "ft_token_id": "near",
            "owner_id": user2.id(),
            "price": price.clone(),
        }))?
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await?;
    check_outcome_fail(outcome.status, "No sale").await;

    let outcome = user3
        .call(&worker, market.id().clone(), "cancel_bid")
        .args_json(serde_json::json!({
            "nft_contract_id": nft.id().clone(),
            "token_id": "another_token_id".to_string(),
            "ft_token_id": "near",
            "owner_id": user2.id(),
            "price": price.clone(),
        }))?
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await?;
    check_outcome_fail(outcome.status, "No sale").await;

    // Should panic if there is no bids with `ft_token_id`
    let outcome = user3
        .call(&worker, market.id().clone(), "cancel_bid")
        .args_json(serde_json::json!({
            "nft_contract_id": nft.id().clone(),
            "token_id": token1.clone(),
            "ft_token_id": "not_near",
            "owner_id": user2.id(),
            "price": price.clone(),
        }))?
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await?;
    check_outcome_fail(outcome.status, "No token").await;

    // Should panic if there is no bid with given `owner_id` and `price`
    let outcome = user3
        .call(&worker, market.id().clone(), "cancel_bid")
        .args_json(serde_json::json!({
            "nft_contract_id": nft.id().clone(),
            "token_id": token1.clone(),
            "ft_token_id": "near",
            "owner_id": user1.id(),
            "price": price.clone(),
        }))?
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await?;
    check_outcome_fail(outcome.status, "No such bid").await;

    let outcome = user3
        .call(&worker, market.id().clone(), "cancel_bid")
        .args_json(serde_json::json!({
            "nft_contract_id": nft.id().clone(),
            "token_id": token1.clone(),
            "ft_token_id": "near",
            "owner_id": user2.id(),
            "price": "1100".to_string(),
        }))?
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await?;
    check_outcome_fail(outcome.status, "No such bid").await;

    Ok(())
}

/*
- TODO: Refunds all expired bids, removes them from the list
*/
#[tokio::test]
async fn cancel_expired_bids_positive() -> anyhow::Result<()> {
    Ok(())
}

/*
- TODO: Should panic if there is no sale with the given `nft_contract_id` and `token_id`
- TODO: Should panic if there is no bids with `ft_token_id`
*/
#[tokio::test]
async fn cancel_expired_bids_negative() -> anyhow::Result<()> {
    Ok(())
}
