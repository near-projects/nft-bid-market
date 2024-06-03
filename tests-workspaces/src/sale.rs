use std::{
    collections::HashMap,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::utils::{
    create_series, create_series_raw, deposit, init_market, init_nft, mint_token, nft_approve,
    offer,
};
use anyhow::Result;
use near_contract_standards::non_fungible_token::Token;
use near_units::{parse_gas, parse_near};
use nft_bid_market::{BidId, SaleJson};
use nft_contract::common::{U128, U64};

use crate::transaction_status::StatusCheck;
use serde_json::json;
pub use workspaces::result::CallExecutionDetails;
use workspaces::AccountId;

/*
- Can only be called via cross-contract call
- `owner_id` must be the signer
- Panics if `owner_id` didn't pay for one more sale/auction
- Panics if the given `ft_token_id` is not supported by the market
- Panics if `msg` doesn't contain valid parameters for sale or auction
 */
#[tokio::test]
async fn nft_on_approve_negative() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;
    let market = init_market(&worker, worker.root_account().id(), vec![nft.id()]).await?;

    let user1 = owner
        .create_subaccount(&worker, "user1")
        .initial_balance(parse_near!("10 N"))
        .transact()
        .await?
        .unwrap();

    let series: String = user1
        .call(&worker, nft.id(), "nft_create_series")
        .args_json(json!({
        "token_metadata":
        {
            "title": "some title",
            "media": "ipfs://QmTqZsmhZLLbi8vxZwm21wjKRFRBUQFzMFtTiyh3DJ2CCz",
            "copies": 10
        },
        "royalty":
        {
            owner.id().as_ref(): 1000
        }}))?
        .deposit(parse_near!("0.005 N"))
        .transact()
        .await?
        .json()?;
    let token1 = mint_token(&worker, nft.id(), &user1, user1.id(), &series).await?;

    let msg = json!({"Sale": {
        "sale_conditions": { "near": "10000" },
        "token_type": Some(&series),
    }});

    // try to call nft_on_approve without cross contract call
    let outcome = user1
        .call(&worker, market.id(), "nft_on_approve")
        .args_json(json!({
            "token_id": token1,
            "owner_id": user1.id(),
            "approval_id": 1u64,
            "msg": msg.to_string()
        }))?
        .transact()
        .await;
    outcome
        .assert_err("nft_on_approve should only be called via cross-contract call")
        .unwrap();

    // TODO: to test `owner_id` must be the signer need to create another contract

    let msg = json!({"Sale": {
        "sale_conditions": { "near": "10000" },
        "token_type": Some(&series),
    }});

    // fail without storage deposit
    let outcome = user1
        .call(&worker, nft.id(), "nft_approve")
        .args_json(json!({
            "token_id": token1,
            "account_id": market.id(),
            "msg": msg.to_string()
        }))?
        .deposit(parse_near!("1 N"))
        .gas(parse_gas!("200 Tgas") as u64)
        .transact()
        .await;
    outcome.assert_err("Insufficient storage paid").unwrap();

    let msg = json!({"Sale": {
        "sale_conditions": { "near": "10000" },
        "token_type": Some(&series),
    }});

    // not supported ft
    deposit(&worker, market.id(), &user1).await.unwrap();
    let outcome = user1
        .call(&worker, nft.id(), "nft_approve")
        .args_json(json!({
            "token_id": "wrong_token",
            "account_id": market.id(),
            "msg": msg.to_string()
        }))?
        .deposit(parse_near!("1 N"))
        .gas(parse_gas!("200 Tgas") as u64)
        .transact()
        .await;
    outcome.assert_err("Token not found").unwrap();

    // bad message, sale/auction shouldn't be added
    let outcome = user1
        .call(&worker, nft.id(), "nft_approve")
        .args_json(json!({
            "token_id": token1,
            "account_id": market.id(),
            "msg": json!({
                    "a": "b"
            }).to_string()
        }))?
        .deposit(parse_near!("1 N"))
        .gas(parse_gas!("200 Tgas") as u64)
        .transact()
        .await;
    outcome.assert_err("Not valid args").unwrap();

    Ok(())
}

/*
- Start time is set to `block_timestamp` if it is not specified explicitly
- Creates a new sale/auction
 */
