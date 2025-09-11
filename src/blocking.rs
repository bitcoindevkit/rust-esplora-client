// Bitcoin Dev Kit
// Written in 2020 by Alekos Filini <alekos.filini@gmail.com>
//
// Copyright (c) 2020-2021 Bitcoin Dev Kit Developers
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

//! Esplora by way of `minreq` HTTP client.

use std::collections::HashMap;
use std::convert::TryFrom;
use std::str::FromStr;
use std::thread;

use bitcoin::consensus::encode::serialize_hex;
#[allow(unused_imports)]
use log::{debug, error, info, trace};

use minreq::{Proxy, Request, Response};

use bitcoin::consensus::{deserialize, serialize, Decodable};
use bitcoin::hashes::{sha256, Hash};
use bitcoin::hex::{DisplayHex, FromHex};
use bitcoin::Address;
use bitcoin::{
    block::Header as BlockHeader, Block, BlockHash, MerkleBlock, Script, Transaction, Txid,
};

use crate::api::AddressStats;
use crate::{
    BlockStatus, BlockSummary, Builder, Error, MerkleProof, OutputStatus, SubmitPackageResult, Tx,
    TxStatus, Utxo, BASE_BACKOFF_MILLIS, RETRYABLE_ERROR_CODES,
};

#[derive(Debug, Clone)]
pub struct BlockingClient {
    /// The URL of the Esplora server.
    url: String,
    /// The proxy is ignored when targeting `wasm32`.
    pub proxy: Option<String>,
    /// Socket timeout.
    pub timeout: Option<u64>,
    /// HTTP headers to set on every request made to Esplora server
    pub headers: HashMap<String, String>,
    /// Number of times to retry a request
    pub max_retries: usize,
}

impl BlockingClient {
    /// Build a blocking client from a [`Builder`]
    pub fn from_builder(builder: Builder) -> Self {
        Self {
            url: builder.base_url,
            proxy: builder.proxy,
            timeout: builder.timeout,
            headers: builder.headers,
            max_retries: builder.max_retries,
        }
    }

    /// Get the underlying base URL.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Perform a raw HTTP GET request with the given URI `path`.
    pub fn get_request(&self, path: &str) -> Result<Request, Error> {
        let mut request = minreq::get(format!("{}{}", self.url, path));

        if let Some(proxy) = &self.proxy {
            let proxy = Proxy::new(proxy.as_str())?;
            request = request.with_proxy(proxy);
        }

        if let Some(timeout) = &self.timeout {
            request = request.with_timeout(*timeout);
        }

        if !self.headers.is_empty() {
            for (key, value) in &self.headers {
                request = request.with_header(key, value);
            }
        }

        Ok(request)
    }

    fn post_request<T>(&self, path: &str, body: T) -> Result<Request, Error>
    where
        T: Into<Vec<u8>>,
    {
        let mut request = minreq::post(format!("{}{}", self.url, path)).with_body(body);

        if let Some(proxy) = &self.proxy {
            let proxy = Proxy::new(proxy.as_str())?;
            request = request.with_proxy(proxy);
        }

        if let Some(timeout) = &self.timeout {
            request = request.with_timeout(*timeout);
        }

        Ok(request)
    }

    fn get_opt_response<T: Decodable>(&self, path: &str) -> Result<Option<T>, Error> {
        match self.get_with_retry(path) {
            Ok(resp) if is_status_not_found(resp.status_code) => Ok(None),
            Ok(resp) if !is_status_ok(resp.status_code) => {
                let status = u16::try_from(resp.status_code).map_err(Error::StatusCode)?;
                let message = resp.as_str().unwrap_or_default().to_string();
                Err(Error::HttpResponse { status, message })
            }
            Ok(resp) => Ok(Some(deserialize::<T>(resp.as_bytes())?)),
            Err(e) => Err(e),
        }
    }

    fn get_opt_response_txid(&self, path: &str) -> Result<Option<Txid>, Error> {
        match self.get_with_retry(path) {
            Ok(resp) if is_status_not_found(resp.status_code) => Ok(None),
            Ok(resp) if !is_status_ok(resp.status_code) => {
                let status = u16::try_from(resp.status_code).map_err(Error::StatusCode)?;
                let message = resp.as_str().unwrap_or_default().to_string();
                Err(Error::HttpResponse { status, message })
            }
            Ok(resp) => Ok(Some(
                Txid::from_str(resp.as_str().map_err(Error::Minreq)?).map_err(Error::HexToArray)?,
            )),
            Err(e) => Err(e),
        }
    }

