// SPDX-License-Identifier: MIT OR Apache-2.0

//! # Tests for Block Methods

#![allow(unused_imports)]
#![cfg(all(feature = "blocking", feature = "async", feature = "tokio"))]

use std::str::FromStr;

use bitcoin::hashes::Hash;
use bitcoin::Amount;
use bitcoin::BlockHash;

use esplora_client::BlockStatus;

use testenv::TestEnv;

mod testenv;

#[tokio::test]
async fn test_get_header_by_hash() {
    let env = TestEnv::new();
    let (blocking_client, async_client) = env.setup_clients();

    let block_hash = env
        .bitcoind_client()
        .get_block_hash(23)
        .unwrap()
        .block_hash()
        .unwrap();

    let block_header = blocking_client.get_header_by_hash(&block_hash).unwrap();
    let block_header_async = async_client.get_header_by_hash(&block_hash).await.unwrap();
    assert_eq!(block_header, block_header_async);
}

#[tokio::test]
async fn test_get_block_status() {
    let env = TestEnv::new();
    let (blocking_client, async_client) = env.setup_clients();

    let block_hash = env
        .bitcoind_client()
        .get_block_hash(21)
        .unwrap()
        .block_hash()
        .unwrap();
    let next_block_hash = env
        .bitcoind_client()
        .get_block_hash(22)
        .unwrap()
        .block_hash()
        .unwrap();

    let expected = BlockStatus {
        in_best_chain: true,
        height: Some(21),
        next_best: Some(next_block_hash),
    };

    let block_status = blocking_client.get_block_status(&block_hash).unwrap();
    let block_status_async = async_client.get_block_status(&block_hash).await.unwrap();
    assert_eq!(expected, block_status);
    assert_eq!(expected, block_status_async);
}

#[tokio::test]
async fn test_get_non_existing_block_status() {
    // Esplora returns the same status for orphaned blocks as for non-existing
    // blocks: non-existing: https://blockstream.info/api/block/0000000000000000000000000000000000000000000000000000000000000000/status
    // orphaned: https://blockstream.info/api/block/000000000000000000181b1a2354620f66868a723c0c4d5b24e4be8bdfc35a7f/status
    // (Here the block is cited as orphaned: https://bitcoinchain.com/block_explorer/block/000000000000000000181b1a2354620f66868a723c0c4d5b24e4be8bdfc35a7f/ )
    // For this reason, we only test for the non-existing case here.

    let env = TestEnv::new();
    let (blocking_client, async_client) = env.setup_clients();

    let block_hash = BlockHash::all_zeros();

    let expected = BlockStatus {
        in_best_chain: false,
        height: None,
        next_best: None,
    };

    let block_status = blocking_client.get_block_status(&block_hash).unwrap();
    let block_status_async = async_client.get_block_status(&block_hash).await.unwrap();
    assert_eq!(expected, block_status);
    assert_eq!(expected, block_status_async);
}

#[tokio::test]
async fn test_get_block_by_hash() {
    let env = TestEnv::new();
    let (blocking_client, async_client) = env.setup_clients();

    let block_hash = env
        .bitcoind_client()
        .get_block_hash(21)
        .unwrap()
        .block_hash()
        .unwrap();

    let expected = Some(env.bitcoind_client().get_block(block_hash).unwrap());

    let block = blocking_client.get_block_by_hash(&block_hash).unwrap();
    let block_async = async_client.get_block_by_hash(&block_hash).await.unwrap();
    assert_eq!(expected, block);
    assert_eq!(expected, block_async);
}

#[tokio::test]
async fn test_get_block_by_hash_not_existing() {
    let env = TestEnv::new();
    let (blocking_client, async_client) = env.setup_clients();

    let block = blocking_client
        .get_block_by_hash(&BlockHash::all_zeros())
        .unwrap();
    let block_async = async_client
        .get_block_by_hash(&BlockHash::all_zeros())
        .await
        .unwrap();
    assert!(block.is_none());
    assert!(block_async.is_none());
}

#[tokio::test]
async fn test_get_merkle_block() {
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

    let merkle_block = blocking_client.get_merkle_block(&txid).unwrap().unwrap();
    let merkle_block_async = async_client.get_merkle_block(&txid).await.unwrap().unwrap();
    assert_eq!(merkle_block, merkle_block_async);

    let mut matches = vec![txid];
    let mut indexes = vec![];
    let root = merkle_block
        .txn
        .extract_matches(&mut matches, &mut indexes)
        .unwrap();
    assert_eq!(root, merkle_block.header.merkle_root);
    assert_eq!(indexes.len(), 1);
    assert!(indexes[0] > 0);
}