#[tokio::test]
async fn nft_on_approve_positive() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;
    let market = init_market(&worker, worker.root_account().id(), vec![nft.id()]).await?;

    let user1 = owner
        .create_subaccount(&worker, "user1")
        .initial_balance(parse_near!("10 N"))
        .transact()
        .await?
        .unwrap();

    let series: String = user1
        .call(&worker, nft.id(), "nft_create_series")
        .args_json(json!({
        "token_metadata":
        {
            "title": "some title",
            "media": "ipfs://QmTqZsmhZLLbi8vxZwm21wjKRFRBUQFzMFtTiyh3DJ2CCz",
            "copies": 10u64
        },
        "royalty":
        {
            owner.id().as_ref(): 1000
        }}))?
        .deposit(parse_near!("0.005 N"))
        .transact()
        .await?
        .json()?;
    let token1 = mint_token(&worker, nft.id(), &user1, user1.id(), &series).await?;

    deposit(&worker, market.id(), &user1).await?;

    let msg = json!({"Sale": {
        "sale_conditions": { "near": "10000" },
        "token_type": Some(series),
    }});

    user1
        .call(&worker, nft.id(), "nft_approve")
        .args_json(json!({
            "token_id": token1,
            "account_id": market.id(),
            "msg": msg.to_string()
        }))?
        .deposit(parse_near!("1 N"))
        .gas(parse_gas!("200 Tgas") as u64)
        .transact()
        .await?;
    let since_the_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    let sale_json: SaleJson = market
        .view(
            &worker,
            "get_sale",
            json!({
               "nft_contract_id": nft.id(),
               "token_id": token1
            })
            .to_string()
            .into_bytes(),
        )
        .await?
        .json()?;

    let time_passed = since_the_epoch - Duration::from_nanos(sale_json.start.unwrap().0);
    assert!(time_passed < Duration::from_secs(60)); // shouldn't be 60 secs even in worse case
    Ok(())
}

/**
    - Should panic if there is no sale with given `contract_and_token_id`
    - Should panic if the sale is not in progress
    - Should panic if the NFT owner tries to make a bid on his own sale
    - Should panic if the deposit equal to 0
    - Should panic if the NFT can't be bought by `ft_token_id`
- If the `attached_deposit` is equal to the price + fees
  -  panics if number of payouts plus number of bids exceeds 10
- If the `attached_deposit` is not equal to the price + fees
  - should panic if `ft_token_id` is not supported
  - panics if the bid smaller or equal to the previous one
  - panic if origin fee exceeds ORIGIN_FEE_MAX
    */
