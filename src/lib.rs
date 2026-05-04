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
#![allow(deprecated)]

use std::collections::HashMap;
use std::fmt;
use std::num::TryFromIntError;
#[cfg(any(feature = "blocking", feature = "async"))]
use std::time::Duration;

#[cfg(feature = "async")]
pub use r#async::Sleeper;

pub mod api;
#[cfg(feature = "async")]
pub mod r#async;
#[cfg(feature = "blocking")]
pub mod blocking;

pub use api::*;
#[cfg(any(feature = "blocking", feature = "async"))]
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
#[cfg(any(feature = "blocking", feature = "async"))]
const BASE_BACKOFF_MILLIS: Duration = Duration::from_millis(256);

/// Default max retries.
const DEFAULT_MAX_RETRIES: usize = 6;

/// Default max cached connections
#[cfg(feature = "async")]
const DEFAULT_MAX_CONNECTIONS: usize = 10;

/// Check if [`Response`] status is within 100-199.
#[allow(unused)]
#[cfg(any(feature = "blocking", feature = "async"))]
fn is_informational(response: &Response) -> bool {
    (100..200).contains(&response.status_code)
}

/// Check if [`Response`] status is within 200-299.
#[cfg(any(feature = "blocking", feature = "async"))]
fn is_success(response: &Response) -> bool {
    (200..300).contains(&response.status_code)
}

/// Check if [`Response`] status is within 300-399.
#[allow(unused)]
#[cfg(any(feature = "blocking", feature = "async"))]
fn is_redirection(response: &Response) -> bool {
    (300..400).contains(&response.status_code)
}

/// Check if [`Response`] status is within 400-499.
#[allow(unused)]
#[cfg(any(feature = "blocking", feature = "async"))]
fn is_client_error(response: &Response) -> bool {
    (400..500).contains(&response.status_code)
}

/// Check if [`Response`] status is within 500-599.
#[allow(unused)]
#[cfg(any(feature = "blocking", feature = "async"))]
fn is_server_error(response: &Response) -> bool {
    (500..600).contains(&response.status_code)
}

/// Check if [`Response`] status is within the retryable ones.
#[cfg(any(feature = "blocking", feature = "async"))]
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
pub fn sat_per_vbyte_to_feerate(estimates: HashMap<u16, f64>) -> HashMap<u16, FeeRate> {
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
        match self {
            #[cfg(any(feature = "blocking", feature = "async"))]
            Error::BitReq(e) => write!(f, "Bitreq HTTP error: {e}"),
            Error::SerdeJson(e) => write!(f, "JSON (de)serialization error: {e}"),
            Error::HttpResponse { status, message } => {
                write!(f, "HTTP error {status}: {message}")
            }
            Error::Parsing(e) => write!(f, "Failed to parse invalid number: {e}"),
            Error::StatusCode(e) => write!(f, "Invalid status code: {e}"),
            Error::BitcoinEncoding(e) => write!(f, "Invalid Bitcoin data: {e}"),
            Error::HexToArray(e) => write!(f, "Invalid hex to array conversion: {e}"),
            Error::HexToBytes(e) => write!(f, "Invalid hex to bytes conversion: {e}"),
            Error::TransactionNotFound(txid) => {
                write!(f, "Transaction not found: {txid}")
            }
            Error::HeaderHeightNotFound(height) => {
                write!(f, "Block header at height {height} not found")
            }
            Error::HeaderHashNotFound(hash) => {
                write!(f, "Block header with hash {hash} not found")
            }
            Error::InvalidHttpHeaderName(name) => {
                write!(f, "Invalid HTTP header name: {name}")
            }
            Error::InvalidHttpHeaderValue(value) => {
                write!(f, "Invalid HTTP header value: {value}")
            }
            Error::InvalidResponse => write!(f, "The server sent an invalid response"),
        }
    }
}

impl std::error::Error for Error {}

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

#[cfg(any(feature = "blocking", feature = "async"))]
impl_error!(::bitreq::Error, BitReq, Error);
impl_error!(serde_json::Error, SerdeJson, Error);
impl_error!(std::num::ParseIntError, Parsing, Error);
impl_error!(bitcoin::consensus::encode::Error, BitcoinEncoding, Error);
impl_error!(bitcoin::hex::HexToArrayError, HexToArray, Error);
impl_error!(bitcoin::hex::HexToBytesError, HexToBytes, Error);
