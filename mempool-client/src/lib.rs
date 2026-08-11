// SPDX-License-Identifier: MIT OR Apache-2.0

//! # Mempool Client
//!
//! A client library for querying the [mempool.space] HTTP API from Rust.
//!
//! This crate extends [`esplora-client`] by adding mempool.space specific
//! endpoints while maintaining full compatibility with the standard Esplora API.
//! It uses [`Deref`](std::ops::Deref) to expose all esplora-client methods
//! automatically, requiring no code duplication.
//!
//! [`Builder`] wraps [`esplora-client`]'s builder of the same name so that
//! `Builder::new(url).build_blocking()`/`build_async()` -- the idiomatic
//! `esplora-client` construction path return the mempool-aware
//! [`BlockingClient`]/[`AsyncClient`] instead. [`Error`] and all public
//! response types from [`mempool-types`] are re-exported unchanged. Both
//! clients share the same configuration for the base URL, proxy, timeout,
//! custom headers, and retry policy.
//!
//! Because every name and method signature matches `esplora-client`
//! exactly, migrating existing code only means swapping the dependency and
//! updating the crate path in `use` statements no other source changes,
//! not even to how the client is constructed.
//!
//! # Client Modes
//!
//! Enable the `blocking` feature to use [`BlockingClient`], whose methods block
//! the current thread until each request completes. Enable the `async` feature
//! to use [`AsyncClient`], whose methods return futures and require an async
//! runtime. The default async sleeper is backed by Tokio when the `tokio`
//! feature is enabled; custom runtimes can supply their own `Sleeper`.
//!
//! # Examples
//!
//! Create a blocking client:
//!
//! ```rust,ignore
//! use mempool_client::Builder;
//!
//! fn main() -> Result<(), mempool_client::Error> {
//!     let client = Builder::new("https://mempool.space/api").build_blocking();
//!
//!     // Standard Esplora methods via Deref.
//!     let height = client.get_height()?;
//!
//!     // Mempool-specific methods.
//!     let fees = client.get_recommended_fees()?;
//!
//!     Ok(())
//! }
//! ```
//!
//! Create an async client:
//!
//! ```rust,ignore
//! use mempool_client::Builder;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), mempool_client::Error> {
//!     let client = Builder::new("https://mempool.space/api").build_async()?;
//!
//!     // Standard Esplora methods via Deref.
//!     let height = client.get_height().await?;
//!
//!     // Mempool-specific methods.
//!     let fees = client.get_recommended_fees().await?;
//!
//!     Ok(())
//! }
//! ```
//!
//! # Retries
//!
//! Both clients retry responses with status codes listed in
//! [`RETRYABLE_ERROR_CODES`]. Retry attempts use exponential backoff starting
//! at 256 milliseconds and are controlled by [`Builder::max_retries`].
//!
//! # Features
//!
//! By default the crate enables all features. To select only the pieces you
//! need, set `default-features = false` in `Cargo.toml` and list the desired
//! features explicitly:
//!
//! ```toml
//! mempool-client = { version = "*", default-features = false, features = ["blocking"] }
//! ```
//!
//! * `blocking` enables [`bitreq`], the blocking client with proxy.
//! * `blocking-https` enables [`bitreq`], the blocking client with proxy and TLS (SSL) capabilities
//!   using the default [`bitreq`] backend.
//! * `blocking-https-rustls` enables [`bitreq`], the blocking client with proxy and TLS (SSL)
//!   capabilities using the `rustls` backend.
//! * `blocking-https-native` enables [`bitreq`], the blocking client with proxy and TLS (SSL)
//!   capabilities using the platform's native TLS backend (likely OpenSSL).
//! * `blocking-https-rustls-probe` enables [`bitreq`], the blocking client with proxy and TLS (SSL)
//!   capabilities using `rustls` and probed system roots.
//! * `async` enables [`bitreq`], the async client with proxy capabilities.
//! * `async-https` enables [`bitreq`], the async client with support for proxying and TLS (SSL)
//!   using the default [`bitreq`] TLS backend.
//! * `async-https-native` enables [`bitreq`], the async client with support for proxying and TLS
//!   (SSL) using the platform's native TLS backend (likely OpenSSL).
//! * `async-https-rustls` enables [`bitreq`], the async client with support for proxying and TLS
//!   (SSL) using the `rustls` TLS backend.
//! * `async-https-rustls-probe` enables [`bitreq`], the async client with support for proxying and
//!   TLS (SSL) using `rustls` and probed system roots.
//! * `tokio` enables the default async sleeper used by [`Builder::build_async`].
//!
//! [mempool.space]: https://mempool.space/docs/api/rest
//! [`esplora-client`]: https://docs.rs/esplora-client
//! [`mempool-types`]: https://docs.rs/mempool-types

#![warn(missing_docs)]
#![allow(deprecated)]

// Re-export everything from esplora-client for drop-in replacement compatibility.
// This includes: Builder, Error, Sleeper, api module, RETRYABLE_ERROR_CODES,
// convert_fee_rate(), sat_per_vbyte_to_feerate(), and all esplora-types.
pub use esplora_client::*;

