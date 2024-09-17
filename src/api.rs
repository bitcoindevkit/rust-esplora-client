//! Structures from the esplora API
//!
//! See: <https://github.com/Blockstream/esplora/blob/master/API.md>

use core::str;
use std::{future::Future, str::FromStr};

use async_trait::async_trait;
use bitcoin::consensus::Decodable;
pub use bitcoin::consensus::{deserialize, serialize};
use bitcoin::hashes::sha256::Hash;
pub use bitcoin::hex::FromHex;
use bitcoin::Weight;
pub use bitcoin::{
    transaction, Amount, BlockHash, OutPoint, ScriptBuf, Transaction, TxIn, TxOut, Txid, Witness,
};

use hex::DisplayHex;
use serde::Deserialize;

/// An HTTP request method.
pub enum Method {
    /// The GET method
    Get,
    /// The POST method
    Post,
}

/// A URL type for requests.
type Url = String;

/// A minimal HTTP request.
pub struct Request {
    pub method: Method,
    pub url: Url,
    pub body: Option<Vec<u8>>,
}

impl Request {
    fn new(method: Method, url: Url, body: Option<Vec<u8>>) -> Self {
        Self { method, url, body }
    }
}

#[derive(Debug)]
#[allow(unused)]
pub struct Response {
    pub status_code: i32,
    pub body: Vec<u8>,
    // pub reason: String,
    // pub headers: HashMap<String, String>,
    // pub url: Url,
}

impl Response {
    pub fn new(
        status_code: i32,
        body: Vec<u8>,
        // reason_phrase: String,
        // headers: HashMap<String, String>,
        // url: Url,
    ) -> Self {
        Self {
            status_code,
            body,
            // reason: reason_phrase,
            // headers,
            // url,
        }
    }

    pub fn is_status_ok(&self) -> bool {
        self.status_code == 200
    }

    pub fn as_str(&self) -> Result<&str, crate::Error> {
        match str::from_utf8(&self.body) {
            Ok(s) => Ok(s),
            Err(e) => Err(crate::Error::InvalidUtf8InBody(e)),
        }
    }
}

pub enum TransactionApi {
    Tx(Txid),
    TxInfo(Txid),
    TxStatus(Txid),
    TxMerkeBlockProof(Txid),
    TxMerkleProof(Txid),
    TxOutputStatus(Txid, u64),
    Broadcast(Transaction),
}

impl Client for TransactionApi {
    fn request(&self, base_url: &str) -> Request {
        match self {
            TransactionApi::Tx(txid) => {
                Request::new(Method::Get, format!("{base_url}/tx/{txid}/raw"), None)
            }
            TransactionApi::TxStatus(txid) => {
                Request::new(Method::Get, format!("{base_url}/tx/{txid}/status"), None)
            }
            TransactionApi::TxInfo(txid) => {
                Request::new(Method::Get, format!("{base_url}/tx/{txid}"), None)
            }
            TransactionApi::TxMerkeBlockProof(txid) => Request::new(
                Method::Get,
                format!("{base_url}/tx/{txid}/merkleblock-proof"),
                None,
            ),
            TransactionApi::TxMerkleProof(txid) => Request::new(
                Method::Get,
                format!("{base_url}/tx/{txid}/merkle-proof"),
                None,
            ),
            TransactionApi::TxOutputStatus(txid, index) => Request::new(
                Method::Get,
                format!("{base_url}/tx/{txid}/outspend/{index}"),
                None,
            ),
            TransactionApi::Broadcast(tx) => Request::new(
                Method::Post,
                format!("{base_url}/tx"),
                Some(
                    bitcoin::consensus::encode::serialize(tx)
                        .to_lower_hex_string()
                        .as_bytes()
                        .to_vec(),
                ),
            ),
        }
    }

    fn deserialize_decodable<T: Decodable>(&self, response: &Response) -> Result<T, crate::Error> {
        if !response.is_status_ok() {
            let status = u16::try_from(response.status_code).map_err(crate::Error::StatusCode)?;
            let message = response.as_str()?.to_string();
            return Err(crate::Error::HttpResponse { status, message });
        }

        match self {
            TransactionApi::TxMerkeBlockProof(_) => {
                let hex_str = response.as_str()?;
                let hex_vec = Vec::from_hex(hex_str)?;
                deserialize::<T>(&hex_vec).map_err(crate::Error::BitcoinEncoding)
            }
            _ => deserialize::<T>(&response.body).map_err(crate::Error::BitcoinEncoding),
        }
    }

    fn deserialize_json<T: serde::de::DeserializeOwned>(
        &self,
        response: &Response,
    ) -> Result<T, crate::Error> {
        if !response.is_status_ok() {
            let status = u16::try_from(response.status_code).map_err(crate::Error::StatusCode)?;
            let message = response.as_str()?.to_string();
            return Err(crate::Error::HttpResponse { status, message });
        }

        serde_json::from_slice(&response.body).map_err(crate::Error::SerdeJsonError)
    }

