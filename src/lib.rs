//! An extensible blocking and async Esplora client.
//!
//! This library provides a blocking client built on [`minreq`] and an async
//! client built on [`reqwest`] for interacting with an
//! [Esplora](https://github.com/Blockstream/esplora) server.
//!
//! Both clients support communicating via a proxy and TLS (SSL).
//!
//! # Blocking Client
//!
//! ```rust,ignore
//! use esplora_client::Builder;
//! let client = Builder::new("https://mempool.space/api").build_blocking();
//! let height = client.get_height().unwrap();
//! ```
//!
//! # Async Client
//!
//! ```rust,ignore
//! use esplora_client::Builder;
//! async fn example() {
//!     let client = Builder::new("https://mempool.space/api")
//!         .build_async()
//!         .unwrap();
//!     let height = client.get_height().await.unwrap();
//! }
//! ```
//!
//! # Features
//!
//! By default, all features are enabled. To use a specific feature
//! combination, set `default-features = false` and explicitly enable
//! the desired features in your `Cargo.toml` manifest:
//!
//! `esplora-client = { version = "*", default-features = false, features = ["blocking"] }`
//!
//! ### Blocking
//!
//! | Feature | Description |
//! |---------|-------------|
//! | `blocking` | Enables the blocking client with proxy support. |
//! | `blocking-https` | Enables the blocking client with proxy and TLS using the default [`minreq`](https://docs.rs/minreq) backend. |
//! | `blocking-https-rustls` | Enables the blocking client with proxy and TLS using [`rustls`](https://docs.rs/rustls). |
//! | `blocking-https-native` | Enables the blocking client with proxy and TLS using the platform's native TLS backend. |
//! | `blocking-https-bundled` | Enables the blocking client with proxy and TLS using a bundled OpenSSL backend. |
//!
//! ### Async
//!
//! | Feature | Description |
//! |---------|-------------|
//! | `async` | Enables the async client with proxy support. |
//! | `tokio` | Enables the Tokio runtime for the async client. |
//! | `async-https` | Enables the async client with proxy and TLS using the default [`reqwest`](https://docs.rs/reqwest) backend. |
//! | `async-https-native` | Enables the async client with proxy and TLS using the platform's native TLS backend. |
//! | `async-https-rustls` | Enables the async client with proxy and TLS using [`rustls`](https://docs.rs/rustls). |
//! | `async-https-rustls-manual-roots` | Enables the async client with proxy and TLS using `rustls` without default root certificates. |
//!
//! [`dont remove the 2 lines below or `cargo doc` will break`]: https://example.com
#![cfg_attr(not(feature = "minreq"), doc = "[`minreq`]: https://docs.rs/minreq")]
#![cfg_attr(not(feature = "reqwest"), doc = "[`reqwest`]: https://docs.rs/reqwest")]
#![allow(clippy::result_large_err)]
#![warn(missing_docs)]

use core::fmt;
use core::fmt::Display;
use core::fmt::Formatter;
use core::time::Duration;
use std::collections::HashMap;

#[cfg(feature = "async")]
pub use r#async::Sleeper;

pub mod api;
#[cfg(feature = "async")]
pub mod r#async;
#[cfg(feature = "blocking")]
pub mod blocking;

pub use api::*;
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
#[doc(hidden)]
pub const BASE_BACKOFF_MILLIS: Duration = Duration::from_millis(256);

/// Default max retries.
#[doc(hidden)]
pub const DEFAULT_MAX_RETRIES: usize = 6;

/// Returns the [`FeeRate`] for the given confirmation target in blocks.
///
/// Selects the highest confirmation target from `estimates` that is at or
/// below `target_blocks`, and returns its [`FeeRate`]. Returns `None` if no
/// matching estimate is found.
///
/// # Example
///
/// ```rust
/// use bitcoin::FeeRate;
/// use esplora_client::convert_fee_rate;
/// use std::collections::HashMap;
///
/// let mut estimates = HashMap::new();
/// estimates.insert(1u16, FeeRate::from_sat_per_vb(10).unwrap());
/// estimates.insert(6u16, FeeRate::from_sat_per_vb(5).unwrap());
///
/// assert_eq!(
///     convert_fee_rate(6, estimates.clone()),
///     Some(FeeRate::from_sat_per_vb(5).unwrap())
/// );
/// assert_eq!(
///     convert_fee_rate(1, estimates.clone()),
///     Some(FeeRate::from_sat_per_vb(10).unwrap())
/// );
/// assert_eq!(convert_fee_rate(0, estimates), None);
/// ```
pub fn convert_fee_rate(target_blocks: usize, estimates: HashMap<u16, FeeRate>) -> Option<FeeRate> {
    estimates
        .into_iter()
        .filter(|(k, _)| *k as usize <= target_blocks)
        .max_by_key(|(k, _)| *k)
        .map(|(_, feerate)| feerate)
}

