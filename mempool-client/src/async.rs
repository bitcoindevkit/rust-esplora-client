// SPDX-License-Identifier: MIT OR Apache-2.0

//! # Asynchronous Mempool Client
//!
//! This module implements [`AsyncClient`], an asynchronous HTTP client for
//! interacting with a [mempool.space] server by way of [`esplora_client::AsyncClient`].
//!
//! Use this client from async applications and libraries. Each method returns a
//! future that sends the request, waits for the response, and decodes the body
//! into the requested type.
//!
//! The client is configured through [`Builder`], including the
//! base URL, proxy, socket timeout, custom headers, retry count, and maximum
//! number of cached connections. Retry sleeping is abstracted through
//! [`Sleeper`], so runtimes other than Tokio can provide their own sleep
//! implementation.
//!
//! # Deref Behavior
//!
//! [`AsyncClient`] derefs to [`esplora_client::AsyncClient`], so every
//! standard Esplora method is available unchanged, with no code written here for any of them.
//! Methods whose behavior differs on mempool.space or are entirely new are added
//! as inherent methods.
//!
//! # Example
//!
//! ```rust,ignore
//! # use mempool_client::Builder;
//! # async fn example() -> Result<(), mempool_client::Error> {
//!
//! let client = Builder::new("https://mempool.space/api").build_async()?;
//!
//! // Standard Esplora methods via Deref
//! let height = client.get_height().await?;
//!
//! // Mempool-specific methods
//! let fees = client.get_recommended_fees().await?;
//!
//! # Ok(())
//! # }
//! ```
//!
//! [mempool.space]: https://mempool.space/docs/api

use crate::{
    sat_per_vbyte_to_feerate, BlockAtTimestamp, BlockDetails, BlockHash, Builder, CpfpInfo,
    DifficultyAdjustment, Error, FeeRate, HistoricalPrice, MempoolBlock, Prices, RbfInfo,
    RecommendedFees, ReplacementTree, Txid, ValidateAddress,
};
use esplora_client::r#async::{AsyncClient as EsploraAsyncClient, DefaultSleeper, Sleeper};
use std::collections::HashMap;
use std::ops::Deref;

/// An async client for mempool.space.
///
/// Derefs to [`esplora_client::AsyncClient`] for every standard Esplora
/// method this crate doesn't explicitly shadow or add.
///
/// The generic parameter `S` determines the asynchronous runtime used for
/// sleeping between retries. Defaults to the Tokio-backed [`DefaultSleeper`].
#[derive(Clone)]
pub struct AsyncClient<S = DefaultSleeper>(EsploraAsyncClient<S>);

impl<S: Sleeper> AsyncClient<S> {
    /// Wrap an already-built [`esplora_client::r#async::AsyncClient`].
    ///
    /// Only visible within this crate; [`Builder::build_async`]/
    /// [`Builder::build_async_with_sleeper`] and [`Self::from_builder`]/
    /// [`Self::new`] are the public entry points.
    pub(crate) fn from_inner(inner: EsploraAsyncClient<S>) -> Self {
        Self(inner)
    }

    /// Create an [`AsyncClient`] from an already-configured [`Builder`].
    ///
    /// Use this when you need to set a timeout, proxy, custom headers, or
    /// retry count. Equivalent to [`Builder::build_async_with_sleeper`].
    /// No network request is made until a client method is awaited.
    ///
    /// ```no_run
    /// use mempool_client::{r#async::AsyncClient, Builder};
    /// use std::time::Duration;
    ///
    /// # async fn example() -> Result<(), mempool_client::Error> {
    /// let client: AsyncClient = AsyncClient::from_builder(
    ///     Builder::new("https://mempool.space/api")
    ///         .timeout(Duration::from_secs(10))
    ///         .max_retries(3),
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn from_builder(builder: Builder) -> Result<Self, Error> {
        builder.build_async_with_sleeper()
    }

