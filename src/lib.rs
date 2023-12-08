//! An extensible blocking/async Esplora client
//!
//! This library provides an extensible blocking and
//! async Esplora client to query Esplora's backend.
//!
//! The library provides the possibility to build a blocking
//! client using [`minreq`] and an async client using [`reqwest`],
//! and an anonymized async client using [`arti-hyper`].
//! The library supports communicating to Esplora via a Tor, proxy,
//! and also using TLS (SSL) for secure communication.
//!
//!
//! ## Usage
//!
//! You can create a blocking client as follows:
//!
//! ```no_run
//! # #[cfg(feature = "blocking")]
//! # {
//! use esplora_client::Builder;
//! let builder = Builder::new("https://blockstream.info/testnet/api");
//! let blocking_client = builder.build_blocking();
//! # Ok::<(), esplora_client::Error>(());
//! # }
//! ```
//!
//! Here is an example of how to create an asynchronous client.
//!
//! ```no_run
//! # #[cfg(feature = "async")]
//! # {
//! use esplora_client::Builder;
//! let builder = Builder::new("https://blockstream.info/testnet/api");
//! let async_client = builder.build_async();
//! # Ok::<(), esplora_client::Error>(());
//! # }
//! ```
//!
//! // FIXME: (@leonardo) fix this documentation
//! Here is an example of how to create an anonymized asynchronous client.
//!
//! ```no_run
//! # #[cfg(feature = "async-arti-hyper")]
//! # {
//! use esplora_client::Builder;
//! let builder = Builder::new("https://blockstream.info/testnet/api");
//! let async_client = builder.build_async_anonymized();
//! # Ok::<(), esplora_client::Error>(());
//! # }
//! ```
//!
//!
//! ## Features
//!
//! By default the library enables all features. To specify
//! specific features, set `default-features` to `false` in your `Cargo.toml`
//! and specify the features you want. This will look like this:
//!
//! `esplora-client = { version = "*", default-features = false, features = ["blocking"] }`
//!
//! * `blocking` enables [`minreq`], the blocking client with proxy.
//! * `blocking-https` enables [`minreq`], the blocking client with proxy and TLS (SSL)
//!   capabilities using the default [`minreq`] backend.
//! * `blocking-https-rustls` enables [`minreq`], the blocking client with proxy and TLS (SSL)
//!   capabilities using the `rustls` backend.
//! * `blocking-https-native` enables [`minreq`], the blocking client with proxy and TLS (SSL)
//!   capabilities using the platform's native TLS backend (likely OpenSSL).
//! * `blocking-https-bundled` enables [`minreq`], the blocking client with proxy and TLS (SSL)
//!   capabilities using a bundled OpenSSL library backend.
//! * `async` enables [`reqwest`], the async client with proxy capabilities.
//! * `async-https` enables [`reqwest`], the async client with support for proxying and TLS (SSL)
//!   using the default [`reqwest`] TLS backend.
//! * `async-https-native` enables [`reqwest`], the async client with support for proxying and TLS
//!   (SSL) using the platform's native TLS backend (likely OpenSSL).
//! * `async-https-rustls` enables [`reqwest`], the async client with support for proxying and TLS
//!   (SSL) using the `rustls` TLS backend.
//! * `async-https-rustls-manual-roots` enables [`reqwest`], the async client with support for
//!   proxying and TLS (SSL) using the `rustls` TLS backend without using its the default root
//!   certificates.
//! * `async-arti-hyper` enables [`arti_hyper`], the async anonymized client support for TLS (SSL) over Tor,
//!   using the default [`arti_hyper`] TLS backend.
//! * `async-arti-hyper-native` enables [`arti_hyper`], the async anonymized client support for TLS (SSL) over Tor,
//!   using the platform's native TLS backend (likely OpenSSL).
//! * `async-arti-hyper-rustls` enables [`arti_hyper`], the async anonymized client support for TLS (SSL) over Tor,
//!   using the `rustls` TLS backend without using its the default root certificates.
//!
//!

#![allow(clippy::result_large_err)]

use std::collections::HashMap;
use std::fmt;
use std::num::TryFromIntError;

use bitcoin::consensus;

pub mod api;

#[cfg(any(feature = "async", feature = "async-arti-hyper"))]
pub mod r#async;
#[cfg(feature = "blocking")]
pub mod blocking;

pub use api::*;
#[cfg(feature = "blocking")]
pub use blocking::BlockingClient;
#[cfg(feature = "async-arti-hyper")]
pub use r#async::AsyncAnonymizedClient;
#[cfg(feature = "async")]
pub use r#async::AsyncClient;

