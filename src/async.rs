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

//! Esplora by way of `reqwest` HTTP client.

use std::collections::HashMap;
use std::str::FromStr;

use bitcoin::hashes::{sha256, Hash};
use bitcoin::{
    block::Header as BlockHeader, Block, BlockHash, MerkleBlock, Script, Transaction, Txid,
};

#[allow(unused_imports)]
use log::{debug, error, info, trace};

use reqwest::header;

use crate::{
    AddressApi, BlockStatus, BlockSummary, BlocksApi, Builder, Client, Error, FeeEstimatesApi,
    MerkleProof, OutputStatus, Response, TransactionApi, Tx, TxStatus,
};

pub(crate) async fn handler(
    client: &reqwest::Client,
    request: crate::Request,
) -> Result<Response, reqwest::Error> {
    let reqwest_req = match request.method {
        crate::Method::Get => client.request(
            reqwest::Method::GET,
            reqwest::Url::from_str(&request.url).unwrap(),
        ),
        crate::Method::Post => client
            .request(
                reqwest::Method::POST,
                reqwest::Url::from_str(&request.url).unwrap(),
            )
            .body(request.body.expect("It should've a non-empty body!")),
    };

    let response = reqwest_req.send().await?;

    Ok(Response::new(
        response.status().as_u16().into(),
        response.bytes().await.unwrap().to_vec(),
    ))
}

#[derive(Debug, Clone)]
pub struct AsyncClient {
    url: String,
    client: reqwest::Client,
}

impl AsyncClient {
    /// build an async client from a builder
    pub fn from_builder(builder: Builder) -> Result<Self, Error> {
        let mut client_builder = reqwest::Client::builder();

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(proxy) = &builder.proxy {
            client_builder = client_builder
                .proxy(reqwest::Proxy::all(proxy).map_err(crate::api::Error::Client)?);
        }

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(timeout) = builder.timeout {
            client_builder = client_builder.timeout(core::time::Duration::from_secs(timeout));
        }

        if !builder.headers.is_empty() {
            let mut headers = header::HeaderMap::new();
            for (k, v) in builder.headers {
                let header_name = header::HeaderName::from_lowercase(k.to_lowercase().as_bytes())
                    .map_err(|_| Error::InvalidHttpHeaderName(k))?;
                let header_value = header::HeaderValue::from_str(&v)
                    .map_err(|_| Error::InvalidHttpHeaderValue(v))?;
                headers.insert(header_name, header_value);
            }
            client_builder = client_builder.default_headers(headers);
        }

        Ok(Self::from_client(
            builder.base_url,
            client_builder.build().map_err(crate::api::Error::Client)?,
        ))
    }

    /// build an async client from the base url and [`reqwest::Client`]
    pub fn from_client(url: String, client: reqwest::Client) -> Self {
        AsyncClient { url, client }
    }

