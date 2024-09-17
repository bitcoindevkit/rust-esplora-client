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

#[allow(unused_imports)]
use log::{debug, error, info, trace};

use minreq::Proxy;

use bitcoin::hashes::{sha256, Hash};
use bitcoin::{
    block::Header as BlockHeader, Block, BlockHash, MerkleBlock, Script, Transaction, Txid,
};

use crate::{
    AddressApi, BlockStatus, BlockSummary, BlocksApi, Builder, Client, Error, FeeEstimatesApi,
    MerkleProof, OutputStatus, TransactionApi, Tx, TxStatus,
};

pub(crate) fn handler(
    client: &BlockingClient,
) -> impl FnMut(crate::Request) -> Result<crate::Response, minreq::Error> + '_ {
    move |request| {
        let mut minreq_request = match request.method {
            crate::Method::Get => minreq::Request::new(minreq::Method::Get, request.url),
            crate::Method::Post => minreq::Request::new(minreq::Method::Post, request.url)
                .with_body(request.body.expect("It should've a non-empty body!")),
        };

        // FIXME: (@leonardo) I don't think that we should have the proxy, timeout and headers
        // coming from client. How should we do it ?

        if let Some(proxy) = &client.proxy {
            let proxy = Proxy::new(proxy.as_str())?;
            minreq_request = minreq_request.with_proxy(proxy);
        }

        if let Some(timeout) = client.timeout {
            minreq_request = minreq_request.with_timeout(timeout);
        }

        if !client.headers.is_empty() {
            for (key, value) in &client.headers {
                minreq_request = minreq_request.with_header(key, value);
            }
        }

        let minreq_response = minreq_request.send()?;

        let response = crate::Response::new(
            minreq_response.status_code,
            minreq_response.as_bytes().to_vec(),
            // minreq_response.reason_phrase,
            // minreq_response.headers,
            // minreq_response.url,
        );

        Ok(response)
    }
}

#[derive(Debug, Clone)]
pub struct BlockingClient {
    url: String,
    /// The proxy is ignored when targeting `wasm32`.
    pub proxy: Option<String>,
    /// Socket timeout.
    pub timeout: Option<u64>,
    /// HTTP headers to set on every request made to Esplora server
    pub headers: HashMap<String, String>,
}

impl BlockingClient {
    /// build a blocking client from a [`Builder`]
    pub fn from_builder(builder: Builder) -> Self {
        Self {
            url: builder.base_url,
            proxy: builder.proxy,
            timeout: builder.timeout,
            headers: builder.headers,
        }
    }