/// Get a fee value in sats/vbytes from the estimates
/// that matches the confirmation target set as parameter.
pub fn convert_fee_rate(target: usize, estimates: HashMap<u16, f64>) -> Result<f32, Error> {
    let fee_val = {
        let mut pairs = estimates.into_iter().collect::<Vec<(u16, f64)>>();
        pairs.sort_unstable_by_key(|(k, _)| std::cmp::Reverse(*k));
        pairs
            .into_iter()
            .find(|(k, _)| *k as usize <= target)
            .map(|(_, v)| v)
            .unwrap_or(1.0)
    };
    Ok(fee_val as f32)
}

#[derive(Debug, Clone)]
pub struct Builder {
    pub base_url: String,
    /// Optional URL of the proxy to use to make requests to the Esplora server
    ///
    /// The string should be formatted as: `<protocol>://<user>:<password>@host:<port>`.
    ///
    /// Note that the format of this value and the supported protocols change slightly between the
    /// blocking version of the client (using `minreq`) and the async version (using `reqwest`). For more
    /// details check with the documentation of the two crates. Both of them are compiled with
    /// the `socks` feature enabled.
    ///
    /// The proxy is ignored when targeting `wasm32`.
    pub proxy: Option<String>, // TODO: (@leonardo) should this be available for `async-arti-hyper`
    /// Socket timeout.
    pub timeout: Option<u64>,
}

impl Builder {
    /// Instantiate a new builder
    pub fn new(base_url: &str) -> Self {
        Builder {
            base_url: base_url.to_string(),
            proxy: None,
            timeout: None,
        }
    }

    /// Set the proxy of the builder
    pub fn proxy(mut self, proxy: &str) -> Self {
        self.proxy = Some(proxy.to_string());
        self
    }

    /// Set the timeout of the builder
    pub fn timeout(mut self, timeout: u64) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// build a blocking client from builder
    #[cfg(feature = "blocking")]
    pub fn build_blocking(self) -> BlockingClient {
        BlockingClient::from_builder(self)
    }

    // build an asynchronous client from builder
    #[cfg(feature = "async")]
    pub fn build_async(self) -> Result<AsyncClient, Error> {
        AsyncClient::from_builder(self)
    }

    // build an asynchronous anonymized, over Tor, client from builder
    #[cfg(feature = "async-arti-hyper")]
    pub async fn build_async_anonymized(self) -> Result<AsyncAnonymizedClient, Error> {
        AsyncAnonymizedClient::from_builder(self).await
    }
}

/// Errors that can happen during a sync with `Esplora`
#[derive(Debug)]
pub enum Error {
    /// Error during `minreq` HTTP request
    #[cfg(feature = "blocking")]
    Minreq(::minreq::Error),
    /// Error during reqwest HTTP request
    #[cfg(feature = "async")]
    Reqwest(::reqwest::Error),
    /// Error during hyper HTTP request
    #[cfg(feature = "async-arti-hyper")]
    Hyper(::hyper::Error),
    /// Error during hyper HTTP request
    #[cfg(feature = "async-arti-hyper")]
    InvalidUri,
    /// Error during hyper HTTP request body creation
    #[cfg(feature = "async-arti-hyper")]
    InvalidBody,
    /// Error during Tor client creation
    #[cfg(feature = "async-arti-hyper")]
    ArtiClient(::arti_client::Error),
    /// Error during [`TlsConnector`] building
    #[cfg(feature = "async-arti-hyper")]
    TlsConnector,
    /// Error during response decoding
    #[cfg(feature = "async-arti-hyper")]
    ResponseDecoding,
    /// HTTP response error
    HttpResponse { status: u16, message: String },
    /// Invalid number returned
    Parsing(std::num::ParseIntError),
    /// Invalid status code, unable to convert to `u16`
    StatusCode(TryFromIntError),
    /// Invalid Bitcoin data returned
    BitcoinEncoding(bitcoin::consensus::encode::Error),
    /// Invalid hex data returned (attempting to create an array)
    HexToArray(bitcoin::hex::HexToArrayError),
    /// Invalid hex data returned (attempting to create a vector)
    HexToBytes(bitcoin::hex::HexToBytesError),
    /// Transaction not found
    TransactionNotFound(Txid),
    /// Header height not found
    HeaderHeightNotFound(u32),
    /// Header hash not found
    HeaderHashNotFound(BlockHash),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

macro_rules! impl_error {
    ( $from:ty, $to:ident ) => {
        impl_error!($from, $to, Error);
    };
    ( $from:ty, $to:ident, $impl_for:ty ) => {
        impl std::convert::From<$from> for $impl_for {
            fn from(err: $from) -> Self {
                <$impl_for>::$to(err)
            }
        }
    };
}

impl std::error::Error for Error {}
#[cfg(feature = "blocking")]
impl_error!(::minreq::Error, Minreq, Error);
#[cfg(feature = "async")]
impl_error!(::reqwest::Error, Reqwest, Error);
#[cfg(feature = "async-arti-hyper")]
impl_error!(::hyper::Error, Hyper, Error);
#[cfg(feature = "async-arti-hyper")]
impl_error!(::arti_client::Error, ArtiClient, Error);
impl_error!(std::num::ParseIntError, Parsing, Error);
impl_error!(consensus::encode::Error, BitcoinEncoding, Error);
impl_error!(bitcoin::hex::HexToArrayError, HexToArray, Error);
impl_error!(bitcoin::hex::HexToBytesError, HexToBytes, Error);

#[cfg(test)]
mod test {
    use super::*;
    #[allow(unused_imports)]
    use bitcoin::hashes::Hash;
    use electrsd::{bitcoind, bitcoind::BitcoinD, ElectrsD};
    use lazy_static::lazy_static;
    use std::env;
    use tokio::sync::Mutex;
    #[cfg(all(feature = "blocking", feature = "async"))]
    use {
        bitcoin::Amount,
        electrsd::{
            bitcoind::bitcoincore_rpc::json::AddressType, bitcoind::bitcoincore_rpc::RpcApi,
            electrum_client::ElectrumApi,
        },
        std::time::Duration,
        tokio::sync::OnceCell,
    };

