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

//! Blocking Esplora client.
//!
//! This module provides [`BlockingClient`], a blocking HTTP client for interacting
//! with an [Esplora](https://github.com/Blockstream/esplora/blob/master/API.md)
//! server, built on top of [`minreq`].
//!
//! # Example
//!
//! ```rust,no_run
//! # use esplora_client::Builder;
//! let client = Builder::new("https://mempool.space/api").build_blocking();
//! let height = client.get_height()?;
//! # Ok::<(), esplora_client::Error>(())
//! ```

use std::collections::HashMap;
use std::convert::TryFrom;
use std::str::FromStr;
use std::thread;

use minreq::{Proxy, Request, Response};

use bitcoin::block::Header as BlockHeader;
use bitcoin::consensus::encode::serialize_hex;
use bitcoin::consensus::{deserialize, serialize, Decodable};
use bitcoin::hashes::{sha256, Hash};
use bitcoin::hex::{DisplayHex, FromHex};
use bitcoin::{Address, Block, BlockHash, FeeRate, MerkleBlock, Script, Transaction, Txid};

use crate::{
    sat_per_vbyte_to_feerate, AddressStats, BlockInfo, BlockStatus, Builder, Error, EsploraTx,
    MempoolRecentTx, MempoolStats, MerkleProof, OutputStatus, ScriptHashStats, SubmitPackageResult,
    TxStatus, Utxo, BASE_BACKOFF_MILLIS, RETRYABLE_ERROR_CODES,
};

/// Returns `true` if the given HTTP status code indicates a successful response.
fn is_status_ok(status: i32) -> bool {
    status == 200
}

/// Returns `true` if the given HTTP status code indicates a resource was not found.
fn is_status_not_found(status: i32) -> bool {
    status == 404
}

/// Returns `true` if the given HTTP status code should trigger a retry.
///
/// See [`RETRYABLE_ERROR_CODES`] for the list of retryable status codes.
fn is_status_retryable(status: i32) -> bool {
    let status = status as u16;
    RETRYABLE_ERROR_CODES.contains(&status)
}