    fn get_opt_response_hex<T: Decodable>(&self, path: &str) -> Result<Option<T>, Error> {
        match self.get_with_retry(path) {
            Ok(resp) if is_status_not_found(resp.status_code) => Ok(None),
            Ok(resp) if !is_status_ok(resp.status_code) => {
                let status = u16::try_from(resp.status_code).map_err(Error::StatusCode)?;
                let message = resp.as_str().unwrap_or_default().to_string();
                Err(Error::HttpResponse { status, message })
            }
            Ok(resp) => {
                let hex_str = resp.as_str().map_err(Error::Minreq)?;
                let hex_vec = Vec::from_hex(hex_str).unwrap();
                deserialize::<T>(&hex_vec)
                    .map_err(Error::BitcoinEncoding)
                    .map(|r| Some(r))
            }
            Err(e) => Err(e),
        }
    }

    fn get_response_hex<T: Decodable>(&self, path: &str) -> Result<T, Error> {
        match self.get_with_retry(path) {
            Ok(resp) if !is_status_ok(resp.status_code) => {
                let status = u16::try_from(resp.status_code).map_err(Error::StatusCode)?;
                let message = resp.as_str().unwrap_or_default().to_string();
                Err(Error::HttpResponse { status, message })
            }
            Ok(resp) => {
                let hex_str = resp.as_str().map_err(Error::Minreq)?;
                let hex_vec = Vec::from_hex(hex_str).unwrap();
                deserialize::<T>(&hex_vec).map_err(Error::BitcoinEncoding)
            }
            Err(e) => Err(e),
        }
    }

