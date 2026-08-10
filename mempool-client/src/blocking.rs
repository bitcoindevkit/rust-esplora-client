// SPDX-License-Identifier: MIT OR Apache-2.0

//! # Blocking Mempool Client
//!
//! This module implements [`BlockingClient`], a synchronous HTTP client for
//! interacting with a [mempool.space] server by way of [`esplora_client::BlockingClient`].
//!
//! Use this client from synchronous applications, command-line tools, tests,
//! or code paths where blocking the current thread is acceptable. Each method
//! sends the request immediately and returns only after the response body has
//! been read and decoded.
//!
//! The client is configured through [`Builder`], including the
//! base URL, proxy, socket timeout, custom headers, and retry count.
//!
//! # Deref Behavior
//!
//! [`BlockingClient`] derefs to [`esplora_client::BlockingClient`], so every
//! standard Esplora method is available unchanged, with no code written here for any of them.
//! Methods whose behavior differs on mempool.space or are entirely new are added
//! as inherent methods.
//!
//! # Example
//!
//! ```rust,no_run
//! # fn example() -> Result<(), mempool_client::Error> {
//!
//! let client = mempool_client::Builder::new("https://mempool.space/api").build_blocking();
//!
//! // Standard Esplora methods via Deref
//! let height = client.get_height()?;
//!
//! // Mempool-specific methods
//! let fees = client.get_recommended_fees()?;
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
use esplora_client::BlockingClient as EsploraBlockingClient;
use std::collections::HashMap;
use std::ops::Deref;

/// A blocking client for mempool.space.
///
/// Derefs to [`esplora_client::BlockingClient`] for every standard Esplora
/// method this crate doesn't explicitly shadow or add.
#[derive(Debug, Clone)]
pub struct BlockingClient(EsploraBlockingClient);

impl BlockingClient {
    /// Wrap an already-built [`esplora_client::BlockingClient`].
    ///
    /// Only visible within this crate; [`Builder::build_blocking`] and
    /// [`Self::from_builder`]/[`Self::new`] are the public entry points.
    pub(crate) fn from_inner(inner: EsploraBlockingClient) -> Self {
        Self(inner)
    }

    /// Create a [`BlockingClient`] from an already-configured [`Builder`].
    ///
    /// Use this when you need to set a timeout, proxy, custom headers, or
    /// retry count. Equivalent to [`Builder::build_blocking`].
    /// No network request is made until a client method is called.
    ///
    /// ```no_run
    /// use mempool_client::{BlockingClient, Builder};
    /// use std::time::Duration;
    ///
    /// let client = BlockingClient::from_builder(
    ///     Builder::new("https://mempool.space/api")
    ///         .timeout(Duration::from_secs(10))
    ///         .max_retries(3),
    /// );
    /// ```
    pub fn from_builder(builder: Builder) -> Self {
        builder.build_blocking()
    }

    /// Create a [`BlockingClient`] for the given mempool.space-compatible
    /// server base URL, using default configuration.
    ///
    /// Equivalent to `Builder::new(base_url).build_blocking()`.
    /// Use [`Self::from_builder`] directly if you need to configure the
    /// client beyond just its base URL.
    pub fn new(base_url: &str) -> Self {
        Builder::new(base_url).build_blocking()
    }