    fn deserialize_str<T: FromStr>(&self, _response: &Response) -> Result<T, crate::Error> {
        unimplemented!("It's currently not required by `TransactionApi`")
    }
}

pub enum AddressApi {
    ScriptHashTxHistory(Hash),
    ScriptHashConfirmedTxHistory(Hash, Txid),
}

impl Client for AddressApi {
    fn request(&self, base_url: &str) -> Request {
        match self {
            AddressApi::ScriptHashTxHistory(script_hash) => Request::new(
                Method::Get,
                format!("{base_url}/scripthash/{:x}/txs", script_hash),
                None,
            ),
            AddressApi::ScriptHashConfirmedTxHistory(script_hash, last_seen) => Request::new(
                Method::Get,
                format!(
                    "{base_url}/scripthash/{:x}/txs/chain/{}",
                    script_hash, last_seen
                ),
                None,
            ),
        }
    }

    fn deserialize_decodable<T: Decodable>(&self, _response: &Response) -> Result<T, crate::Error> {
        unimplemented!("It's currently not required by `AddressApi`")
    }

    fn deserialize_json<T: serde::de::DeserializeOwned>(
        &self,
        response: &Response,
    ) -> Result<T, crate::Error> {
        if !response.is_status_ok() {
            let status = u16::try_from(response.status_code).map_err(crate::Error::StatusCode)?;
            let message = response.as_str()?.to_string();
            return Err(crate::Error::HttpResponse { status, message });
        }

        serde_json::from_slice(&response.body).map_err(crate::Error::SerdeJsonError)
    }

    fn deserialize_str<T: FromStr>(&self, _response: &Response) -> Result<T, crate::Error> {
        unimplemented!("It's currently not required by `AddressApi`")
    }
}

pub enum BlocksApi {
    BlockTxIdAtIndex(BlockHash, usize),
    BlockHeader(BlockHash),
    BlockStatus(BlockHash),
    BlockRaw(BlockHash),
    BlockTipHeight,
    BlockTipHash,
    BlockHash(u32),
    BlockSummaries(Option<u32>),
}

impl Client for BlocksApi {
    fn request(&self, base_url: &str) -> Request {
        match self {
            BlocksApi::BlockTxIdAtIndex(block_hash, index) => Request::new(
                Method::Get,
                format!("{base_url}/block/{block_hash}/txid/{index}"),
                None,
            ),
            BlocksApi::BlockHeader(block_hash) => Request::new(
                Method::Get,
                format!("{base_url}/block/{block_hash}/header"),
                None,
            ),
            BlocksApi::BlockStatus(block_hash) => Request::new(
                Method::Get,
                format!("{base_url}/block/{block_hash}/status"),
                None,
            ),
            BlocksApi::BlockRaw(block_hash) => Request::new(
                Method::Get,
                format!("{base_url}/block/{block_hash}/raw"),
                None,
            ),
            BlocksApi::BlockTipHeight => {
                Request::new(Method::Get, format!("{base_url}/blocks/tip/height"), None)
            }
            BlocksApi::BlockTipHash => {
                Request::new(Method::Get, format!("{base_url}/blocks/tip/hash"), None)
            }
            BlocksApi::BlockHash(block_height) => Request::new(
                Method::Get,
                format!("{base_url}/block-height/{block_height}"),
                None,
            ),
            BlocksApi::BlockSummaries(block_height) => match block_height {
                Some(height) => {
                    Request::new(Method::Get, format!("{base_url}/blocks/{height}"), None)
                }
                None => Request::new(Method::Get, format!("{base_url}/blocks"), None),
            },
        }
    }

    fn deserialize_decodable<T: Decodable>(&self, response: &Response) -> Result<T, crate::Error> {
        if !response.is_status_ok() {
            let status = u16::try_from(response.status_code).map_err(crate::Error::StatusCode)?;
            let message = response.as_str()?.to_string();
            return Err(crate::Error::HttpResponse { status, message });
        }

        match self {
            BlocksApi::BlockHeader(_) => {
                let hex_str = response.as_str()?;
                let hex_vec = Vec::from_hex(hex_str)?;
                deserialize::<T>(&hex_vec).map_err(crate::Error::BitcoinEncoding)
            },
            BlocksApi::BlockRaw(_) => {
                deserialize::<T>(&response.body).map_err(crate::Error::BitcoinEncoding)
            },
            _ => unimplemented!("It cannot be deserialized by `deserialize_decodable`, use either `deserialize_str` or `deserialize_json` instead.")
        }
    }