/// A builder for an [`AsyncClient`] or [`BlockingClient`].
///
/// Use [`Builder::new`] to create a new builder, configure it with the
/// chainable methods, then call [`Builder::build_blocking`] or
/// [`Builder::build_async`] to construct the client.
#[derive(Debug, Clone)]
pub struct Builder {
    /// The URL of the Esplora server.
    pub base_url: String,
    /// Optional URL of the proxy to use to make requests to the Esplora server.
    ///
    /// The string should be formatted as:
    /// `<protocol>://<user>:<password>@host:<port>`.
    ///
    /// Note that the format of this value and the supported protocols change
    /// slightly between the blocking client (using [`minreq`]) and the async
    /// client (using [`reqwest`]). Both are compiled with the `socks` feature
    /// enabled.
    ///
    /// The proxy is ignored when targeting `wasm32`.
    pub proxy: Option<String>,
    /// The socket's timeout, in seconds.
    pub timeout: Option<u64>,
    /// HTTP headers to set on every request made to the Esplora server.
    pub headers: HashMap<String, String>,
    /// Maximum number of times to retry a request.
    pub max_retries: usize,
}

impl Builder {
    /// Create a new [`Builder`] with the given Esplora server URL.
    pub fn new(base_url: &str) -> Self {
        Builder {
            base_url: base_url.to_string(),
            proxy: None,
            timeout: None,
            headers: HashMap::new(),
            max_retries: DEFAULT_MAX_RETRIES,
        }
    }

    /// Set the proxy URL.
    ///
    /// See [`Builder::proxy`] for the expected format.
    pub fn proxy(mut self, proxy: &str) -> Self {
        self.proxy = Some(proxy.to_string());
        self
    }

    /// Set the socket's timeout, in seconds.
    pub fn timeout(mut self, timeout: u64) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Add an HTTP header to set on every request.
    pub fn header(mut self, key: &str, value: &str) -> Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    /// Set the maximum number of times to retry a request.
    ///
    /// Retries are only attempted for responses
    /// with status codes defined in [`RETRYABLE_ERROR_CODES`].
    pub fn max_retries(mut self, count: usize) -> Self {
        self.max_retries = count;
        self
    }

    /// Build a [`BlockingClient`] from this [`Builder`].
    #[cfg(feature = "blocking")]
    pub fn build_blocking(self) -> BlockingClient {
        BlockingClient::from_builder(self)
    }

    /// Build an [`AsyncClient`] from this [`Builder`].
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] if the underlying [`reqwest::Client`] fails to build.
    #[cfg(all(feature = "async", feature = "tokio"))]
    pub fn build_async(self) -> Result<AsyncClient, Error> {
        AsyncClient::from_builder(self)
    }

    /// Build an [`AsyncClient`] from this [`Builder`] with a custom [`Sleeper`].
    ///
    /// Use this instead of [`Builder::build_async`] when you want to use a
    /// runtime other than Tokio for sleeping between retries.
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] if the underlying [`reqwest::Client`] fails to build.
    #[cfg(feature = "async")]
    pub fn build_async_with_sleeper<S: Sleeper>(self) -> Result<AsyncClient<S>, Error> {
        AsyncClient::from_builder(self)
    }
}

/// Errors that can occur during a request to an Esplora server.
#[derive(Debug)]
pub enum Error {
    /// A [`minreq`] error occurred during a blocking HTTP request.
    #[cfg(feature = "blocking")]
    Minreq(minreq::Error),
    /// A [`reqwest`] error occurred during an async HTTP request.
    #[cfg(feature = "async")]
    Reqwest(reqwest::Error),
    /// An error occurred during JSON serialization or deserialization.
    SerdeJson(serde_json::Error),
    /// The server returned a non-success HTTP status code.
    HttpResponse {
        /// The HTTP status code returned by the server.
        status: u16,
        /// The error message returned by the server.
        message: String,
    },
    /// Failed to parse an integer from the server response.
    Parsing(core::num::ParseIntError),
    /// Failed to convert an HTTP status code to `u16`.
    StatusCode(core::num::TryFromIntError),
    /// Failed to decode a Bitcoin consensus-encoded value.
    BitcoinEncoding(bitcoin::consensus::encode::Error),
    /// Failed to decode a hex string into a fixed-size array.
    HexToArray(bitcoin::hex::HexToArrayError),
    /// Failed to decode a hex string into a vector of bytes.
    HexToBytes(bitcoin::hex::HexToBytesError),
    /// The requested [`Transaction`] was not found.
    TransactionNotFound(Txid),
    /// No [`block header`](bitcoin::blockdata::block::Header) was found at the given height.
    HeaderHeightNotFound(u32),
    /// No [`block header`](bitcoin::blockdata::block::Header) was found with the given
    /// [`BlockHash`].
    HeaderHashNotFound(BlockHash),
    /// The specified HTTP header name is invalid.
    InvalidHttpHeaderName(String),
    /// The specified HTTP header value is invalid.
    InvalidHttpHeaderValue(String),
    /// The server returned an invalid or unexpected response.
    InvalidResponse,
}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