    fn get_response_json<'a, T: serde::de::DeserializeOwned>(
        &'a self,
        path: &'a str,
    ) -> Result<T, Error> {
        let response = self.get_with_retry(path);
        match response {
            Ok(resp) if !is_status_ok(resp.status_code) => {
                let status = u16::try_from(resp.status_code).map_err(Error::StatusCode)?;
                let message = resp.as_str().unwrap_or_default().to_string();
                Err(Error::HttpResponse { status, message })
            }
            Ok(resp) => Ok(resp.json::<T>().map_err(Error::Minreq)?),
            Err(e) => Err(e),
        }
    }

    fn get_opt_response_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
    ) -> Result<Option<T>, Error> {
        match self.get_with_retry(path) {
            Ok(resp) if is_status_not_found(resp.status_code) => Ok(None),
            Ok(resp) if !is_status_ok(resp.status_code) => {
                let status = u16::try_from(resp.status_code).map_err(Error::StatusCode)?;
                let message = resp.as_str().unwrap_or_default().to_string();
                Err(Error::HttpResponse { status, message })
            }
            Ok(resp) => Ok(Some(resp.json::<T>()?)),
            Err(e) => Err(e),
        }
    }

    fn get_response_str(&self, path: &str) -> Result<String, Error> {
        match self.get_with_retry(path) {
            Ok(resp) if !is_status_ok(resp.status_code) => {
                let status = u16::try_from(resp.status_code).map_err(Error::StatusCode)?;
                let message = resp.as_str().unwrap_or_default().to_string();
                Err(Error::HttpResponse { status, message })
            }
            Ok(resp) => Ok(resp.as_str()?.to_string()),
            Err(e) => Err(e),
        }
    }

    /// Get a [`Transaction`] option given its [`Txid`]
    pub fn get_tx(&self, txid: &Txid) -> Result<Option<Transaction>, Error> {
        self.get_opt_response(&format!("/tx/{txid}/raw"))
    }

    /// Get a [`Transaction`] given its [`Txid`].
    pub fn get_tx_no_opt(&self, txid: &Txid) -> Result<Transaction, Error> {
        match self.get_tx(txid) {
            Ok(Some(tx)) => Ok(tx),
            Ok(None) => Err(Error::TransactionNotFound(*txid)),
            Err(e) => Err(e),
        }
    }

    /// Get a [`Txid`] of a transaction given its index in a block with a given
    /// hash.
    pub fn get_txid_at_block_index(
        &self,
        block_hash: &BlockHash,
        index: usize,
    ) -> Result<Option<Txid>, Error> {
        self.get_opt_response_txid(&format!("/block/{block_hash}/txid/{index}"))
    }

    /// Get the status of a [`Transaction`] given its [`Txid`].
    pub fn get_tx_status(&self, txid: &Txid) -> Result<TxStatus, Error> {
        self.get_response_json(&format!("/tx/{txid}/status"))
    }

    /// Get transaction info given it's [`Txid`].
    pub fn get_tx_info(&self, txid: &Txid) -> Result<Option<Tx>, Error> {
        self.get_opt_response_json(&format!("/tx/{txid}"))
    }

    /// Get a [`BlockHeader`] given a particular block hash.
    pub fn get_header_by_hash(&self, block_hash: &BlockHash) -> Result<BlockHeader, Error> {
        self.get_response_hex(&format!("/block/{block_hash}/header"))
    }

    /// Get the [`BlockStatus`] given a particular [`BlockHash`].
    pub fn get_block_status(&self, block_hash: &BlockHash) -> Result<BlockStatus, Error> {
        self.get_response_json(&format!("/block/{block_hash}/status"))
    }

    /// Get a [`Block`] given a particular [`BlockHash`].
    pub fn get_block_by_hash(&self, block_hash: &BlockHash) -> Result<Option<Block>, Error> {
        self.get_opt_response(&format!("/block/{block_hash}/raw"))
    }

    /// Get a merkle inclusion proof for a [`Transaction`] with the given
    /// [`Txid`].
    pub fn get_merkle_proof(&self, txid: &Txid) -> Result<Option<MerkleProof>, Error> {
        self.get_opt_response_json(&format!("/tx/{txid}/merkle-proof"))
    }

    /// Get a [`MerkleBlock`] inclusion proof for a [`Transaction`] with the
    /// given [`Txid`].
    pub fn get_merkle_block(&self, txid: &Txid) -> Result<Option<MerkleBlock>, Error> {
        self.get_opt_response_hex(&format!("/tx/{txid}/merkleblock-proof"))
    }

    /// Get the spending status of an output given a [`Txid`] and the output
    /// index.
    pub fn get_output_status(
        &self,
        txid: &Txid,
        index: u64,
    ) -> Result<Option<OutputStatus>, Error> {
        self.get_opt_response_json(&format!("/tx/{txid}/outspend/{index}"))
    }

    /// Broadcast a [`Transaction`] to Esplora
    pub fn broadcast(&self, transaction: &Transaction) -> Result<Txid, Error> {
        let request = self.post_request(
            "/tx",
            serialize(transaction)
                .to_lower_hex_string()
                .as_bytes()
                .to_vec(),
        )?;

        match request.send() {
            Ok(resp) if !is_status_ok(resp.status_code) => {
                let status = u16::try_from(resp.status_code).map_err(Error::StatusCode)?;
                let message = resp.as_str().unwrap_or_default().to_string();
                Err(Error::HttpResponse { status, message })
            }
            Ok(resp) => {
                let txid =
                    Txid::from_str(resp.as_str().unwrap_or_default()).map_err(Error::HexToArray)?;
                Ok(txid)
            }
            Err(e) => Err(Error::Minreq(e)),
        }
    }

    /// Broadcast a package of [`Transaction`] to Esplora
    ///
    /// if `maxfeerate` is provided, any transaction whose
    /// fee is higher will be rejected
    ///
    /// if  `maxburnamount` is provided, any transaction
    /// with higher provably unspendable outputs amount
    /// will be rejected
    pub fn submit_package(
        &self,
        transactions: &[Transaction],
        maxfeerate: Option<f64>,
        maxburnamount: Option<f64>,
    ) -> Result<SubmitPackageResult, Error> {
        let serialized_txs = transactions
            .iter()
            .map(|tx| serialize_hex(&tx))
            .collect::<Vec<_>>();

        let mut request = self.post_request(
            "/txs/package",
            serde_json::to_string(&serialized_txs)
                .unwrap()
                .as_bytes()
                .to_vec(),
        )?;

        if let Some(maxfeerate) = maxfeerate {
            request = request.with_param("maxfeerate", maxfeerate.to_string())
        }

        if let Some(maxburnamount) = maxburnamount {
            request = request.with_param("maxburnamount", maxburnamount.to_string())
        }

        match request.send() {
            Ok(resp) if !is_status_ok(resp.status_code) => {
                let status = u16::try_from(resp.status_code).map_err(Error::StatusCode)?;
                let message = resp.as_str().unwrap_or_default().to_string();
                Err(Error::HttpResponse { status, message })
            }
            Ok(resp) => Ok(resp.json::<SubmitPackageResult>().map_err(Error::Minreq)?),
            Err(e) => Err(Error::Minreq(e)),
        }
    }

    /// Get the height of the current blockchain tip.
    pub fn get_height(&self) -> Result<u32, Error> {
        self.get_response_str("/blocks/tip/height")
            .map(|s| u32::from_str(s.as_str()).map_err(Error::Parsing))?
    }

    /// Get the [`BlockHash`] of the current blockchain tip.
    pub fn get_tip_hash(&self) -> Result<BlockHash, Error> {
        self.get_response_str("/blocks/tip/hash")
            .map(|s| BlockHash::from_str(s.as_str()).map_err(Error::HexToArray))?
    }

    /// Get the [`BlockHash`] of a specific block height
    pub fn get_block_hash(&self, block_height: u32) -> Result<BlockHash, Error> {
        self.get_response_str(&format!("/block-height/{block_height}"))
            .map(|s| BlockHash::from_str(s.as_str()).map_err(Error::HexToArray))?
    }

    /// Get an map where the key is the confirmation target (in number of
    /// blocks) and the value is the estimated feerate (in sat/vB).
    pub fn get_fee_estimates(&self) -> Result<HashMap<u16, f64>, Error> {
        self.get_response_json("/fee-estimates")
    }

    /// Get information about a specific address, includes confirmed balance and transactions in
    /// the mempool.
    pub fn get_address_stats(&self, address: &Address) -> Result<AddressStats, Error> {
        let path = format!("/address/{address}");
        self.get_response_json(&path)
    }

    /// Get transaction history for the specified address/scripthash, sorted with newest first.
    ///
    /// Returns up to 50 mempool transactions plus the first 25 confirmed transactions.
    /// More can be requested by specifying the last txid seen by the previous query.
    pub fn get_address_txs(
        &self,
        address: &Address,
        last_seen: Option<Txid>,
    ) -> Result<Vec<Tx>, Error> {
        let path = match last_seen {
            Some(last_seen) => format!("/address/{address}/txs/chain/{last_seen}"),
            None => format!("/address/{address}/txs"),
        };

        self.get_response_json(&path)
    }

    /// Get confirmed transaction history for the specified address/scripthash,
    /// sorted with newest first. Returns 25 transactions per page.
    /// More can be requested by specifying the last txid seen by the previous
    /// query.
    pub fn scripthash_txs(
        &self,
        script: &Script,
        last_seen: Option<Txid>,
    ) -> Result<Vec<Tx>, Error> {
        let script_hash = sha256::Hash::hash(script.as_bytes());
        let path = match last_seen {
            Some(last_seen) => format!("/scripthash/{script_hash:x}/txs/chain/{last_seen}"),
            None => format!("/scripthash/{script_hash:x}/txs"),
        };
        self.get_response_json(&path)
    }

    /// Gets some recent block summaries starting at the tip or at `height` if
    /// provided.
    ///
    /// The maximum number of summaries returned depends on the backend itself:
    /// esplora returns `10` while [mempool.space](https://mempool.space/docs/api) returns `15`.
    pub fn get_blocks(&self, height: Option<u32>) -> Result<Vec<BlockSummary>, Error> {
        let path = match height {
            Some(height) => format!("/blocks/{height}"),
            None => "/blocks".to_string(),
        };
        let blocks: Vec<BlockSummary> = self.get_response_json(&path)?;
        if blocks.is_empty() {
            return Err(Error::InvalidResponse);
        }
        Ok(blocks)
    }

    /// Get all UTXOs locked to an address.
    pub fn get_address_utxos(&self, address: &Address) -> Result<Vec<Utxo>, Error> {
        self.get_response_json(&format!("/address/{address}/utxo"))
    }

    /// Sends a GET request to the given `url`, retrying failed attempts
    /// for retryable error codes until max retries hit.
    fn get_with_retry(&self, url: &str) -> Result<Response, Error> {
        let mut delay = BASE_BACKOFF_MILLIS;
        let mut attempts = 0;

        loop {
            match self.get_request(url)?.send()? {
                resp if attempts < self.max_retries && is_status_retryable(resp.status_code) => {
                    thread::sleep(delay);
                    attempts += 1;
                    delay *= 2;
                }
                resp => return Ok(resp),
            }
        }
    }
}

fn is_status_ok(status: i32) -> bool {
    status == 200
}

fn is_status_not_found(status: i32) -> bool {
    status == 404
}

fn is_status_retryable(status: i32) -> bool {
    let status = status as u16;
    RETRYABLE_ERROR_CODES.contains(&status)
}
