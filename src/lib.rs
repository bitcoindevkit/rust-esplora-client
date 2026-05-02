// SPDX-License-Identifier: MIT OR Apache-2.0

//! An extensible blocking/async Esplora client
//!
//! This library provides an extensible blocking and
//! async Esplora client to query Esplora's backend.
//!
//! The library provides the possibility to build a blocking
//! or async client, both using [`bitreq`].
//! The library supports communicating to Esplora via a proxy
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
//! # #[cfg(all(feature = "async", feature = "tokio"))]
//! # {
//! use esplora_client::Builder;
//! let builder = Builder::new("https://blockstream.info/testnet/api");
//! let async_client = builder.build_async();
//! # Ok::<(), esplora_client::Error>(());
//! # }
//! ```
//!
//! ## Features
//!
//! By default the library enables all features. To specify
//! specific features, set `default-features` to `false` in your `Cargo.toml`
//! and specify the features you want. This will look like this:
//!
//! `esplora-client = { version = "*", default-features = false, features =
//! ["blocking"] }`
//!
//! * `blocking` enables [`bitreq`], the blocking client with proxy.
//! * `blocking-https` enables [`bitreq`], the blocking client with proxy and TLS (SSL) capabilities
//!   using the default [`bitreq`] backend.
//! * `blocking-https-rustls` enables [`bitreq`], the blocking client with proxy and TLS (SSL)
//!   capabilities using the `rustls` backend.
//! * `blocking-https-native` enables [`bitreq`], the blocking client with proxy and TLS (SSL)
//!   capabilities using the platform's native TLS backend (likely OpenSSL).
//! * `blocking-https-bundled` enables [`bitreq`], the blocking client with proxy and TLS (SSL)
//!   capabilities using a bundled OpenSSL library backend.
//! * `async` enables [`bitreq`], the async client with proxy capabilities.
//! * `async-https` enables [`bitreq`], the async client with support for proxying and TLS (SSL)
//!   using the default [`bitreq`] TLS backend.
//! * `async-https-native` enables [`bitreq`], the async client with support for proxying and TLS
//!   (SSL) using the platform's native TLS backend (likely OpenSSL).
//! * `async-https-rustls` enables [`bitreq`], the async client with support for proxying and TLS
//!   (SSL) using the `rustls` TLS backend.
//! * `async-https-rustls-manual-roots` enables [`bitreq`], the async client with support for
//!   proxying and TLS (SSL) using the `rustls` TLS backend without using the default root
//!   certificates.
//!
//! [`dont remove this line or cargo doc will break`]: https://example.com
#![cfg_attr(not(feature = "bitreq"), doc = "[`bitreq`]: https://docs.rs/bitreq")]
#![allow(clippy::result_large_err)]
#![warn(missing_docs)]

use std::collections::HashMap;
use std::fmt;
use std::num::TryFromIntError;
use std::time::Duration;

#[cfg(feature = "async")]
pub use r#async::Sleeper;

pub mod api;
#[cfg(feature = "async")]
pub mod r#async;
#[cfg(feature = "blocking")]
pub mod blocking;

pub use api::*;
use bitreq::Response;
#[cfg(feature = "blocking")]
pub use blocking::BlockingClient;
#[cfg(feature = "async")]
pub use r#async::AsyncClient;

/// Response status codes for which the request may be retried.
pub const RETRYABLE_ERROR_CODES: [u16; 3] = [
    429, // TOO_MANY_REQUESTS
    500, // INTERNAL_SERVER_ERROR
    503, // SERVICE_UNAVAILABLE
];

/// Base backoff in milliseconds.
const BASE_BACKOFF_MILLIS: Duration = Duration::from_millis(256);

/// Default max retries.
const DEFAULT_MAX_RETRIES: usize = 6;

/// Default max cached connections
#[cfg(feature = "async")]
const DEFAULT_MAX_CONNECTIONS: usize = 10;

/// Check if [`Response`] status is within 100-199.
#[allow(unused)]
fn is_informational(response: &Response) -> bool {
    (100..200).contains(&response.status_code)
}

/// Check if [`Response`] status is within 200-299.
fn is_success(response: &Response) -> bool {
    (200..300).contains(&response.status_code)
}

