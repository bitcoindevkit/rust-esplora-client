// SPDX-License-Identifier: MIT OR Apache-2.0

//! # Tests for Queries Pinned at a Block and Chain-Tip Stamps
//!
//! The `asof` query parameter and the chain-tip response headers are
//! extensions the Esplora server of the regtest test environment does not
//! implement, so these tests run against a mocked HTTP server.

#![cfg(all(feature = "blocking", feature = "async", feature = "tokio"))]

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::str::FromStr;
use std::thread::JoinHandle;

use bitcoin::{BlockHash, Txid};
use esplora_client::{Builder, ChainTip, Error, Stamped, TxStatus};

const BLOCK_HASH: &str = "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f";
const TXID: &str = "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b";
const HEIGHT: u32 = 21;

/// Serves `count` requests on a local port, answering each with `status`,
/// the extra `headers`, and `body`.
///
/// Returns the base URL of the server and a handle yielding the request line
/// of every request received.
fn mock_server(
    count: usize,
    status: &'static str,
    headers: Vec<(&'static str, String)>,
    body: &'static str,
) -> (String, JoinHandle<Vec<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("should bind the TCP listener");
    let base_url = format!("http://{}", listener.local_addr().unwrap());

    let handle = std::thread::spawn(move || {
        let mut request_lines = vec![];
        for _ in 0..count {
            let (mut stream, _) = listener.accept().expect("should accept connection");
            let mut reader = BufReader::new(stream.try_clone().unwrap());

            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            request_lines.push(request_line.trim_end().to_string());

            let mut line = String::new();
            while reader.read_line(&mut line).unwrap() > 0 && line != "\r\n" {
                line.clear();
            }

            let mut response = format!(
                "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n",
                body.len()
            );
            for (name, value) in &headers {
                response.push_str(&format!("{name}: {value}\r\n"));
            }
            response.push_str("\r\n");
            response.push_str(body);
            stream.write_all(response.as_bytes()).unwrap();
        }
        request_lines
    });

    (base_url, handle)
}

fn chain_tip_headers() -> Vec<(&'static str, String)> {
    vec![
        ("X-Bitcoin-Chain-Tip", BLOCK_HASH.to_string()),
        ("X-Bitcoin-Chain-Tip-Height", HEIGHT.to_string()),
    ]
}

fn unconfirmed() -> TxStatus {
    TxStatus {
        confirmed: false,
        block_height: None,
        block_hash: None,
        block_time: None,
    }
}

#[tokio::test]
async fn test_as_of_sends_param_and_returns_chain_tip() {
    let block_hash = BlockHash::from_str(BLOCK_HASH).unwrap();
    let txid = Txid::from_str(TXID).unwrap();

    let (base_url, server) =
        mock_server(2, "200 OK", chain_tip_headers(), r#"{"confirmed":false}"#);
    let blocking_client = Builder::new(&base_url).build_blocking();
    let async_client = Builder::new(&base_url).build_async().unwrap();

    let status_blocking =
        tokio::task::spawn_blocking(move || blocking_client.as_of(block_hash).get_tx_status(&txid))
            .await
            .unwrap()
            .unwrap();
    let status_async = async_client
        .as_of(block_hash)
        .get_tx_status(&txid)
        .await
        .unwrap();

    let expected = Stamped {
        value: unconfirmed(),
        chain_tip: Some(ChainTip {
            hash: block_hash,
            height: HEIGHT,
        }),
    };
    assert_eq!(status_blocking, expected);
    assert_eq!(status_async, expected);

    let expected_request_line = format!("GET /tx/{TXID}/status?asof={BLOCK_HASH} HTTP/1.1");
    assert_eq!(
        server.join().unwrap(),
        vec![expected_request_line.clone(), expected_request_line]
    );
}

#[tokio::test]
async fn test_as_of_without_chain_tip_headers() {
    let block_hash = BlockHash::from_str(BLOCK_HASH).unwrap();
    let txid = Txid::from_str(TXID).unwrap();

    let (base_url, server) = mock_server(2, "200 OK", vec![], r#"{"confirmed":false}"#);
    let blocking_client = Builder::new(&base_url).build_blocking();
    let async_client = Builder::new(&base_url).build_async().unwrap();

    let status_blocking =
        tokio::task::spawn_blocking(move || blocking_client.as_of(block_hash).get_tx_status(&txid))
            .await
            .unwrap()
            .unwrap();
    let status_async = async_client
        .as_of(block_hash)
        .get_tx_status(&txid)
        .await
        .unwrap();

    let expected = Stamped {
        value: unconfirmed(),
        chain_tip: None,
    };
    assert_eq!(status_blocking, expected);
    assert_eq!(status_async, expected);
    server.join().unwrap();
}

#[tokio::test]
async fn test_as_of_unknown_block_is_an_error() {
    let block_hash = BlockHash::from_str(BLOCK_HASH).unwrap();
    let txid = Txid::from_str(TXID).unwrap();

    let (base_url, server) = mock_server(2, "404 Not Found", vec![], "Not Found");
    let blocking_client = Builder::new(&base_url).build_blocking();
    let async_client = Builder::new(&base_url).build_async().unwrap();

    let res_blocking = tokio::task::spawn_blocking(move || {
        blocking_client
            .as_of(block_hash)
            .get_output_status(&txid, 0)
    })
    .await
    .unwrap();
    let res_async = async_client
        .as_of(block_hash)
        .get_output_status(&txid, 0)
        .await;

    for res in [res_blocking, res_async] {
        match res {
            Err(Error::HttpResponse { status: 404, .. }) => {}
            other => panic!("expected a 404 error, got {other:?}"),
        }
    }
    server.join().unwrap();
}

#[tokio::test]
async fn test_unpinned_request_sends_no_as_of_param() {
    let txid = Txid::from_str(TXID).unwrap();

    let (base_url, server) =
        mock_server(2, "200 OK", chain_tip_headers(), r#"{"confirmed":false}"#);
    let blocking_client = Builder::new(&base_url).build_blocking();
    let async_client = Builder::new(&base_url).build_async().unwrap();

    let status_blocking = tokio::task::spawn_blocking(move || blocking_client.get_tx_status(&txid))
        .await
        .unwrap()
        .unwrap();
    let status_async = async_client.get_tx_status(&txid).await.unwrap();
    assert_eq!(status_blocking, unconfirmed());
    assert_eq!(status_async, unconfirmed());

    let expected_request_line = format!("GET /tx/{TXID}/status HTTP/1.1");
    assert_eq!(
        server.join().unwrap(),
        vec![expected_request_line.clone(), expected_request_line]
    );
}

#[tokio::test]
async fn test_get_tip_hash_stamped() {
    let block_hash = BlockHash::from_str(BLOCK_HASH).unwrap();

    let (base_url, server) = mock_server(2, "200 OK", chain_tip_headers(), BLOCK_HASH);
    let blocking_client = Builder::new(&base_url).build_blocking();
    let async_client = Builder::new(&base_url).build_async().unwrap();

    let tip_blocking = tokio::task::spawn_blocking(move || blocking_client.get_tip_hash_stamped())
        .await
        .unwrap()
        .unwrap();
    let tip_async = async_client.get_tip_hash_stamped().await.unwrap();

    let expected = Stamped {
        value: block_hash,
        chain_tip: Some(ChainTip {
            hash: block_hash,
            height: HEIGHT,
        }),
    };
    assert_eq!(tip_blocking, expected);
    assert_eq!(tip_async, expected);

    let expected_request_line = "GET /blocks/tip/hash HTTP/1.1".to_string();
    assert_eq!(
        server.join().unwrap(),
        vec![expected_request_line.clone(), expected_request_line]
    );
}

#[tokio::test]
async fn test_get_tip_hash_stamped_without_chain_tip_headers() {
    let block_hash = BlockHash::from_str(BLOCK_HASH).unwrap();

    let (base_url, server) = mock_server(2, "200 OK", vec![], BLOCK_HASH);
    let blocking_client = Builder::new(&base_url).build_blocking();
    let async_client = Builder::new(&base_url).build_async().unwrap();

    let tip_blocking = tokio::task::spawn_blocking(move || blocking_client.get_tip_hash_stamped())
        .await
        .unwrap()
        .unwrap();
    let tip_async = async_client.get_tip_hash_stamped().await.unwrap();

    let expected = Stamped {
        value: block_hash,
        chain_tip: None,
    };
    assert_eq!(tip_blocking, expected);
    assert_eq!(tip_async, expected);
    server.join().unwrap();
}
