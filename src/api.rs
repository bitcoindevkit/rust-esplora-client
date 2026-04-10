// Bitcoin Dev Kit
// Written in 2020 by Alekos Filini <alekos.filini@gmail.com>
//
// Copyright (c) 2020-2026 Bitcoin Dev Kit Developers
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

//! Structs and types returned by the Esplora API.
//!
//! This module defines the data structures used to deserialize responses from
//! an [Esplora](https://github.com/Blockstream/esplora) server. These types
//! are used throughout the [`crate::blocking`] and [`crate::async`] clients.
//!
//! See the [Esplora API documentation](https://github.com/Blockstream/esplora/blob/master/API.md)
//! for the full API reference.

use bitcoin::hash_types;
use serde::Deserialize;
use std::collections::HashMap;

pub use bitcoin::consensus::{deserialize, serialize};
pub use bitcoin::hex::FromHex;
pub use bitcoin::{
    absolute, block, transaction, Address, Amount, Block, BlockHash, CompactTarget, FeeRate,
    OutPoint, Script, ScriptBuf, ScriptHash, Transaction, TxIn, TxMerkleNode, TxOut, Txid, Weight,
    Witness, Wtxid,
};

// ----> TRANSACTION

/// An input to a [`Transaction`].
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Vin {
    /// The [`Txid`] of the previous [`Transaction`] this input spends from.
    pub txid: Txid,
    /// The output index of the previous output in the [`Transaction`] that created it.
    pub vout: u32,
    /// The previous output amount and ScriptPubKey.
    /// `None` if this is a coinbase input.
    pub prevout: Option<Vout>,
    /// The ScriptSig that authorizes spending this input.
    pub scriptsig: ScriptBuf,
    /// The witness that authorizes spending this input, if this is a SegWit spend.
    #[serde(deserialize_with = "deserialize_witness", default)]
    pub witness: Vec<Vec<u8>>,
    /// The sequence value for this input, used to set RBF and locktime rules.
    pub sequence: u32,
    /// Whether this is a coinbase input.
    pub is_coinbase: bool,
}

/// An output from a [`Transaction`].
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Vout {
    /// The value of the output, in satoshis.
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    pub value: Amount,
    /// The ScriptPubKey that the output is locked to.
    pub scriptpubkey: ScriptBuf,
}

/// The confirmation status of a [`Transaction`].
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TxStatus {
    /// Whether the [`Transaction`] is confirmed or not.
    pub confirmed: bool,
    /// The block height the [`Transaction`] was confirmed in.
    pub block_height: Option<u32>,
    /// The [`BlockHash`] of the block the [`Transaction`] was confirmed in.
    pub block_hash: Option<BlockHash>,
    /// The time that the block was mined at, as a UNIX timestamp.
    ///
    /// Note: this timestamp is set by the miner and may not reflect the exact time of mining.
    pub block_time: Option<u64>,
}

/// A transaction in the format returned by Esplora.
///
/// Unlike the native [`Transaction`] type from `rust-bitcoin`, this struct
/// includes additional metadata such as the confirmation status, fee, and
/// weight, as reported by the Esplora API.
///
/// Use [`EsploraTx::to_tx`] or `.into()` to convert it to a [`Transaction`].
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct EsploraTx {
    /// The [`Txid`] of the [`Transaction`].
    pub txid: Txid,
    /// The version number of the [`Transaction`].
    pub version: i32,
    /// The locktime of the [`Transaction`].
    /// Sets a time or height after which the [`Transaction`] can be mined.
    pub locktime: u32,
    /// The array of inputs in the [`Transaction`].
    pub vin: Vec<Vin>,
    /// The array of outputs in the [`Transaction`].
    pub vout: Vec<Vout>,
    /// The [`Transaction`] size in raw bytes (NOT virtual bytes).
    pub size: usize,
    /// The [`Transaction`]'s weight.
    pub weight: Weight,
    /// The confirmation status of the [`Transaction`].
    pub status: TxStatus,
    /// The fee paid by the [`Transaction`], in satoshis.
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    pub fee: Amount,
}

