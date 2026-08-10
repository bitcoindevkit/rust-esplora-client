// SPDX-License-Identifier: MIT OR Apache-2.0

//! # Mempool Types
//!
//! mempool.space specific types, layered on top of [`esplora_types`]. Most
//! of mempool.space's legacy `/api/*` endpoints return byte-compatible
//! Esplora response shapes, so this crate re-exports [`esplora_types`] in
//! full rather than redefining them, and only adds the types unique to
//! mempool.space's `/api/v1/*` endpoints.
//!
//! Refer to the [Mempool API] specification for the complete API reference.
//!
//! [Mempool API]: <https://mempool.space/docs/api/rest>
#![warn(missing_docs)]

pub use esplora_types::*;

use serde::Deserialize;

/// A set of recommended fee estimates.
///
/// Returned by both the `/api/v1/fees/recommended` and `/api/v1/fees/precise`
/// endpoints. The precise endpoint provides sub-satoshi resolution; the
/// recommended endpoint rounds to the nearest sat/vB.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecommendedFees {
    /// The recommended fee rate for the next block.
    pub fastest_fee: f64,
    /// The recommended fee rate targeting confirmation within approximately 30 minutes.
    pub half_hour_fee: f64,
    /// The recommended fee rate targeting confirmation within approximately one hour.
    pub hour_fee: f64,
    /// The recommended economical fee rate.
    pub economy_fee: f64,
    /// The minimum relay fee rate currently accepted.
    pub minimum_fee: f64,
}

/// A projected mempool block.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MempoolBlock {
    /// The projected block size in bytes.
    pub block_size: usize,
    /// The projected block size in virtual bytes.
    pub block_v_size: f64,
    /// The number of transactions in this projected block.
    pub n_tx: usize,
    /// The total fees paid by transactions in this projected block, in satoshis.
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    pub total_fees: Amount,
    /// The median fee rate in this projected block, in sat/vB.
    pub median_fee: f64,
    /// The fee rate distribution across this projected block, in sat/vB.
    ///
    /// Values are ordered from lowest to highest.
    pub fee_range: Vec<f64>,
}

/// Difficulty adjustment statistics for the current epoch.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DifficultyAdjustment {
    /// The percentage of the current epoch completed.
    pub progress_percent: f64,
    /// The estimated percentage change in difficulty at the next retarget.
    pub difficulty_change: f64,
    /// The estimated retarget date as a UNIX timestamp in milliseconds.
    pub estimated_retarget_date: u64,
    /// The number of blocks remaining until the next retarget.
    pub remaining_blocks: u32,
    /// The estimated remaining time until the next retarget, in seconds.
    pub remaining_time: u64,
    /// The percentage difficulty change from the previous retarget.
    pub previous_retarget: f64,
    /// The UNIX timestamp of the previous retarget block.
    pub previous_time: u64,
    /// The block height of the next retarget.
    pub next_retarget_height: u32,
    /// The average time between blocks in the current epoch, in milliseconds.
    pub time_avg: u64,
    /// The adjusted average time between blocks in the current epoch, in milliseconds.
    pub adjusted_time_avg: u64,
    /// The time offset applied to the average, in milliseconds.
    pub time_offset: i64,
    /// The expected number of blocks mined so far in the current epoch.
    pub expected_blocks: f64,
}

/// Current Bitcoin price in multiple fiat currencies.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct Prices {
    /// The UNIX timestamp of the price data.
    #[serde(rename = "time")]
    pub time: u64,
    /// The price in US Dollars.
    pub usd: u64,
    /// The price in Euros.
    pub eur: u64,
    /// The price in British Pounds.
    pub gbp: u64,
    /// The price in Canadian Dollars.
    pub cad: u64,
    /// The price in Swiss Francs.
    pub chf: u64,
    /// The price in Australian Dollars.
    pub aud: u64,
    /// The price in Japanese Yen.
    pub jpy: u64,
}

/// A Bitcoin price at a specific point in time.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub struct HistoricalPricePoint {
    /// The UNIX timestamp of this price entry.
    #[serde(rename = "time")]
    pub time: u64,
    /// The price in US Dollars, if requested.
    pub usd: Option<f64>,
    /// The price in Euros, if requested.
    pub eur: Option<f64>,
    /// The price in British Pounds, if requested.
    pub gbp: Option<f64>,
    /// The price in Canadian Dollars, if requested.
    pub cad: Option<f64>,
    /// The price in Swiss Francs, if requested.
    pub chf: Option<f64>,
    /// The price in Australian Dollars, if requested.
    pub aud: Option<f64>,
    /// The price in Japanese Yen, if requested.
    pub jpy: Option<f64>,
}

