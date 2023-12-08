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

//! Esplora by way of `reqwest`, and `arti-hyper` HTTP client.

use std::collections::HashMap;
use std::str::FromStr;

use bitcoin::consensus::{deserialize, serialize};
use bitcoin::hashes::{sha256, Hash};
use bitcoin::hex::{DisplayHex, FromHex};
use bitcoin::{
    block::Header as BlockHeader, Block, BlockHash, MerkleBlock, Script, Transaction, Txid,
};

#[allow(unused_imports)]
use log::{debug, error, info, trace};

#[cfg(feature = "async")]
use reqwest::Client;

#[cfg(feature = "async-arti-hyper")]
use {
    arti_client::{TorClient, TorClientConfig},
    arti_hyper::ArtiHttpConnector,
    hyper::service::Service,
    hyper::{Body, Request, Response, Uri},
    tls_api::{TlsConnector as TlsConnectorTrait, TlsConnectorBuilder},
    tor_rtcompat::PreferredRuntime,
};

#[cfg(feature = "async-arti-hyper")]
#[cfg(not(target_vendor = "apple"))]
use tls_api_native_tls::TlsConnector;
#[cfg(feature = "async-arti-hyper")]
#[cfg(target_vendor = "apple")]
use tls_api_openssl::TlsConnector;

use crate::{BlockStatus, BlockSummary, Builder, Error, MerkleProof, OutputStatus, Tx, TxStatus};

#[cfg(feature = "async")]
#[derive(Debug, Clone)]
pub struct AsyncClient {
    url: String,
    client: Client,
}

#[cfg(feature = "async")]
impl AsyncClient {
    /// build an async client from a builder
    pub fn from_builder(builder: Builder) -> Result<Self, Error> {
        let mut client_builder = Client::builder();

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(proxy) = &builder.proxy {
            client_builder = client_builder.proxy(reqwest::Proxy::all(proxy)?);
        }

        #[cfg(not(target_arch = "wasm32"))]
        if let Some(timeout) = builder.timeout {
            client_builder = client_builder.timeout(core::time::Duration::from_secs(timeout));
        }

        Ok(Self::from_client(builder.base_url, client_builder.build()?))
    }

    /// build an async client from the base url and [`Client`]
    pub fn from_client(url: String, client: Client) -> Self {
        AsyncClient { url, client }
    }

