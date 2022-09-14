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

//! Esplora by way of `ureq` HTTP client.

use std::collections::HashMap;
use std::io;
use std::io::Read;
use std::str::FromStr;
use std::time::Duration;

#[allow(unused_imports)]
use log::{debug, error, info, trace};

use ureq::{Agent, Proxy, Response};

use bitcoin::consensus::{deserialize, serialize};
use bitcoin::hashes::hex::{FromHex, ToHex};
use bitcoin::hashes::{sha256, Hash};
use bitcoin::{BlockHash, BlockHeader, Script, Transaction, Txid};

use crate::{Builder, Error, MerkleProof, OutputStatus, Tx, TxStatus};

#[derive(Debug, Clone)]
pub struct BlockingClient {
    url: String,
    agent: Agent,
}

impl BlockingClient {
    /// build a blocking client from a [`Builder`]
    pub fn from_builder(builder: Builder) -> Result<Self, Error> {
        let mut agent_builder = ureq::AgentBuilder::new();

        if let Some(timeout) = builder.timeout {
            agent_builder = agent_builder.timeout(Duration::from_secs(timeout));
        }

        if let Some(proxy) = &builder.proxy {
            agent_builder = agent_builder.proxy(Proxy::new(proxy)?);
        }

        Ok(Self::from_agent(builder.base_url, agent_builder.build()))
    }

    /// build a blocking client from an [`Agent`]
    pub fn from_agent(url: String, agent: Agent) -> Self {
        BlockingClient { url, agent }
    }