// Re-export all mempool specific types (which also re-exports esplora-types).
pub use mempool_types::*;

// Mempool-specific client implementations.
// These shadow the esplora_client::{BlockingClient, AsyncClient} exports above,
// providing mempool.space specific methods while maintaining full Esplora API
// compatibility via Deref.
#[cfg(feature = "blocking")]
pub mod blocking;
#[cfg(feature = "blocking")]
pub use blocking::BlockingClient;

#[cfg(feature = "async")]
pub mod r#async;
#[cfg(feature = "async")]
pub use r#async::AsyncClient;

// Wraps esplora\_client::Builder so that `Builder::build_blocking()`/
// `build_async()` return the mempool-aware client types, instead of
// the plain esplora-client ones. This is what lets `Builder::new(url).build_blocking()` - the
// idiomatic esplora-client construction path keep working unchanged for
// downstream code that only swaps its Cargo.toml dependency.
//
// This shadows the glob-imported `esplora_client::Builder` the same way
// `BlockingClient`/`AsyncClient` above shadow their esplora-client
// counterparts. It lives here instead of its own module because, unlike
// `BlockingClient`/`AsyncClient`, it has no mempool specific methods of its
// own to justify one. It exists purely to redirect two return types.
//
// Every setter and build method below mirrors its `esplora_client::Builder`
// counterpart 1:1 - same name, same signature, same `#[cfg(...)]` gate,
// same order -- and does nothing but delegate to it. This is deliberate:
// it's not new configuration surface, just the minimum indirection needed to
// hand back a different return type; Rust doesn't allow adding inherent
// methods to a foreign type, so there's no way to intercept
// `esplora_client::Builder::build_blocking()` from outside `esplora-client`.
//
// `esplora_client::Builder` itself is still reachable through `Deref`, so
// reading its public fields works exactly as before.
/// Configures and constructs [`BlockingClient`] and [`AsyncClient`].
///
/// Derefs to [`esplora_client::Builder`] for read access to its fields.
/// Every setter is re-implemented here (rather than exposed only through
/// `Deref`) because they consume `self` and return `Self` -- `Deref`
/// cannot forward owned-`self` methods, only `&self`/`&mut self` ones.
#[derive(Debug, Clone)]
pub struct Builder(esplora_client::Builder);

impl Builder {
    /// Create a [`Builder`] for a mempool.space compatible server base URL.
    pub fn new(base_url: &str) -> Self {
        Self(esplora_client::Builder::new(base_url))
    }

    /// Set the proxy URL used for requests.
    ///
    /// The proxy is ignored when targeting `wasm32`.
    pub fn proxy(mut self, proxy: &str) -> Self {
        self.0 = self.0.proxy(proxy);
        self
    }

    /// Set the per-request socket timeout.
    pub fn timeout(mut self, timeout: std::time::Duration) -> Self {
        self.0 = self.0.timeout(timeout);
        self
    }

    /// Add or replace an HTTP header sent with every request.
    pub fn header(mut self, key: &str, value: &str) -> Self {
        self.0 = self.0.header(key, value);
        self
    }

    /// Set the maximum number of retry attempts for retryable responses.
    ///
    /// Only responses whose status code is listed in
    /// [`RETRYABLE_ERROR_CODES`] are retried.
    pub fn max_retries(mut self, count: usize) -> Self {
        self.0 = self.0.max_retries(count);
        self
    }

    /// Set the maximum number of cached connections in the async client.
    #[cfg(feature = "async")]
    pub fn max_connections(mut self, count: usize) -> Self {
        self.0 = self.0.max_connections(count);
        self
    }

    /// Build a [`BlockingClient`] from this configuration.
    #[cfg(feature = "blocking")]
    pub fn build_blocking(self) -> BlockingClient {
        BlockingClient::from_inner(self.0.build_blocking())
    }

    /// Build an [`AsyncClient`] from this configuration.
    ///
    /// This uses `DefaultSleeper`, which is backed by Tokio.
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] if the async HTTP client cannot be constructed.
    #[cfg(all(feature = "async", feature = "tokio"))]
    pub fn build_async(self) -> Result<AsyncClient, Error> {
        Ok(AsyncClient::from_inner(self.0.build_async()?))
    }

    /// Build an [`AsyncClient`] with a user-defined [`Sleeper`].
    ///
    /// Use this when integrating with an async runtime other than Tokio or
    /// when tests need a custom sleep implementation.
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] if the async HTTP client cannot be constructed.
    #[cfg(feature = "async")]
    pub fn build_async_with_sleeper<S: Sleeper>(self) -> Result<AsyncClient<S>, Error> {
        Ok(AsyncClient::from_inner(self.0.build_async_with_sleeper()?))
    }
}

impl std::ops::Deref for Builder {
    type Target = esplora_client::Builder;

    fn deref(&self) -> &esplora_client::Builder {
        &self.0
    }
}