    /// Get the underlying base URL.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Get a [`Transaction`] option given its [`Txid`]
    pub fn get_tx(&self, txid: &Txid) -> Result<Option<Transaction>, Error> {
        let tx_api = TransactionApi::Tx(*txid);
        let response = tx_api.send(&self.url, &mut handler(self))?;
        match tx_api.deserialize_decodable::<Transaction>(&response) {
            Ok(transaction) => Ok(Some(transaction)),
            Err(Error::HttpResponse { status: 404, .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get a [`Transaction`] given its [`Txid`].
    pub fn get_tx_no_opt(&self, txid: &Txid) -> Result<Transaction, Error> {
        match self.get_tx(txid) {
            Ok(Some(tx)) => Ok(tx),
            Ok(None) => Err(Error::TransactionNotFound(*txid)),
            Err(e) => Err(e),
        }
    }

    /// Get the status of a [`Transaction`] given its [`Txid`].
    pub fn get_tx_status(&self, txid: &Txid) -> Result<TxStatus, Error> {
        let tx_api = TransactionApi::TxStatus(*txid);
        let response = tx_api.send(&self.url, &mut handler(self))?;
        match tx_api.deserialize_json::<TxStatus>(&response) {
            Ok(tx_status) => Ok(tx_status),
            Err(e) => Err(e),
        }
    }

    /// Get transaction info given it's [`Txid`].
    pub fn get_tx_info(&self, txid: &Txid) -> Result<Option<Tx>, Error> {
        let tx_api = TransactionApi::TxInfo(*txid);
        let response = tx_api.send(&self.url, &mut handler(self))?;
        match tx_api.deserialize_json::<Tx>(&response) {
            Ok(tx) => Ok(Some(tx)),
            Err(Error::HttpResponse { status: 404, .. }) => Ok(None),
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
        let api = BlocksApi::BlockTxIdAtIndex(*block_hash, index);
        let response = api.send(&self.url, &mut handler(self))?;
        match api.deserialize_str::<Txid>(&response) {
            Ok(txid) => Ok(Some(txid)),
            Err(Error::HttpResponse { status: 404, .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get a [`BlockHeader`] given a particular block hash.
    pub fn get_header_by_hash(&self, block_hash: &BlockHash) -> Result<BlockHeader, Error> {
        let api = BlocksApi::BlockHeader(*block_hash);
        let response = api.send(&self.url, &mut handler(self))?;
        api.deserialize_decodable::<BlockHeader>(&response)
    }

    /// Get the [`BlockStatus`] given a particular [`BlockHash`].
    pub fn get_block_status(&self, block_hash: &BlockHash) -> Result<BlockStatus, Error> {
        let api = BlocksApi::BlockStatus(*block_hash);
        let response = api.send(&self.url, &mut handler(self))?;
        api.deserialize_json::<BlockStatus>(&response)
    }

    /// Get a [`Block`] given a particular [`BlockHash`].
    pub fn get_block_by_hash(&self, block_hash: &BlockHash) -> Result<Option<Block>, Error> {
        let api = BlocksApi::BlockRaw(*block_hash);
        let response = api.send(&self.url, &mut handler(self))?;
        match api.deserialize_decodable::<Block>(&response) {
            Ok(block) => Ok(Some(block)),
            Err(Error::HttpResponse { status: 404, .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get a merkle inclusion proof for a [`Transaction`] with the given
    /// [`Txid`].
    pub fn get_merkle_proof(&self, txid: &Txid) -> Result<Option<MerkleProof>, Error> {
        let tx_api = TransactionApi::TxMerkleProof(*txid);
        let response = tx_api.send(&self.url, &mut handler(self))?;
        match tx_api.deserialize_json::<MerkleProof>(&response) {
            Ok(merkle_proof) => Ok(Some(merkle_proof)),
            Err(Error::HttpResponse { status: 404, .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get a [`MerkleBlock`] inclusion proof for a [`Transaction`] with the
    /// given [`Txid`].
    pub fn get_merkle_block(&self, txid: &Txid) -> Result<Option<MerkleBlock>, Error> {
        let tx_api = TransactionApi::TxMerkeBlockProof(*txid);
        let response = tx_api.send(&self.url, &mut handler(self))?;
        match tx_api.deserialize_decodable::<MerkleBlock>(&response) {
            Ok(merkle_block) => Ok(Some(merkle_block)),
            Err(Error::HttpResponse { status: 404, .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get the spending status of an output given a [`Txid`] and the output
    /// index.
    pub fn get_output_status(
        &self,
        txid: &Txid,
        index: u64,
    ) -> Result<Option<OutputStatus>, Error> {
        let tx_api = TransactionApi::TxOutputStatus(*txid, index);
        let response = tx_api.send(&self.url, &mut handler(self))?;
        match tx_api.deserialize_json::<OutputStatus>(&response) {
            Ok(output_status) => Ok(Some(output_status)),
            Err(Error::HttpResponse { status: 404, .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Broadcast a [`Transaction`] to Esplora
    pub fn broadcast(&self, transaction: &Transaction) -> Result<(), Error> {
        let tx_api = TransactionApi::Broadcast(transaction.clone());
        let response = tx_api.send(&self.url, &mut handler(self))?;

        if !response.is_status_ok() {
            let status = u16::try_from(response.status_code).map_err(Error::StatusCode)?;
            let message = response.as_str().unwrap_or_default().to_string();
            return Err(Error::HttpResponse { status, message });
        }

        Ok(())
    }

    /// Get the height of the current blockchain tip.
    pub fn get_height(&self) -> Result<u32, Error> {
        let api = BlocksApi::BlockTipHeight;
        let response = api.send(&self.url, &mut handler(self))?;
        api.deserialize_str::<u32>(&response)
    }

    /// Get the [`BlockHash`] of the current blockchain tip.
    pub fn get_tip_hash(&self) -> Result<BlockHash, Error> {
        let api = BlocksApi::BlockTipHash;
        let response = api.send(&self.url, &mut handler(self))?;
        api.deserialize_str::<BlockHash>(&response)
    }

    /// Get the [`BlockHash`] of a specific block height
    pub fn get_block_hash(&self, block_height: u32) -> Result<BlockHash, Error> {
        let api = BlocksApi::BlockHash(block_height);
        let response = api.send(&self.url, &mut handler(self))?;
        api.deserialize_str::<BlockHash>(&response)
    }

    /// Get an map where the key is the confirmation target (in number of
    /// blocks) and the value is the estimated feerate (in sat/vB).
    pub fn get_fee_estimates(&self) -> Result<HashMap<u16, f64>, Error> {
        let api = FeeEstimatesApi::FeeRate;
        let response = api.send(&self.url, &mut handler(self))?;
        api.deserialize_json::<HashMap<u16, f64>>(&response)
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
        let address_api = match last_seen {
            Some(last_seen) => AddressApi::ScriptHashConfirmedTxHistory(script_hash, last_seen),
            None => AddressApi::ScriptHashTxHistory(script_hash),
        };
        let response = address_api.send(&self.url, &mut handler(self))?;
        match address_api.deserialize_json::<Vec<Tx>>(&response) {
            Ok(txs) => Ok(txs),
            Err(e) => Err(e),
        }
    }

    /// Gets some recent block summaries starting at the tip or at `height` if
    /// provided.
    ///
    /// The maximum number of summaries returned depends on the backend itself:
    /// esplora returns `10` while [mempool.space](https://mempool.space/docs/api) returns `15`.
    pub fn get_blocks(&self, height: Option<u32>) -> Result<Vec<BlockSummary>, Error> {
        let api = BlocksApi::BlockSummaries(height);
        let response = api.send(&self.url, &mut handler(self))?;
        api.deserialize_json::<Vec<BlockSummary>>(&response)
    }
}
