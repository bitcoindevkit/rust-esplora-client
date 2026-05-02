// SPDX-License-Identifier: MIT OR Apache-2.0

//! # Tests for Address Methods

#![allow(unused_imports)]
#![cfg(all(feature = "blocking", feature = "async", feature = "tokio"))]

use bitcoin::{opcodes::all, Amount};

use testenv::TestEnv;

mod testenv;

#[tokio::test]
async fn test_get_address_stats() {
    let env = TestEnv::new();
    let (blocking_client, async_client) = env.setup_clients();

    let address = env.get_legacy_address();
    let _txid = env
        .bitcoind_client()
        .send_to_address(&address, Amount::from_sat(1000))
        .unwrap()
        .txid()
        .unwrap();

    let address_stats_blocking = blocking_client.get_address_stats(&address).unwrap();
    let address_stats_async = async_client.get_address_stats(&address).await.unwrap();
    assert_eq!(address_stats_blocking, address_stats_async);
    assert_eq!(address_stats_async.chain_stats.funded_txo_count, 0);

    env.mine_and_wait(1);

    let address_stats_blocking = blocking_client.get_address_stats(&address).unwrap();
    let address_stats_async = async_client.get_address_stats(&address).await.unwrap();
    assert_eq!(address_stats_blocking, address_stats_async);
    assert_eq!(address_stats_async.chain_stats.funded_txo_count, 1);
    assert_eq!(
        address_stats_async.chain_stats.funded_txo_sum,
        Amount::from_sat(1000)
    );
}

#[tokio::test]
async fn test_get_address_txs() {
    let env = TestEnv::new();
    let (blocking_client, async_client) = env.setup_clients();

    let address = env.get_legacy_address();
    let txid = env
        .bitcoind_client()
        .send_to_address(&address, Amount::from_sat(1000))
        .unwrap()
        .txid()
        .unwrap();
    env.mine_and_wait(1);

    let address_txs_blocking = blocking_client.get_address_txs(&address, None).unwrap();
    let address_txs_async = async_client.get_address_txs(&address, None).await.unwrap();

    assert_eq!(address_txs_blocking, address_txs_async);
    assert_eq!(address_txs_async[0].txid, txid);
}

#[tokio::test]
async fn test_get_address_utxos() {
    let env = TestEnv::new();
    let (blocking_client, async_client) = env.setup_clients();

    let address = env.get_legacy_address();
    let _txid = env
        .bitcoind_client()
        .send_to_address(&address, Amount::from_sat(21000))
        .unwrap()
        .txid()
        .unwrap();

    env.mine_and_wait(1);

    let address_utxos_blocking = blocking_client.get_address_utxos(&address).unwrap();
    let address_utxos_async = async_client.get_address_utxos(&address).await.unwrap();

    assert_ne!(address_utxos_blocking.len(), 0);
    assert_ne!(address_utxos_async.len(), 0);
    assert_eq!(address_utxos_blocking, address_utxos_async);
}