/// Check if [`Response`] status is within 300-399.
#[allow(unused)]
fn is_redirection(response: &Response) -> bool {
    (300..400).contains(&response.status_code)
}

/// Check if [`Response`] status is within 400-499.
#[allow(unused)]
fn is_client_error(response: &Response) -> bool {
    (400..500).contains(&response.status_code)
}

/// Check if [`Response`] status is within 500-599.
#[allow(unused)]
fn is_server_error(response: &Response) -> bool {
    (500..600).contains(&response.status_code)
}

/// Check if [`Response`] status is within the retryable ones.
fn is_retryable(response: &Response) -> bool {
    RETRYABLE_ERROR_CODES.contains(&(response.status_code as u16))
}

/// Returns the [`FeeRate`] for the given confirmation target in blocks.
///
/// Selects the highest confirmation target from `estimates` that is at or
/// below `target_blocks`, and returns its [`FeeRate`]. Returns `None` if no
/// matching estimate is found.
pub fn convert_fee_rate(target_blocks: usize, estimates: HashMap<u16, FeeRate>) -> Option<FeeRate> {
    estimates
        .into_iter()
        .filter(|(k, _)| *k as usize <= target_blocks)
        .max_by_key(|(k, _)| *k)
        .map(|(_, feerate)| feerate)
}

/// Converts a [`HashMap`] of fee estimates expressed as sat/vB ([`f64`]) into a [`FeeRate`].
pub(crate) fn sat_per_vbyte_to_feerate(estimates: HashMap<u16, f64>) -> HashMap<u16, FeeRate> {
    estimates
        .into_iter()
        .map(|(k, v)| (k, FeeRate::from_sat_per_kwu((v * 250.0).round() as u64)))
        .collect()
}

/// A builder for an [`AsyncClient`] or [`BlockingClient`].
#[derive(Debug, Clone)]
pub struct Builder {
    /// The URL of the Esplora server.
    pub base_url: String,
    /// Optional URL of the proxy to use to make requests to the Esplora server
    ///
    /// The string should be formatted as:
    /// `<protocol>://<user>:<password>@host:<port>`.
    ///
    /// Note that the format of this value and the supported protocols change
    /// slightly between the blocking version of the client (using `minreq`)
    /// and the async version (using `bitreq`). For more details check with
    /// the documentation of the two crates. Both of them are compiled with
    /// the `socks` feature enabled.
    ///
    /// The proxy is ignored when targeting `wasm32`.
    pub proxy: Option<String>,
    /// Socket timeout.
    pub timeout: Option<u64>,
    /// HTTP headers to set on every request made to Esplora server.
    pub headers: HashMap<String, String>,
    /// Max retries
    pub max_retries: usize,
    /// The maximum number of cached connections.
    #[cfg(feature = "async")]
    pub max_connections: usize,
}