#[tokio::test]
async fn offer_negative() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;
    let market = init_market(&worker, worker.root_account().id(), vec![nft.id()]).await?;

    let user1 = owner
        .create_subaccount(&worker, "user1")
        .initial_balance(parse_near!("10 N"))
        .transact()
        .await?
        .unwrap();

    let user2 = owner
        .create_subaccount(&worker, "user2")
        .initial_balance(parse_near!("10 N"))
        .transact()
        .await?
        .unwrap();

    // No sale with given `contract_and_token_id`
    // let outcome = user1
    //     .call(&worker, market.id(), "offer")
    //     .args_json(json!({
    //         "nft_contract_id": nft.id(),
    //         "token_id": "1:1",
    //         "ft_token_id": "near",
    //         "offered_price": "500",
    //     }))?
    //     .deposit(1)
    //     .transact()
    //     .await;

    offer(&worker, nft.id(), market.id(), &user1, "1:1", U128(500))
        .await
        .assert_err("No sale")
        .unwrap();

    // Sale is not in progress
    let series = create_series(&worker, nft.id(), &user1, owner.id()).await?;
    let token1 = mint_token(&worker, nft.id(), &user1, user1.id(), &series).await?;

    deposit(&worker, market.id(), &user1).await?;
    let since_the_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    let waiting_time = Duration::from_secs(15);
    let epoch_plus_waiting_time = (since_the_epoch + waiting_time).as_nanos();
    let sale_conditions = HashMap::<String, U128>::from([("near".to_string(), U128(10000))]);

    let msg = json!({"Sale": {
        "sale_conditions": &sale_conditions,
        "token_type": Some(series),
        "start": Some(U64(epoch_plus_waiting_time as u64)),
    }});

    // Depricated: now contract allows to create a bid, even if the sale hasn't started
    user1
        .call(&worker, nft.id(), "nft_approve")
        .args_json(json!({
            "token_id": token1,
            "account_id": market.id(),
            "msg": msg.to_string()
        }))?
        .deposit(parse_near!("1 N"))
        .gas(parse_gas!("200 Tgas") as u64)
        .transact()
        .await?;
    // Depricated: now contract allows to create a bid, even if the sale hasn't started
    // let outcome = user2
    //     .call(&worker, market.id(), "offer")
    //     .args_json(json!({
    //         "nft_contract_id": nft.id(),
    //         "token_id": token1,
    //         "ft_token_id": "near",
    //         "offered_price": "500",
    //     }))?
    //     .deposit(1)
    //     .transact()
    //     .await;
    // outcome
    //     .assert_err("Either the sale is finished or it hasn't started yet")
    //     .unwrap();

    tokio::time::sleep(waiting_time).await;
    let price: U128 = market
        .view(
            &worker,
            "price_with_fees",
            json!({
                "price": sale_conditions.get("near").unwrap(),
            })
            .to_string()
            .into_bytes(),
        )
        .await?
        .json()?;
    // NFT owner tries to make a bid on his own sale
    let outcome = user1
        .call(&worker, market.id(), "offer")
        .args_json(json!({
            "nft_contract_id": nft.id(),
            "token_id": token1,
            "ft_token_id": "near",
            "offered_price": price,
        }))?
        .deposit(1)
        .transact()
        .await;
    outcome.assert_err("Cannot bid on your own sale.").unwrap();

    // Deposit not equal to 1
    let outcome = user2
        .call(&worker, market.id(), "offer")
        .args_json(json!({
            "nft_contract_id": nft.id(),
            "token_id": token1,
            "ft_token_id": "near",
            "offered_price": "500",
        }))?
        .deposit(2)
        .transact()
        .await;
    outcome
        .assert_err("Requires attached deposit of exactly 1 yoctoNEAR")
        .unwrap();

    // Offered deposit not equal to 0
    let outcome = user2
        .call(&worker, market.id(), "offer")
        .args_json(json!({
            "nft_contract_id": nft.id(),
            "token_id": token1,
            "ft_token_id": "near",
            "offered_price": "0",
        }))?
        .deposit(1)
        .transact()
        .await;
    outcome
        .assert_err("Offered price must be greater than 0")
        .unwrap();

    // Not supported ft
    let outcome = user2
        .call(&worker, market.id(), "offer")
        .args_json(json!({
            "nft_contract_id": nft.id(),
            "token_id": token1,
            "ft_token_id": "nearcoin",
            "offered_price": "1000",
        }))?
        .deposit(1)
        .transact()
        .await;
    outcome.assert_err("Not supported ft").unwrap();

    // the bid smaller or equal to the previous one (deprecated)
    // user2
    //     .call(&worker, market.id(), "offer")
    //     .args_json(json!({
    //         "nft_contract_id": nft.id(),
    //         "token_id": token1,
    //         "ft_token_id": "near",
    //         "offered_price": 500,
    //     }))?
    //     .deposit(1)
    //     .gas(parse_gas!("300 Tgas") as u64)
    //     .transact()
    //     .await?;
    // let outcome = user2
    //     .call(&worker, market.id(), "offer")
    //     .args_json(json!({
    //         "nft_contract_id": nft.id(),
    //         "token_id": token1,
    //         "ft_token_id": "near",
    //         "offered_price": 400, // less
    //     }))?
    //     .deposit(1)
    //     .gas(parse_gas!("300 Tgas") as u64)
    //     .transact()
    //     .await?;
    // //check_outcome_fail(
    //     outcome.status,
    //     "Can't pay less than or equal to current bid price:",
    // )
    // .await;
    // let outcome = user2
    //     .call(&worker, market.id(), "offer")
    //     .args_json(json!({
    //         "nft_contract_id": nft.id(),
    //         "token_id": token1,
    //         "ft_token_id": "near",
    //         "offered_price": 500, // equal
    //     }))?
    //     .deposit(1)
    //     .gas(parse_gas!("300 Tgas") as u64)
    //     .transact()
    //     .await?;
    // //check_outcome_fail(
    //     outcome.status,
    //     "Can't pay less than or equal to current bid price:",
    // )
    // .await;

    // Exceeding ORIGIN_FEE_MAX
    let outcome = user2
        .call(&worker, market.id(), "offer")
        .args_json(json!({
            "nft_contract_id": nft.id(),
            "token_id": token1,
            "ft_token_id": "near",
            "offered_price": "2000", // equal
            "origins": {
                "user1": 4701,
            }
        }))?
        .deposit(1)
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await;
    outcome.assert_err("Max origins exceeded").unwrap();

    // number of payouts plus number of bids exceeds 10
    let too_much_origins: HashMap<AccountId, u32> = HashMap::from([
        ("acc1.near".parse().unwrap(), 100),
        ("acc2.near".parse().unwrap(), 100),
        ("acc3.near".parse().unwrap(), 100),
        ("acc4.near".parse().unwrap(), 100),
        ("acc5.near".parse().unwrap(), 100),
        ("acc6.near".parse().unwrap(), 100),
        ("acc7.near".parse().unwrap(), 100),
        ("acc8.near".parse().unwrap(), 100),
        ("acc9.near".parse().unwrap(), 100),
        ("acc10.near".parse().unwrap(), 100),
        ("acc11.near".parse().unwrap(), 100),
        ("acc12.near".parse().unwrap(), 100),
    ]);
    let price: U128 = market
        .view(
            &worker,
            "price_with_fees",
            json!({
                "price": sale_conditions.get("near").unwrap(),
                "origins": too_much_origins
            })
            .to_string()
            .into_bytes(),
        )
        .await?
        .json()?;
    let outcome = user2
        .call(&worker, market.id(), "offer")
        .args_json(json!({
            "nft_contract_id": nft.id(),
            "token_id": token1,
            "ft_token_id": "near",
            "offered_price": price,
        }))?
        .deposit(1)
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await;
    // Promise of offer returning empty value, because of panic on nft_transfer_payout, but
    // TODO: we need to check Failure on nft contract when workspaces add feature to check not only FinalExecutionStatus
    // if let near_primitives::views::FinalExecutionStatus::SuccessValue(empty_string) = outcome.status
    // {
    //     assert!(empty_string.is_empty(), "The string is not empty {:?}", empty_string)
    // } else {
    //     panic!("Expected failure {:?}", outcome.status)
    // };
    //check_outcome_success(outcome.status).await;
    assert!(
        outcome.is_ok(),
        "Failed with error {}",
        outcome.err().unwrap()
    );

    Ok(())
}