impl EsploraTx {
    /// Convert this [`EsploraTx`] into a `rust-bitcoin` [`Transaction`].
    ///
    /// Drops the Esplora-specific metadata (fee, weight, confirmation status)
    /// and reconstructs the [`Transaction`] from its inputs and outputs.
    pub fn to_tx(&self) -> Transaction {
        Transaction {
            version: transaction::Version::non_standard(self.version),
            lock_time: bitcoin::absolute::LockTime::from_consensus(self.locktime),
            input: self
                .vin
                .iter()
                .cloned()
                .map(|vin| TxIn {
                    previous_output: OutPoint {
                        txid: vin.txid,
                        vout: vin.vout,
                    },
                    script_sig: vin.scriptsig,
                    sequence: bitcoin::Sequence(vin.sequence),
                    witness: Witness::from_slice(&vin.witness),
                })
                .collect(),
            output: self
                .vout
                .iter()
                .cloned()
                .map(|vout| TxOut {
                    value: vout.value,
                    script_pubkey: vout.scriptpubkey,
                })
                .collect(),
        }
    }

    /// Get the confirmation time of this [`EsploraTx`].
    ///
    /// Returns a [`BlockTime`] containing the block height and timestamp if the
    /// [`Transaction`] is confirmed, or `None` if it is unconfirmed.
    pub fn confirmation_time(&self) -> Option<BlockTime> {
        match self.status {
            TxStatus {
                confirmed: true,
                block_height: Some(height),
                block_time: Some(timestamp),
                ..
            } => Some(BlockTime { timestamp, height }),
            _ => None,
        }
    }

    /// Get the previous [`TxOut`]s spent by this [`EsploraTx`]'s inputs.
    ///
    /// Returns a [`Vec`] of [`Option<TxOut>`], one per input, in order.
    /// Each entry is `None` if the input is a coinbase input (which has no previous output).
    pub fn previous_outputs(&self) -> Vec<Option<TxOut>> {
        self.vin
            .iter()
            .cloned()
            .map(|vin| {
                vin.prevout.map(|prevout| TxOut {
                    script_pubkey: prevout.scriptpubkey,
                    value: prevout.value,
                })
            })
            .collect()
    }
}

impl From<EsploraTx> for Transaction {
    fn from(tx: EsploraTx) -> Self {
        tx.to_tx()
    }
}

impl From<&EsploraTx> for Transaction {
    fn from(tx: &EsploraTx) -> Self {
        tx.to_tx()
    }
}

/// A Merkle inclusion proof for a [`Transaction`], given its [`Txid`].
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct MerkleProof {
    /// The height of the block the [`Transaction`] was confirmed in.
    pub block_height: u32,
    /// A list of transaction hashes the current hash is paired with,
    /// recursively, in order to trace up to obtain the Merkle root of the
    /// [`Block`], deepest pairing first.
    pub merkle: Vec<Txid>,
    /// The 0-based index of the position of the [`Transaction`] in the
    /// ordered list of [`Transaction`]s in the [`Block`].
    pub pos: usize,
}

/// The spend status of a [`TxOut`].
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct OutputStatus {
    /// Whether the [`TxOut`] has been spent or not.
    pub spent: bool,
    /// The [`Txid`] of the [`Transaction`] that spent this [`TxOut`].
    pub txid: Option<Txid>,
    /// The input index of this [`TxOut`] in the [`Transaction`] that spent it.
    pub vin: Option<u64>,
    /// Information about the [`Transaction`] that spent this [`TxOut`].
    pub status: Option<TxStatus>,
}

// ----> BLOCK

/// The timestamp and height of a [`Block`].
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BlockTime {
    /// The [`Block`]'s timestamp.
    pub timestamp: u64,
    /// The [`Block`]'s height.
    pub height: u32,
}

