use crate::utils::{create_series, create_subaccount, init_nft, mint_token};
use anyhow::Result;
use nft_contract::common::U128;
use nft_contract::TokenSeriesJson;
use serde_json::json;
/*
- Panics if the series wasn't found
- Returns the series with given `token_series_id`
*/
#[tokio::test]
async fn series_views_nft_get_series() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;

    let user1 = create_subaccount(&worker, &owner, "user1").await?;

    let _series = create_series(&worker, nft.id(), &user1, owner.id()).await?;

    // Check that method fails in case of wrong `token_series_id`
    let outcome = nft
        .view(
            &worker,
            "nft_get_series",
            json!({ "token_series_id": "42".to_string() })
                .to_string()
                .into_bytes(),
        )
        .await;
    match outcome {
        Err(err) => {
            println!("{}", err);
        }
        Ok(_) => panic!("Expected failure"),
    };

    let token_series: TokenSeriesJson = nft
        .view(
            &worker,
            "nft_get_series",
            json!({ "token_series_id": "1".to_string() })
                .to_string()
                .into_bytes(),
        )
        .await?
        .json()?;

    assert_eq!(token_series.owner_id, "user1.test.near".parse().unwrap());
    assert_eq!(token_series.metadata.title, Some("some title".to_string()));
    assert_eq!(
        token_series.metadata.media,
        Some("ipfs://QmTqZsmhZLLbi8vxZwm21wjKRFRBUQFzMFtTiyh3DJ2CCz".to_string())
    );
    assert_eq!(token_series.metadata.copies, Some(10));

    Ok(())
}

/*
- Panics in case of incorrect `from_index` or `limit`
- Returns a vector of series
*/
#[tokio::test]
async fn series_views_nft_series() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;

    let user1 = create_subaccount(&worker, &owner, "user1").await?;
    let user2 = create_subaccount(&worker, &owner, "user2").await?;

    let _series1 = create_series(&worker, nft.id(), &user1, owner.id()).await?;

    // Check that method fails in case of wrong `from_index`
    let outcome = nft
        .view(
            &worker,
            "nft_series",
            json!({ "from_index": "42".to_string(), "limit": 43 })
                .to_string()
                .into_bytes(),
        )
        .await;
    match outcome {
        Err(err) => {
            println!("{}", err);
        }
        Ok(_) => panic!("Expected failure"),
    };

    // Check that method fails in case of wrong `limit`
    let outcome = nft
        .view(
            &worker,
            "nft_series",
            json!({ "from_index": "1".to_string(), "limit": 0 })
                .to_string()
                .into_bytes(),
        )
        .await;
    match outcome {
        Err(err) => {
            println!("{}", err);
        }
        Ok(_) => panic!("Expected failure"),
    };

    let vec_token_series: Vec<TokenSeriesJson> = nft
        .view(&worker, "nft_series", json!({}).to_string().into_bytes())
        .await?
        .json()?;
    assert_eq!(vec_token_series.len(), 1);
    let token = &vec_token_series[0];
    assert_eq!(token.owner_id, "user1.test.near".parse().unwrap());
    assert_eq!(token.metadata.title, Some("some title".to_string()));
    assert_eq!(
        token.metadata.media,
        Some("ipfs://QmTqZsmhZLLbi8vxZwm21wjKRFRBUQFzMFtTiyh3DJ2CCz".to_string())
    );
    assert_eq!(token.metadata.copies, Some(10));

    let _series2 = create_series(&worker, nft.id(), &user1, owner.id()).await?;
    let _series3 = create_series(&worker, nft.id(), &user2, owner.id()).await?;

    let vec_token_series: Vec<TokenSeriesJson> = nft
        .view(&worker, "nft_series", json!({}).to_string().into_bytes())
        .await?
        .json()?;
    assert_eq!(vec_token_series.len(), 3);
    Ok(())
}

/*
- Panics if the series wasn't found
- Returns the number of tokens in the series
*/
#[tokio::test]
async fn series_views_nft_supply_for_series() -> Result<()> {
    let worker = workspaces::sandbox().await?;
    let owner = worker.root_account();
    let nft = init_nft(&worker, owner.id()).await?;

    let user1 = create_subaccount(&worker, &owner, "user1").await?;

    let series = create_series(&worker, nft.id(), &user1, owner.id()).await?;

    // Check that method fails in case of wrong `token_series_id`
    let outcome = nft
        .view(
            &worker,
            "nft_supply_for_series",
            json!({ "token_series_id": "42".to_string() })
                .to_string()
                .into_bytes(),
        )
        .await;
    match outcome {
        Err(err) => {
            println!("{}", err);
        }
        Ok(_) => panic!("Expected failure"),
    };

    let supply: U128 = nft
        .view(
            &worker,
            "nft_supply_for_series",
            json!({ "token_series_id": "1".to_string() })
                .to_string()
                .into_bytes(),
        )
        .await?
        .json()?;
    assert_eq!(supply.0, 0);

    let _token1 = mint_token(&worker, nft.id(), &user1, user1.id(), &series).await?;

    let supply: U128 = nft
        .view(
            &worker,
            "nft_supply_for_series",
            json!({ "token_series_id": "1".to_string() })
                .to_string()
                .into_bytes(),
        )
        .await?
        .json()?;
    assert_eq!(supply.0, 1);

    let _token2 = mint_token(&worker, nft.id(), &user1, user1.id(), &series).await?;
    let _token3 = mint_token(&worker, nft.id(), &user1, user1.id(), &series).await?;
    let _token4 = mint_token(&worker, nft.id(), &user1, user1.id(), &series).await?;
    let _token5 = mint_token(&worker, nft.id(), &user1, user1.id(), &series).await?;

    let supply: U128 = nft
        .view(
            &worker,
            "nft_supply_for_series",
            json!({ "token_series_id": "1".to_string() })
                .to_string()
                .into_bytes(),
        )
        .await?
        .json()?;
    assert_eq!(supply.0, 5);

    Ok(())
}