macro_rules! impl_error {
    ( $from:ty, $to:ident ) => {
        impl_error!($from, $to, Error);
    };
    ( $from:ty, $to:ident, $impl_for:ty ) => {
        impl core::convert::From<$from> for $impl_for {
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
impl_error!(serde_json::Error, SerdeJson, Error);
impl_error!(core::num::ParseIntError, Parsing, Error);
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
        electrsd::{corepc_node, electrum_client::ElectrumApi, ElectrsD},
        std::time::Duration,
    };

    /// Struct that holds regtest `bitcoind` and `electrsd` instances.
    #[cfg(all(feature = "blocking", feature = "async"))]
    struct TestEnv {
        bitcoind: corepc_node::Node,
        electrsd: ElectrsD,
    }

    /// Configuration parameters for the [`TestEnv`].
    #[cfg(all(feature = "blocking", feature = "async"))]
    pub struct Config<'a> {
        /// Configuration params for the [`corepc_node::Node`].
        pub bitcoind: corepc_node::Conf<'a>,
        /// Configuration params for the [`electrsd::ElectrsD`].
        pub electrsd: electrsd::Conf<'a>,
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
    impl Default for Config<'_> {
        /// Use the default configuration for [`corepc_node::Node`], and enable
        /// HTTP for [`electrsd::ElectrsD`], exposing an Esplora API endpoint.
        fn default() -> Self {
            Self {
                bitcoind: corepc_node::Conf::default(),
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
                .or_else(|| corepc_node::downloaded_exe_path().ok())
                .expect(
                    "Provide a BITCOIND_EXE environment variable, or specify a `bitcoind` version feature",
                );
            let bitcoind = corepc_node::Node::with_conf(bitcoind_exe, &config.bitcoind).unwrap();

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

        /// Get the [`bitcoind` RPC client](corepc_node::Client).
        fn bitcoind_client(&self) -> &corepc_node::Client {
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

        // Convert fees from sat/vB (`f64`) to `FeeRate`.
        // Note that `get_fee_estimates` already returns `HashMap<u16, FeeRate>`.
        let esplora_fees = sat_per_vbyte_to_feerate(esplora_fees_raw);

        assert!(convert_fee_rate(1, HashMap::new()).is_none());
        assert_eq!(
            convert_fee_rate(6, esplora_fees.clone()),
            Some(FeeRate::from_sat_per_kwu((2.236_f64 * 250_000.0) as u64))
        );
        assert_eq!(
            convert_fee_rate(26, esplora_fees.clone()),
            Some(FeeRate::from_sat_per_kwu((1.015_f64 * 250_000.0) as u64)),
            "should inherit from value for 25"
        );
        assert!(
            convert_fee_rate(0, esplora_fees).is_none(),
            "should not return feerate for 0 target"
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

    // TODO(@luisschwab): remove on `v0.14.0`
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
    async fn test_get_script_hash_txs() {
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
        let script_hash_txs_txids_blocking: Vec<Txid> = blocking_client
            .get_script_hash_txs(script, None)
            .unwrap()
            .iter()
            .map(|tx| tx.txid)
            .collect();
        let script_hash_txs_txids_async: Vec<Txid> = async_client
            .get_scripthash_txs(script, None)
            .await
            .unwrap()
            .iter()
            .map(|tx| tx.txid)
            .collect();
        assert_eq!(script_hash_txs_txids_blocking, script_hash_txs_txids_async);
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

    #[cfg(all(feature = "blocking", feature = "async"))]
    #[tokio::test]
    async fn test_get_block_infos() {
        let env = TestEnv::new();
        let (blocking_client, async_client) = env.setup_clients();

        let start_height = env.bitcoind_client().get_block_count().unwrap().0;
        let blocks1 = blocking_client.get_block_infos(None).unwrap();
        let blocks_async1 = async_client.get_block_infos(None).await.unwrap();
        assert_eq!(blocks1[0].height, start_height as u32);
        assert_eq!(blocks1, blocks_async1);
        env.mine_and_wait(1);

        let blocks2 = blocking_client.get_block_infos(None).unwrap();
        let blocks_async2 = async_client.get_block_infos(None).await.unwrap();
        assert_eq!(blocks2, blocks_async2);
        assert_ne!(blocks2, blocks1);

        let blocks3 = blocking_client
            .get_block_infos(Some(start_height as u32))
            .unwrap();
        let blocks_async3 = async_client
            .get_block_infos(Some(start_height as u32))
            .await
            .unwrap();
        assert_eq!(blocks3, blocks_async3);
        assert_eq!(blocks3[0].height, start_height as u32);
        assert_eq!(blocks3, blocks1);

        let blocks_genesis = blocking_client.get_block_infos(Some(0)).unwrap();
        let blocks_genesis_async = async_client.get_block_infos(Some(0)).await.unwrap();
        assert_eq!(blocks_genesis, blocks_genesis_async);
    }

    #[cfg(all(feature = "blocking", feature = "async"))]
    #[tokio::test]
    async fn test_get_tx_with_http_headers() {
        use corepc_node::get_available_port;
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

        // minreq's blocking client sends title-case headers: "Authorization"
        assert_request("User-Agent: blocking", exp_header_key);
        // reqwest's async client sends lowercase headers: "authorization"
        assert_request("user-agent: async", &exp_header_key.to_lowercase());

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