/// The status of a [`Block`].
#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BlockStatus {
    /// Whether this [`Block`] belongs to the chain with the most
    /// Proof-of-Work (`false` for [`Block`]s that belong to a stale chain).
    pub in_best_chain: bool,
    /// The height of this [`Block`].
    pub height: Option<u32>,
    /// The [`BlockHash`] of the [`Block`] that builds on top of this one.
    pub next_best: Option<BlockHash>,
}

// TODO(@luisschwab): remove on `v0.14.0`
/// Summary about a [`Block`].
#[deprecated(since = "0.12.3", note = "use `BlockInfo` instead")]
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BlockSummary {
    /// The [`Block`]'s hash.
    pub id: BlockHash,
    /// The [`Block`]'s timestamp and height.
    #[serde(flatten)]
    pub time: BlockTime,
    /// The [`BlockHash`] of the previous [`Block`] (`None` for the genesis [`Block`]).
    pub previousblockhash: Option<BlockHash>,
    /// The Merkle root of the [`Block`]'s [`Transaction`]s.
    pub merkle_root: TxMerkleNode,
}

/// A summary of a bitcoin [`Block`].
///
/// Contains block metadata as returned by the Esplora API, but not the
/// full block contents. Use the client's `get_block_by_hash` to retrieve
/// the full [`Block`].
#[derive(Debug, Clone, Deserialize)]
pub struct BlockInfo {
    /// The [`Block`]'s [`BlockHash`].
    pub id: BlockHash,
    /// The [`Block`]'s height.
    pub height: u32,
    /// The [`Block`]'s version.
    pub version: block::Version,
    /// The [`Block`]'s UNIX timestamp.
    pub timestamp: u64,
    /// The [`Block`]'s [`Transaction`] count.
    pub tx_count: u64,
    /// The [`Block`]'s size, in bytes.
    pub size: usize,
    /// The [`Block`]'s weight.
    pub weight: Weight,
    /// The Merkle root of the [`Transaction`]s in the [`Block`].
    pub merkle_root: hash_types::TxMerkleNode,
    /// The [`BlockHash`] of the previous [`Block`] (`None` for the genesis block).
    pub previousblockhash: Option<BlockHash>,
    /// The [`Block`]'s MTP (Median Time Past).
    pub mediantime: u64,
    /// The [`Block`]'s nonce value.
    pub nonce: u32,
    /// The [`Block`]'s `bits` value as a [`CompactTarget`].
    pub bits: CompactTarget,
    /// The [`Block`]'s difficulty target value.
    ///
    /// Uses a manual [`PartialEq`] impl because [`f64`] does not implement [`Eq`].
    pub difficulty: f64,
}

// Manual PartialEq impl required because `difficulty` is an `f64`, which does not implement `Eq`.
// `NaN` values are considered equal to each other for the purposes of this comparison.
impl PartialEq for BlockInfo {
    fn eq(&self, other: &Self) -> bool {
        let Self { difficulty: d1, .. } = self;
        let Self { difficulty: d2, .. } = other;

        self.id == other.id
            && self.height == other.height
            && self.version == other.version
            && self.timestamp == other.timestamp
            && self.tx_count == other.tx_count
            && self.size == other.size
            && self.weight == other.weight
            && self.merkle_root == other.merkle_root
            && self.previousblockhash == other.previousblockhash
            && self.mediantime == other.mediantime
            && self.nonce == other.nonce
            && self.bits == other.bits
            && ((d1.is_nan() && d2.is_nan()) || (d1 == d2))
    }
}
impl Eq for BlockInfo {}

// ----> ADDRESS

/// Statistics about an [`Address`].
///
/// The address is stored as a [`String`] rather than an [`Address`] because
/// the Esplora API returns it without network context.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct AddressStats {
    /// The [`Address`], as a string.
    pub address: String,
    /// The summary of confirmed [`Transaction`]s for this [`Address`].
    pub chain_stats: AddressTxsSummary,
    /// The summary of unconfirmed mempool [`Transaction`]s for this [`Address`].
    pub mempool_stats: AddressTxsSummary,
}