/*
- If the `attached_deposit` is equal to the price + fees
    -  NFT is transferred to the buyer
    -  the sale is removed from the list of sales
    -  ft transferred to the previous owner
    -  protocol, royalty and origin fees are paid
    -  royalty paid from seller side
    -  previous bids refunded
- If the `attached_deposit` is not equal to the price + fees
  - a new bid should be added
  - if the number of stored bids exceeds `bid_history_length`, the earliest bid is removed and refunded (depricated)
*/
#[tokio::test]
async fn offer_positive() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;
    let market = init_market(&worker, worker.root_account().id(), vec![nft.id()]).await?;

    let user1 = owner
        .create_subaccount(&worker, "user1")
        .initial_balance(parse_near!("10 N"))
        .transact()
        .await?
        .unwrap();

    let user2 = owner
        .create_subaccount(&worker, "user2")
        .initial_balance(parse_near!("10 N"))
        .transact()
        .await?
        .unwrap();

    let series = create_series(&worker, nft.id(), &user1, owner.id()).await?;
    let token1 = mint_token(&worker, nft.id(), &user1, user1.id(), &series).await?;

    deposit(&worker, market.id(), &user1).await?;
    user2
        .call(&worker, market.id(), "bid_deposit")
        .args_json(json!({}))?
        .deposit(100_000_000)
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await?;

    let sale_conditions = HashMap::from([("near".parse().unwrap(), 10000.into())]);
    nft_approve(
        &worker,
        nft.id(),
        market.id(),
        &user1,
        &token1,
        &sale_conditions,
        &series,
    )
    .await?;

    // Check if bids can be added
    let token2 = mint_token(&worker, nft.id(), &user1, user1.id(), &series).await?;
    let sale_conditions = HashMap::from([("near".parse().unwrap(), 10000.into())]);
    nft_approve(
        &worker,
        nft.id(),
        market.id(),
        &user1,
        &token2,
        &sale_conditions,
        &series,
    )
    .await?;
    let initial_price = 100;
    user2
        .call(&worker, market.id(), "offer")
        .args_json(json!({
            "nft_contract_id": nft.id(),
            "token_id": token2,
            "ft_token_id": "near",
            "offered_price": initial_price.to_string(),
        }))?
        .deposit(1)
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await?;
    let bids_by_owner: Vec<BidId> = market
        .view(
            &worker,
            "get_bids_id_by_account",
            json!({
                "owner_id": user2.id().to_string(),
            })
            .to_string()
            .into_bytes(),
        )
        .await?
        .json()?;
    assert!(bids_by_owner.len() == 1, "Bid not added");

    // check that the buyer can buyout the token
    let price: U128 = market
        .view(
            &worker,
            "price_with_fees",
            json!({
                "price": sale_conditions.get("near").unwrap(),
            })
            .to_string()
            .into_bytes(),
        )
        .await?
        .json()?;

    let before_sell: Option<SaleJson> = market
        .view(
            &worker,
            "get_sale",
            json!({
               "nft_contract_id": nft.id(),
               "token_id": token1
            })
            .to_string()
            .into_bytes(),
        )
        .await?
        .json()?;
    user2
        .call(&worker, market.id(), "offer")
        .args_json(json!({
            "nft_contract_id": nft.id(),
            "token_id": token1,
            "ft_token_id": "near",
            "offered_price": price,
        }))?
        .deposit(1)
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await?;

    let token: Token = nft
        .view(
            &worker,
            "nft_token",
            json!({ "token_id": token1 }).to_string().into_bytes(),
        )
        .await?
        .json()?;

    // NFT is transferred to the buyer
    assert_eq!(token.owner_id.as_str(), user2.id().as_ref());
    // the sale is removed from the list of sales
    let after_sell: Option<SaleJson> = market
        .view(
            &worker,
            "get_sale",
            json!({
               "nft_contract_id": nft.id(),
               "token_id": token1
            })
            .to_string()
            .into_bytes(),
        )
        .await?
        .json()?;

    assert!(
        after_sell.is_none(),
        "Sale is still active, when it shouldn't"
    );
    assert!(
        before_sell.is_some(),
        "Sale is not active, when it should be"
    );

    // Depricated because now all bids are kept, BID_HISTORY_LENGTH_DEFAULT was depricated
    // let first_bid = bids.get(0).unwrap();
    // // Earliest bid should be removed
    // for i in 1..=BID_HISTORY_LENGTH_DEFAULT {
    //     user2
    //         .call(&worker, market.id(), "offer")
    //         .args_json(json!({
    //             "nft_contract_id": nft.id(),
    //             "token_id": token2,
    //             "ft_token_id": "near",
    //             "offered_price": initial_price * (i + 1) as u128,
    //         }))?
    //         .deposit(1)
    //         .gas(parse_gas!("300 Tgas") as u64)
    //         .transact()
    //         .await?;
    //     let sale_json: SaleJson = market
    //         .view(
    //             &worker,
    //             "get_sale",
    //             json!({
    //                "nft_contract_id": nft.id(),
    //                "token_id": token2
    //             })
    //             .to_string()
    //             .into_bytes(),
    //         )
    //         .await?
    //         .json()?;
    //     let bids = sale_json
    //         .bids
    //         .get(&AccountId::new_unchecked("near".to_string()))
    //         .unwrap();
    //     if i < BID_HISTORY_LENGTH_DEFAULT {
    //         assert_eq!(bids.get(0).unwrap(), first_bid);
    //     }
    // }
    // // new bid removed last bid
    // let sale_json: SaleJson = market
    //     .view(
    //         &worker,
    //         "get_sale",
    //         json!({
    //            "nft_contract_id": nft.id(),
    //            "token_id": token2
    //         })
    //         .to_string()
    //         .into_bytes(),
    //     )
    //     .await?
    //     .json()?;
    // let bids = sale_json
    //     .bids
    //     .get(&AccountId::new_unchecked("near".to_string()))
    //     .unwrap();

    // assert_ne!(bids.get(0).unwrap(), first_bid);
    Ok(())
}