    /// Get a [`Transaction`] option given its [`Txid`]
    pub async fn get_tx(&self, txid: &Txid) -> Result<Option<Transaction>, Error> {
        let resp = self
            .client
            .get(&format!("{}/tx/{}/raw", self.url, txid))
            .send()
            .await?;

        if let reqwest::StatusCode::NOT_FOUND = resp.status() {
            return Ok(None);
        }

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: resp.text().await?,
            })
        } else {
            Ok(Some(deserialize(&resp.bytes().await?)?))
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

    /// Get a [`Txid`] of a transaction given its index in a block with a given hash.
    pub async fn get_txid_at_block_index(
        &self,
        block_hash: &BlockHash,
        index: usize,
    ) -> Result<Option<Txid>, Error> {
        let resp = self
            .client
            .get(&format!("{}/block/{}/txid/{}", self.url, block_hash, index))
            .send()
            .await?;

        if let reqwest::StatusCode::NOT_FOUND = resp.status() {
            return Ok(None);
        }

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: resp.text().await?,
            })
        } else {
            Ok(Some(Txid::from_str(&resp.text().await?)?))
        }
    }

    /// Get the status of a [`Transaction`] given its [`Txid`].
    pub async fn get_tx_status(&self, txid: &Txid) -> Result<TxStatus, Error> {
        let resp = self
            .client
            .get(&format!("{}/tx/{}/status", self.url, txid))
            .send()
            .await?;
        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: resp.text().await?,
            })
        } else {
            Ok(resp.json().await?)
        }
    }

    /// Get a [`BlockHeader`] given a particular block hash.
    pub async fn get_header_by_hash(&self, block_hash: &BlockHash) -> Result<BlockHeader, Error> {
        let resp = self
            .client
            .get(&format!("{}/block/{}/header", self.url, block_hash))
            .send()
            .await?;

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: resp.text().await?,
            })
        } else {
            let header = deserialize(&Vec::from_hex(&resp.text().await?)?)?;
            Ok(header)
        }
    }

    /// Get the [`BlockStatus`] given a particular [`BlockHash`].
    pub async fn get_block_status(&self, block_hash: &BlockHash) -> Result<BlockStatus, Error> {
        let resp = self
            .client
            .get(&format!("{}/block/{}/status", self.url, block_hash))
            .send()
            .await?;

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: resp.text().await?,
            })
        } else {
            Ok(resp.json().await?)
        }
    }

    /// Get a [`Block`] given a particular [`BlockHash`].
    pub async fn get_block_by_hash(&self, block_hash: &BlockHash) -> Result<Option<Block>, Error> {
        let resp = self
            .client
            .get(&format!("{}/block/{}/raw", self.url, block_hash))
            .send()
            .await?;

        if let reqwest::StatusCode::NOT_FOUND = resp.status() {
            return Ok(None);
        }

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: resp.text().await?,
            })
        } else {
            Ok(Some(deserialize(&resp.bytes().await?)?))
        }
    }

    /// Get a merkle inclusion proof for a [`Transaction`] with the given [`Txid`].
    pub async fn get_merkle_proof(&self, tx_hash: &Txid) -> Result<Option<MerkleProof>, Error> {
        let resp = self
            .client
            .get(&format!("{}/tx/{}/merkle-proof", self.url, tx_hash))
            .send()
            .await?;

        if let reqwest::StatusCode::NOT_FOUND = resp.status() {
            return Ok(None);
        }

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: resp.text().await?,
            })
        } else {
            Ok(Some(resp.json().await?))
        }
    }

    /// Get a [`MerkleBlock`] inclusion proof for a [`Transaction`] with the given [`Txid`].
    pub async fn get_merkle_block(&self, tx_hash: &Txid) -> Result<Option<MerkleBlock>, Error> {
        let resp = self
            .client
            .get(&format!("{}/tx/{}/merkleblock-proof", self.url, tx_hash))
            .send()
            .await?;

        if let reqwest::StatusCode::NOT_FOUND = resp.status() {
            return Ok(None);
        }

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: resp.text().await?,
            })
        } else {
            let merkle_block = deserialize(&Vec::from_hex(&resp.text().await?)?)?;
            Ok(Some(merkle_block))
        }
    }

    /// Get the spending status of an output given a [`Txid`] and the output index.
    pub async fn get_output_status(
        &self,
        txid: &Txid,
        index: u64,
    ) -> Result<Option<OutputStatus>, Error> {
        let resp = self
            .client
            .get(&format!("{}/tx/{}/outspend/{}", self.url, txid, index))
            .send()
            .await?;

        if let reqwest::StatusCode::NOT_FOUND = resp.status() {
            return Ok(None);
        }

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: resp.text().await?,
            })
        } else {
            Ok(Some(resp.json().await?))
        }
    }

    /// Broadcast a [`Transaction`] to Esplora
    pub async fn broadcast(&self, transaction: &Transaction) -> Result<(), Error> {
        let resp = self
            .client
            .post(&format!("{}/tx", self.url))
            .body(serialize(transaction).to_lower_hex_string())
            .send()
            .await?;

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: resp.text().await?,
            })
        } else {
            Ok(())
        }
    }

    /// Get the current height of the blockchain tip
    pub async fn get_height(&self) -> Result<u32, Error> {
        let resp = self
            .client
            .get(&format!("{}/blocks/tip/height", self.url))
            .send()
            .await?;

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: resp.text().await?,
            })
        } else {
            Ok(resp.text().await?.parse()?)
        }
    }

    /// Get the [`BlockHash`] of the current blockchain tip.
    pub async fn get_tip_hash(&self) -> Result<BlockHash, Error> {
        let resp = self
            .client
            .get(&format!("{}/blocks/tip/hash", self.url))
            .send()
            .await?;

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: resp.text().await?,
            })
        } else {
            Ok(BlockHash::from_str(&resp.text().await?)?)
        }
    }

    /// Get the [`BlockHash`] of a specific block height
    pub async fn get_block_hash(&self, block_height: u32) -> Result<BlockHash, Error> {
        let resp = self
            .client
            .get(&format!("{}/block-height/{}", self.url, block_height))
            .send()
            .await?;

        if let reqwest::StatusCode::NOT_FOUND = resp.status() {
            return Err(Error::HeaderHeightNotFound(block_height));
        }

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: resp.text().await?,
            })
        } else {
            Ok(BlockHash::from_str(&resp.text().await?)?)
        }
    }

    /// Get confirmed transaction history for the specified address/scripthash,
    /// sorted with newest first. Returns 25 transactions per page.
    /// More can be requested by specifying the last txid seen by the previous query.
    pub async fn scripthash_txs(
        &self,
        script: &Script,
        last_seen: Option<Txid>,
    ) -> Result<Vec<Tx>, Error> {
        let script_hash = sha256::Hash::hash(script.as_bytes());
        let url = match last_seen {
            Some(last_seen) => format!(
                "{}/scripthash/{:x}/txs/chain/{}",
                self.url, script_hash, last_seen
            ),
            None => format!("{}/scripthash/{:x}/txs", self.url, script_hash),
        };

        let resp = self.client.get(url).send().await?;

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: resp.text().await?,
            })
        } else {
            Ok(resp.json::<Vec<Tx>>().await?)
        }
    }

    /// Get an map where the key is the confirmation target (in number of blocks)
    /// and the value is the estimated feerate (in sat/vB).
    pub async fn get_fee_estimates(&self) -> Result<HashMap<u16, f64>, Error> {
        let resp = self
            .client
            .get(&format!("{}/fee-estimates", self.url,))
            .send()
            .await?;

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: resp.text().await?,
            })
        } else {
            Ok(resp.json::<HashMap<u16, f64>>().await?)
        }
    }

    /// Gets some recent block summaries starting at the tip or at `height` if provided.
    ///
    /// The maximum number of summaries returned depends on the backend itself: esplora returns `10`
    /// while [mempool.space](https://mempool.space/docs/api) returns `15`.
    pub async fn get_blocks(&self, height: Option<u32>) -> Result<Vec<BlockSummary>, Error> {
        let url = match height {
            Some(height) => format!("{}/blocks/{}", self.url, height),
            None => format!("{}/blocks", self.url),
        };

        let resp = self.client.get(&url).send().await?;

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: resp.text().await?,
            })
        } else {
            Ok(resp.json::<Vec<BlockSummary>>().await?)
        }
    }

    /// Get the underlying base URL.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Get the underlying [`Client`].
    pub fn client(&self) -> &Client {
        &self.client
    }
}

