// SPDX-License-Identifier: MIT OR Apache-2.0

//! # Tests for Transaction Methods

#![allow(unused_imports)]
#![cfg(all(feature = "blocking", feature = "async", feature = "tokio"))]

use bitcoin::hashes::Hash;
use bitcoin::Amount;
use bitcoin::Transaction;
use bitcoin::Txid;
use electrsd::bitcoind::get_available_port;
use std::collections::HashMap;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;

use testenv::TestEnv;

mod testenv;

#[tokio::test]
async fn test_get_tx() {
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

    let tx = blocking_client.get_tx(&txid).unwrap();
    let tx_async = async_client.get_tx(&txid).await.unwrap();
    assert_eq!(tx, tx_async);
}

#[tokio::test]
async fn test_get_tx_no_opt() {
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

    let tx_no_opt = blocking_client.get_tx_no_opt(&txid).unwrap();
    let tx_no_opt_async = async_client.get_tx_no_opt(&txid).await.unwrap();
    assert_eq!(tx_no_opt, tx_no_opt_async);
}

#[tokio::test]
async fn test_get_tx_status() {
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

    let tx_status = blocking_client.get_tx_status(&txid).unwrap();
    let tx_status_async = async_client.get_tx_status(&txid).await.unwrap();
    assert_eq!(tx_status, tx_status_async);
    assert!(tx_status.confirmed);

    // Bogus txid returns a TxStatus with false, None, None, None
    let txid = Txid::hash(b"ayyyy lmao");
    let tx_status = blocking_client.get_tx_status(&txid).unwrap();
    let tx_status_async = async_client.get_tx_status(&txid).await.unwrap();
    assert_eq!(tx_status, tx_status_async);
    assert!(!tx_status.confirmed);
    assert!(tx_status.block_height.is_none());
    assert!(tx_status.block_hash.is_none());
    assert!(tx_status.block_time.is_none());
}

#[tokio::test]
async fn test_get_tx_info() {
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

    let tx_res = env
        .bitcoind_client()
        .get_transaction(txid)
        .unwrap()
        .into_model()
        .unwrap();
    let tx_exp: Transaction = tx_res.tx;
    let tx_block_height = env
        .bitcoind_client()
        .get_block_header_verbose(&tx_res.block_hash.unwrap())
        .unwrap()
        .into_model()
        .unwrap()
        .height;

    let tx_info = blocking_client
        .get_tx_info(&txid)
        .unwrap()
        .expect("must get tx");
    let tx_info_async = async_client
        .get_tx_info(&txid)
        .await
        .unwrap()
        .expect("must get tx");
    assert_eq!(tx_info, tx_info_async);
    assert_eq!(tx_info.txid, txid);
    assert_eq!(tx_info.to_tx(), tx_exp);
    assert_eq!(tx_info.size, tx_exp.total_size());
    assert_eq!(tx_info.weight, tx_exp.weight());
    assert_eq!(tx_info.fee, tx_res.fee.unwrap().unsigned_abs());
    assert!(tx_info.status.confirmed);
    assert_eq!(tx_info.status.block_height, Some(tx_block_height));
    assert_eq!(tx_info.status.block_hash, tx_res.block_hash);
    assert_eq!(
        tx_info.status.block_time,
        tx_res.block_time.map(|bt| bt as u64)
    );

    let txid = Txid::hash(b"not exist");
    assert_eq!(blocking_client.get_tx_info(&txid).unwrap(), None);
    assert_eq!(async_client.get_tx_info(&txid).await.unwrap(), None);
}

#[tokio::test]
async fn test_get_tx_merkle_proof() {
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

    let merkle_proof = blocking_client.get_merkle_proof(&txid).unwrap().unwrap();
    let merkle_proof_async = async_client.get_merkle_proof(&txid).await.unwrap().unwrap();
    assert_eq!(merkle_proof, merkle_proof_async);
    assert!(merkle_proof.pos > 0);
}

#[tokio::test]
async fn test_get_tx_output_status() {
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

    let output_status = blocking_client
        .get_output_status(&txid, 1)
        .unwrap()
        .unwrap();
    let output_status_async = async_client
        .get_output_status(&txid, 1)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(output_status, output_status_async);
}

#[tokio::test]
async fn test_get_height() {
    let env = TestEnv::new();
    let (blocking_client, async_client) = env.setup_clients();

    let block_height = blocking_client.get_height().unwrap();
    let block_height_async = async_client.get_height().await.unwrap();
    assert!(block_height > 0);
    assert_eq!(block_height, block_height_async);
}

#[tokio::test]
async fn test_get_tip_hash() {
    let env = TestEnv::new();
    let (blocking_client, async_client) = env.setup_clients();

    let tip_hash = blocking_client.get_tip_hash().unwrap();
    let tip_hash_async = async_client.get_tip_hash().await.unwrap();
    assert_eq!(tip_hash, tip_hash_async);
}