/// Fiat-to-fiat exchange rates returned alongside historical price data.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ExchangeRates {
    /// USD to EUR exchange rate.
    #[serde(rename = "USDEUR")]
    pub usd_eur: f64,
    /// USD to GBP exchange rate.
    #[serde(rename = "USDGBP")]
    pub usd_gbp: f64,
    /// USD to CAD exchange rate.
    #[serde(rename = "USDCAD")]
    pub usd_cad: f64,
    /// USD to CHF exchange rate.
    #[serde(rename = "USDCHF")]
    pub usd_chf: f64,
    /// USD to AUD exchange rate.
    #[serde(rename = "USDAUD")]
    pub usd_aud: f64,
    /// USD to JPY exchange rate.
    #[serde(rename = "USDJPY")]
    pub usd_jpy: f64,
}

/// Historical Bitcoin price data.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoricalPrice {
    /// The historical price entries for the requested currency and time range.
    pub prices: Vec<HistoricalPricePoint>,
    /// Fiat-to-fiat exchange rates at the time of the query.
    pub exchange_rates: ExchangeRates,
}

/// Address validation result.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct ValidateAddress {
    /// Whether the address is valid.
    #[serde(rename = "isvalid")]
    pub is_valid: bool,
    /// The address that was validated.
    pub address: String,
    /// The scriptPubKey hex for this address.
    #[serde(rename = "scriptPubKey")]
    pub script_pub_key: String,
    /// Whether the address is a script hash (P2SH).
    #[serde(rename = "isscript")]
    pub is_script: bool,
    /// Whether the address is a witness address (SegWit).
    #[serde(rename = "iswitness")]
    pub is_witness: bool,
    /// The SegWit witness version, if applicable.
    pub witness_version: Option<u8>,
    /// The SegWit witness program hex, if applicable.
    pub witness_program: Option<String>,
}

/// Mining pool information for a block.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MiningPool {
    /// The pool's internal identifier.
    pub id: u32,
    /// The pool's display name.
    pub name: String,
    /// The pool's URL slug.
    pub slug: String,
    /// The pool's known miner names, if any.
    pub miner_names: Option<Vec<String>>,
}

/// Extended block statistics.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockExtras {
    /// Total fees collected in this block, in satoshis.
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    pub total_fees: Amount,
    /// Median fee rate in this block, in sat/vB.
    pub median_fee: f64,
    /// Fee rate distribution across this block, in sat/vB.
    pub fee_range: Vec<f64>,
    /// Total block reward (subsidy + fees), in satoshis.
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    pub reward: Amount,
    /// The mining pool that mined this block.
    pub pool: MiningPool,
    /// Average fee per transaction in this block, in satoshis.
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    pub avg_fee: Amount,
    /// Average fee rate in this block, in sat/vB.
    pub avg_fee_rate: f64,
    /// The raw coinbase transaction hex.
    pub coinbase_raw: String,
    /// The primary coinbase output address, if applicable.
    pub coinbase_address: Option<String>,
    /// All coinbase output addresses.
    pub coinbase_addresses: Vec<String>,
    /// The coinbase script in human-readable form.
    pub coinbase_signature: String,
    /// The coinbase script decoded as ASCII.
    pub coinbase_signature_ascii: Option<String>,
    /// Average transaction size in this block, in bytes.
    pub avg_tx_size: f64,
    /// Total number of inputs across all transactions.
    pub total_inputs: u32,
    /// Total number of outputs across all transactions.
    pub total_outputs: u32,
    /// Total output amount across all transactions, in satoshis.
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    pub total_output_amt: Amount,
    /// Median fee amount per transaction in this block, in satoshis.
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    pub median_fee_amt: Amount,
    /// Fee percentile distribution in this block, in satoshis.
    pub fee_percentiles: Vec<u64>,
    /// Number of SegWit transactions in this block.
    pub segwit_total_txs: u32,
    /// Total size of SegWit transaction data, in bytes.
    pub segwit_total_size: f64,
    /// Total weight of SegWit transaction data.
    pub segwit_total_weight: u64,
    /// The raw block header hex.
    pub header: String,
    /// Net change in the UTXO set size from this block.
    pub utxo_set_change: i64,
    /// UTXO set size after this block.
    pub utxo_set_size: u64,
    /// Total input amount across all transactions, in satoshis.
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    pub total_input_amt: Amount,
    /// The block's virtual size in vbytes.
    pub virtual_size: f64,
    /// UNIX timestamp when this block was first seen, if available.
    pub first_seen: Option<u64>,
    /// Orphaned transactions replaced by this block, if any.
    pub orphans: Vec<String>,
    /// Percentage of expected transactions included in this block.
    pub match_rate: Option<f64>,
    /// Expected total fees for this block, in satoshis.
    pub expected_fees: Option<u64>,
    /// Expected total weight for this block.
    pub expected_weight: Option<u64>,
}

