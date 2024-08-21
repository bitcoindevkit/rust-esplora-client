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

use bitcoin::consensus::{deserialize, serialize};
use bitcoin::hashes::{sha256, Hash};
use bitcoin::hex::{DisplayHex, FromHex};
use bitcoin::{
    block::Header as BlockHeader, Block, BlockHash, MerkleBlock, Script, Transaction, Txid,
};

#[allow(unused_imports)]
use log::{debug, error, info, trace};

use reqwest::{header, Client, StatusCode};

use crate::{BlockStatus, BlockSummary, Builder, Error, MerkleProof, OutputStatus, Tx, TxStatus};

#[derive(Debug, Clone)]
pub struct AsyncClient {
    /// The URL of the Esplora Server.
    url: String,
    /// The inner [`reqwest::Client`] to make HTTP requests.
    client: Client,
}

impl AsyncClient {
    /// Build an async client from a builder
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

        Ok(Self::from_client(builder.base_url, client_builder.build()?))
    }

    /// Build an async client from the base url and [`Client`]
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

        if let StatusCode::NOT_FOUND = resp.status() {
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

    /// Get a [`Txid`] of a transaction given its index in a block with a given
    /// hash.
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

        if let StatusCode::NOT_FOUND = resp.status() {
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

    /// Get transaction info given it's [`Txid`].
    pub async fn get_tx_info(&self, txid: &Txid) -> Result<Option<Tx>, Error> {
        let resp = self
            .client
            .get(&format!("{}/tx/{}", self.url, txid))
            .send()
            .await?;
        if resp.status() == StatusCode::NOT_FOUND {
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

        if let StatusCode::NOT_FOUND = resp.status() {
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

    /// Get a merkle inclusion proof for a [`Transaction`] with the given
    /// [`Txid`].
    pub async fn get_merkle_proof(&self, tx_hash: &Txid) -> Result<Option<MerkleProof>, Error> {
        let resp = self
            .client
            .get(&format!("{}/tx/{}/merkle-proof", self.url, tx_hash))
            .send()
            .await?;

        if let StatusCode::NOT_FOUND = resp.status() {
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

    /// Get a [`MerkleBlock`] inclusion proof for a [`Transaction`] with the
    /// given [`Txid`].
    pub async fn get_merkle_block(&self, tx_hash: &Txid) -> Result<Option<MerkleBlock>, Error> {
        let resp = self
            .client
            .get(&format!("{}/tx/{}/merkleblock-proof", self.url, tx_hash))
            .send()
            .await?;

        if let StatusCode::NOT_FOUND = resp.status() {
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

    /// Get the spending status of an output given a [`Txid`] and the output
    /// index.
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

        if let StatusCode::NOT_FOUND = resp.status() {
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

        if let StatusCode::NOT_FOUND = resp.status() {
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
    /// More can be requested by specifying the last txid seen by the previous
    /// query.
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

    /// Get an map where the key is the confirmation target (in number of
    /// blocks) and the value is the estimated feerate (in sat/vB).
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

    /// Gets some recent block summaries starting at the tip or at `height` if
    /// provided.
    ///
    /// The maximum number of summaries returned depends on the backend itself:
    /// esplora returns `10` while [mempool.space](https://mempool.space/docs/api) returns `15`.
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
