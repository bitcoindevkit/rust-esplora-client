//! Structs from the Esplora API
//!
//! See: <https://github.com/Blockstream/esplora/blob/master/API.md>

pub use bitcoin::consensus::{deserialize, serialize};
pub use bitcoin::hex::FromHex;
use bitcoin::Weight;
pub use bitcoin::{
    transaction, Amount, BlockHash, OutPoint, ScriptBuf, Transaction, TxIn, TxOut, Txid, Witness,
};

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