/// Extended block information.
///
/// Extends [`BlockInfo`] with mempool-specific [`BlockExtras`] statistics.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct BlockDetails {
    /// The standard block summary fields.
    #[serde(flatten)]
    pub info: BlockInfo,
    /// Additional mempool-specific statistics for this block.
    pub extras: BlockExtras,
}

/// The block closest to a given timestamp.
///
/// Returned by the `/api/v1/mining/blocks/timestamp/:timestamp` endpoint.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct BlockAtTimestamp {
    /// The block height.
    pub height: u32,
    /// The block hash.
    pub hash: BlockHash,
    /// The block timestamp as an ISO 8601 datetime string.
    pub timestamp: String,
}

/// A transaction in a CPFP ancestor or descendant chain.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CpfpTransaction {
    /// The transaction ID.
    pub txid: Txid,
    /// The fee paid by this transaction, in satoshis.
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    pub fee: Amount,
    /// The weight of this transaction.
    pub weight: u64,
    /// The adjusted virtual size used for fee-rate calculations.
    pub adjusted_vsize: f64,
    /// The number of signature operations in this transaction.
    pub sigops: u32,
    /// The effective fee rate in sat/vB.
    pub fee_per_vsize: f64,
    /// The adjusted effective fee rate in sat/vB.
    pub adjusted_fee_per_vsize: f64,
}

/// CPFP (Child Pays For Parent) data for a transaction.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CpfpInfo {
    /// Unconfirmed ancestor transactions boosted by this transaction's fee.
    pub ancestors: Vec<CpfpTransaction>,
    /// Unconfirmed descendant transactions that boost this transaction's fee.
    #[serde(default)]
    pub descendants: Vec<CpfpTransaction>,
    /// The effective fee rate across the CPFP package, in sat/vB.
    pub effective_fee_per_vsize: Option<f64>,
}

/// A compact transaction summary used within an RBF replacement tree.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct RbfTransaction {
    /// The transaction ID.
    pub txid: Txid,
    /// The fee paid by this transaction, in satoshis.
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    pub fee: Amount,
    /// The virtual size of this transaction.
    pub vsize: f64,
    /// The total output value of this transaction, in satoshis.
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    pub value: Amount,
}

/// A node in an RBF replacement tree.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RbfTree {
    /// The transaction at this node.
    pub tx: RbfTransaction,
    /// The UNIX timestamp when this replacement was first seen.
    pub time: u64,
    /// Milliseconds since the previous replacement, if known.
    pub interval: Option<u64>,
    /// Whether this is a full RBF replacement (without opt-in signaling).
    pub full_rbf: bool,
    /// Whether this transaction has been mined.
    pub mined: bool,
    /// The transactions this one replaced.
    pub replaces: Vec<RbfTree>,
}

/// RBF replacement history for a transaction.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct RbfInfo {
    /// The replacement tree for this transaction, if it was replaced.
    pub replacements: Option<RbfTree>,
    /// The transaction this one replaced, if any.
    pub replaces: Option<RbfTree>,
}

/// A compact transaction summary used within a mempool replacement tree.
///
/// Similar to [`RbfTransaction`] but includes additional fields.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplacementTransaction {
    /// The transaction ID.
    pub txid: Txid,
    /// The fee paid by this transaction, in satoshis.
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    pub fee: Amount,
    /// The virtual size of this transaction.
    pub vsize: f64,
    /// The total output value of this transaction, in satoshis.
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    pub value: Amount,
    /// The fee rate of this transaction, in sat/vB.
    pub rate: f64,
    /// Whether this transaction signals opt-in RBF (BIP125).
    pub rbf: bool,
    /// Whether this is a full RBF replacement (without opt-in signaling).
    #[serde(default)]
    pub full_rbf: bool,
}

/// A node in a mempool replacement tree.
///
/// Returned as elements of the list from `mempool_client::BlockingClient::get_replacements`
/// and `mempool_client::BlockingClient::get_full_rbf_replacements`.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplacementTree {
    /// The transaction at this node.
    pub tx: ReplacementTransaction,
    /// The UNIX timestamp when this replacement was first seen.
    pub time: u64,
    /// Whether this is a full RBF replacement (without opt-in signaling).
    ///
    /// Absent on opt-in RBF entries returned by `/api/v1/replacements`; defaults to `false`.
    #[serde(default)]
    pub full_rbf: bool,
    /// The transactions this one replaced, if any.
    pub replaces: Option<Vec<ReplacementTree>>,
}