#[tokio::test]
async fn test_get_block_txids() {
    let env = TestEnv::new();
    let (blocking_client, async_client) = env.setup_clients();

    let address = env.get_legacy_address();

    // Create 5 transactions and mine a block.
    let txids: Vec<_> = (0..5)
        .map(|_| {
            env.bitcoind_client()
                .send_to_address(&address, Amount::from_sat(1000))
                .unwrap()
                .txid()
                .unwrap()
        })
        .collect();
    env.mine_and_wait(1);

    // Get the block hash at the chain's tip.
    let blockhash = blocking_client.get_tip_hash().unwrap();

    let txids_async = async_client.get_block_txids(&blockhash).await.unwrap();
    let txids_blocking = blocking_client.get_block_txids(&blockhash).unwrap();

    assert_eq!(txids_async, txids_blocking);

    // Compare expected and received (skipping the coinbase TXID).
    for expected_txid in txids.iter() {
        assert!(txids_async.contains(expected_txid));
    }
}

#[tokio::test]
async fn test_get_block_txs() {
    let env = TestEnv::new();
    let (blocking_client, async_client) = env.setup_clients();

    let blockhash = blocking_client.get_tip_hash().unwrap();

    let txs_blocking = blocking_client.get_block_txs(&blockhash, None).unwrap();
    let txs_async = async_client.get_block_txs(&blockhash, None).await.unwrap();

    assert_ne!(txs_blocking.len(), 0);
    assert_eq!(txs_blocking.len(), txs_async.len());
}

#[allow(deprecated)]
#[tokio::test]
async fn test_get_blocks() {
    let env = TestEnv::new();
    let (blocking_client, async_client) = env.setup_clients();

    let start_height = env.bitcoind_client().get_block_count().unwrap().0;
    let blocks1 = blocking_client.get_blocks(None).unwrap();
    let blocks_async1 = async_client.get_blocks(None).await.unwrap();
    assert_eq!(blocks1[0].time.height, start_height as u32);
    assert_eq!(blocks1, blocks_async1);
    env.mine_and_wait(1);

    let blocks2 = blocking_client.get_blocks(None).unwrap();
    let blocks_async2 = async_client.get_blocks(None).await.unwrap();
    assert_eq!(blocks2, blocks_async2);
    assert_ne!(blocks2, blocks1);

    let blocks3 = blocking_client
        .get_blocks(Some(start_height as u32))
        .unwrap();
    let blocks_async3 = async_client
        .get_blocks(Some(start_height as u32))
        .await
        .unwrap();
    assert_eq!(blocks3, blocks_async3);
    assert_eq!(blocks3[0].time.height, start_height as u32);
    assert_eq!(blocks3, blocks1);

    let blocks_genesis = blocking_client.get_blocks(Some(0)).unwrap();
    let blocks_genesis_async = async_client.get_blocks(Some(0)).await.unwrap();
    assert_eq!(blocks_genesis, blocks_genesis_async);
}

#[tokio::test]
async fn test_get_block_info() {
    let env = TestEnv::new();
    let (blocking_client, async_client) = env.setup_clients();

    // Genesis block `BlockHash` on regtest.
    let blockhash_genesis =
        BlockHash::from_str("0f9188f13cb7b2c71f2a335e3a4fc328bf5beb436012afca590b1a11466e2206")
            .unwrap();

    let block_info_blocking = blocking_client.get_block_info(&blockhash_genesis).unwrap();
    let block_info_async = async_client
        .get_block_info(&blockhash_genesis)
        .await
        .unwrap();

    assert_eq!(block_info_async, block_info_blocking);
    assert_eq!(block_info_async.id, blockhash_genesis);
    assert_eq!(block_info_async.height, 0);
    assert_eq!(block_info_async.previousblockhash, None);
}

#[tokio::test]
async fn test_get_txid_at_block_index() {
    let env = TestEnv::new();
    let (blocking_client, async_client) = env.setup_clients();

    let block_hash = env
        .bitcoind_client()
        .get_block_hash(23)
        .unwrap()
        .block_hash()
        .unwrap();

    let txid_at_block_index = blocking_client
        .get_txid_at_block_index(&block_hash, 0)
        .unwrap()
        .unwrap();
    let txid_at_block_index_async = async_client
        .get_txid_at_block_index(&block_hash, 0)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(txid_at_block_index, txid_at_block_index_async);
}