#[tokio::test]
async fn test_get_block_hash() {
    let env = TestEnv::new();
    let (blocking_client, async_client) = env.setup_clients();

    let block_hash = env
        .bitcoind_client()
        .get_block_hash(21)
        .unwrap()
        .block_hash()
        .unwrap();

    let block_hash_blocking = blocking_client.get_block_hash(21).unwrap();
    let block_hash_async = async_client.get_block_hash(21).await.unwrap();
    assert_eq!(block_hash, block_hash_blocking);
    assert_eq!(block_hash, block_hash_async);
}

#[tokio::test]
async fn test_broadcast() {
    let env = TestEnv::new();
    let (blocking_client, async_client) = env.setup_clients();

    let address = env.get_legacy_address();
    let txid = env
        .bitcoind_client()
        .send_to_address(&address, Amount::from_sat(1000))
        .unwrap()
        .txid()
        .unwrap();

    let tx = env
        .bitcoind_client()
        .get_transaction(txid)
        .expect("tx should exist for given `txid`")
        .into_model()
        .expect("should convert successfully")
        .tx;

    let blocking_res = blocking_client
        .broadcast(&tx)
        .expect("should successfully broadcast tx");
    let async_res = async_client
        .broadcast(&tx)
        .await
        .expect("should successfully broadcast tx");

    assert_eq!(blocking_res, txid);
    assert_eq!(async_res, txid);
}

#[tokio::test]
async fn test_get_tx_outspends() {
    let env = TestEnv::new();
    let (blocking_client, async_client) = env.setup_clients();

    let address = env.get_legacy_address();
    let txid = env
        .bitcoind_client()
        .send_to_address(&address, Amount::from_sat(21000))
        .unwrap()
        .txid()
        .unwrap();
    env.mine_and_wait(1);

    let outspends_blocking = blocking_client.get_tx_outspends(&txid).unwrap();
    let outspends_async = async_client.get_tx_outspends(&txid).await.unwrap();

    // Assert that there are 2 outputs: 21K sat and (coinbase - 21K sat).
    assert_eq!(outspends_blocking.len(), 2);
    assert_eq!(outspends_async.len(), 2);
    assert_eq!(outspends_blocking, outspends_async);

    // Assert that both outputs are returned as unspent (spent == false).
    assert!(outspends_blocking.iter().all(|output| !output.spent));
}

#[tokio::test]
async fn test_get_tx_with_http_headers() {
    async fn handle_requests(listener: TcpListener, count: usize) -> Vec<[u8; 4096]> {
        let mut raw_requests = vec![];
        for _ in 0..count {
            let (mut stream, _) = listener.accept().await.expect("should accept connection!");
            let mut buf = [0u8; 4096];
            AsyncReadExt::read(&mut stream, &mut buf)
                .await
                .expect("should read from stream");
            raw_requests.push(buf);
        }
        raw_requests
    }

    // setup a mocked HTTP server.
    let base_url = format!(
        "127.0.0.1:{}",
        get_available_port().expect("should get an available port successfully!")
    );

    let listener = TcpListener::bind(&base_url)
        .await
        .expect("should bind the TCP listener successfully");

    // setup `TestEnv` and expected HTTP headers.
    let env = TestEnv::new();
    let exp_header_key = "Authorization";
    let exp_header_value = "Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==";
    let headers = HashMap::from([(exp_header_key.to_string(), exp_header_value.to_string())]);

    let (blocking_client, async_client) = env.setup_clients_with_headers(&base_url, headers);

    let address = env.get_legacy_address();
    let txid = env
        .bitcoind_client()
        .send_to_address(&address, Amount::from_sat(1000))
        .unwrap()
        .txid()
        .unwrap();
    env.mine_and_wait(1);

    let blocking_task = tokio::task::spawn_blocking(move || blocking_client.get_tx(&txid));
    let async_task = tokio::task::spawn(async move { async_client.get_tx(&txid).await });

    let raw_requests = handle_requests(listener, 2).await;
    let requests = raw_requests
        .iter()
        .map(|raw| {
            String::from_utf8(raw.to_vec()).expect("should parse HTTP requests successfully")
        })
        .collect::<Vec<String>>();

    assert_eq!(
        requests.len(),
        2,
        "it MUST contain ONLY two requests (i.e a single one from each client)"
    );

    let assert_request = |user_agent: &str, header_key: &str| {
        let expected_path = format!("GET /tx/{txid}/raw");
        let expected_auth = format!("{header_key}: {exp_header_value}");

        assert!(
            requests.iter().any(|req| {
                req.contains(&expected_path)
                    && req.contains(&expected_auth)
                    && req.contains(user_agent)
            }),
            "request MUST call `{expected_path}` with `{user_agent}` and expected authorization header"
        );
    };

    // both clients should send the expected headers properly
    assert_request("User-Agent: blocking", exp_header_key);
    assert_request("User-Agent: async", exp_header_key);

    // cleanup any remaining spawned tasks
    let _ = blocking_task.await.expect("blocking task should not panic");
    let _ = async_task.await.expect("async task should not panic");
}