#[cfg(feature = "async-arti-hyper")]
#[derive(Debug, Clone)]
pub struct AsyncAnonymizedClient {
    url: String,
    client: hyper::Client<ArtiHttpConnector<PreferredRuntime, TlsConnector>>,
}

#[cfg(feature = "async-arti-hyper")]
impl AsyncAnonymizedClient {
    /// build an async [`TorClient`] with default Tor configuration
    async fn create_tor_client() -> Result<TorClient<PreferredRuntime>, arti_client::Error> {
        let config = TorClientConfig::default();
        TorClient::create_bootstrapped(config).await
    }

    /// build an [`AsyncAnonymizedClient`] from a [`Builder`]
    pub async fn from_builder(builder: Builder) -> Result<Self, Error> {
        let tor_client = Self::create_tor_client().await?.isolated_client();

        let tls_conn: TlsConnector = TlsConnector::builder()
            .map_err(|_| Error::TlsConnector)?
            .build()
            .map_err(|_| Error::TlsConnector)?;

        let connector = ArtiHttpConnector::new(tor_client, tls_conn);

        // TODO: (@leonardo) how to handle/pass the timeout option ?
        let client = hyper::Client::builder().build::<_, Body>(connector);
        Ok(Self::from_client(builder.base_url, client))
    }

    /// build an async client from the base url and [`Client`]
    pub fn from_client(
        url: String,
        client: hyper::Client<ArtiHttpConnector<PreferredRuntime, TlsConnector>>,
    ) -> Self {
        AsyncAnonymizedClient { url, client }
    }