    /// Get a [`Transaction`] option given its [`Txid`]
    pub fn get_tx(&self, txid: &Txid) -> Result<Option<Transaction>, Error> {
        let resp = self
            .agent
            .get(&format!("{}/tx/{}/raw", self.url, txid))
            .call();

        match resp {
            Ok(resp) => Ok(Some(deserialize(&into_bytes(resp)?)?)),
            Err(ureq::Error::Status(code, _)) => {
                if is_status_not_found(code) {
                    return Ok(None);
                }
                Err(Error::HttpResponse(code))
            }
            Err(e) => Err(Error::Ureq(e)),
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

    /// Get a [`Txid`] of a transaction given its index in a block with a given hash.
    pub fn get_txid_at_block_index(
        &self,
        block_hash: &BlockHash,
        index: usize,
    ) -> Result<Option<Txid>, Error> {
        let resp = self
            .agent
            .get(&format!(
                "{}/block/{}/txid/{}",
                self.url,
                block_hash.to_string(),
                index
            ))
            .call();

        match resp {
            Ok(resp) => Ok(Some(Txid::from_str(&resp.into_string()?)?)),
            Err(ureq::Error::Status(code, _)) => {
                if is_status_not_found(code) {
                    return Ok(None);
                }
                Err(Error::HttpResponse(code))
            }
            Err(e) => Err(Error::Ureq(e)),
        }
    }

    /// Get the status of a [`Transaction`] given its [`Txid`].
    pub fn get_tx_status(&self, txid: &Txid) -> Result<Option<TxStatus>, Error> {
        let resp = self
            .agent
            .get(&format!("{}/tx/{}/status", self.url, txid))
            .call();

        match resp {
            Ok(resp) => Ok(Some(resp.into_json()?)),
            Err(ureq::Error::Status(code, _)) => {
                if is_status_not_found(code) {
                    return Ok(None);
                }
                Err(Error::HttpResponse(code))
            }
            Err(e) => Err(Error::Ureq(e)),
        }
    }

    /// Get a [`BlockHeader`] given a particular block height.
    pub fn get_header(&self, block_height: u32) -> Result<BlockHeader, Error> {
        let resp = self
            .agent
            .get(&format!("{}/block-height/{}", self.url, block_height))
            .call();

        let bytes = match resp {
            Ok(resp) => Ok(into_bytes(resp)?),
            Err(ureq::Error::Status(code, _)) => Err(Error::HttpResponse(code)),
            Err(e) => Err(Error::Ureq(e)),
        }?;

        let hash =
            std::str::from_utf8(&bytes).map_err(|_| Error::HeaderHeightNotFound(block_height))?;

        let resp = self
            .agent
            .get(&format!("{}/block/{}/header", self.url, hash))
            .call();

        match resp {
            Ok(resp) => Ok(deserialize(&Vec::from_hex(&resp.into_string()?)?)?),
            Err(ureq::Error::Status(code, _)) => Err(Error::HttpResponse(code)),
            Err(e) => Err(Error::Ureq(e)),
        }
    }

    /// Get a merkle inclusion proof for a [`Transaction`] with the given [`Txid`].
    pub fn get_merkle_proof(&self, txid: &Txid) -> Result<Option<MerkleProof>, Error> {
        let resp = self
            .agent
            .get(&format!("{}/tx/{}/merkle-proof", self.url, txid))
            .call();

        match resp {
            Ok(resp) => Ok(Some(resp.into_json()?)),
            Err(ureq::Error::Status(code, _)) => {
                if is_status_not_found(code) {
                    return Ok(None);
                }
                Err(Error::HttpResponse(code))
            }
            Err(e) => Err(Error::Ureq(e)),
        }
    }

    /// Get the spending status of an output given a [`Txid`] and the output index.
    pub fn get_output_status(
        &self,
        txid: &Txid,
        index: u64,
    ) -> Result<Option<OutputStatus>, Error> {
        let resp = self
            .agent
            .get(&format!("{}/tx/{}/outspend/{}", self.url, txid, index))
            .call();

        match resp {
            Ok(resp) => Ok(Some(resp.into_json()?)),
            Err(ureq::Error::Status(code, _)) => {
                if is_status_not_found(code) {
                    return Ok(None);
                }
                Err(Error::HttpResponse(code))
            }
            Err(e) => Err(Error::Ureq(e)),
        }
    }

    /// Broadcast a [`Transaction`] to Esplora
    pub fn broadcast(&self, transaction: &Transaction) -> Result<(), Error> {
        let resp = self
            .agent
            .post(&format!("{}/tx", self.url))
            .send_string(&serialize(transaction).to_hex());

        match resp {
            Ok(_) => Ok(()), // We do not return the txid?
            Err(ureq::Error::Status(code, _)) => Err(Error::HttpResponse(code)),
            Err(e) => Err(Error::Ureq(e)),
        }
    }

    /// Get the height of the current blockchain tip.
    pub fn get_height(&self) -> Result<u32, Error> {
        let resp = self
            .agent
            .get(&format!("{}/blocks/tip/height", self.url))
            .call();

        match resp {
            Ok(resp) => Ok(resp.into_string()?.parse()?),
            Err(ureq::Error::Status(code, _)) => Err(Error::HttpResponse(code)),
            Err(e) => Err(Error::Ureq(e)),
        }
    }

    /// Get the [`BlockHash`] of the current blockchain tip.
    pub fn get_tip_hash(&self) -> Result<BlockHash, Error> {
        let resp = self
            .agent
            .get(&format!("{}/blocks/tip/hash", self.url))
            .call();

        match resp {
            Ok(resp) => Ok(BlockHash::from_str(&resp.into_string()?)?),
            Err(ureq::Error::Status(code, _)) => Err(Error::HttpResponse(code)),
            Err(e) => Err(Error::Ureq(e)),
        }
    }

    /// Get an map where the key is the confirmation target (in number of blocks)
    /// and the value is the estimated feerate (in sat/vB).
    pub fn get_fee_estimates(&self) -> Result<HashMap<String, f64>, Error> {
        let resp = self
            .agent
            .get(&format!("{}/fee-estimates", self.url,))
            .call();

        let map = match resp {
            Ok(resp) => {
                let map: HashMap<String, f64> = resp.into_json()?;
                Ok(map)
            }
            Err(ureq::Error::Status(code, _)) => Err(Error::HttpResponse(code)),
            Err(e) => Err(Error::Ureq(e)),
        }?;

        Ok(map)
    }

    /// Get confirmed transaction history for the specified address/scripthash,
    /// sorted with newest first. Returns 25 transactions per page.
    /// More can be requested by specifying the last txid seen by the previous query.
    pub fn scripthash_txs(
        &self,
        script: &Script,
        last_seen: Option<Txid>,
    ) -> Result<Vec<Tx>, Error> {
        let script_hash = sha256::Hash::hash(script.as_bytes()).into_inner().to_hex();
        let url = match last_seen {
            Some(last_seen) => format!(
                "{}/scripthash/{}/txs/chain/{}",
                self.url, script_hash, last_seen
            ),
            None => format!("{}/scripthash/{}/txs", self.url, script_hash),
        };
        Ok(self.agent.get(&url).call()?.into_json()?)
    }
}

fn is_status_not_found(status: u16) -> bool {
    status == 404
}

fn into_bytes(resp: Response) -> Result<Vec<u8>, io::Error> {
    const BYTES_LIMIT: usize = 10 * 1_024 * 1_024;

    let mut buf: Vec<u8> = vec![];
    resp.into_reader()
        .take((BYTES_LIMIT + 1) as u64)
        .read_to_end(&mut buf)?;
    if buf.len() > BYTES_LIMIT {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "response too big for into_bytes",
        ));
    }

    Ok(buf)
}

impl From<ureq::Error> for Error {
    fn from(e: ureq::Error) -> Self {
        match e {
            ureq::Error::Status(code, _) => Error::HttpResponse(code),
            e => Error::Ureq(e),
        }
    }
}