    /// Get fee estimates for a range of confirmation targets.
    ///
    /// Returns a [`HashMap`] where the key is the confirmation target in blocks
    /// and the value is the estimated [`FeeRate`].
    #[deprecated(
        note = "This method uses mempool.space's deprecated `/fee-estimates` endpoint, which will be removed in a future mempool release. Use `get_precise_fees()` instead."
    )]
    pub fn get_fee_estimates(&self) -> Result<HashMap<u16, FeeRate>, Error> {
        let estimates_raw: HashMap<u16, f64> = self.0.get_response_json("/fee-estimates")?;
        let estimates = sat_per_vbyte_to_feerate(estimates_raw);

        Ok(estimates)
    }

    /// Get fee estimates with sub-satoshi precision.
    ///
    /// Returns [`RecommendedFees`] containing the current fee estimates from the
    /// `/api/v1/fees/precise` endpoint.
    pub fn get_precise_fees(&self) -> Result<RecommendedFees, Error> {
        self.0.get_response_json("/v1/fees/precise")
    }

    /// Get currently recommended fee estimates.
    ///
    /// Returns [`RecommendedFees`] containing the current fee estimates from the
    /// `/api/v1/fees/recommended` endpoint. Values are rounded to the nearest
    /// sat/vB. For sub-satoshi precision use [`Self::get_precise_fees`].
    pub fn get_recommended_fees(&self) -> Result<RecommendedFees, Error> {
        self.0.get_response_json("/v1/fees/recommended")
    }

    /// Get the current mempool represented as projected blocks.
    ///
    /// Returns a [`Vec`] of [`MempoolBlock`], each representing a block's worth
    /// of transactions currently in the mempool, ordered from next-to-confirm
    /// to furthest from confirmation. Each entry includes the projected block
    /// size, transaction count, total fees, median fee rate, and fee rate
    /// distribution.
    pub fn get_mempool_block_fees(&self) -> Result<Vec<MempoolBlock>, Error> {
        self.0.get_response_json("/v1/fees/mempool-blocks")
    }

    /// Get difficulty adjustment statistics for the current epoch.
    ///
    /// Returns a [`DifficultyAdjustment`] containing progress, estimated retarget
    /// date, remaining blocks, and block interval averages.
    pub fn get_difficulty_adjustment(&self) -> Result<DifficultyAdjustment, Error> {
        self.0.get_response_json("/v1/difficulty-adjustment")
    }

    /// Get the current Bitcoin price in multiple fiat currencies.
    ///
    /// Returns a [`Prices`] containing the latest price in USD, EUR, GBP, CAD,
    /// CHF, AUD, and JPY from the `/api/v1/prices` endpoint.
    pub fn get_price(&self) -> Result<Prices, Error> {
        self.0.get_response_json("/v1/prices")
    }

    /// Get historical Bitcoin price data.
    ///
    /// Returns a [`HistoricalPrice`] containing price entries and exchange rates.
    ///
    /// `currency` filters results to a specific fiat currency (e.g. `"USD"`).
    /// `timestamp` returns the price at or before that UNIX timestamp.
    /// Both parameters are optional.
    pub fn get_historical_price(
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
        self.0.get_response_json(&path)
    }

    /// Validate a Bitcoin address.
    ///
    /// Returns a [`ValidateAddress`] indicating whether the address is valid,
    /// its script type, and SegWit details if applicable.
    pub fn get_address_validation(&self, address: &str) -> Result<ValidateAddress, Error> {
        self.0
            .get_response_json(&format!("/v1/validate-address/{address}"))
    }

    /// Get extended block details by block hash.
    ///
    /// Returns a [`BlockDetails`] containing both the standard [`crate::BlockInfo`]
    /// fields and mempool-specific [`crate::BlockExtras`] statistics.
    pub fn get_block_details(&self, hash: &BlockHash) -> Result<BlockDetails, Error> {
        self.0.get_response_json(&format!("/v1/block/{hash}"))
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
    pub fn get_blocks_details(
        &self,
        start_height: Option<u32>,
    ) -> Result<Vec<BlockDetails>, Error> {
        let path = match start_height {
            Some(h) => format!("/v1/blocks/{h}"),
            None => "/v1/blocks".to_string(),
        };
        let blocks: Vec<BlockDetails> = self.0.get_response_json(&path)?;
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
    pub fn get_blocks_bulk(
        &self,
        min_height: u32,
        max_height: u32,
    ) -> Result<Vec<BlockDetails>, Error> {
        let blocks: Vec<BlockDetails> = self
            .0
            .get_response_json(&format!("/v1/blocks-bulk/{min_height}/{max_height}"))?;
        if blocks.is_empty() {
            return Err(Error::InvalidResponse);
        }
        Ok(blocks)
    }

    /// Get the block closest to a given timestamp.
    ///
    /// Returns a [`BlockAtTimestamp`] with the height, hash, and ISO 8601 timestamp
    /// of the block nearest to `timestamp` (a UNIX timestamp in seconds).
    pub fn get_block_by_timestamp(&self, timestamp: u64) -> Result<BlockAtTimestamp, Error> {
        self.0
            .get_response_json(&format!("/v1/mining/blocks/timestamp/{timestamp}"))
    }

    /// Get CPFP (Child Pays For Parent) data for a transaction.
    ///
    /// Returns a [`CpfpInfo`] describing the unconfirmed ancestors whose fees
    /// are being boosted by this transaction, any descendants boosting this
    /// transaction, and the effective fee rate across the package.
    pub fn get_tx_cpfp(&self, txid: &Txid) -> Result<CpfpInfo, Error> {
        self.0.get_response_json(&format!("/v1/cpfp/{txid}"))
    }

    /// Get RBF (Replace By Fee) replacement history for a transaction.
    ///
    /// Returns an [`RbfInfo`] containing the replacement tree for this
    /// transaction and the transaction it replaced, if any.
    pub fn get_tx_rbf(&self, txid: &Txid) -> Result<RbfInfo, Error> {
        self.0.get_response_json(&format!("/v1/tx/{txid}/rbf"))
    }

    /// Get the first-seen timestamps for a list of transactions.
    ///
    /// Returns a [`Vec<u64>`] of UNIX timestamps, one per [`Txid`], in the
    /// same order as the input. A value of `0` means the transaction was not
    /// found in the mempool index.
    pub fn get_transaction_times(&self, txids: &[Txid]) -> Result<Vec<u64>, Error> {
        let params = txids
            .iter()
            .map(|t| format!("txId[]={t}"))
            .collect::<Vec<_>>()
            .join("&");
        self.0
            .get_response_json(&format!("/v1/transaction-times?{params}"))
    }

    /// Get recent opt-in RBF replacement transactions from the mempool.
    ///
    /// Returns a list of [`ReplacementTree`] nodes representing the most recent
    /// RBF (Replace By Fee) replacements detected in the mempool, including
    /// both opt-in and full-RBF replacements.
    pub fn get_replacements(&self) -> Result<Vec<ReplacementTree>, Error> {
        self.0.get_response_json("/v1/replacements")
    }

    /// Get recent full-RBF replacement transactions from the mempool.
    ///
    /// Returns a list of [`ReplacementTree`] nodes representing only the
    /// full-RBF replacements detected in the mempool, those that replaced
    /// transactions without opt-in signaling.
    pub fn get_full_rbf_replacements(&self) -> Result<Vec<ReplacementTree>, Error> {
        self.0.get_response_json("/v1/fullrbf/replacements")
    }
}

impl Deref for BlockingClient {
    type Target = EsploraBlockingClient;

    fn deref(&self) -> &EsploraBlockingClient {
        &self.0
    }
}