    /// Get a [`Option<Transaction>`] given its [`Txid`]
    pub async fn get_tx(&self, txid: &Txid) -> Result<Option<Transaction>, Error> {
        let path = format!("{}/tx/{}/raw", self.url, txid);
        let uri = Uri::from_str(&path).map_err(|_| Error::InvalidUri)?;

        let resp = self.client.get(uri).await?;

        if let hyper::StatusCode::NOT_FOUND = resp.status() {
            return Ok(None);
        }

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: Self::text(resp).await?,
            })
        } else {
            let body = resp.into_body();
            let bytes = hyper::body::to_bytes(body).await?;
            Ok(Some(deserialize(&bytes)?))
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

    /// Get a [`Txid`] of a transaction given its index in a block with a given hash.
    pub async fn get_txid_at_block_index(
        &self,
        block_hash: &BlockHash,
        index: usize,
    ) -> Result<Option<Txid>, Error> {
        let path = format!("{}/block/{}/txid/{}", self.url, block_hash, index);
        let uri = Uri::from_str(&path).map_err(|_| Error::InvalidUri)?;

        let resp = self.client.get(uri).await?;

        if let hyper::StatusCode::NOT_FOUND = resp.status() {
            return Ok(None);
        }

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: Self::text(resp).await?,
            })
        } else {
            let text = Self::text(resp).await?;
            let txid = Txid::from_str(&text)?;
            Ok(Some(txid))
        }
    }

    /// Get the status of a [`Transaction`] given its [`Txid`].
    pub async fn get_tx_status(&self, txid: &Txid) -> Result<TxStatus, Error> {
        let path = format!("{}/tx/{}/status", self.url, txid);
        let uri = Uri::from_str(&path).map_err(|_| Error::InvalidUri)?;

        let resp = self.client.get(uri).await?;

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: Self::text(resp).await?,
            })
        } else {
            let body = resp.into_body();
            let bytes = hyper::body::to_bytes(body).await?;
            let tx_status =
                serde_json::from_slice::<TxStatus>(&bytes).map_err(|_| Error::ResponseDecoding)?;
            Ok(tx_status)
        }
    }

    /// Get a [`BlockHeader`] given a particular block hash.
    pub async fn get_header_by_hash(&self, block_hash: &BlockHash) -> Result<BlockHeader, Error> {
        let path = format!("{}/block/{}/header", self.url, block_hash);
        let uri = Uri::from_str(&path).map_err(|_| Error::InvalidUri)?;

        let resp = self.client.get(uri).await?;

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: Self::text(resp).await?,
            })
        } else {
            let text = Self::text(resp).await?;
            let block_header = deserialize(&Vec::from_hex(&text)?)?;
            Ok(block_header)
        }
    }

    /// Get the [`BlockStatus`] given a particular [`BlockHash`].
    pub async fn get_block_status(&self, block_hash: &BlockHash) -> Result<BlockStatus, Error> {
        let path = &format!("{}/block/{}/status", self.url, block_hash);
        let uri = Uri::from_str(path).map_err(|_| Error::InvalidUri)?;
        let resp = self.client.get(uri).await?;

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: Self::text(resp).await?,
            })
        } else {
            let body = resp.into_body();
            let bytes = hyper::body::to_bytes(body).await?;

            let block_status = serde_json::from_slice::<BlockStatus>(&bytes)
                .map_err(|_| Error::ResponseDecoding)?;
            Ok(block_status)
        }
    }

    /// Get a [`Block`] given a particular [`BlockHash`].
    pub async fn get_block_by_hash(&self, block_hash: &BlockHash) -> Result<Option<Block>, Error> {
        let path = format!("{}/block/{}/raw", self.url, block_hash);
        let uri = Uri::from_str(&path).map_err(|_| Error::InvalidUri)?;
        let resp = self.client.get(uri).await?;

        if let hyper::StatusCode::NOT_FOUND = resp.status() {
            return Ok(None);
        }

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: Self::text(resp).await?,
            })
        } else {
            let body = resp.into_body();
            let bytes = hyper::body::to_bytes(body).await?;
            Ok(Some(deserialize(&bytes)?))
        }
    }

    /// Get a merkle inclusion proof for a [`Transaction`] with the given [`Txid`].
    pub async fn get_merkle_proof(&self, tx_hash: &Txid) -> Result<Option<MerkleProof>, Error> {
        let path = format!("{}/tx/{}/merkle-proof", self.url, tx_hash);
        let uri = Uri::from_str(&path).map_err(|_| Error::InvalidUri)?;

        let resp = self.client.get(uri).await?;

        if let hyper::StatusCode::NOT_FOUND = resp.status() {
            return Ok(None);
        }

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: Self::text(resp).await?,
            })
        } else {
            let body = resp.into_body();
            let bytes = hyper::body::to_bytes(body).await?;
            let merkle_proof = serde_json::from_slice::<MerkleProof>(&bytes)
                .map_err(|_| Error::ResponseDecoding)?;
            Ok(Some(merkle_proof))
        }
    }

    /// Get a [`MerkleBlock`] inclusion proof for a [`Transaction`] with the given [`Txid`].
    pub async fn get_merkle_block(&self, tx_hash: &Txid) -> Result<Option<MerkleBlock>, Error> {
        let path = format!("{}/tx/{}/merkleblock-proof", self.url, tx_hash);
        let uri = Uri::from_str(&path).map_err(|_| Error::InvalidUri)?;

        let resp = self.client.get(uri).await?;

        if let hyper::StatusCode::NOT_FOUND = resp.status() {
            return Ok(None);
        }

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: Self::text(resp).await?,
            })
        } else {
            let text = Self::text(resp).await?;
            let merkle_block = deserialize(&Vec::from_hex(&text)?)?;
            Ok(Some(merkle_block))
        }
    }

    /// Get the spending status of an output given a [`Txid`] and the output index.
    pub async fn get_output_status(
        &self,
        txid: &Txid,
        index: u64,
    ) -> Result<Option<OutputStatus>, Error> {
        let path = &format!("{}/tx/{}/outspend/{}", self.url, txid, index);
        let uri = Uri::from_str(path).map_err(|_| Error::InvalidUri)?;
        let resp = self.client.get(uri).await?;

        if let hyper::StatusCode::NOT_FOUND = resp.status() {
            return Ok(None);
        }

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: Self::text(resp).await?,
            })
        } else {
            let body = resp.into_body();
            let bytes = hyper::body::to_bytes(body).await?;

            let output_status = serde_json::from_slice::<OutputStatus>(&bytes)
                .map_err(|_| Error::ResponseDecoding)?;
            Ok(Some(output_status))
        }
    }

    // /// Broadcast a [`Transaction`] to Esplora
    pub async fn broadcast(&mut self, transaction: &Transaction) -> Result<(), Error> {
        let path = &format!("{}/tx", self.url);
        let uri = Uri::from_str(path).map_err(|_| Error::InvalidUri)?;

        let body = Body::from(serialize(transaction).to_lower_hex_string());
        let req = Request::post(uri)
            .body(body)
            .map_err(|_| Error::InvalidBody)?;

        let resp = self.client.call(req).await?;

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: Self::text(resp).await?,
            })
        } else {
            Ok(())
        }
    }

    /// Get the current height of the blockchain tip
    pub async fn get_height(&self) -> Result<u32, Error> {
        let path = &format!("{}/blocks/tip/height", self.url);
        let uri = Uri::from_str(path).map_err(|_| Error::InvalidUri)?;
        let resp = self.client.get(uri).await?;

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: Self::text(resp).await?,
            })
        } else {
            let body = resp.into_body();
            let bytes = hyper::body::to_bytes(body).await?;

            let block_height =
                serde_json::from_slice::<u32>(&bytes).map_err(|_| Error::ResponseDecoding)?;
            Ok(block_height)
        }
    }

    /// Get the [`BlockHash`] of the current blockchain tip.
    pub async fn get_tip_hash(&self) -> Result<BlockHash, Error> {
        let path = &format!("{}/blocks/tip/hash", self.url);
        let uri = Uri::from_str(path).map_err(|_| Error::InvalidUri)?;
        let resp = self.client.get(uri).await?;

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: Self::text(resp).await?,
            })
        } else {
            let text = Self::text(resp).await?;
            let block_hash = BlockHash::from_str(&text).map_err(|_| Error::ResponseDecoding)?;
            Ok(block_hash)
        }
    }

    /// Get the [`BlockHash`] of a specific block height
    pub async fn get_block_hash(&self, block_height: u32) -> Result<BlockHash, Error> {
        let path = &format!("{}/block-height/{}", self.url, block_height);
        let uri = Uri::from_str(path).map_err(|_| Error::InvalidUri)?;
        let resp = self.client.get(uri).await?;

        if let hyper::StatusCode::NOT_FOUND = resp.status() {
            return Err(Error::HeaderHeightNotFound(block_height));
        }

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: Self::text(resp).await?,
            })
        } else {
            let text = Self::text(resp).await?;
            let block_hash = BlockHash::from_str(&text).map_err(|_| Error::ResponseDecoding)?;
            Ok(block_hash)
        }
    }

    /// Get an map where the key is the confirmation target (in number of blocks)
    /// and the value is the estimated feerate (in sat/vB).
    pub async fn get_fee_estimates(&self) -> Result<HashMap<String, f64>, Error> {
        let path = &format!("{}/fee-estimates", self.url);
        let uri = Uri::from_str(path).map_err(|_| Error::InvalidUri)?;
        let resp = self.client.get(uri).await?;

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: Self::text(resp).await?,
            })
        } else {
            let body = resp.into_body();
            let bytes = hyper::body::to_bytes(body).await?;
            let fee_estimates = serde_json::from_slice::<HashMap<String, f64>>(&bytes)
                .map_err(|_| Error::ResponseDecoding)?;
            Ok(fee_estimates)
        }
    }

    /// Get confirmed transaction history for the specified address/scripthash,
    /// sorted with newest first. Returns 25 transactions per page.
    /// More can be requested by specifying the last txid seen by the previous query.
    pub async fn scripthash_txs(
        &self,
        script: &Script,
        last_seen: Option<Txid>,
    ) -> Result<Vec<Tx>, Error> {
        let script_hash = sha256::Hash::hash(script.as_bytes());
        let path = match last_seen {
            Some(last_seen) => format!(
                "{}/scripthash/{:x}/txs/chain/{}",
                self.url, script_hash, last_seen
            ),
            None => format!("{}/scripthash/{:x}/txs", self.url, script_hash),
        };

        let uri = Uri::from_str(&path).map_err(|_| Error::InvalidUri)?;
        let resp = self.client.get(uri).await?;

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: Self::text(resp).await?,
            })
        } else {
            let body = resp.into_body();
            let bytes = hyper::body::to_bytes(body).await?;
            let txs =
                serde_json::from_slice::<Vec<Tx>>(&bytes).map_err(|_| Error::ResponseDecoding)?;
            Ok(txs)
        }
    }

    /// Gets some recent block summaries starting at the tip or at `height` if provided.
    ///
    /// The maximum number of summaries returned depends on the backend itself: esplora returns `10`
    /// while [mempool.space](https://mempool.space/docs/api) returns `15`.
    pub async fn get_blocks(&self, height: Option<u32>) -> Result<Vec<BlockSummary>, Error> {
        let path = match height {
            Some(height) => format!("{}/blocks/{}", self.url, height),
            None => format!("{}/blocks", self.url),
        };
        let uri = Uri::from_str(&path).map_err(|_| Error::InvalidUri)?;

        let resp = self.client.get(uri).await?;

        if resp.status().is_server_error() || resp.status().is_client_error() {
            Err(Error::HttpResponse {
                status: resp.status().as_u16(),
                message: Self::text(resp).await?,
            })
        } else {
            let body = resp.into_body();
            let bytes = hyper::body::to_bytes(body).await?;
            let blocks = serde_json::from_slice::<Vec<BlockSummary>>(&bytes)
                .map_err(|_| Error::ResponseDecoding)?;
            Ok(blocks)
        }
    }

    /// Get the underlying base URL.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Get the underlying [`hyper::Client`].
    pub fn client(&self) -> &hyper::Client<ArtiHttpConnector<PreferredRuntime, TlsConnector>> {
        &self.client
    }

    /// Get the given [`Response<Body>`] as [`String`].
    async fn text(response: Response<Body>) -> Result<String, Error> {
        let body = response.into_body();
        let bytes = hyper::body::to_bytes(body).await?;

        match std::str::from_utf8(&bytes) {
            Ok(text) => Ok(text.to_string()),
            Err(_) => Err(Error::ResponseDecoding),
        }
    }
}
