//! Structs from the Esplora API
//!
//! See: <https://github.com/Blockstream/esplora/blob/master/API.md>

use std::collections::HashMap;

pub use bitcoin::consensus::{deserialize, serialize};
pub use bitcoin::hex::FromHex;
pub use bitcoin::{
    transaction, Amount, BlockHash, OutPoint, ScriptBuf, Transaction, TxIn, TxOut, Txid, Witness,
};
use bitcoin::{FeeRate, Weight, Wtxid};
use serde::Deserialize;

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PrevOut {
    pub value: u64,
    pub scriptpubkey: ScriptBuf,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Vin {
    pub txid: Txid,
    pub vout: u32,
    // None if coinbase
    pub prevout: Option<PrevOut>,
    pub scriptsig: ScriptBuf,
    #[serde(deserialize_with = "deserialize_witness", default)]
    pub witness: Vec<Vec<u8>>,
    pub sequence: u32,
    pub is_coinbase: bool,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Vout {
    pub value: u64,
    pub scriptpubkey: ScriptBuf,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TxStatus {
    pub confirmed: bool,
    pub block_height: Option<u32>,
    pub block_hash: Option<BlockHash>,
    pub block_time: Option<u64>,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct MerkleProof {
    pub block_height: u32,
    pub merkle: Vec<Txid>,
    pub pos: usize,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct OutputStatus {
    pub spent: bool,
    pub txid: Option<Txid>,
    pub vin: Option<u64>,
    pub status: Option<TxStatus>,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BlockStatus {
    pub in_best_chain: bool,
    pub height: Option<u32>,
    pub next_best: Option<BlockHash>,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Tx {
    pub txid: Txid,
    pub version: i32,
    pub locktime: u32,
    pub vin: Vec<Vin>,
    pub vout: Vec<Vout>,
    /// Transaction size in raw bytes (NOT virtual bytes).
    pub size: usize,
    /// Transaction weight units.
    pub weight: u64,
    pub status: TxStatus,
    pub fee: u64,
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BlockTime {
    pub timestamp: u64,
    pub height: u32,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct BlockSummary {
    pub id: BlockHash,
    #[serde(flatten)]
    pub time: BlockTime,
    /// Hash of the previous block, will be `None` for the genesis block.
    pub previousblockhash: Option<bitcoin::BlockHash>,
    pub merkle_root: bitcoin::hash_types::TxMerkleNode,
}

/// Address statistics, includes the address, and the utxo information for the address.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct AddressStats {
    /// The address.
    pub address: String,
    /// The summary of transactions for this address, already on chain.
    pub chain_stats: AddressTxsSummary,
    /// The summary of transactions for this address, currently in the mempool.
    pub mempool_stats: AddressTxsSummary,
}

/// Contains a summary of the transactions for an address.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Deserialize)]
pub struct AddressTxsSummary {
    /// The number of funded transaction outputs.
    pub funded_txo_count: u32,
    /// The sum of the funded transaction outputs, in satoshis.
    pub funded_txo_sum: u64,
    /// The number of spent transaction outputs.
    pub spent_txo_count: u32,
    /// The sum of the spent transaction outputs, in satoshis.
    pub spent_txo_sum: u64,
    /// The total number of transactions.
    pub tx_count: u32,
}

/// Information about an UTXO's status: confirmation status,
/// confirmation height, confirmation block hash and confirmation block time.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Deserialize)]
pub struct UtxoStatus {
    /// Whether or not the UTXO is confirmed.
    pub confirmed: bool,
    /// The block height in which the UTXO was confirmed.
    pub block_height: Option<u32>,
    /// The block hash in which the UTXO was confirmed.
    pub block_hash: Option<BlockHash>,
    /// The UNIX timestamp in which the UTXO was confirmed.
    pub block_time: Option<u64>,
}

/// Information about an UTXO's outpoint, confirmation status and value.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Deserialize)]
pub struct Utxo {
    /// The [`Txid`] of the transaction that created the UTXO.
    pub txid: Txid,
    /// The output index of the UTXO on the transaction that created the it.
    pub vout: u32,
    /// The confirmation status of the UTXO.
    pub status: UtxoStatus,
    /// The value of the UTXO as an [`Amount`].
    pub value: Amount,
}

#[derive(Deserialize, Debug)]
pub struct SubmitPackageResult {
    /// The transaction package result message. "success" indicates all transactions were accepted
    /// into or are already in the mempool.
    pub package_msg: String,
    /// Transaction results keyed by [`Wtxid`].
    #[serde(rename = "tx-results")]
    pub tx_results: HashMap<Wtxid, TxResult>,
    /// List of txids of replaced transactions.
    #[serde(rename = "replaced-transactions")]
    pub replaced_transactions: Option<Vec<Txid>>,
}

#[derive(Deserialize, Debug)]
pub struct TxResult {
    /// The transaction id.
    pub txid: Txid,
    /// The [`Wtxid`] of a different transaction with the same [`Txid`] but different witness found
    /// in the mempool.
    ///
    /// If set, this means the submitted transaction was ignored.
    #[serde(rename = "other-wtxid")]
    pub other_wtxid: Option<Wtxid>,
    /// Sigops-adjusted virtual transaction size.
    pub vsize: Option<u32>,
    /// Transaction fees.
    pub fees: Option<MempoolFeesSubmitPackage>,
    /// The transaction error string, if it was rejected by the mempool
    pub error: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct MempoolFeesSubmitPackage {
    /// Transaction fee.
    #[serde(with = "bitcoin::amount::serde::as_btc")]
    pub base: Amount,
    /// The effective feerate.
    ///
    /// Will be `None` if the transaction was already in the mempool. For example, the package
    /// feerate and/or feerate with modified fees from the `prioritisetransaction` JSON-RPC method.
    #[serde(
        rename = "effective-feerate",
        default,
        deserialize_with = "deserialize_feerate"
    )]
    pub effective_feerate: Option<FeeRate>,
    /// If [`Self::effective_fee_rate`] is provided, this holds the [`Wtxid`]s of the transactions
    /// whose fees and vsizes are included in effective-feerate.
    #[serde(rename = "effective-includes")]
    pub effective_includes: Option<Vec<Wtxid>>,
}

impl Tx {
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
                    value: Amount::from_sat(vout.value),
                    script_pubkey: vout.scriptpubkey,
                })
                .collect(),
        }
    }

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

    pub fn previous_outputs(&self) -> Vec<Option<TxOut>> {
        self.vin
            .iter()
            .cloned()
            .map(|vin| {
                vin.prevout.map(|po| TxOut {
                    script_pubkey: po.scriptpubkey,
                    value: Amount::from_sat(po.value),
                })
            })
            .collect()
    }

    pub fn weight(&self) -> Weight {
        Weight::from_wu(self.weight)
    }

    pub fn fee(&self) -> Amount {
        Amount::from_sat(self.fee)
    }
}

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