impl Builder {
    /// Instantiate a new builder
    pub fn new(base_url: &str) -> Self {
        Builder {
            base_url: base_url.to_string(),
            proxy: None,
            timeout: None,
            headers: HashMap::new(),
            max_retries: DEFAULT_MAX_RETRIES,
            #[cfg(feature = "async")]
            max_connections: DEFAULT_MAX_CONNECTIONS,
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

    /// Add a header to set on each request
    pub fn header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    /// Set the maximum number of times to retry a request if the response status
    /// is one of [`RETRYABLE_ERROR_CODES`].
    pub fn max_retries(mut self, count: usize) -> Self {
        self.max_retries = count;
        self
    }

    /// Set the maximum number of cached connections in the client.
    #[cfg(feature = "async")]
    pub fn max_connections(mut self, count: usize) -> Self {
        self.max_connections = count;
        self
    }

    /// Build a blocking client from builder
    #[cfg(feature = "blocking")]
    pub fn build_blocking(self) -> BlockingClient {
        BlockingClient::from_builder(self)
    }

    /// Build an asynchronous client from builder
    #[cfg(all(feature = "async", feature = "tokio"))]
    pub fn build_async(self) -> Result<AsyncClient, Error> {
        AsyncClient::from_builder(self)
    }

    /// Build an asynchronous client from builder where the returned client uses a
    /// user-defined [`Sleeper`].
    #[cfg(feature = "async")]
    pub fn build_async_with_sleeper<S: Sleeper>(self) -> Result<AsyncClient<S>, Error> {
        AsyncClient::from_builder(self)
    }
}

/// Errors that can happen during a request to `Esplora` servers.
#[derive(Debug)]
pub enum Error {
    /// Error during `bitreq` HTTP request
    #[cfg(any(feature = "blocking", feature = "async"))]
    BitReq(bitreq::Error),
    /// Error during JSON (de)serialization
    SerdeJson(serde_json::Error),
    /// HTTP response error
    HttpResponse {
        /// The HTTP status code returned by the server.
        status: u16,
        /// The error message content.
        message: String,
    },
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
    /// Block Header height not found
    HeaderHeightNotFound(u32),
    /// Block Header hash not found
    HeaderHashNotFound(BlockHash),
    /// Invalid HTTP Header name specified
    InvalidHttpHeaderName(String),
    /// Invalid HTTP Header value specified
    InvalidHttpHeaderValue(String),
    /// The server sent an invalid response
    InvalidResponse,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
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
#[cfg(any(feature = "blocking", feature = "async"))]
impl_error!(::bitreq::Error, BitReq, Error);
impl_error!(serde_json::Error, SerdeJson, Error);
impl_error!(std::num::ParseIntError, Parsing, Error);
impl_error!(bitcoin::consensus::encode::Error, BitcoinEncoding, Error);
impl_error!(bitcoin::hex::HexToArrayError, HexToArray, Error);
impl_error!(bitcoin::hex::HexToBytesError, HexToBytes, Error);

#[cfg(test)]
mod test {
    use super::*;

    #[cfg(all(feature = "blocking", feature = "async"))]
    use {
        bitcoin::{hashes::Hash, Address, Amount},
        core::str::FromStr,
        electrsd::bitcoind::{self, BitcoinD},
        electrsd::{electrum_client::ElectrumApi, ElectrsD},
        std::time::Duration,
    };

    /// Struct that holds regtest `bitcoind` and `electrsd` instances.
    #[cfg(all(feature = "blocking", feature = "async"))]
    struct TestEnv {
        bitcoind: BitcoinD,
        electrsd: ElectrsD,
    }

    /// Configuration parameters for the [`TestEnv`].
    #[cfg(all(feature = "blocking", feature = "async"))]
    pub struct Config<'a> {
        /// Configuration params for the [`corepc_node::Node`].
        pub bitcoind: bitcoind::Conf<'a>,
        /// Configuration params for the [`electrsd::ElectrsD`].
        pub electrsd: electrsd::Conf<'a>,
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
    impl Default for Config<'_> {
        /// Use the default configuration for [`corepc_node::Node`], and enable
        /// HTTP for [`electrsd::ElectrsD`], exposing an Esplora API endpoint.
        fn default() -> Self {
            Self {
                bitcoind: bitcoind::Conf::default(),
                electrsd: {
                    let mut config = electrsd::Conf::default();
                    config.http_enabled = true;
                    config
                },
            }
        }
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
    impl TestEnv {
        /// Instantiate a [`TestEnv`] with default [`Config`].
        pub fn new() -> Self {
            TestEnv::new_with_config(Config::default())
        }

        /// Instantiate a [`TestEnv`] with a custom [`Config`].
        fn new_with_config(config: Config) -> Self {
            const SETUP_BLOCK_COUNT: usize = 101;

            let bitcoind_exe = std::env::var("BITCOIND_EXE")
                .ok()
                .or_else(|| bitcoind::downloaded_exe_path().ok())
                .expect(
                    "Provide a BITCOIND_EXE environment variable, or specify a `bitcoind` version feature",
                );
            let bitcoind = BitcoinD::with_conf(bitcoind_exe, &config.bitcoind).unwrap();

            let electrs_exe = std::env::var("ELECTRS_EXE")
                .ok()
                .or_else(electrsd::downloaded_exe_path)
                .expect(
                    "Provide an ELECTRS_EXE environment variable, or specify an `electrsd` version feature",
                );
            let electrsd = ElectrsD::with_conf(electrs_exe, &bitcoind, &config.electrsd).unwrap();

            let env = Self { bitcoind, electrsd };

            env.bitcoind_client()
                .generate_to_address(SETUP_BLOCK_COUNT, &env.get_mining_address())
                .unwrap();
            env.wait_until_electrum_sees_block(SETUP_BLOCK_COUNT);
            env
        }

        /// Get the [`bitcoind` RPC client](bitcoind::Client).
        fn bitcoind_client(&self) -> &bitcoind::Client {
            &self.bitcoind.client
        }

        /// Setup both [`BlockingClient`] and [`AsyncClient`].
        fn setup_clients(&self) -> (BlockingClient, AsyncClient) {
            self.setup_clients_with_headers(
                self.electrsd.esplora_url.as_ref().unwrap(),
                HashMap::new(),
            )
        }

        /// Setup both [`BlockingClient`] and [`AsyncClient`] with custom HTTP headers.
        fn setup_clients_with_headers(
            &self,
            url: &str,
            headers: HashMap<String, String>,
        ) -> (BlockingClient, AsyncClient) {
            let mut builder = Builder::new(&format!("http://{url}"));
            for (k, v) in &headers {
                builder = builder.header(k, v);
            }
            let blocking_client = builder
                .clone()
                .header("User-Agent", "blocking")
                .build_blocking();
            let async_client = builder.header("User-Agent", "async").build_async().unwrap();

            (blocking_client, async_client)
        }

        /// Mine `count` blocks.
        fn mine_blocks(&self, count: usize) {
            self.bitcoind
                .client
                .generate_to_address(count, &self.get_mining_address())
                .unwrap();
        }

        /// Wait until the [electrum server](electrsd::ElectrsD) sees a new block.
        fn wait_until_electrum_sees_block(&self, min_height: usize) {
            let electrsd = &self.electrsd;
            let mut header = electrsd.client.block_headers_subscribe().unwrap();
            loop {
                if header.height >= min_height {
                    break;
                }
                header = self.poll_exp_backoff(|| {
                    electrsd.trigger().unwrap();
                    electrsd.client.ping().unwrap();
                    electrsd.client.block_headers_pop().unwrap()
                });
            }
        }

        /// Mine `count` blocks and wait until the
        /// [electrum server](electrsd::ElectrsD) sees a new block.
        fn mine_and_wait(&self, count: usize) {
            let current_height = self
                .electrsd
                .client
                .block_headers_subscribe()
                .unwrap()
                .height;
            self.mine_blocks(count);
            self.wait_until_electrum_sees_block(current_height + count);
        }

        /// Poll the [electrum server](electrsd::ElectrsD) in exponentially increasing intervals.
        fn poll_exp_backoff<T, F>(&self, mut poll: F) -> T
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

        /// Get a `Legacy` regtest address.
        fn get_legacy_address(&self) -> Address {
            Address::from_str("mvUsRD2pNeQQ8nZq8CDEx6fjVQsyzqyhVC")
                .unwrap()
                .assume_checked()
        }

        /// Get a `Nested SegWit` (P2SH-P2WSH) regtest address.
        fn get_nested_segwit_address(&self) -> Address {
            Address::from_str("2N2bJevrSwzv5C6dGm9kQAivDYnvDBPbUxM")
                .unwrap()
                .assume_checked()
        }

        /// Get a `bech32` regtest address.
        fn get_bech32_address(&self) -> Address {
            Address::from_str("bcrt1qedegah48k0uft3ez7u8ywg2hf0ygexgvhps0wp")
                .unwrap()
                .assume_checked()
        }

        /// Get a `bech32m` regtest address.
        fn get_bech32m_address(&self) -> Address {
            Address::from_str("bcrt1p970nsjmz8ls34ty229n6zu534mumc2j74skuxe2lzcqdqxuwwhxsftk7al")
                .unwrap()
                .assume_checked()
        }

        /// Get an address which coinbase outputs should be sent to.
        fn get_mining_address(&self) -> Address {
            Address::from_str("bcrt1qj5gx4t0n8lrl0clddmpn0pee4r4fds7stwyj0j")
                .unwrap()
                .assume_checked()
        }
    }

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

    #[cfg(all(feature = "blocking", feature = "async"))]
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

    #[cfg(all(feature = "blocking", feature = "async"))]
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

    #[cfg(all(feature = "blocking", feature = "async"))]
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

    #[cfg(all(feature = "blocking", feature = "async"))]
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

    #[cfg(all(feature = "blocking", feature = "async"))]
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

    #[cfg(all(feature = "blocking", feature = "async"))]
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

    #[cfg(all(feature = "blocking", feature = "async"))]
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

    #[cfg(all(feature = "blocking", feature = "async"))]
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

    #[cfg(all(feature = "blocking", feature = "async"))]
    #[tokio::test]
    async fn test_that_errors_are_propagated() {
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
        let async_res = async_client.broadcast(tx.as_ref().unwrap()).await;
        println!("{:?}", async_res);
        let blocking_res = blocking_client.broadcast(tx.as_ref().unwrap());
        assert!(async_res.is_err());
        assert!(matches!(
            async_res.unwrap_err(),
            Error::HttpResponse { status: 400, message } if message.contains("-27")
        ));
        assert!(blocking_res.is_err());
        assert!(matches!(
            blocking_res.unwrap_err(),
            Error::HttpResponse { status: 400, message } if message.contains("-27")
        ));
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
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

    #[cfg(all(feature = "blocking", feature = "async"))]
    #[tokio::test]
    async fn test_get_merkle_proof() {
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

    #[cfg(all(feature = "blocking", feature = "async"))]
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

    #[cfg(all(feature = "blocking", feature = "async"))]
    #[tokio::test]
    async fn test_get_output_status() {
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

    #[cfg(all(feature = "blocking", feature = "async"))]
    #[tokio::test]
    async fn test_get_height() {
        let env = TestEnv::new();
        let (blocking_client, async_client) = env.setup_clients();

        let block_height = blocking_client.get_height().unwrap();
        let block_height_async = async_client.get_height().await.unwrap();
        assert!(block_height > 0);
        assert_eq!(block_height, block_height_async);
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
    #[tokio::test]
    async fn test_get_tip_hash() {
        let env = TestEnv::new();
        let (blocking_client, async_client) = env.setup_clients();

        let tip_hash = blocking_client.get_tip_hash().unwrap();
        let tip_hash_async = async_client.get_tip_hash().await.unwrap();
        assert_eq!(tip_hash, tip_hash_async);
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
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

    #[cfg(all(feature = "blocking", feature = "async"))]
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

    #[cfg(all(feature = "blocking", feature = "async"))]
    #[tokio::test]
    async fn test_get_fee_estimates() {
        let env = TestEnv::new();
        let (blocking_client, async_client) = env.setup_clients();

        let fee_estimates = blocking_client.get_fee_estimates().unwrap();
        let fee_estimates_async = async_client.get_fee_estimates().await.unwrap();
        assert_eq!(fee_estimates.len(), fee_estimates_async.len());
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
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

    #[cfg(all(feature = "blocking", feature = "async"))]
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

    #[cfg(all(feature = "blocking", feature = "async"))]
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

    #[cfg(all(feature = "blocking", feature = "async"))]
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
    #[cfg(all(feature = "blocking", feature = "async"))]
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

    #[cfg(all(feature = "blocking", feature = "async"))]
    #[tokio::test]
    async fn test_get_tx_with_http_headers() {
        use bitcoind::get_available_port;
        use tokio::io::AsyncReadExt;
        use tokio::net::TcpListener;

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

    #[cfg(all(feature = "blocking", feature = "async"))]
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

    #[cfg(all(feature = "blocking", feature = "async"))]
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

    #[cfg(all(feature = "blocking", feature = "async"))]
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

    #[cfg(all(feature = "blocking", feature = "async"))]
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

    #[cfg(all(feature = "blocking", feature = "async"))]
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

    #[cfg(all(feature = "blocking", feature = "async"))]
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

    #[cfg(all(feature = "blocking", feature = "async"))]
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
        let mempool_address_txs_blocking =
            blocking_client.get_mempool_address_txs(&address).unwrap();
        let mempool_address_txs_async = async_client
            .get_mempool_address_txs(&address)
            .await
            .unwrap();
        assert_eq!(mempool_address_txs_blocking, mempool_address_txs_async);
        assert_eq!(mempool_address_txs_blocking.len(), 5);
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
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
}