/// A summary of [`Transaction`]s in which an [`Address`] was involved.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Deserialize)]
pub struct AddressTxsSummary {
    /// The number of funded [`TxOut`]s.
    pub funded_txo_count: u32,
    /// The total value of funded [`TxOut`]s, in satoshis.
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    pub funded_txo_sum: Amount,
    /// The number of spent [`TxOut`]s.
    pub spent_txo_count: u32,
    /// The total value of spent [`TxOut`]s, in satoshis.
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    pub spent_txo_sum: Amount,
    /// The total number of [`Transaction`]s.
    pub tx_count: u32,
}

// ----> SCRIPT HASH

/// Statistics about a [`Script`] hash's confirmed and mempool transactions.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Deserialize)]
pub struct ScriptHashStats {
    /// The summary of confirmed [`Transaction`]s for this [`Script`] hash.
    pub chain_stats: ScriptHashTxsSummary,
    /// The summary of unconfirmed mempool [`Transaction`]s for this [`Script`] hash.
    pub mempool_stats: ScriptHashTxsSummary,
}

/// A summary of [`Transaction`]s for a particular [`Script`] hash.
///
/// Identical in structure to [`AddressTxsSummary`].
pub type ScriptHashTxsSummary = AddressTxsSummary;

// ----> UTXO

/// The confirmation status of a [`TxOut`].
#[derive(Debug, Copy, Clone, PartialEq, Eq, Deserialize)]
pub struct UtxoStatus {
    /// Whether the [`TxOut`] is confirmed.
    pub confirmed: bool,
    /// The block height in which the [`TxOut`] was confirmed.
    pub block_height: Option<u32>,
    /// The [`BlockHash`] of the block in which the [`TxOut`] was confirmed.
    pub block_hash: Option<BlockHash>,
    /// The UNIX timestamp of the block in which the [`TxOut`] was confirmed.
    pub block_time: Option<u64>,
}

/// An unspent [`TxOut`], including its outpoint, confirmation status and value.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Deserialize)]
pub struct Utxo {
    /// The [`Txid`] of the [`Transaction`] that created this [`TxOut`].
    pub txid: Txid,
    /// The output index of this [`TxOut`] in the [`Transaction`] that created it.
    pub vout: u32,
    /// The confirmation status of this [`TxOut`].
    pub status: UtxoStatus,
    /// The value of this [`TxOut`], in satoshis.
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    pub value: Amount,
}

// ----> MEMPOOL

/// Statistics about the mempool.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct MempoolStats {
    /// The number of [`Transaction`]s in the mempool.
    pub count: usize,
    /// The total size of mempool [`Transaction`]s, in virtual bytes.
    pub vsize: usize,
    /// The total fee paid by mempool [`Transaction`]s, in satoshis.
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    pub total_fee: Amount,
    /// The mempool's fee rate distribution histogram.
    ///
    /// An array of `(feerate, vsize)` tuples, where each entry's `vsize` is the total vsize
    /// of [`Transaction`]s paying more than `feerate` but less than the previous entry's `feerate`
    /// (except for the first entry, which has no upper bound).
    pub fee_histogram: Vec<(f64, usize)>,
}

/// A [`Transaction`] that recently entered the mempool.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct MempoolRecentTx {
    /// The [`Transaction`]'s [`Txid`].
    pub txid: Txid,
    /// The fee paid by the [`Transaction`], in satoshis.
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    pub fee: Amount,
    /// The [`Transaction`]'s size, in virtual bytes.
    pub vsize: usize,
    /// The total output value of the [`Transaction`], in satoshis.
    #[serde(with = "bitcoin::amount::serde::as_sat")]
    pub value: Amount,
}

/// The result of broadcasting a package of [`Transaction`]s.
#[derive(Deserialize, Debug)]
pub struct SubmitPackageResult {
    /// The transaction package result message.
    ///
    /// `"success"` indicates all transactions were accepted into or are already in the mempool.
    pub package_msg: String,
    /// Transaction results keyed by [`Wtxid`].
    #[serde(rename = "tx-results")]
    pub tx_results: HashMap<Wtxid, TxResult>,
    /// List of [`Txid`]s of transactions replaced by this package.
    #[serde(rename = "replaced-transactions")]
    pub replaced_transactions: Option<Vec<Txid>>,
}

