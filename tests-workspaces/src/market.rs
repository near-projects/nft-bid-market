use anyhow::Result;
use std::collections::HashMap;

use near_units::parse_near;
use serde_json::json;

use crate::utils::{create_series_raw, init_market, init_nft, mint_token, nft_approve};

use crate::transaction_status::StatusCheck;
pub use workspaces::result::CallExecutionDetails;

#[tokio::test]
async fn storage_deposit() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;
    let market = init_market(&worker, worker.root_account().id(), vec![nft.id()]).await?;

    let user = owner
        .create_subaccount(&worker, "user1")
        .initial_balance(parse_near!("10 N"))
        .transact()
        .await?
        .unwrap();

    // Negative
    let outcome = user
        .call(&worker, market.id(), "storage_deposit")
        .deposit(20)
        .args_json(json!({}))
        .unwrap()
        .transact()
        .await;
    outcome.assert_err("Requires minimum deposit of").unwrap();

    // Positive
    let outcome = user
        .call(&worker, market.id(), "storage_deposit")
        .deposit(parse_near!("0.01 N"))
        .args_json(json!({}))
        .unwrap()
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

#[tokio::test]
async fn storage_withdraw() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;
    let market = init_market(&worker, worker.root_account().id(), vec![nft.id()]).await?;

    let user = owner
        .create_subaccount(&worker, "user1")
        .initial_balance(parse_near!("10 N"))
        .transact()
        .await?
        .unwrap();
    let outcome = user
        .call(&worker, market.id(), "storage_deposit")
        .deposit(parse_near!("5 N"))
        .args_json(json!({}))
        .unwrap()
        .transact()
        .await;
    //check_outcome_success(outcome.status).await;
    assert!(
        outcome.is_ok(),
        "Failed with error {}",
        outcome.err().unwrap()
    );
    let series = create_series_raw(
        &worker,
        nft.id(),
        &user,
        Some(4),
        HashMap::from([(user.id(), 500)]),
    )
    .await?;
    let token = mint_token(&worker, nft.id(), &user, user.id(), &series).await?;
    let sale_conditions = HashMap::from([("near".parse().unwrap(), 42000.into())]);
    nft_approve(
        &worker,
        nft.id(),
        market.id(),
        &user,
        &token,
        &sale_conditions,
        &series,
    )
    .await?;

    // Negative
    // - requires 1 yocto
    let outcome = user
        .call(&worker, market.id(), "storage_withdraw")
        .transact()
        .await;
    outcome
        .assert_err("Requires attached deposit of exactly 1 yoctoNEAR")
        .unwrap();

    // Positive
    // - deposit refunded
    let outcome = user
        .call(&worker, market.id(), "storage_withdraw")
        .deposit(1)
        .transact()
        .await;
    //check_outcome_success(outcome.status).await;
    assert!(
        outcome.is_ok(),
        "Failed with error {}",
        outcome.err().unwrap()
    );

    // TODO: check balances
    Ok(())
}
