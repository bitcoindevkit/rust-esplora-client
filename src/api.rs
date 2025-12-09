//! Structs from the Esplora API
//!
//! See: <https://github.com/Blockstream/esplora/blob/master/API.md>

use bitcoin::hash_types;
use serde::Deserialize;

pub use bitcoin::consensus::{deserialize, serialize};
pub use bitcoin::hex::FromHex;
pub use bitcoin::{
    absolute, block, transaction, Amount, Block, BlockHash, CompactTarget, OutPoint, Script,
    ScriptBuf, ScriptHash, Transaction, TxIn, TxOut, Txid, Weight, Witness,
};

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

/// Information about a bitcoin [`Block`].
#[derive(Debug, Clone, Deserialize)]
pub struct BlockInformation {
    /// The block's [`BlockHash`].
    pub id: BlockHash,
    /// The block's height.
    pub height: u32,
    /// The block's version.
    pub version: block::Version,
    /// The block's timestamp.
    pub timestamp: u64,
    /// The block's transaction count.
    pub tx_count: u64,
    /// The block's size, in bytes.
    pub size: usize,
    /// The block's weight.
    pub weight: u64,
    /// The merkle root of the transactions in the block.
    pub merkle_root: hash_types::TxMerkleNode,
    /// The [`BlockHash`] of the previous block (`None` for the genesis block).
    pub previousblockhash: Option<BlockHash>,
    /// The block's MTP (Median Time Past).
    pub mediantime: u64,
    /// The block's nonce value.
    pub nonce: u32,
    /// The block's `bits` value as a [`CompactTarget`].
    pub bits: CompactTarget,
    /// The block's difficulty target value.
    pub difficulty: f64,
}

impl PartialEq for BlockInformation {
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
impl Eq for BlockInformation {}

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

/// Statistics about a particular [`Script`] hash's confirmed and mempool transactions.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Deserialize)]
pub struct ScriptHashStats {
    /// The summary of confirmed transactions for this [`Script`] hash.
    pub chain_stats: ScriptHashTxsSummary,
    /// The summary of mempool transactions for this [`Script`] hash.
    pub mempool_stats: ScriptHashTxsSummary,
}

/// Contains a summary of the transactions for a particular [`Script`] hash.
pub type ScriptHashTxsSummary = AddressTxsSummary;

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

/// Statistics about the mempool.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct MempoolStats {
    /// The number of transactions in the mempool.
    pub count: usize,
    /// The total size of mempool transactions in virtual bytes.
    pub vsize: usize,
    /// The total fee paid by mempool transactions, in sats.
    pub total_fee: u64,
    /// The mempool's fee rate distribution histogram.
    ///
    /// An array of `(feerate, vsize)` tuples, where each entry's `vsize` is the total vsize
    /// of transactions paying more than `feerate` but less than the previous entry's `feerate`
    /// (except for the first entry, which has no upper bound).
    pub fee_histogram: Vec<(f64, usize)>,
}

/// A [`Transaction`] that recently entered the mempool.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct MempoolRecentTx {
    /// Transaction ID as a [`Txid`].
    pub txid: Txid,
    /// [`Amount`] of fees paid by the transaction, in satoshis.
    pub fee: u64,
    /// The transaction size, in virtual bytes.
    pub vsize: usize,
    /// Combined [`Amount`] of the transaction, in satoshis.
    pub value: u64,
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