/// A blocking client for interacting with an Esplora API server.
///
/// Use [`Builder`] to construct an instance of this client.
///
/// # Retries
///
/// Failed requests are automatically retried up to `max_retries` times
/// (configured via [`Builder`]) with exponential backoff, but only for
/// retryable HTTP status codes. See [`RETRYABLE_ERROR_CODES`] for the
/// full list.
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
    // ----> CLIENT

    /// Build a [`BlockingClient`] from a [`Builder`].
    pub fn from_builder(builder: Builder) -> Self {
        Self {
            url: builder.base_url,
            proxy: builder.proxy,
            timeout: builder.timeout,
            headers: builder.headers,
            max_retries: builder.max_retries,
        }
    }

    /// Returns the base URL of the Esplora server this client connects to.
    pub fn url(&self) -> &str {
        &self.url
    }

    // ----> INTERNAL

    /// Performs a raw HTTP GET request to the given `path`.
    ///
    /// Configures the request with the proxy, timeout, and headers set on
    /// this client. Used internally by all other GET helper methods.
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] if the proxy configuration is invalid.
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

    /// Sends a GET request to `url`, retrying on retryable status codes
    /// with exponential backoff until [`BlockingClient::max_retries`] is reached.
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

    /// Makes a POST request to `path` with `body`.
    ///
    /// Configures the request with the proxy and timeout set on this client.
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] if the proxy configuration is invalid.
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

    /// Makes a GET request to `path`, deserializing the response body as
    /// raw bytes into `T` using [`bitcoin::consensus::Decodable`].
    ///
    /// Use this for endpoints that return raw binary Bitcoin data.
    ///
    /// Returns `None` on a 404 response.
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] if the request fails or deserialization fails.
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

    /// Makes a GET request to `path`, deserializing the response body as
    /// JSON into `T` using [`serde::de::DeserializeOwned`].
    ///
    /// Use this for endpoints that return Esplora-specific JSON types,
    /// as defined in [`crate::api`].
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] if the request fails or JSON deserialization fails.
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

    /// Makes a GET request to `path`, returning `None` on a 404 response.
    ///
    /// Delegates to [`Self::get_response_json`]. See its documentation for details.
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

    /// Makes a GET request to `path`, deserializing the hex-encoded response
    /// body into `T` using [`bitcoin::consensus::Decodable`].
    ///
    /// Use this for endpoints that return hex-encoded Bitcoin data.
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] if the request fails, hex decoding fails,
    /// or consensus deserialization fails.
    fn get_response_hex<T: Decodable>(&self, path: &str) -> Result<T, Error> {
        match self.get_with_retry(path) {
            Ok(resp) if !is_status_ok(resp.status_code) => {
                let status = u16::try_from(resp.status_code).map_err(Error::StatusCode)?;
                let message = resp.as_str().unwrap_or_default().to_string();
                Err(Error::HttpResponse { status, message })
            }
            Ok(resp) => {
                let hex_str = resp.as_str().map_err(Error::Minreq)?;
                let hex_vec = Vec::from_hex(hex_str)?;
                deserialize::<T>(&hex_vec).map_err(Error::BitcoinEncoding)
            }
            Err(e) => Err(e),
        }
    }

    /// Makes a GET request to `path`, deserializing a [`Txid`] from the
    /// hex-encoded response body.
    ///
    /// Returns `None` on a 404 response.
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] if the request fails or the response cannot be
    /// parsed as a [`Txid`].
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

    /// Makes a GET request to `path`, deserializing the hex-encoded response
    /// body into `T` using [`bitcoin::consensus::Decodable`].
    ///
    /// Use this for endpoints that return hex-encoded Bitcoin data.
    ///
    /// Returns `None` on a 404 response.
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] if the request fails, hex decoding fails,
    /// or consensus deserialization fails.
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
                let hex_vec = Vec::from_hex(hex_str)?;
                deserialize::<T>(&hex_vec)
                    .map_err(Error::BitcoinEncoding)
                    .map(|r| Some(r))
            }
            Err(e) => Err(e),
        }
    }

    /// Makes a GET request to `path`, returning the response body as a [`String`].
    ///
    /// Use this for endpoints that return plain text data that needs
    /// further parsing downstream.
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] if the request fails.
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

    // ----> TRANSACTION

    /// Broadcast a [`Transaction`] to the Esplora server.
    ///
    /// The transaction is serialized and sent as a hex-encoded string.
    /// Returns the [`Txid`] of the broadcasted transaction.
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] if the request fails or the server rejects the transaction.
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
                let txid = Txid::from_str(resp.as_str()?).map_err(Error::HexToArray)?;
                Ok(txid)
            }
            Err(e) => Err(Error::Minreq(e)),
        }
    }

    /// Broadcast a package of [`Transaction`]s to the Esplora server.
    ///
    /// Returns a [`SubmitPackageResult`] containing the result for each
    /// transaction in the package, keyed by [`Wtxid`](bitcoin::Wtxid).
    ///
    /// Optionally, `maxfeerate` (in sat/vB) and `maxburnamount` (in BTC) can
    /// be provided to reject transactions that exceed these thresholds.
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] if the request fails or the server rejects the package.
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
                .map_err(Error::SerdeJson)?
                .into_bytes(),
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

    /// Get a raw [`Transaction`] given its [`Txid`].
    ///
    /// Returns `None` if the transaction is not found.
    pub fn get_tx(&self, txid: &Txid) -> Result<Option<Transaction>, Error> {
        self.get_opt_response(&format!("/tx/{txid}/raw"))
    }

    /// Get a [`Transaction`] given its [`Txid`].
    ///
    /// Returns an [`Error::TransactionNotFound`] if the transaction is not found.
    /// Prefer [`Self::get_tx`] if you want to handle the not-found case explicitly.
    pub fn get_tx_no_opt(&self, txid: &Txid) -> Result<Transaction, Error> {
        match self.get_tx(txid) {
            Ok(Some(tx)) => Ok(tx),
            Ok(None) => Err(Error::TransactionNotFound(*txid)),
            Err(e) => Err(e),
        }
    }

    /// Get a [`EsploraTx`] given its [`Txid`].
    ///
    /// Unlike [`Self::get_tx`], this returns the Esplora-specific [`EsploraTx`] type,
    /// which includes additional metadata such as confirmation status, fee,
    /// and weight. Returns `None` if the transaction is not found.
    pub fn get_tx_info(&self, txid: &Txid) -> Result<Option<EsploraTx>, Error> {
        self.get_opt_response_json(&format!("/tx/{txid}"))
    }

    /// Get the confirmation status of a [`Transaction`] given its [`Txid`].
    ///
    /// Returns a [`TxStatus`] containing whether the transaction is confirmed,
    /// and if so, the block height, hash, and timestamp it was confirmed in.
    pub fn get_tx_status(&self, txid: &Txid) -> Result<TxStatus, Error> {
        self.get_response_json(&format!("/tx/{txid}/status"))
    }

    /// Get the spend status of all outputs in a [`Transaction`], given its [`Txid`].
    ///
    /// Returns a [`Vec`] of [`OutputStatus`], one per output, ordered as they appear in the
    /// [`Transaction`].
    pub fn get_tx_outspends(&self, txid: &Txid) -> Result<Vec<OutputStatus>, Error> {
        self.get_response_json(&format!("/tx/{txid}/outspends"))
    }

    /// Get the [`Txid`] of the transaction at position `index` within the
    /// block identified by `block_hash`.
    ///
    /// Returns `None` if the block or index is not found.
    pub fn get_txid_at_block_index(
        &self,
        block_hash: &BlockHash,
        index: usize,
    ) -> Result<Option<Txid>, Error> {
        self.get_opt_response_txid(&format!("/block/{block_hash}/txid/{index}"))
    }

    /// Get a Merkle inclusion proof for a [`Transaction`] given its [`Txid`].
    ///
    /// Returns a [`MerkleProof`] that can be used to verify the transaction's
    /// inclusion in a block. Returns `None` if the transaction is not found
    /// or is unconfirmed.
    pub fn get_merkle_proof(&self, txid: &Txid) -> Result<Option<MerkleProof>, Error> {
        self.get_opt_response_json(&format!("/tx/{txid}/merkle-proof"))
    }

    /// Get a [`MerkleBlock`] inclusion proof for a [`Transaction`] given its [`Txid`].
    ///
    /// Returns `None` if the transaction is not found or is unconfirmed.
    pub fn get_merkle_block(&self, txid: &Txid) -> Result<Option<MerkleBlock>, Error> {
        self.get_opt_response_hex(&format!("/tx/{txid}/merkleblock-proof"))
    }

    /// Get the spend status of a specific output, identified by its [`Txid`]
    /// and output index.
    ///
    /// Returns an [`OutputStatus`] indicating whether the output has been
    /// spent, and if so, by which transaction. Returns `None` if not found.
    pub fn get_output_status(
        &self,
        txid: &Txid,
        index: u64,
    ) -> Result<Option<OutputStatus>, Error> {
        self.get_opt_response_json(&format!("/tx/{txid}/outspend/{index}"))
    }

    // ----> BLOCK

    /// Get the block height of the current blockchain tip.
    pub fn get_height(&self) -> Result<u32, Error> {
        self.get_response_str("/blocks/tip/height")
            .map(|s| u32::from_str(s.as_str()).map_err(Error::Parsing))?
    }

    /// Get the [`BlockHash`] of the current blockchain tip.
    pub fn get_tip_hash(&self) -> Result<BlockHash, Error> {
        self.get_response_str("/blocks/tip/hash")
            .map(|s| BlockHash::from_str(s.as_str()).map_err(Error::HexToArray))?
    }

    /// Get the [`BlockHash`] of a [`Block`] given its `height`.
    pub fn get_block_hash(&self, block_height: u32) -> Result<BlockHash, Error> {
        self.get_response_str(&format!("/block-height/{block_height}"))
            .map(|s| BlockHash::from_str(s.as_str()).map_err(Error::HexToArray))?
    }

    /// Get the [`BlockHeader`] of a [`Block`] given its [`BlockHash`].
    pub fn get_header_by_hash(&self, block_hash: &BlockHash) -> Result<BlockHeader, Error> {
        self.get_response_hex(&format!("/block/{block_hash}/header"))
    }

    /// Get the full [`Block`] with the given [`BlockHash`].
    ///
    /// Returns `None` if the [`Block`] is not found.
    pub fn get_block_by_hash(&self, block_hash: &BlockHash) -> Result<Option<Block>, Error> {
        self.get_opt_response(&format!("/block/{block_hash}/raw"))
    }

    /// Get the [`BlockStatus`] of a [`Block`] given its [`BlockHash`].
    ///
    /// Returns a [`BlockStatus`] indicating whether this [`Block`] is part of the
    /// best chain, its height, and the [`BlockHash`] of the next [`Block`], if any.
    pub fn get_block_status(&self, block_hash: &BlockHash) -> Result<BlockStatus, Error> {
        self.get_response_json(&format!("/block/{block_hash}/status"))
    }

    /// Get a [`BlockInfo`] summary for the [`Block`] with the given [`BlockHash`].
    ///
    /// [`BlockInfo`] includes metadata such as the height, timestamp,
    /// [`Transaction`] count, size, and [weight](bitcoin::Weight).
    ///
    /// **This method does not return the full [`Block`].**
    pub fn get_block_info(&self, blockhash: &BlockHash) -> Result<BlockInfo, Error> {
        let path = format!("/block/{blockhash}");

        self.get_response_json(&path)
    }

    /// Get [`BlockInfo`] summaries for recent [`Block`]s.
    ///
    /// If `height` is `Some(h)`, returns blocks starting from height `h`.
    /// If `height` is `None`, returns blocks starting from the current tip.
    ///
    /// The number of blocks returned depends on the backend:
    ///   - Esplora returns 10 [`Block`]s.
    ///   - [Mempool.space](https://mempool.space/docs/api/rest#get-blocks) returns 10 [`Block`]s.
    ///
    /// # Errors
    ///
    /// Returns [`Error::InvalidResponse`] if the server returns an empty list.
    ///
    /// **This method does not return the full [`Block`].**
    pub fn get_block_infos(&self, height: Option<u32>) -> Result<Vec<BlockInfo>, Error> {
        let path = match height {
            Some(height) => format!("/blocks/{height}"),
            None => "/blocks".to_string(),
        };
        let block_infos: Vec<BlockInfo> = self.get_response_json(&path)?;
        if block_infos.is_empty() {
            return Err(Error::InvalidResponse);
        }
        Ok(block_infos)
    }

    /// Get all [`Txid`]s of [`Transaction`]s included in the [`Block`] with the given
    /// [`BlockHash`].
    pub fn get_block_txids(&self, blockhash: &BlockHash) -> Result<Vec<Txid>, Error> {
        let path = format!("/block/{blockhash}/txids");

        self.get_response_json(&path)
    }

    /// Get up to 25 [`EsploraTx`]s from the block with the given [`BlockHash`],
    /// starting at `start_index`.
    ///
    /// If `start_index` is `None`, starts from the first transaction (index 0).
    ///
    /// Note that `start_index` **MUST** be a multiple of 25,
    /// otherwise the server will return an error.
    pub fn get_block_txs(
        &self,
        blockhash: &BlockHash,
        start_index: Option<u32>,
    ) -> Result<Vec<EsploraTx>, Error> {
        let path = match start_index {
            None => format!("/block/{blockhash}/txs"),
            Some(start_index) => format!("/block/{blockhash}/txs/{start_index}"),
        };

        self.get_response_json(&path)
    }

    /// Get fee estimates for a range of confirmation targets.
    ///
    /// Returns a [`HashMap`] where the key is the confirmation target in blocks
    /// and the value is the estimated [`FeeRate`].
    pub fn get_fee_estimates(&self) -> Result<HashMap<u16, FeeRate>, Error> {
        let estimates_raw: HashMap<u16, f64> = self.get_response_json("/fee-estimates")?;
        let estimates = sat_per_vbyte_to_feerate(estimates_raw);

        Ok(estimates)
    }

    // ----> ADDRESS

    /// Get statistics about an [`Address`].
    ///
    /// Returns an [`AddressStats`] containing confirmed and mempool
    /// [transaction summaries](crate::api::AddressTxsSummary) for the given address,
    /// including funded and spent output counts and their total values.
    pub fn get_address_stats(&self, address: &Address) -> Result<AddressStats, Error> {
        let path = format!("/address/{address}");
        self.get_response_json(&path)
    }

    /// Get confirmed transaction history for an [`Address`], sorted newest first.
    ///
    /// Returns up to 50 mempool transactions plus the first 25 confirmed transactions.
    /// To paginate, pass the [`Txid`] of the last transaction seen in the previous
    /// response as `last_seen`.
    pub fn get_address_txs(
        &self,
        address: &Address,
        last_seen: Option<Txid>,
    ) -> Result<Vec<EsploraTx>, Error> {
        let path = match last_seen {
            Some(last_seen) => format!("/address/{address}/txs/chain/{last_seen}"),
            None => format!("/address/{address}/txs"),
        };

        self.get_response_json(&path)
    }

    /// Get all confirmed [`Utxo`]s locked to the given [`Address`].
    pub fn get_address_utxos(&self, address: &Address) -> Result<Vec<Utxo>, Error> {
        let path = format!("/address/{address}/utxo");

        self.get_response_json(&path)
    }

    /// Get unconfirmed mempool [`EsploraTx`]s for an [`Address`], sorted newest first.
    pub fn get_mempool_address_txs(&self, address: &Address) -> Result<Vec<EsploraTx>, Error> {
        let path = format!("/address/{address}/txs/mempool");

        self.get_response_json(&path)
    }

    // ----> SCRIPT HASH

    /// Get statistics about a [`Script`] hash's confirmed and mempool transactions.
    ///
    /// Returns a [`ScriptHashStats`] containing
    /// [transaction summaries](crate::api::AddressTxsSummary)
    /// for the SHA256 hash of the given [`Script`].
    pub fn get_scripthash_stats(&self, script: &Script) -> Result<ScriptHashStats, Error> {
        let script_hash = sha256::Hash::hash(script.as_bytes());
        let path = format!("/scripthash/{script_hash}");
        self.get_response_json(&path)
    }

    /// Get confirmed transaction history for a [`Script`] hash, sorted newest first.
    ///
    /// Returns 25 transactions per page. To paginate, pass the [`Txid`] of the
    /// last transaction seen in the previous response as `last_seen`.
    pub fn get_script_hash_txs(
        &self,
        script: &Script,
        last_seen: Option<Txid>,
    ) -> Result<Vec<EsploraTx>, Error> {
        let script_hash = sha256::Hash::hash(script.as_bytes());
        let path = match last_seen {
            Some(last_seen) => format!("/scripthash/{script_hash:x}/txs/chain/{last_seen}"),
            None => format!("/scripthash/{script_hash:x}/txs"),
        };

        self.get_response_json(&path)
    }

    /// Get all confirmed [`Utxo`]s locked to the given [`Script`].
    pub fn get_scripthash_utxos(&self, script: &Script) -> Result<Vec<Utxo>, Error> {
        let script_hash = sha256::Hash::hash(script.as_bytes());
        let path = format!("/scripthash/{script_hash}/utxo");

        self.get_response_json(&path)
    }

    /// Get unconfirmed mempool [`EsploraTx`]s for a [`Script`] hash, sorted newest first.
    pub fn get_mempool_scripthash_txs(&self, script: &Script) -> Result<Vec<EsploraTx>, Error> {
        let script_hash = sha256::Hash::hash(script.as_bytes());
        let path = format!("/scripthash/{script_hash:x}/txs/mempool");

        self.get_response_json(&path)
    }

    // ----> MEMPOOL

    /// Get global statistics about the mempool.
    ///
    /// Returns a [`MempoolStats`] containing the transaction count, total
    /// virtual size, total fees, and fee rate histogram.
    pub fn get_mempool_stats(&self) -> Result<MempoolStats, Error> {
        self.get_response_json("/mempool")
    }

    /// Get the last 10 [`MempoolRecentTx`]s to enter the mempool.
    pub fn get_mempool_recent_txs(&self) -> Result<Vec<MempoolRecentTx>, Error> {
        self.get_response_json("/mempool/recent")
    }

    /// Get the full list of [`Txid`]s currently in the mempool.
    ///
    /// The order of the returned [`Txid`]s is arbitrary.
    pub fn get_mempool_txids(&self) -> Result<Vec<Txid>, Error> {
        self.get_response_json("/mempool/txids")
    }
}