    /// Get a [`Transaction`] option given its [`Txid`]
    pub async fn get_tx(&self, txid: &Txid) -> Result<Option<Transaction>, Error> {
        let tx_api = TransactionApi::Tx(*txid);
        let response = tx_api
            .send_async(&self.url, &mut move |request| {
                handler(&self.client, request)
            })
            .await?;
        match tx_api.deserialize_decodable::<Transaction>(&response) {
            Ok(transaction) => Ok(Some(transaction)),
            Err(Error::HttpResponse { status: 404, .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get a [`Transaction`] given its [`Txid`].
    pub async fn get_tx_no_opt(&self, txid: &Txid) -> Result<Transaction, Error> {
        match self.get_tx(txid).await {
            Ok(Some(tx)) => Ok(tx),
            Ok(None) => Err(Error::TransactionNotFound(*txid)),
            Err(e) => Err(e),
        }
    }

    /// Get the status of a [`Transaction`] given its [`Txid`].
    pub async fn get_tx_status(&self, txid: &Txid) -> Result<TxStatus, Error> {
        let tx_api = TransactionApi::TxStatus(*txid);
        let response = tx_api
            .send_async(&self.url, &mut move |request: crate::Request| {
                handler(&self.client, request)
            })
            .await?;
        match tx_api.deserialize_json::<TxStatus>(&response) {
            Ok(tx_status) => Ok(tx_status),
            Err(e) => Err(e),
        }
    }

    /// Get transaction info given it's [`Txid`].
    pub async fn get_tx_info(&self, txid: &Txid) -> Result<Option<Tx>, Error> {
        let tx_api = TransactionApi::TxInfo(*txid);
        let response = tx_api
            .send_async(&self.url, &mut move |request: crate::Request| {
                handler(&self.client, request)
            })
            .await?;
        match tx_api.deserialize_json::<Tx>(&response) {
            Ok(tx) => Ok(Some(tx)),
            Err(Error::HttpResponse { status: 404, .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get a [`Txid`] of a transaction given its index in a block with a given
    /// hash.
    pub async fn get_txid_at_block_index(
        &self,
        block_hash: &BlockHash,
        index: usize,
    ) -> Result<Option<Txid>, Error> {
        let api = BlocksApi::BlockTxIdAtIndex(*block_hash, index);
        let response = api
            .send_async(&self.url, &mut move |request: crate::Request| {
                handler(&self.client, request)
            })
            .await?;
        match api.deserialize_str::<Txid>(&response) {
            Ok(txid) => Ok(Some(txid)),
            Err(Error::HttpResponse { status: 404, .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get a [`BlockHeader`] given a particular block hash.
    pub async fn get_header_by_hash(&self, block_hash: &BlockHash) -> Result<BlockHeader, Error> {
        let api = BlocksApi::BlockHeader(*block_hash);
        let response = api
            .send_async(&self.url, &mut move |request: crate::Request| {
                handler(&self.client, request)
            })
            .await?;
        api.deserialize_decodable::<BlockHeader>(&response)
    }

    /// Get the [`BlockStatus`] given a particular [`BlockHash`].
    pub async fn get_block_status(&self, block_hash: &BlockHash) -> Result<BlockStatus, Error> {
        let api = BlocksApi::BlockStatus(*block_hash);
        let response = api
            .send_async(&self.url, &mut move |request: crate::Request| {
                handler(&self.client, request)
            })
            .await?;
        api.deserialize_json::<BlockStatus>(&response)
    }

    /// Get a [`Block`] given a particular [`BlockHash`].
    pub async fn get_block_by_hash(&self, block_hash: &BlockHash) -> Result<Option<Block>, Error> {
        let api = BlocksApi::BlockRaw(*block_hash);
        let response = api
            .send_async(&self.url, &mut move |request: crate::Request| {
                handler(&self.client, request)
            })
            .await?;
        match api.deserialize_decodable::<Block>(&response) {
            Ok(block) => Ok(Some(block)),
            Err(Error::HttpResponse { status: 404, .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get a merkle inclusion proof for a [`Transaction`] with the given
    /// [`Txid`].
    pub async fn get_merkle_proof(&self, txid: &Txid) -> Result<Option<MerkleProof>, Error> {
        let tx_api = TransactionApi::TxMerkleProof(*txid);
        let response = tx_api
            .send_async(&self.url, &mut move |request: crate::Request| {
                handler(&self.client, request)
            })
            .await?;
        match tx_api.deserialize_json::<MerkleProof>(&response) {
            Ok(merkle_proof) => Ok(Some(merkle_proof)),
            Err(Error::HttpResponse { status: 404, .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get a [`MerkleBlock`] inclusion proof for a [`Transaction`] with the
    /// given [`Txid`].
    pub async fn get_merkle_block(&self, txid: &Txid) -> Result<Option<MerkleBlock>, Error> {
        let tx_api = TransactionApi::TxMerkeBlockProof(*txid);
        let response = tx_api
            .send_async(&self.url, &mut move |request: crate::Request| {
                handler(&self.client, request)
            })
            .await?;
        match tx_api.deserialize_decodable::<MerkleBlock>(&response) {
            Ok(merkle_block) => Ok(Some(merkle_block)),
            Err(Error::HttpResponse { status: 404, .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Get the spending status of an output given a [`Txid`] and the output
    /// index.
    pub async fn get_output_status(
        &self,
        txid: &Txid,
        index: u64,
    ) -> Result<Option<OutputStatus>, Error> {
        let tx_api = TransactionApi::TxOutputStatus(*txid, index);
        let response = tx_api
            .send_async(&self.url, &mut move |request: crate::Request| {
                handler(&self.client, request)
            })
            .await?;
        match tx_api.deserialize_json::<OutputStatus>(&response) {
            Ok(output_status) => Ok(Some(output_status)),
            Err(Error::HttpResponse { status: 404, .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Broadcast a [`Transaction`] to Esplora
    pub async fn broadcast(&self, transaction: &Transaction) -> Result<(), Error> {
        let tx_api = TransactionApi::Broadcast(transaction.clone());
        let response = tx_api
            .send_async(&self.url, &mut move |request: crate::Request| {
                handler(&self.client, request)
            })
            .await?;

        if !response.is_status_ok() {
            let status = u16::try_from(response.status_code).map_err(Error::StatusCode)?;
            let message = response.as_str().unwrap_or_default().to_string();
            return Err(Error::HttpResponse { status, message });
        }

        Ok(())
    }

    /// Get the height of the current blockchain tip.
    pub async fn get_height(&self) -> Result<u32, Error> {
        let api = BlocksApi::BlockTipHeight;
        let response = api
            .send_async(&self.url, &mut move |request: crate::Request| {
                handler(&self.client, request)
            })
            .await?;
        api.deserialize_str::<u32>(&response)
    }

    /// Get the [`BlockHash`] of the current blockchain tip.
    pub async fn get_tip_hash(&self) -> Result<BlockHash, Error> {
        let api = BlocksApi::BlockTipHash;
        let response = api
            .send_async(&self.url, &mut move |request: crate::Request| {
                handler(&self.client, request)
            })
            .await?;
        api.deserialize_str::<BlockHash>(&response)
    }

    /// Get the [`BlockHash`] of a specific block height
    pub async fn get_block_hash(&self, block_height: u32) -> Result<BlockHash, Error> {
        let api = BlocksApi::BlockHash(block_height);
        let response = api
            .send_async(&self.url, &mut move |request: crate::Request| {
                handler(&self.client, request)
            })
            .await?;
        api.deserialize_str::<BlockHash>(&response)
    }

    /// Get an map where the key is the confirmation target (in number of
    /// blocks) and the value is the estimated feerate (in sat/vB).
    pub async fn get_fee_estimates(&self) -> Result<HashMap<u16, f64>, Error> {
        let api = FeeEstimatesApi::FeeRate;
        let response = api
            .send_async(&self.url, &mut move |request: crate::Request| {
                handler(&self.client, request)
            })
            .await?;
        api.deserialize_json::<HashMap<u16, f64>>(&response)
    }

    /// Get confirmed transaction history for the specified address/scripthash,
    /// sorted with newest first. Returns 25 transactions per page.
    /// More can be requested by specifying the last txid seen by the previous
    /// query.
    pub async fn scripthash_txs(
        &self,
        script: &Script,
        last_seen: Option<Txid>,
    ) -> Result<Vec<Tx>, Error> {
        let script_hash = sha256::Hash::hash(script.as_bytes());
        let address_api = match last_seen {
            Some(last_seen) => AddressApi::ScriptHashConfirmedTxHistory(script_hash, last_seen),
            None => AddressApi::ScriptHashTxHistory(script_hash),
        };
        let response = address_api
            .send_async(&self.url, &mut move |request: crate::Request| {
                handler(&self.client, request)
            })
            .await?;
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
    pub async fn get_blocks(&self, height: Option<u32>) -> Result<Vec<BlockSummary>, Error> {
        let api = BlocksApi::BlockSummaries(height);
        let response = api
            .send_async(&self.url, &mut move |request: crate::Request| {
                handler(&self.client, request)
            })
            .await?;
        api.deserialize_json::<Vec<BlockSummary>>(&response)
    }

    /// Get the underlying base URL.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Get the underlying [`reqwest::Client`].
    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }
}