    /// Get fee estimates for a range of confirmation targets.
    ///
    /// Returns a [`HashMap`] where the key is the confirmation target in blocks
    /// and the value is the estimated [`FeeRate`].
    #[deprecated(
        note = "This method uses mempool.space's deprecated `/fee-estimates` endpoint, which will be removed in a future mempool release. Use `get_precise_fees()` instead."
    )]
    pub async fn get_fee_estimates(&self) -> Result<HashMap<u16, FeeRate>, Error> {
        let estimates_raw: HashMap<u16, f64> = self.0.get_response_json("/fee-estimates").await?;
        let estimates = sat_per_vbyte_to_feerate(estimates_raw);

        Ok(estimates)
    }

    /// Get fee estimates with sub-satoshi precision.
    ///
    /// Returns [`RecommendedFees`] containing the current fee estimates from the
    /// `/api/v1/fees/precise` endpoint.
    pub async fn get_precise_fees(&self) -> Result<RecommendedFees, Error> {
        self.0.get_response_json("/v1/fees/precise").await
    }

    /// Get currently recommended fee estimates.
    ///
    /// Returns [`RecommendedFees`] containing the current fee estimates from the
    /// `/api/v1/fees/recommended` endpoint. Values are rounded to the nearest
    /// sat/vB. For sub-satoshi precision use [`Self::get_precise_fees`].
    pub async fn get_recommended_fees(&self) -> Result<RecommendedFees, Error> {
        self.0.get_response_json("/v1/fees/recommended").await
    }

    /// Get the current mempool represented as projected blocks.
    ///
    /// Returns a [`Vec`] of [`MempoolBlock`], each representing a block's worth
    /// of transactions currently in the mempool, ordered from next-to-confirm
    /// to furthest from confirmation. Each entry includes the projected block
    /// size, transaction count, total fees, median fee rate, and fee rate
    /// distribution.
    pub async fn get_mempool_block_fees(&self) -> Result<Vec<MempoolBlock>, Error> {
        self.0.get_response_json("/v1/fees/mempool-blocks").await
    }

    /// Get difficulty adjustment statistics for the current epoch.
    ///
    /// Returns a [`DifficultyAdjustment`] containing progress, estimated retarget
    /// date, remaining blocks, and block interval averages.
    pub async fn get_difficulty_adjustment(&self) -> Result<DifficultyAdjustment, Error> {
        self.0.get_response_json("/v1/difficulty-adjustment").await
    }

    /// Get the current Bitcoin price in multiple fiat currencies.
    ///
    /// Returns a [`Prices`] containing the latest price in USD, EUR, GBP, CAD,
    /// CHF, AUD, and JPY from the `/api/v1/prices` endpoint.
    pub async fn get_price(&self) -> Result<Prices, Error> {
        self.0.get_response_json("/v1/prices").await
    }

    /// Get historical Bitcoin price data.
    ///
    /// Returns a [`HistoricalPrice`] containing price entries and exchange rates.
    ///
    /// `currency` filters results to a specific fiat currency (e.g. `"USD"`).
    /// `timestamp` returns the price at or before that UNIX timestamp.
    pub async fn get_historical_price(
        &self,
        currency: Option<&str>,
        timestamp: Option<u64>,
    ) -> Result<HistoricalPrice, Error> {
        let mut params = vec![];
        if let Some(c) = currency {
            params.push(format!("currency={c}"));
        }
        if let Some(t) = timestamp {
            params.push(format!("timestamp={t}"));
        }
        let path = if params.is_empty() {
            "/v1/historical-price".to_string()
        } else {
            format!("/v1/historical-price?{}", params.join("&"))
        };
        self.0.get_response_json(&path).await
    }

    /// Validate a Bitcoin address.
    ///
    /// Returns a [`ValidateAddress`] indicating whether the address is valid,
    /// its script type, and SegWit details if applicable.
    pub async fn get_address_validation(&self, address: &str) -> Result<ValidateAddress, Error> {
        self.0
            .get_response_json(&format!("/v1/validate-address/{address}"))
            .await
    }

    /// Get extended block details by block hash.
    ///
    /// Returns a [`BlockDetails`] containing both the standard [`crate::BlockInfo`]
    /// fields and mempool-specific [`crate::BlockExtras`] statistics.
    pub async fn get_block_details(&self, hash: &BlockHash) -> Result<BlockDetails, Error> {
        self.0.get_response_json(&format!("/v1/block/{hash}")).await
    }

    /// Get [`BlockDetails`] for recent blocks.
    ///
    /// If `start_height` is `Some(h)`, returns blocks starting from height `h`.
    /// If `start_height` is `None`, returns blocks starting from the current tip.
    ///
    /// Returns up to 15 blocks per call.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidResponse`] if the server returns an empty list.
    pub async fn get_blocks_details(
        &self,
        start_height: Option<u32>,
    ) -> Result<Vec<BlockDetails>, Error> {
        let path = match start_height {
            Some(h) => format!("/v1/blocks/{h}"),
            None => "/v1/blocks".to_string(),
        };
        let blocks: Vec<BlockDetails> = self.0.get_response_json(&path).await?;
        if blocks.is_empty() {
            return Err(Error::InvalidResponse);
        }
        Ok(blocks)
    }

    /// Get [`BlockDetails`] for a range of blocks in bulk.
    ///
    /// Returns all blocks between `min_height` and `max_height` inclusive.
    /// The range is limited to 10 blocks per call by the server.
    ///
    /// **Note:** This endpoint is disabled on the public mempool.space API.
    /// It requires a self-hosted instance with `config.MEMPOOL.MAX_BLOCKS_BULK_QUERY`
    /// set to a positive number.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidResponse`] if the server returns an empty list.
    pub async fn get_blocks_bulk(
        &self,
        min_height: u32,
        max_height: u32,
    ) -> Result<Vec<BlockDetails>, Error> {
        let blocks: Vec<BlockDetails> = self
            .0
            .get_response_json(&format!("/v1/blocks-bulk/{min_height}/{max_height}"))
            .await?;
        if blocks.is_empty() {
            return Err(Error::InvalidResponse);
        }
        Ok(blocks)
    }

    /// Get the block closest to a given timestamp.
    ///
    /// Returns a [`BlockAtTimestamp`] with the height, hash, and ISO 8601 timestamp
    /// of the block nearest to `timestamp` (a UNIX timestamp in seconds).
    pub async fn get_block_by_timestamp(&self, timestamp: u64) -> Result<BlockAtTimestamp, Error> {
        self.0
            .get_response_json(&format!("/v1/mining/blocks/timestamp/{timestamp}"))
            .await
    }

    /// Get CPFP (Child Pays For Parent) data for a transaction.
    ///
    /// Returns a [`CpfpInfo`] describing the unconfirmed ancestors whose fees
    /// are being boosted by this transaction, any descendants boosting this
    /// transaction, and the effective fee rate across the package.
    pub async fn get_tx_cpfp(&self, txid: &Txid) -> Result<CpfpInfo, Error> {
        self.0.get_response_json(&format!("/v1/cpfp/{txid}")).await
    }

    /// Get RBF (Replace By Fee) replacement history for a transaction.
    ///
    /// Returns an [`RbfInfo`] containing the replacement tree for this
    /// transaction and the transaction it replaced, if any.
    pub async fn get_tx_rbf(&self, txid: &Txid) -> Result<RbfInfo, Error> {
        self.0
            .get_response_json(&format!("/v1/tx/{txid}/rbf"))
            .await
    }

    /// Get the first-seen timestamps for a list of transactions.
    ///
    /// Returns a [`Vec<u64>`] of UNIX timestamps, one per [`Txid`], in the
    /// same order as the input. A value of `0` means the transaction was not
    /// found in the mempool index.
    pub async fn get_transaction_times(&self, txids: &[Txid]) -> Result<Vec<u64>, Error> {
        let params = txids
            .iter()
            .map(|t| format!("txId[]={t}"))
            .collect::<Vec<_>>()
            .join("&");
        self.0
            .get_response_json(&format!("/v1/transaction-times?{params}"))
            .await
    }

    /// Get recent opt-in RBF replacement transactions from the mempool.
    ///
    /// Returns a list of [`ReplacementTree`] nodes representing the most recent
    /// RBF (Replace By Fee) replacements detected in the mempool, including
    /// both opt-in and full-RBF replacements.
    pub async fn get_replacements(&self) -> Result<Vec<ReplacementTree>, Error> {
        self.0.get_response_json("/v1/replacements").await
    }

    /// Get recent full-RBF replacement transactions from the mempool.
    ///
    /// Returns a list of [`ReplacementTree`] nodes representing only the
    /// full-RBF replacements detected in the mempool — those that replaced
    /// transactions without opt-in signaling.
    pub async fn get_full_rbf_replacements(&self) -> Result<Vec<ReplacementTree>, Error> {
        self.0.get_response_json("/v1/fullrbf/replacements").await
    }
}

impl<S> Deref for AsyncClient<S> {
    type Target = EsploraAsyncClient<S>;

    fn deref(&self) -> &EsploraAsyncClient<S> {
        &self.0
    }
}

/// Create a [`AsyncClient`] with the default Tokio-backed sleeper.
///
/// This is only available when the `tokio` feature is enabled.
#[cfg(feature = "tokio")]
impl AsyncClient<DefaultSleeper> {
    /// Create an [`AsyncClient`] for the given mempool.space compatible
    /// server base URL, using default configuration and the Tokio sleeper.
    ///
    /// Equivalent to `Builder::new(base_url).build_async()`.
    /// Use [`Self::from_builder`] directly if you need to configure the
    /// client beyond just its base URL.
    pub fn new(base_url: &str) -> Result<Self, Error> {
        Builder::new(base_url).build_async()
    }
}
