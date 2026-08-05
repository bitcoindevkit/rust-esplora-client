// SPDX-License-Identifier: MIT OR Apache-2.0

//! # Tests for Mempool Methods

#![allow(unused_imports)]
#![cfg(all(feature = "blocking", feature = "async", feature = "tokio"))]

use std::collections::HashMap;

use bitcoin::Amount;
use bitcoin::FeeRate;
use esplora_client::convert_fee_rate;
use esplora_client::sat_per_vbyte_to_feerate;

use testenv::TestEnv;

mod testenv;

#[test]
fn test_feerate_parsing() {
    let esplora_fees_raw = serde_json::from_str::<HashMap<u16, f64>>(
        r#"{
    "1": 1.952,
    "2": 1.952,
    "3": 1.199,
    "4": 1.013,
    "5": 1.013,
    "6": 1.013,
    "7": 1.013,
    "8": 1.013,
    "9": 1.013,
    "10": 1.013,
    "11": 1.013,
    "12": 1.013,
    "13": 0.748,
    "14": 0.748,
    "15": 0.748,
    "16": 0.748,
    "17": 0.748,
    "18": 0.748,
    "19": 0.748,
    "20": 0.748,
    "21": 0.748,
    "22": 0.748,
    "23": 0.748,
    "24": 0.748,
    "25": 0.748,
    "144": 0.693,
    "504": 0.693,
    "1008": 0.693
}"#,
    )
    .unwrap();

    // Convert fees from sat/vB (`f64`) to `FeeRate`.
    // Note that `get_fee_estimates` already returns `HashMap<u16, FeeRate>`.
    let esplora_fees = sat_per_vbyte_to_feerate(esplora_fees_raw);

    assert!(convert_fee_rate(1, HashMap::new()).is_none());
    assert_eq!(
        convert_fee_rate(6, esplora_fees.clone()),
        Some(FeeRate::from_sat_per_kwu((1.013_f64 * 250.0).round() as u64)),
        "should inherit from value for target=6"
    );
    assert_eq!(
        convert_fee_rate(26, esplora_fees.clone()),
        Some(FeeRate::from_sat_per_kwu((0.748_f64 * 250.0).round() as u64)),
        "should inherit from value for target=25"
    );
    assert!(
        convert_fee_rate(0, esplora_fees).is_none(),
        "should not return feerate for target=0"
    );
}

#[tokio::test]
async fn test_get_fee_estimates() {
    let env = TestEnv::new();
    let (blocking_client, async_client) = env.setup_clients();

    let fee_estimates = blocking_client.get_fee_estimates().unwrap();
    let fee_estimates_async = async_client.get_fee_estimates().await.unwrap();
    assert_eq!(fee_estimates.len(), fee_estimates_async.len());
}

#[tokio::test]
async fn test_mempool_methods() {
    let env = TestEnv::new();
    let (blocking_client, async_client) = env.setup_clients();

    let address = env.get_legacy_address();
    for _ in 0..5 {
        let _txid = env
            .bitcoind_client()
            .send_to_address(&address, Amount::from_sat(1000))
            .unwrap()
            .txid()
            .unwrap();
    }

    // Wait for transactions to propagate to electrs' mempool.
    tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

    // Test `get_mempool_stats`
    let stats_blocking = blocking_client.get_mempool_stats().unwrap();
    let stats_async = async_client.get_mempool_stats().await.unwrap();
    assert_eq!(stats_blocking, stats_async);
    assert!(stats_blocking.count >= 5);

    // Test `get_mempool_recent_txs`
    let recent_blocking = blocking_client.get_mempool_recent_txs().unwrap();
    let recent_async = async_client.get_mempool_recent_txs().await.unwrap();
    assert_eq!(recent_blocking, recent_async);
    assert!(recent_blocking.len() <= 10);
    assert!(!recent_blocking.is_empty());

    // Test `get_mempool_txids`
    let txids_blocking = blocking_client.get_mempool_txids().unwrap();
    let txids_async = async_client.get_mempool_txids().await.unwrap();
    assert_eq!(txids_blocking, txids_async);
    assert!(txids_blocking.len() >= 5);

    // Test `get_mempool_scripthash_txs`
    let script = address.script_pubkey();
    let scripthash_txs_blocking = blocking_client.get_mempool_scripthash_txs(&script).unwrap();
    let scripthash_txs_async = async_client
        .get_mempool_scripthash_txs(&script)
        .await
        .unwrap();
    assert_eq!(scripthash_txs_blocking, scripthash_txs_async);
    assert_eq!(scripthash_txs_blocking.len(), 5);

    // Test `get_mempool_address_txs`
    let mempool_address_txs_blocking = blocking_client.get_mempool_address_txs(&address).unwrap();
    let mempool_address_txs_async = async_client
        .get_mempool_address_txs(&address)
        .await
        .unwrap();
    assert_eq!(mempool_address_txs_blocking, mempool_address_txs_async);
    assert_eq!(mempool_address_txs_blocking.len(), 5);
}