    lazy_static! {
        static ref BITCOIND: BitcoinD = {
            let bitcoind_exe = env::var("BITCOIND_EXE")
                .ok()
                .or_else(|| bitcoind::downloaded_exe_path().ok())
                .expect(
                    "you need to provide an env var BITCOIND_EXE or specify a bitcoind version feature",
                );
            let conf = bitcoind::Conf::default();
            BitcoinD::with_conf(bitcoind_exe, &conf).unwrap()
        };
        static ref ELECTRSD: ElectrsD = {
            let electrs_exe = env::var("ELECTRS_EXE")
                .ok()
                .or_else(electrsd::downloaded_exe_path)
                .expect(
                    "you need to provide env var ELECTRS_EXE or specify an electrsd version feature",
                );
            let mut conf = electrsd::Conf::default();
            conf.http_enabled = true;
            ElectrsD::with_conf(electrs_exe, &BITCOIND, &conf).unwrap()
        };
        static ref MINER: Mutex<()> = Mutex::new(());
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
    static PREMINE: OnceCell<()> = OnceCell::const_new();

    #[cfg(all(feature = "blocking", feature = "async"))]
    async fn setup_clients() -> (BlockingClient, AsyncClient) {
        PREMINE
            .get_or_init(|| async {
                let _miner = MINER.lock().await;
                generate_blocks_and_wait(101);
            })
            .await;

        let esplora_url = ELECTRSD.esplora_url.as_ref().unwrap();

        let builder = Builder::new(&format!("http://{}", esplora_url));
        let blocking_client = builder.build_blocking();

        let builder_async = Builder::new(&format!("http://{}", esplora_url));
        let async_client = builder_async.build_async().unwrap();

        (blocking_client, async_client)
    }

    #[cfg(feature = "async-arti-hyper")]
    async fn setup_anonymized_client() -> AsyncAnonymizedClient {
        const ESPLORA_URL: &str = "https://mempool.space/api";
        // const ESPLORA_URL: &str = "https://blockstream.info/testnet/api";
        // const ESPLORA_URL: &str = "http://explorerzydxu5ecjrkwceayqybizmpjjznk5izmitf2modhcusuqlid.onion/api";

        let builder_async_anonymized = Builder::new(ESPLORA_URL);

        builder_async_anonymized
            .build_async_anonymized()
            .await
            .unwrap()
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
    fn generate_blocks_and_wait(num: usize) {
        let cur_height = BITCOIND.client.get_block_count().unwrap();
        generate_blocks(num);
        wait_for_block(cur_height as usize + num);
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
    fn generate_blocks(num: usize) {
        let address = BITCOIND
            .client
            .get_new_address(Some("test"), Some(AddressType::Legacy))
            .unwrap()
            .assume_checked();
        let _block_hashes = BITCOIND
            .client
            .generate_to_address(num as u64, &address)
            .unwrap();
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
    fn wait_for_block(min_height: usize) {
        let mut header = ELECTRSD.client.block_headers_subscribe().unwrap();
        loop {
            if header.height >= min_height {
                break;
            }
            header = exponential_backoff_poll(|| {
                ELECTRSD.trigger().unwrap();
                ELECTRSD.client.ping().unwrap();
                ELECTRSD.client.block_headers_pop().unwrap()
            });
        }
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
    fn exponential_backoff_poll<T, F>(mut poll: F) -> T
    where
        F: FnMut() -> Option<T>,
    {
        let mut delay = Duration::from_millis(64);
        loop {
            match poll() {
                Some(data) => break data,
                None if delay.as_millis() < 512 => delay = delay.mul_f32(2.0),
                None => {}
            }

            std::thread::sleep(delay);
        }
    }

    #[test]
    fn feerate_parsing() {
        let esplora_fees = serde_json::from_str::<HashMap<u16, f64>>(
            r#"{
  "25": 1.015,
  "5": 2.3280000000000003,
  "12": 2.0109999999999997,
  "15": 1.018,
  "17": 1.018,
  "11": 2.0109999999999997,
  "3": 3.01,
  "2": 4.9830000000000005,
  "6": 2.2359999999999998,
  "21": 1.018,
  "13": 1.081,
  "7": 2.2359999999999998,
  "8": 2.2359999999999998,
  "16": 1.018,
  "20": 1.018,
  "22": 1.017,
  "23": 1.017,
  "504": 1,
  "9": 2.2359999999999998,
  "14": 1.018,
  "10": 2.0109999999999997,
  "24": 1.017,
  "1008": 1,
  "1": 4.9830000000000005,
  "4": 2.3280000000000003,
  "19": 1.018,
  "144": 1,
  "18": 1.018
}
"#,
        )
        .unwrap();
        assert_eq!(convert_fee_rate(6, esplora_fees.clone()).unwrap(), 2.236);
        assert_eq!(
            convert_fee_rate(26, esplora_fees).unwrap(),
            1.015,
            "should inherit from value for 25"
        );
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
    #[tokio::test]
    async fn test_get_tx() {
        let (blocking_client, async_client) = setup_clients().await;

        let address = BITCOIND
            .client
            .get_new_address(Some("test"), Some(AddressType::Legacy))
            .unwrap()
            .assume_checked();
        let txid = BITCOIND
            .client
            .send_to_address(
                &address,
                Amount::from_sat(1000),
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
        let _miner = MINER.lock().await;
        generate_blocks_and_wait(1);

        let tx = blocking_client.get_tx(&txid).unwrap();
        let tx_async = async_client.get_tx(&txid).await.unwrap();
        assert_eq!(tx, tx_async);
    }

    #[cfg(feature = "async-arti-hyper")]
    #[tokio::test]
    #[ignore = "The `AsyncAnonymizedClient` tests are ignored as they rely on a remote server with available Esplora API"]
    async fn test_anonymized_get_tx() {
        let client = setup_anonymized_client().await;

        let network = bitcoin::Network::Bitcoin;
        let genesis_block = bitcoin::blockdata::constants::genesis_block(network);
        let coinbase_tx = genesis_block.coinbase().unwrap().to_owned();

        let tx_async_anonymized = client.get_tx(&coinbase_tx.txid()).await.unwrap().unwrap();
        assert_eq!(coinbase_tx, tx_async_anonymized);
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
    #[tokio::test]
    async fn test_get_tx_no_opt() {
        let (blocking_client, async_client) = setup_clients().await;

        let address = BITCOIND
            .client
            .get_new_address(Some("test"), Some(AddressType::Legacy))
            .unwrap()
            .assume_checked();
        let txid = BITCOIND
            .client
            .send_to_address(
                &address,
                Amount::from_sat(1000),
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
        let _miner = MINER.lock().await;
        generate_blocks_and_wait(1);

        let tx_no_opt = blocking_client.get_tx_no_opt(&txid).unwrap();
        let tx_no_opt_async = async_client.get_tx_no_opt(&txid).await.unwrap();
        assert_eq!(tx_no_opt, tx_no_opt_async);
    }

    #[cfg(feature = "async-arti-hyper")]
    #[tokio::test]
    #[ignore = "The `AsyncAnonymizedClient` tests are ignored as they rely on a remote server with available Esplora API"]
    async fn test_anonymized_get_tx_no_opt() {
        let client = setup_anonymized_client().await;

        let network = bitcoin::Network::Bitcoin;
        let genesis_block = bitcoin::blockdata::constants::genesis_block(network);
        let coinbase_tx = genesis_block.coinbase().unwrap().to_owned();

        let tx_async_anonymized = client.get_tx_no_opt(&coinbase_tx.txid()).await.unwrap();
        assert_eq!(coinbase_tx, tx_async_anonymized);
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
    #[tokio::test]
    async fn test_get_tx_status() {
        let (blocking_client, async_client) = setup_clients().await;

        let address = BITCOIND
            .client
            .get_new_address(Some("test"), Some(AddressType::Legacy))
            .unwrap()
            .assume_checked();
        let txid = BITCOIND
            .client
            .send_to_address(
                &address,
                Amount::from_sat(1000),
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
        let _miner = MINER.lock().await;
        generate_blocks_and_wait(1);

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

    #[cfg(feature = "async-arti-hyper")]
    #[tokio::test]
    #[ignore = "The `AsyncAnonymizedClient` tests are ignored as they rely on a remote server with available Esplora API"]
    async fn test_anonymized_get_tx_status() {
        let client = setup_anonymized_client().await;

        let network = bitcoin::Network::Bitcoin;
        let genesis_block = bitcoin::blockdata::constants::genesis_block(network);
        let coinbase_txid = genesis_block.coinbase().unwrap().txid();

        let tx_status_async_anonymized = client.get_tx_status(&coinbase_txid).await.unwrap();
        assert!(tx_status_async_anonymized.confirmed);

        // Bogus txid returns a TxStatus with false, None, None, None
        let txid = Txid::hash(b"ayyyy lmao");
        let tx_status_async_anonymized = client.get_tx_status(&txid).await.unwrap();
        assert!(!tx_status_async_anonymized.confirmed);
        assert!(tx_status_async_anonymized.block_height.is_none());
        assert!(tx_status_async_anonymized.block_hash.is_none());
        assert!(tx_status_async_anonymized.block_time.is_none());
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
    #[tokio::test]
    async fn test_get_header_by_hash() {
        let (blocking_client, async_client) = setup_clients().await;

        let block_hash = BITCOIND.client.get_block_hash(23).unwrap();

        let block_header = blocking_client.get_header_by_hash(&block_hash).unwrap();
        let block_header_async = async_client.get_header_by_hash(&block_hash).await.unwrap();
        assert_eq!(block_header, block_header_async);
    }

    #[cfg(feature = "async-arti-hyper")]
    #[tokio::test]
    #[ignore = "The `AsyncAnonymizedClient` tests are ignored as they rely on a remote server with available Esplora API"]

    async fn test_anonymized_get_header_by_hash() {
        let client = setup_anonymized_client().await;

        let network = bitcoin::Network::Bitcoin;
        let genesis_block = bitcoin::blockdata::constants::genesis_block(network);

        let block_header_async_anonymized = client
            .get_header_by_hash(&genesis_block.block_hash())
            .await
            .unwrap();

        assert_eq!(genesis_block.header, block_header_async_anonymized);
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
    #[tokio::test]
    async fn test_get_block_status() {
        let (blocking_client, async_client) = setup_clients().await;

        let block_hash = BITCOIND.client.get_block_hash(21).unwrap();
        let next_block_hash = BITCOIND.client.get_block_hash(22).unwrap();

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

    #[cfg(feature = "async-arti-hyper")]
    #[tokio::test]
    #[ignore = "The `AsyncAnonymizedClient` tests are ignored as they rely on a remote server with available Esplora API"]
    async fn test_anonymized_get_block_status() {
        use std::str::FromStr;

        let client = setup_anonymized_client().await;

        let network = bitcoin::Network::Bitcoin;
        let genesis_block = bitcoin::blockdata::constants::genesis_block(network);
        let genesis_block_status = BlockStatus {
            in_best_chain: true,
            height: Some(0),
            // https://mempool.space/block/00000000839a8e6886ab5951d76f411475428afc90947ee320161bbf18eb6048
            next_best: Some(
                BlockHash::from_str(
                    "00000000839a8e6886ab5951d76f411475428afc90947ee320161bbf18eb6048",
                )
                .unwrap(),
            ),
        };

        let block_status_async_anonymized = client
            .get_block_status(&genesis_block.block_hash())
            .await
            .unwrap();

        assert_eq!(genesis_block_status, block_status_async_anonymized)
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
    #[tokio::test]
    async fn test_get_non_existing_block_status() {
        // Esplora returns the same status for orphaned blocks as for non-existing blocks:
        // non-existing: https://blockstream.info/api/block/0000000000000000000000000000000000000000000000000000000000000000/status
        // orphaned: https://blockstream.info/api/block/000000000000000000181b1a2354620f66868a723c0c4d5b24e4be8bdfc35a7f/status
        // (Here the block is cited as orphaned: https://bitcoinchain.com/block_explorer/block/000000000000000000181b1a2354620f66868a723c0c4d5b24e4be8bdfc35a7f/ )
        // For this reason, we only test for the non-existing case here.

        let (blocking_client, async_client) = setup_clients().await;

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

    #[cfg(all(feature = "blocking", feature = "async"))]
    #[tokio::test]
    async fn test_get_block_by_hash() {
        let (blocking_client, async_client) = setup_clients().await;

        let block_hash = BITCOIND.client.get_block_hash(21).unwrap();

        let expected = Some(BITCOIND.client.get_block(&block_hash).unwrap());

        let block = blocking_client.get_block_by_hash(&block_hash).unwrap();
        let block_async = async_client.get_block_by_hash(&block_hash).await.unwrap();
        assert_eq!(expected, block);
        assert_eq!(expected, block_async);
    }

    #[cfg(feature = "async-arti-hyper")]
    #[tokio::test]
    #[ignore = "The `AsyncAnonymizedClient` tests are ignored as they rely on a remote server with available Esplora API"]
    async fn test_anonymized_get_block_by_hash() {
        let client = setup_anonymized_client().await;

        let network = bitcoin::Network::Bitcoin;
        let genesis_block = bitcoin::blockdata::constants::genesis_block(network);

        let block_status_async_anonymized = client
            .get_block_by_hash(&genesis_block.block_hash())
            .await
            .unwrap()
            .unwrap();

        assert_eq!(genesis_block, block_status_async_anonymized);
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
    #[tokio::test]
    async fn test_that_errors_are_propagated() {
        let (blocking_client, async_client) = setup_clients().await;

        let address = BITCOIND
            .client
            .get_new_address(Some("test"), Some(AddressType::Legacy))
            .unwrap()
            .assume_checked();
        let txid = BITCOIND
            .client
            .send_to_address(
                &address,
                Amount::from_sat(1000),
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
        let _miner = MINER.lock().await;
        generate_blocks_and_wait(1);

        let tx = blocking_client.get_tx(&txid).unwrap();
        let async_res = async_client.broadcast(tx.as_ref().unwrap()).await;
        let blocking_res = blocking_client.broadcast(tx.as_ref().unwrap());
        assert!(async_res.is_err());
        assert_eq!(async_res.unwrap_err().to_string(),"HttpResponse { status: 400, message: \"sendrawtransaction RPC error: {\\\"code\\\":-27,\\\"message\\\":\\\"Transaction already in block chain\\\"}\" }");
        assert!(blocking_res.is_err());
        assert_eq!(blocking_res.unwrap_err().to_string(),"HttpResponse { status: 400, message: \"sendrawtransaction RPC error: {\\\"code\\\":-27,\\\"message\\\":\\\"Transaction already in block chain\\\"}\" }");
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
    #[tokio::test]
    async fn test_get_block_by_hash_not_existing() {
        let (blocking_client, async_client) = setup_clients().await;

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

    #[cfg(all(feature = "blocking", feature = "async"))]
    #[tokio::test]
    async fn test_get_merkle_proof() {
        let (blocking_client, async_client) = setup_clients().await;

        let address = BITCOIND
            .client
            .get_new_address(Some("test"), Some(AddressType::Legacy))
            .unwrap()
            .assume_checked();
        let txid = BITCOIND
            .client
            .send_to_address(
                &address,
                Amount::from_sat(1000),
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
        let _miner = MINER.lock().await;
        generate_blocks_and_wait(1);

        let merkle_proof = blocking_client.get_merkle_proof(&txid).unwrap().unwrap();
        let merkle_proof_async = async_client.get_merkle_proof(&txid).await.unwrap().unwrap();
        assert_eq!(merkle_proof, merkle_proof_async);
        assert!(merkle_proof.pos > 0);
    }

    #[cfg(feature = "async-arti-hyper")]
    #[tokio::test]
    #[ignore = "The `AsyncAnonymizedClient` tests are ignored as they rely on a remote server with available Esplora API"]
    async fn test_anonymized_get_merkle_proof() {
        let client = setup_anonymized_client().await;

        let network = bitcoin::Network::Bitcoin;
        let genesis_block = bitcoin::blockdata::constants::genesis_block(network);

        let coinbase_txid = genesis_block.coinbase().unwrap().txid();
        let merkle_proof = client
            .get_merkle_proof(&coinbase_txid)
            .await
            .unwrap()
            .unwrap();

        assert!(merkle_proof.pos == 0);
        assert!(merkle_proof.block_height == 0);
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
    #[tokio::test]
    async fn test_get_merkle_block() {
        let (blocking_client, async_client) = setup_clients().await;

        let address = BITCOIND
            .client
            .get_new_address(Some("test"), Some(AddressType::Legacy))
            .unwrap()
            .assume_checked();
        let txid = BITCOIND
            .client
            .send_to_address(
                &address,
                Amount::from_sat(1000),
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
        let _miner = MINER.lock().await;
        generate_blocks_and_wait(1);

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

    #[cfg(feature = "async-arti-hyper")]
    #[tokio::test]
    #[ignore = "The `AsyncAnonymizedClient` tests are ignored as they rely on a remote server with available Esplora API"]
    async fn test_anonymized_get_merkle_block() {
        let client = setup_anonymized_client().await;

        let network = bitcoin::Network::Bitcoin;
        let genesis_block = bitcoin::blockdata::constants::genesis_block(network);

        let coinbase_txid = genesis_block.coinbase().unwrap().txid();

        let merkle_block = client
            .get_merkle_block(&coinbase_txid)
            .await
            .unwrap()
            .unwrap();

        let mut matches = vec![coinbase_txid];
        let mut indexes = vec![];
        let root = merkle_block
            .txn
            .extract_matches(&mut matches, &mut indexes)
            .unwrap();
        assert_eq!(root, merkle_block.header.merkle_root);
        assert_eq!(indexes.len(), 1);
        assert!(indexes[0] == 0);
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
    #[tokio::test]
    async fn test_get_output_status() {
        let (blocking_client, async_client) = setup_clients().await;

        let address = BITCOIND
            .client
            .get_new_address(Some("test"), Some(AddressType::Legacy))
            .unwrap()
            .assume_checked();
        let txid = BITCOIND
            .client
            .send_to_address(
                &address,
                Amount::from_sat(1000),
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
        let _miner = MINER.lock().await;
        generate_blocks_and_wait(1);

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

    #[cfg(feature = "async-arti-hyper")]
    #[tokio::test]
    #[ignore = "The `AsyncAnonymizedClient` tests are ignored as they rely on a remote server with available Esplora API"]
    async fn test_anonymized_get_output_status() {
        let client = setup_anonymized_client().await;

        let network = bitcoin::Network::Bitcoin;
        let genesis_block = bitcoin::blockdata::constants::genesis_block(network);
        let coinbase_txid = genesis_block.coinbase().unwrap().txid();

        let tx_status_async_anonymized = client
            .get_output_status(&coinbase_txid, 1)
            .await
            .unwrap()
            .unwrap();

        assert!(!tx_status_async_anonymized.spent);
        assert!(tx_status_async_anonymized.txid.is_none());
        assert!(tx_status_async_anonymized.vin.is_none());
        assert!(tx_status_async_anonymized.status.is_none());
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
    #[tokio::test]
    async fn test_get_height() {
        let (blocking_client, async_client) = setup_clients().await;
        let block_height = blocking_client.get_height().unwrap();
        let block_height_async = async_client.get_height().await.unwrap();
        assert!(block_height > 0);
        assert_eq!(block_height, block_height_async);
    }

    #[cfg(feature = "async-arti-hyper")]
    #[tokio::test]
    #[ignore = "The `AsyncAnonymizedClient` tests are ignored as they rely on a remote server with available Esplora API"]
    async fn test_anonymized_get_height() {
        let client = setup_anonymized_client().await;
        let block_height = client.get_height().await.unwrap();
        assert!(block_height > 0);
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
    #[tokio::test]
    async fn test_get_tip_hash() {
        let (blocking_client, async_client) = setup_clients().await;
        let tip_hash = blocking_client.get_tip_hash().unwrap();
        let tip_hash_async = async_client.get_tip_hash().await.unwrap();
        assert_eq!(tip_hash, tip_hash_async);
    }

    // #[cfg(feature = "async-arti-hyper")]
    // #[tokio::test]
    // #[ignore = "The `AsyncAnonymizedClient` tests are ignored as they rely on a remote server with available Esplora API"]
    // async fn test_anonymized_get_tip_hash() {
    //     unimplemented!()
    // }

    #[cfg(all(feature = "blocking", feature = "async"))]
    #[tokio::test]
    async fn test_get_block_hash() {
        let (blocking_client, async_client) = setup_clients().await;

        let block_hash = BITCOIND.client.get_block_hash(21).unwrap();

        let block_hash_blocking = blocking_client.get_block_hash(21).unwrap();
        let block_hash_async = async_client.get_block_hash(21).await.unwrap();
        assert_eq!(block_hash, block_hash_blocking);
        assert_eq!(block_hash, block_hash_async);
    }

    #[cfg(feature = "async-arti-hyper")]
    #[tokio::test]
    #[ignore = "The `AsyncAnonymizedClient` tests are ignored as they rely on a remote server with available Esplora API"]
    async fn test_anonymized_get_block_hash() {
        let client = setup_anonymized_client().await;

        let network = bitcoin::Network::Bitcoin;
        let genesis_block = bitcoin::blockdata::constants::genesis_block(network);

        let block_hash = client.get_block_hash(0).await.unwrap();
        assert_eq!(block_hash, genesis_block.block_hash());
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
    #[tokio::test]
    async fn test_get_txid_at_block_index() {
        let (blocking_client, async_client) = setup_clients().await;

        let block_hash = BITCOIND.client.get_block_hash(23).unwrap();

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

    #[cfg(feature = "async-arti-hyper")]
    #[tokio::test]
    #[ignore = "The `AsyncAnonymizedClient` tests are ignored as they rely on a remote server with available Esplora API"]
    async fn test_anonymized_get_txid_at_block_index() {
        let client = setup_anonymized_client().await;

        let network = bitcoin::Network::Bitcoin;
        let genesis_block = bitcoin::blockdata::constants::genesis_block(network);

        let genesis_block_hash = genesis_block.block_hash();
        let coinbase_txid = genesis_block.coinbase().unwrap().txid();

        let txid_at_block_index_async_anonymized = client
            .get_txid_at_block_index(&genesis_block_hash, 0)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(coinbase_txid, txid_at_block_index_async_anonymized);
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
    #[tokio::test]
    async fn test_get_fee_estimates() {
        let (blocking_client, async_client) = setup_clients().await;
        let fee_estimates = blocking_client.get_fee_estimates().unwrap();
        let fee_estimates_async = async_client.get_fee_estimates().await.unwrap();
        assert_eq!(fee_estimates.len(), fee_estimates_async.len());
    }

    // #[cfg(feature = "async-arti-hyper")]
    // #[tokio::test]
    // #[ignore = "The `AsyncAnonymizedClient` tests are ignored as they rely on a remote server with available Esplora API"]
    // async fn test_anonymized_get_fee_estimates() {
    //     todo!()
    // }

    #[cfg(all(feature = "blocking", feature = "async"))]
    #[tokio::test]
    async fn test_scripthash_txs() {
        let (blocking_client, async_client) = setup_clients().await;

        let address = BITCOIND
            .client
            .get_new_address(Some("test"), Some(AddressType::Legacy))
            .unwrap()
            .assume_checked();
        let txid = BITCOIND
            .client
            .send_to_address(
                &address,
                Amount::from_sat(1000),
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .unwrap();
        let _miner = MINER.lock().await;
        generate_blocks_and_wait(1);

        let expected_tx = BITCOIND
            .client
            .get_transaction(&txid, None)
            .unwrap()
            .transaction()
            .unwrap();
        let script = &expected_tx.output[0].script_pubkey;
        let scripthash_txs_txids: Vec<Txid> = blocking_client
            .scripthash_txs(script, None)
            .unwrap()
            .iter()
            .map(|tx| tx.txid)
            .collect();
        let scripthash_txs_txids_async: Vec<Txid> = async_client
            .scripthash_txs(script, None)
            .await
            .unwrap()
            .iter()
            .map(|tx| tx.txid)
            .collect();
        assert_eq!(scripthash_txs_txids, scripthash_txs_txids_async);
    }

    #[cfg(feature = "async-arti-hyper")]
    #[tokio::test]
    #[ignore = "The `AsyncAnonymizedClient` tests are ignored as they rely on a remote server with available Esplora API"]
    async fn test_anonymized_scripthash_txs() {
        let client = setup_anonymized_client().await;

        let network = bitcoin::Network::Bitcoin;
        let genesis_block = bitcoin::blockdata::constants::genesis_block(network);

        let coinbase_tx = genesis_block.coinbase().unwrap();

        let script = &coinbase_tx.output[0].script_pubkey;
        let scripthash_txs_txids: Vec<Txid> = client
            .scripthash_txs(script, None)
            .await
            .unwrap()
            .iter()
            .map(|tx| tx.txid)
            .collect();

        assert!(scripthash_txs_txids.contains(&coinbase_tx.txid()))
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
    #[tokio::test]
    async fn test_get_blocks() {
        let (blocking_client, async_client) = setup_clients().await;
        let start_height = BITCOIND.client.get_block_count().unwrap();

        let blocks1 = blocking_client.get_blocks(None).unwrap();
        let blocks_async1 = async_client.get_blocks(None).await.unwrap();
        assert_eq!(blocks1[0].time.height, start_height as u32);
        assert_eq!(blocks1, blocks_async1);

        generate_blocks_and_wait(10);

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

    #[cfg(feature = "async-arti-hyper")]
    #[tokio::test]
    #[ignore = "The `AsyncAnonymizedClient` tests are ignored as they rely on a remote server with available Esplora API"]
    async fn test_anonymized_get_blocks() {
        let client = setup_anonymized_client().await;

        let network = bitcoin::Network::Bitcoin;
        let genesis_block = bitcoin::blockdata::constants::genesis_block(network);

        let blocks = client.get_blocks(Some(1)).await.unwrap();

        assert!(blocks.len() == 2);

        assert_eq!(
            blocks[0].id.to_string(),
            "00000000839a8e6886ab5951d76f411475428afc90947ee320161bbf18eb6048"
        );
        assert_eq!(
            blocks[0].previousblockhash.unwrap(),
            genesis_block.block_hash()
        );

        assert_eq!(blocks[1].id, genesis_block.block_hash());
        assert_eq!(blocks[1].previousblockhash, None);
    }
}
