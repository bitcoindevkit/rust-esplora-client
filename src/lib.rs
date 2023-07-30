//! An extensible blocking/async Esplora client
//!
//! This library provides an extensible blocking and
//! async Esplora client to query Esplora's backend.
//!
//! The library provides the possibility to build a blocking
//! client using [`ureq`] and an async client using [`reqwest`].
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
//! use esplora::Builder;
//! let builder = Builder::new("https://blockstream.info/testnet/api");
//! let blocking_client = builder.build_blocking();
//! # Ok::<(), esplora::Error>(());
//! # }
//! ```
//!
//! Here is an example of how to create an asynchronous client.
//!
//! ```no_run
//! # #[cfg(feature = "async")]
//! # {
//! use esplora::Builder;
//! let builder = Builder::new("https://blockstream.info/testnet/api");
//! let async_client = builder.build_async();
//! # Ok::<(), esplora::Error>(());
//! # }
//! ```
//!
//! ## Features
//!
//! By default the library enables all features. To specify
//! specific features, set `default-features` to `false` in your `Cargo.toml`
//! and specify the features you want. This will look like this:
//!
//! `esplora_client = { version = "*", default-features = false, features = ["blocking"] }`
//!
//! * `blocking` enables [`ureq`], the blocking client with proxy and TLS (SSL) capabilities.
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
//!
//!

#![allow(clippy::result_large_err)]

#[macro_use]
extern crate amplify;
#[macro_use]
extern crate serde;

use amplify::{hex, IoError};
use bp::{BlockHash, Txid};
use std::collections::HashMap;
use std::io;

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

/// Get a fee value in sats/vbytes from the estimates
/// that matches the confirmation target set as parameter.
pub fn convert_fee_rate(target: usize, estimates: HashMap<String, f64>) -> Result<f32, Error> {
    let fee_val = {
        let mut pairs = estimates
            .into_iter()
            .filter_map(|(k, v)| Some((k.parse::<usize>().ok()?, v)))
            .collect::<Vec<_>>();
        pairs.sort_unstable_by_key(|(k, _)| std::cmp::Reverse(*k));
        pairs
            .into_iter()
            .find(|(k, _)| k <= &target)
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
    /// blocking version of the client (using `ureq`) and the async version (using `reqwest`). For more
    /// details check with the documentation of the two crates. Both of them are compiled with
    /// the `socks` feature enabled.
    ///
    /// The proxy is ignored when targeting `wasm32`.
    pub proxy: Option<String>,
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
    pub fn build_blocking(self) -> Result<BlockingClient, Error> {
        BlockingClient::from_builder(self)
    }

    // build an asynchronous client from builder
    #[cfg(feature = "async")]
    pub fn build_async(self) -> Result<AsyncClient, Error> {
        AsyncClient::from_builder(self)
    }
}

/// Errors that can happen during a sync with `Esplora`
#[derive(Debug, Display, Error, From)]
#[display(inner)]
pub enum Error {
    /// Error during ureq HTTP request
    #[cfg(feature = "blocking")]
    #[from]
    #[from(ureq::Transport)]
    Ureq(ureq::Error),

    /// Error during reqwest HTTP request
    #[cfg(feature = "async")]
    #[from]
    Reqwest(reqwest::Error),

    /// HTTP response error {0}
    #[display(doc_comments)]
    HttpResponse(u16),

    /// IO error during ureq response read
    #[from]
    #[from(io::Error)]
    Io(IoError),

    /// no header found in ureq response
    #[display(doc_comments)]
    NoHeader,

    /// Invalid number returned
    #[from]
    Parsing(std::num::ParseIntError),

    /// Invalid Hex data returned
    #[from]
    Hex(hex::Error),

    /// transaction {0} not found
    #[display(doc_comments)]
    TransactionNotFound(Txid),

    /// header for block height {0} not found
    #[display(doc_comments)]
    HeaderHeightNotFound(u32),

    /// header for block hash {0} not found
    #[display(doc_comments)]
    HeaderHashNotFound(BlockHash),
}