    fn deserialize_json<T: serde::de::DeserializeOwned>(
        &self,
        response: &Response,
    ) -> Result<T, crate::Error> {
        if !response.is_status_ok() {
            let status = u16::try_from(response.status_code).map_err(crate::Error::StatusCode)?;
            let message = response.as_str()?.to_string();
            return Err(crate::Error::HttpResponse { status, message });
        }

        match self {
            BlocksApi::BlockStatus(_) | BlocksApi::BlockSummaries(_) => {
                serde_json::from_slice(&response.body).map_err(crate::Error::SerdeJsonError)
            }
            BlocksApi::BlockHeader(_) | BlocksApi::BlockRaw(_) => {
                unimplemented!("It cannot be deserialized by `deserialize_json`, use `deserialize_decodable` instead.")
            }
            BlocksApi::BlockTxIdAtIndex(_, _)
            | BlocksApi::BlockTipHeight
            | BlocksApi::BlockTipHash
            | BlocksApi::BlockHash(_) => {
                unimplemented!("It cannot be deserialized by `deserialize_json`, use `deserialize_str` instead.")
            }
        }
    }

    // TODO: (@leonardo) how can we return proper error here instead of unwrap ?
    fn deserialize_str<T: FromStr>(&self, response: &Response) -> Result<T, crate::Error>
    where
        <T as FromStr>::Err: std::fmt::Debug,
    {
        if !response.is_status_ok() {
            let status = u16::try_from(response.status_code).map_err(crate::Error::StatusCode)?;
            let message = response.as_str()?.to_string();
            return Err(crate::Error::HttpResponse { status, message });
        }

        match self {
            BlocksApi::BlockTxIdAtIndex(_, _) | BlocksApi::BlockTipHash | BlocksApi::BlockHash(_) | BlocksApi::BlockTipHeight => {
                Ok(T::from_str(response.as_str()?).unwrap()) // FIXME: (@leonardo) remove this unwrap
            }
            BlocksApi::BlockHeader(_) | BlocksApi::BlockRaw(_) => unimplemented!("It cannot be deserialized by `deserialize_str`, use `deserialize_decodable` instead."),
            BlocksApi::BlockStatus(_) | BlocksApi::BlockSummaries(_) => unimplemented!("It cannot be deserialized by `deserialize_str`, use `deserialize_json` instead."),
        }
    }
}

pub enum FeeEstimatesApi {
    FeeRate,
}

impl Client for FeeEstimatesApi {
    fn request(&self, base_url: &str) -> Request {
        match self {
            FeeEstimatesApi::FeeRate => {
                Request::new(Method::Get, format!("{base_url}/fee-estimates"), None)
            }
        }
    }

    fn deserialize_decodable<T: Decodable>(&self, _response: &Response) -> Result<T, crate::Error> {
        unimplemented!("It's currently not required by `FeeEstimatesApi`")
    }

    fn deserialize_json<T: serde::de::DeserializeOwned>(
        &self,
        response: &Response,
    ) -> Result<T, crate::Error> {
        if !response.is_status_ok() {
            let status = u16::try_from(response.status_code).map_err(crate::Error::StatusCode)?;
            let message = response.as_str()?.to_string();
            return Err(crate::Error::HttpResponse { status, message });
        }

        serde_json::from_slice(&response.body).map_err(crate::Error::SerdeJsonError)
    }

    fn deserialize_str<T: FromStr>(&self, _response: &Response) -> Result<T, crate::Error>
    where
        <T as FromStr>::Err: std::fmt::Debug,
    {
        unimplemented!("It's currently not required by `FeeEstimatesApi`")
    }
}

#[derive(Debug)]
pub enum Error<E> {
    Client(E),
}

#[async_trait]
pub trait Client {
    fn request(&self, base_url: &str) -> Request;

    fn send<F, E>(&self, base_url: &str, handler: &mut F) -> Result<Response, Error<E>>
    where
        F: FnMut(Request) -> Result<Response, E>,
    {
        let request = self.request(base_url);
        handler(request).map_err(Error::Client)
    }

    async fn send_async<'a, F, Fut, E>(
        &'a self,
        base_url: &'a str,
        handler: &'a mut F,
    ) -> Result<Response, Error<E>>
    where
        F: FnMut(Request) -> Fut + Send,
        Fut: Future<Output = Result<Response, E>> + Send + Sync,
        Self: Sync,
    {
        let request = self.request(base_url);
        handler(request).await.map_err(Error::Client)
    }

    fn deserialize_decodable<T: Decodable>(&self, response: &Response) -> Result<T, crate::Error>;

    fn deserialize_json<T: serde::de::DeserializeOwned>(
        &self,
        response: &Response,
    ) -> Result<T, crate::Error>;

    fn deserialize_str<T: FromStr>(&self, response: &Response) -> Result<T, crate::Error>
    where
        <T as FromStr>::Err: std::fmt::Debug;
}

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