#[tokio::test]
async fn accept_bid_negative() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;
    let market = init_market(&worker, worker.root_account().id(), vec![nft.id()]).await?;

    let user1 = owner
        .create_subaccount(&worker, "user1")
        .initial_balance(parse_near!("10 N"))
        .transact()
        .await?
        .unwrap();

    let user2 = owner
        .create_subaccount(&worker, "user2")
        .initial_balance(parse_near!("10 N"))
        .transact()
        .await?
        .unwrap();
    let series = create_series_raw(
        &worker,
        nft.id(),
        &user1,
        Some(4),
        HashMap::from([(user1.id(), 500)]),
    )
    .await?;
    let token1 = mint_token(&worker, nft.id(), &user1, user1.id(), &series).await?;
    let token2 = mint_token(&worker, nft.id(), &user1, user1.id(), &series).await?;
    deposit(&worker, market.id(), &user1).await?;

    // No sale with the given `nft_contract_id` and `token_id`
    let outcome = user1
        .call(&worker, market.id(), "accept_bid")
        .args_json(json!({
            "nft_contract_id": nft.id(),
            "token_id": token1,
            "ft_token_id": "near",
        }))?
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await;
    outcome.assert_err("No sale").unwrap();

    // no bids with given fungible token
    let sale_conditions = HashMap::from([("near".to_string(), U128(42000))]);
    let since_the_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    let waiting_time = Duration::from_secs(10);
    let epoch_plus_waiting_time = (since_the_epoch + waiting_time).as_nanos();

    let msg = json!({"Sale": {
        "sale_conditions": &sale_conditions,
        "token_type": Some(series.clone()),
        "start": Some(U64(epoch_plus_waiting_time as u64)),
    }});
    user1
        .call(&worker, nft.id(), "nft_approve")
        .args_json(json!({
            "token_id": token1,
            "account_id": market.id(),
            "msg": msg.to_string()
        }))?
        .deposit(parse_near!("1 N"))
        .gas(parse_gas!("200 Tgas") as u64)
        .transact()
        .await?;

    let msg = json!({"Sale": {
        "sale_conditions": &sale_conditions,
        "token_type": Some(series),
    }});
    user1
        .call(&worker, nft.id(), "nft_approve")
        .args_json(json!({
            "token_id": token2,
            "account_id": market.id(),
            "msg": msg.to_string()
        }))?
        .deposit(parse_near!("1 N"))
        .gas(parse_gas!("200 Tgas") as u64)
        .transact()
        .await?;

    let outcome = user1
        .call(&worker, market.id(), "accept_bid")
        .args_json(json!({
            "nft_contract_id": nft.id(),
            "token_id": token1,
            "ft_token_id": "near",
        }))?
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await;
    outcome
        .assert_err("Either the sale is finished or it hasn't started yet")
        .unwrap();

    // no bids
    let outcome = user1
        .call(&worker, market.id(), "accept_bid")
        .args_json(json!({
            "nft_contract_id": nft.id(),
            "token_id": token2,
            "ft_token_id": "near",
        }))?
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await;
    outcome
        .assert_err("No bids for this contract and token id")
        .unwrap();

    // wrong ft token
    user2
        .call(&worker, market.id(), "offer")
        .args_json(json!({
            "nft_contract_id": nft.id(),
            "token_id": token2,
            "ft_token_id": "near",
            "offered_price": "200",
        }))?
        .deposit(1)
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await?;
    let outcome = user1
        .call(&worker, market.id(), "accept_bid")
        .args_json(json!({
            "nft_contract_id": nft.id(),
            "token_id": token2,
            "ft_token_id": "not_near",
        }))?
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await;
    outcome.assert_err("No token").unwrap();

    // there is no valid bids
    let outcome = user1
        .call(&worker, market.id(), "accept_bid")
        .args_json(json!({
            "nft_contract_id": nft.id(),
            "token_id": token2,
            "ft_token_id": "near",
        }))?
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await;
    outcome
        .assert_err("There are no active non-finished bids")
        .unwrap();
    Ok(())
}

