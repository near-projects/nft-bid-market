use anyhow::Result;
use serde_json::json;
use workspaces::network::Sandbox;
use workspaces::{Account, AccountId, Contract, Worker};

use crate::utils::{create_subaccount, init_nft};

use crate::transaction_status::StatusCheck;
pub use workspaces::result::CallExecutionDetails;

pub async fn set_private_minting(
    worker: &Worker<Sandbox>,
    nft: &AccountId,
    user: &Account,
    enabled: bool,
) {
    user.call(worker, nft, "set_private_minting")
        .args_json(json!({
            "enabled": enabled,
        }))
        .unwrap()
        .transact()
        .await
        .unwrap();
}

pub async fn grant(
    worker: &Worker<Sandbox>,
    nft: &AccountId,
    user: &Account,
    account_id: &str,
) -> Result<bool> {
    let result = user
        .call(worker, nft, "grant")
        .args_json(json!({
            "account_id": account_id,
        }))?
        .transact()
        .await?
        .json()?;
    Ok(result)
}

pub async fn is_allowed(
    worker: &Worker<Sandbox>,
    nft: &Contract,
    account_id: &str,
) -> Result<bool> {
    let result: bool = nft
        .view(
            worker,
            "is_allowed",
            json!({
                "account_id": account_id,
            })
            .to_string()
            .into_bytes(),
        )
        .await?
        .json()?;
    Ok(result)
}

/*
- Can only be called by the owner
- Adds a given account to the list of the authorized accounts
- Returns `true` if the new account has been added to the list, `false` otherwise
*/
#[tokio::test]
async fn permissions_grant() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;

    let user1 = create_subaccount(&worker, &owner, "user1").await?;

    set_private_minting(&worker, nft.id(), &owner, true).await;

    // Can only be called by the owner
    let outcome = user1
        .call(&worker, nft.id(), "grant")
        .args_json(json!({
            "account_id": "user1",
        }))?
        .transact()
        .await;
    outcome.assert_err("only owner can grant").unwrap();

    // Adds a given account to the list of the authorized accounts
    let outcome = owner
        .call(&worker, nft.id(), "grant")
        .args_json(json!({
            "account_id": "user1",
        }))?
        .transact()
        .await;
    //check_outcome_success(outcome.clone().status).await;
    assert!(
        outcome.is_ok(),
        "Failed with error {}",
        outcome.err().unwrap()
    );

    // Returns `true` if the new account has been added to the list
    //assert!(outcome.json()?, "Returned false");
    assert!(
        is_allowed(&worker, &nft, "user1").await?,
        "The user is not authorized"
    );

    // `user1` is already in the list, thus `false` is returned
    let outcome = owner
        .call(&worker, nft.id(), "grant")
        .args_json(json!({
            "account_id": "user1",
        }))?
        .transact()
        .await;
    //check_outcome_success(outcome.clone().status).await;
    assert!(
        outcome.is_ok(),
        "Failed with error {}",
        outcome.err().unwrap()
    );
    //assert!(!outcome.json()?, "Returned true");

    Ok(())
}

/*
- Can only be called by the owner
- Removes a given account from the list of the authorized accounts
- Returns `true` if the account has been removed from the list, `false` if it hadn't been in the list
*/
#[tokio::test]
async fn permissions_deny() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;

    let user1 = create_subaccount(&worker, &owner, "user1").await?;

    set_private_minting(&worker, nft.id(), &owner, true).await;
    grant(&worker, nft.id(), &owner, "user1").await?;

    // Can only be called by the owner
    let outcome = user1
        .call(&worker, nft.id(), "deny")
        .args_json(json!({
            "account_id": "user1",
        }))?
        .transact()
        .await;
    outcome.assert_err("only owner can deny").unwrap();

    // Called by the owner
    let outcome = owner
        .call(&worker, nft.id(), "deny")
        .args_json(json!({
            "account_id": "user1",
        }))?
        .transact()
        .await;
    //check_outcome_success(outcome.clone().status).await;
    assert!(
        outcome.is_ok(),
        "Failed with error {}",
        outcome.err().unwrap()
    );

    // Returns `true` if the account has been removed from the list
    //assert!(outcome.json()?, "Returned false");
    let result = is_allowed(&worker, &nft, "user1").await?;
    assert!(!result, "Not authorized");

    // `user1` is not in the list, thus `false` is returned
    let outcome = owner
        .call(&worker, nft.id(), "deny")
        .args_json(json!({
            "account_id": "user1",
        }))?
        .transact()
        .await;
    //check_outcome_success(outcome.clone().status).await;
    assert!(
        outcome.is_ok(),
        "Failed with error {}",
        outcome.err().unwrap()
    );
    //assert!(!outcome.json()?, "Returned true");

    Ok(())
}

/*
- Can only be called by the owner
- If `enabled` is true, turns on private minting
- If `enabled` is false, turns off private minting
*/
#[tokio::test]
async fn permissions_set_private_minting() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;

    let user1 = create_subaccount(&worker, &owner, "user1").await?;

    // Can only be called by the owner
    let outcome = user1
        .call(&worker, nft.id(), "set_private_minting")
        .args_json(json!({
            "enabled": true,
        }))?
        .transact()
        .await;
    outcome
        .assert_err("only owner can enable/disable private minting")
        .unwrap();
    assert!(
        is_allowed(&worker, &nft, "user1").await?,
        "The authorization is turned on"
    );

    // If `enabled` is true, turns on private minting
    let outcome = owner
        .call(&worker, nft.id(), "set_private_minting")
        .args_json(json!({
            "enabled": true,
        }))?
        .transact()
        .await;
    assert!(
        outcome.is_ok(),
        "Failed with error {}",
        outcome.err().unwrap()
    );
    //check_outcome_success(outcome.status).await;
    assert!(
        !is_allowed(&worker, &nft, "user1").await?,
        "The authorization is turned off"
    );

    // If `enabled` is false, turns off private minting
    let outcome = owner
        .call(&worker, nft.id(), "set_private_minting")
        .args_json(json!({
            "enabled": false,
        }))?
        .transact()
        .await;
    //check_outcome_success(outcome.status).await;
    assert!(
        outcome.is_ok(),
        "Failed with error {}",
        outcome.err().unwrap()
    );
    assert!(
        is_allowed(&worker, &nft, "user1").await?,
        "The authorization is turned on"
    );

    Ok(())
}

/*
- Returns true if private minting is not enabled
- If private minting is enabled, returns whether an account is among private minters
*/
#[tokio::test]
async fn permissions_is_allowed() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;

    // Returns true if private minting is not enabled
    let result = is_allowed(&worker, &nft, "user1").await?;
    assert!(result, "Not authorized");

    // If private minting is enabled and `user1` not authorized, returns false
    set_private_minting(&worker, nft.id(), &owner, true).await;
    let result = is_allowed(&worker, &nft, "user1").await?;
    assert!(!result, "Not authorized");

    // If private minting is enabled and `user1` authorized, returns true
    grant(&worker, nft.id(), &owner, "user1").await?;
    let result = is_allowed(&worker, &nft, "user1").await?;
    assert!(result, "Authorized");

    Ok(())
}
