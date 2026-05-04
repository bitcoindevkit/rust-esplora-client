// SPDX-License-Identifier: MIT OR Apache-2.0

//! # Tests for ScriptHash Methods

#![allow(unused_imports)]
#![cfg(all(feature = "blocking", feature = "async", feature = "tokio"))]

use bitcoin::Amount;
use bitcoin::Txid;

use testenv::TestEnv;

mod testenv;

#[tokio::test]
async fn test_get_scripthash_utxos() {
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

    let script = address.script_pubkey();
    let scripthash_utxos_blocking = blocking_client.get_scripthash_utxos(&script).unwrap();
    let scripthash_utxos_async = async_client.get_scripthash_utxos(&script).await.unwrap();

    assert_ne!(scripthash_utxos_blocking.len(), 0);
    assert_ne!(scripthash_utxos_async.len(), 0);
    assert_eq!(scripthash_utxos_blocking, scripthash_utxos_async);
}

#[tokio::test]
async fn test_get_scripthash_txs() {
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

    let expected_tx = env
        .bitcoind_client()
        .get_transaction(txid)
        .unwrap()
        .into_model()
        .unwrap()
        .tx;
    let script = &expected_tx.output[0].script_pubkey;
    let scripthash_txs_txids: Vec<Txid> = blocking_client
        .get_scripthash_txs(script, None)
        .unwrap()
        .iter()
        .map(|tx| tx.txid)
        .collect();
    let scripthash_txs_txids_async: Vec<Txid> = async_client
        .get_scripthash_txs(script, None)
        .await
        .unwrap()
        .iter()
        .map(|tx| tx.txid)
        .collect();
    assert_eq!(scripthash_txs_txids, scripthash_txs_txids_async);
}

#[tokio::test]
async fn test_get_scripthash_stats() {
    let env = TestEnv::new();
    let (blocking_client, async_client) = env.setup_clients();

    let address_legacy = env.get_legacy_address();
    let address_nested_segwit = env.get_nested_segwit_address();
    let address_bech32 = env.get_bech32_address();
    let address_bech32m = env.get_bech32m_address();

    // Send a transaction to each address.
    let _txid = env
        .bitcoind_client()
        .send_to_address(&address_legacy, Amount::from_sat(1000))
        .unwrap()
        .txid()
        .unwrap();
    let _txid = env
        .bitcoind_client()
        .send_to_address(&address_nested_segwit, Amount::from_sat(1000))
        .unwrap()
        .txid()
        .unwrap();
    let _txid = env
        .bitcoind_client()
        .send_to_address(&address_bech32, Amount::from_sat(1000))
        .unwrap()
        .txid()
        .unwrap();
    let _txid = env
        .bitcoind_client()
        .send_to_address(&address_bech32m, Amount::from_sat(1000))
        .unwrap()
        .txid()
        .unwrap();
    env.mine_and_wait(1);

    // Derive each addresses script.
    let script_legacy = address_legacy.script_pubkey();
    let script_nested_segwit = address_nested_segwit.script_pubkey();
    let script_bech32 = address_bech32.script_pubkey();
    let script_bech32m = address_bech32m.script_pubkey();

    // Legacy (P2PKH)
    let scripthash_stats_blocking_legacy = blocking_client
        .get_scripthash_stats(&script_legacy)
        .unwrap();
    let scripthash_stats_async_legacy = async_client
        .get_scripthash_stats(&script_legacy)
        .await
        .unwrap();
    assert_eq!(
        scripthash_stats_blocking_legacy,
        scripthash_stats_async_legacy
    );
    assert_eq!(
        scripthash_stats_blocking_legacy.chain_stats.funded_txo_sum,
        Amount::from_sat(1000)
    );
    assert_eq!(scripthash_stats_blocking_legacy.chain_stats.tx_count, 1);

    // Nested SegWit (P2SH-P2WSH)
    let scripthash_stats_blocking_p2sh_segwit = blocking_client
        .get_scripthash_stats(&script_nested_segwit)
        .unwrap();
    let scripthash_stats_async_p2sh_segwit = async_client
        .get_scripthash_stats(&script_nested_segwit)
        .await
        .unwrap();
    assert_eq!(
        scripthash_stats_blocking_p2sh_segwit,
        scripthash_stats_async_p2sh_segwit
    );
    assert_eq!(
        scripthash_stats_blocking_p2sh_segwit
            .chain_stats
            .funded_txo_sum,
        Amount::from_sat(1000)
    );
    assert_eq!(
        scripthash_stats_blocking_p2sh_segwit.chain_stats.tx_count,
        1
    );

    // Bech32 (P2WPKH / P2WSH)
    let scripthash_stats_blocking_bech32 = blocking_client
        .get_scripthash_stats(&script_bech32)
        .unwrap();
    let scripthash_stats_async_bech32 = async_client
        .get_scripthash_stats(&script_bech32)
        .await
        .unwrap();
    assert_eq!(
        scripthash_stats_blocking_bech32,
        scripthash_stats_async_bech32
    );
    assert_eq!(
        scripthash_stats_blocking_bech32.chain_stats.funded_txo_sum,
        Amount::from_sat(1000)
    );
    assert_eq!(scripthash_stats_blocking_bech32.chain_stats.tx_count, 1);

    // Bech32m (P2TR)
    let scripthash_stats_blocking_bech32m = blocking_client
        .get_scripthash_stats(&script_bech32m)
        .unwrap();
    let scripthash_stats_async_bech32m = async_client
        .get_scripthash_stats(&script_bech32m)
        .await
        .unwrap();
    assert_eq!(
        scripthash_stats_blocking_bech32m,
        scripthash_stats_async_bech32m
    );
    assert_eq!(
        scripthash_stats_blocking_bech32m.chain_stats.funded_txo_sum,
        Amount::from_sat(1000)
    );
    assert_eq!(scripthash_stats_blocking_bech32m.chain_stats.tx_count, 1);
}