// - Nft transfered to the buyer
#[tokio::test]
async fn accept_bid_positive() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;
    let market = init_market(&worker, worker.root_account().id(), vec![nft.id()]).await?;

    let user1 = owner
        .create_subaccount(&worker, "user1")
        .initial_balance(parse_near!("10 N"))
        .transact()
        .await?
        .unwrap();

    let user2 = owner
        .create_subaccount(&worker, "user2")
        .initial_balance(parse_near!("10 N"))
        .transact()
        .await?
        .unwrap();
    let series = create_series_raw(
        &worker,
        nft.id(),
        &user1,
        Some(4),
        HashMap::from([(user1.id(), 500)]),
    )
    .await?;
    let token1 = mint_token(&worker, nft.id(), &user1, user1.id(), &series).await?;
    deposit(&worker, market.id(), &user1).await?;
    user2
        .call(&worker, market.id(), "bid_deposit")
        .args_json(json!({}))?
        .deposit(10000)
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await?;
    let sale_conditions = HashMap::from([("near".parse().unwrap(), 42000.into())]);
    nft_approve(
        &worker,
        nft.id(),
        market.id(),
        &user1,
        &token1,
        &sale_conditions,
        &series,
    )
    .await?;
    user2
        .call(&worker, market.id(), "offer")
        .args_json(json!({
            "nft_contract_id": nft.id(),
            "token_id": token1,
            "ft_token_id": "near",
            "offered_price": "200",
        }))?
        .deposit(1)
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await?;
    user1
        .call(&worker, market.id(), "accept_bid")
        .args_json(json!({
            "nft_contract_id": nft.id(),
            "token_id": token1,
            "ft_token_id": "near",
        }))?
        .gas(parse_gas!("300 Tgas") as u64)
        .transact()
        .await?;
    let token_data: Token = nft
        .view(
            &worker,
            "nft_token",
            json!({ "token_id": token1 }).to_string().into_bytes(),
        )
        .await?
        .json()?;
    assert_eq!(token_data.owner_id.as_ref(), user2.id().as_ref());
    Ok(())
}