/// The result for a single [`Transaction`] in a broadcasted package.
#[derive(Deserialize, Debug)]
pub struct TxResult {
    /// The [`Txid`] of the [`Transaction`].
    pub txid: Txid,
    /// The [`Wtxid`] of a different transaction with the same [`Txid`] but different witness
    /// found in the mempool.
    ///
    /// If set, this means the submitted transaction was ignored.
    #[serde(rename = "other-wtxid")]
    pub other_wtxid: Option<Wtxid>,
    /// Sigops-adjusted virtual transaction size.
    pub vsize: Option<u32>,
    /// Transaction fees.
    pub fees: Option<MempoolFeesSubmitPackage>,
    /// The error string if the [`Transaction`] was rejected by the mempool.
    pub error: Option<String>,
}

/// The fees for a [`Transaction`] submitted as part of a package.
#[derive(Deserialize, Debug)]
pub struct MempoolFeesSubmitPackage {
    /// The base transaction fee, in BTC.
    #[serde(with = "bitcoin::amount::serde::as_btc")]
    pub base: Amount,
    /// The effective feerate.
    ///
    /// `None` if the transaction was already in the mempool. May reflect the package
    /// feerate and/or feerate with modified fees from the `prioritisetransaction` RPC method.
    #[serde(
        rename = "effective-feerate",
        default,
        deserialize_with = "deserialize_feerate"
    )]
    pub effective_feerate: Option<FeeRate>,
    /// The [`Wtxid`]s of the transactions whose fees and vsizes are included in
    /// [`Self::effective_feerate`], if it is present.
    #[serde(rename = "effective-includes")]
    pub effective_includes: Option<Vec<Wtxid>>,
}

/// Converts a [`HashMap`] of fee estimates in sat/vbyte (`f64`) to [`FeeRate`].
pub(crate) fn sat_per_vbyte_to_feerate(estimates: HashMap<u16, f64>) -> HashMap<u16, FeeRate> {
    estimates
        .into_iter()
        .map(|(k, v)| (k, FeeRate::from_sat_per_kwu((v * 250_000.0).round() as u64)))
        .collect()
}

/// Deserializes a witness from a list of hex-encoded strings.
///
/// The Esplora API represents witness data as an array of hex strings,
/// e.g. `["deadbeef", "cafebabe"]`. This deserializer decodes each string
/// into raw bytes.
fn deserialize_witness<'de, D>(d: D) -> Result<Vec<Vec<u8>>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    let list = Vec::<String>::deserialize(d)?;
    list.into_iter()
        .map(|hex_str| Vec::<u8>::from_hex(&hex_str))
        .collect::<Result<Vec<Vec<u8>>, _>>()
        .map_err(serde::de::Error::custom)
}

/// Deserializes an optional [`FeeRate`] from a BTC/kvB `f64` value.
///
/// The Esplora API expresses effective feerates as BTC per kilovirtual-byte.
/// This deserializer converts to sat/kwu as required by [`FeeRate`].
///
/// Returns `None` if the value is absent, and an error if the resulting
/// feerate would overflow.
fn deserialize_feerate<'de, D>(d: D) -> Result<Option<FeeRate>, D::Error>
where
    D: serde::de::Deserializer<'de>,
{
    use serde::de::Error;

    let btc_per_kvb = match Option::<f64>::deserialize(d)? {
        Some(v) => v,
        None => return Ok(None),
    };
    let sat_per_kwu = btc_per_kvb * 25_000_000.0;
    if sat_per_kwu.is_infinite() {
        return Err(D::Error::custom("feerate overflow"));
    }
    Ok(Some(FeeRate::from_sat_per_kwu(sat_per_kwu as u64)))
}