/*
- Should panic unless 1 yoctoNEAR is attached
- Should panic if there is no sale with the given `nft_contract_id` and `token_id`
- Should panic unless it is called by the creator of the sale
- Should panic if `ft_token_id` is not supported
*/
#[tokio::test]
async fn update_price_negative() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;
    let market = init_market(&worker, worker.root_account().id(), vec![nft.id()]).await?;

    let user1 = owner
        .create_subaccount(&worker, "user1")
        .initial_balance(parse_near!("10 N"))
        .transact()
        .await?
        .unwrap();

    let user2 = owner
        .create_subaccount(&worker, "user2")
        .initial_balance(parse_near!("10 N"))
        .transact()
        .await?
        .unwrap();
    let series = create_series_raw(
        &worker,
        nft.id(),
        &user1,
        Some(4),
        HashMap::from([(user1.id(), 500)]),
    )
    .await?;
    let token1 = mint_token(&worker, nft.id(), &user1, user1.id(), &series).await?;
    deposit(&worker, market.id(), &user1).await?;
    let sale_conditions = HashMap::from([("near".parse().unwrap(), 42000.into())]);
    nft_approve(
        &worker,
        nft.id(),
        market.id(),
        &user1,
        &token1,
        &sale_conditions,
        &series,
    )
    .await?;

    // not attaching 1 yocto
    let outcome = user1
        .call(&worker, market.id(), "update_price")
        .args_json(json!({
            "nft_contract_id": nft.id(),
            "token_id": token1,
            "ft_token_id": "near",
            "price": "10000",
        }))?
        .transact()
        .await;
    outcome
        .assert_err("Requires attached deposit of exactly 1 yoctoNEAR")
        .unwrap();

    // no sale with given nft_contract_id:token_id
    let outcome = user1
        .call(&worker, market.id(), "update_price")
        .args_json(json!({
            "nft_contract_id": market.id(),
            "token_id": token1,
            "ft_token_id": "near",
            "price": "10000",
        }))?
        .deposit(1)
        .transact()
        .await;
    outcome.assert_err("No sale").unwrap();

    // called not by the owner
    let outcome = user2
        .call(&worker, market.id(), "update_price")
        .args_json(json!({
            "nft_contract_id": nft.id(),
            "token_id": token1,
            "ft_token_id": "near",
            "price": "10000",
        }))?
        .deposit(1)
        .transact()
        .await;
    outcome.assert_err("Must be sale owner").unwrap();

    // ft must be supported
    let outcome = user1
        .call(&worker, market.id(), "update_price")
        .args_json(json!({
            "nft_contract_id": nft.id(),
            "token_id": token1,
            "ft_token_id": "nearcoin",
            "price": "10000",
        }))?
        .deposit(1)
        .transact()
        .await;
    outcome
        .assert_err("is not supported by this market")
        .unwrap();
    Ok(())
}

// Changes the price
#[tokio::test]
async fn update_price_positive() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;
    let market = init_market(&worker, worker.root_account().id(), vec![nft.id()]).await?;

    let user1 = owner
        .create_subaccount(&worker, "user1")
        .initial_balance(parse_near!("10 N"))
        .transact()
        .await?
        .unwrap();

    let series = create_series_raw(
        &worker,
        nft.id(),
        &user1,
        Some(4),
        HashMap::from([(user1.id(), 500)]),
    )
    .await?;
    let token1 = mint_token(&worker, nft.id(), &user1, user1.id(), &series).await?;
    deposit(&worker, market.id(), &user1).await?;
    let sale_conditions = HashMap::from([("near".parse().unwrap(), 42000.into())]);
    nft_approve(
        &worker,
        nft.id(),
        market.id(),
        &user1,
        &token1,
        &sale_conditions,
        &series,
    )
    .await?;
    user1
        .call(&worker, market.id(), "update_price")
        .args_json(json!({
            "nft_contract_id": nft.id(),
            "token_id": token1,
            "ft_token_id": "near",
            "price": "10000",
        }))?
        .deposit(1)
        .transact()
        .await?;

    let sale_json: SaleJson = market
        .view(
            &worker,
            "get_sale",
            json!({
               "nft_contract_id": nft.id(),
               "token_id": token1
            })
            .to_string()
            .into_bytes(),
        )
        .await?
        .json()?;
    assert_eq!(
        sale_json.sale_conditions.get(&"near".parse().unwrap()),
        Some(&U128(10000))
    );
    Ok(())
}

/*
- Should panic unless 1 yoctoNEAR is attached
- If the sale in progress, only the sale creator can remove the sale
 */
#[tokio::test]
async fn remove_sale_negative() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;
    let market = init_market(&worker, worker.root_account().id(), vec![nft.id()]).await?;

    let user1 = owner
        .create_subaccount(&worker, "user1")
        .initial_balance(parse_near!("10 N"))
        .transact()
        .await?
        .unwrap();

    let user2 = owner
        .create_subaccount(&worker, "user2")
        .initial_balance(parse_near!("10 N"))
        .transact()
        .await?
        .unwrap();

    let series = create_series_raw(
        &worker,
        nft.id(),
        &user1,
        Some(4),
        HashMap::from([(user1.id(), 500)]),
    )
    .await?;
    let token1 = mint_token(&worker, nft.id(), &user1, user1.id(), &series).await?;
    deposit(&worker, market.id(), &user1).await?;
    let sale_conditions = HashMap::from([("near".parse().unwrap(), 42000.into())]);
    nft_approve(
        &worker,
        nft.id(),
        market.id(),
        &user1,
        &token1,
        &sale_conditions,
        &series,
    )
    .await?;

    // 1 yocto is needed
    let outcome = user1
        .call(&worker, market.id(), "remove_sale")
        .args_json(json!({
            "nft_contract_id": nft.id(),
            "token_id": token1
        }))?
        .transact()
        .await;
    outcome
        .assert_err("Requires attached deposit of exactly 1 yoctoNEAR")
        .unwrap();

    // Can be removed only by the owner of the sale, if not finished
    let outcome = user2
        .call(&worker, market.id(), "remove_sale")
        .args_json(json!({
            "nft_contract_id": nft.id(),
            "token_id": token1
        }))?
        .deposit(1)
        .transact()
        .await;
    outcome
        .assert_err("Until the sale is finished, it can only be removed by the sale owner")
        .unwrap();
    Ok(())
}

/*
- Sale removed
- Refunds all bids
*/
#[tokio::test]
async fn remove_sale_positive() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;
    let market = init_market(&worker, worker.root_account().id(), vec![nft.id()]).await?;

    let user1 = owner
        .create_subaccount(&worker, "user1")
        .initial_balance(parse_near!("10 N"))
        .transact()
        .await?
        .unwrap();

    let user2 = owner
        .create_subaccount(&worker, "user2")
        .initial_balance(parse_near!("10 N"))
        .transact()
        .await?
        .unwrap();

    let series = create_series_raw(
        &worker,
        nft.id(),
        &user1,
        Some(4),
        HashMap::from([(user1.id(), 500)]),
    )
    .await?;
    let token1 = mint_token(&worker, nft.id(), &user1, user1.id(), &series).await?;
    deposit(&worker, market.id(), &user1).await?;
    let sale_conditions = HashMap::from([("near".parse().unwrap(), 42000.into())]);
    nft_approve(
        &worker,
        nft.id(),
        market.id(),
        &user1,
        &token1,
        &sale_conditions,
        &series,
    )
    .await?;
    offer(&worker, nft.id(), market.id(), &user2, &token1, 4000.into()).await?;
    user1
        .call(&worker, market.id(), "remove_sale")
        .args_json(json!({
            "nft_contract_id": nft.id(),
            "token_id": token1
        }))?
        .deposit(1)
        .transact()
        .await?;
    let sale_json: Option<SaleJson> = market
        .view(
            &worker,
            "get_sale",
            json!({
               "nft_contract_id": nft.id(),
               "token_id": token1
            })
            .to_string()
            .into_bytes(),
        )
        .await?
        .json()?;
    assert!(sale_json.is_none());
    Ok(())
}
