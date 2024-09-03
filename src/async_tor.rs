// Bitcoin Dev Kit
// Written in 2024 by BDK Developers
//
// Copyright (c) 2020-2024 Bitcoin Dev Kit Developers
//
// This file is licensed under the Apache License, Version 2.0 <LICENSE-APACHE
// or http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your option.
// You may not use this file except in accordance with one or both of these
// licenses.

//! Esplora by way of `arti-client` over `hyper` HTTP client.

use arti_client::{TorClient, TorClientConfig};

use bitcoin::block::Header as BlockHeader;
use bitcoin::hashes::{sha256, Hash};
use hex::{DisplayHex, FromHex};
use http::header::HOST;
use http::{HeaderName, HeaderValue};
use http_body_util::{BodyExt, Empty};
use hyper::body::{Bytes, Incoming};
use hyper::{Request, Response};

use core::str;
use std::collections::HashMap;
use std::str::FromStr;

use bitcoin::consensus::{deserialize, Decodable};
use bitcoin::{Block, BlockHash, MerkleBlock, Script, Transaction, Txid};

#[allow(unused_imports)]
use log::{debug, error, info, trace};

use tokio::io::{AsyncRead, AsyncWrite};
use tor_rtcompat::PreferredRuntime;

use crate::{BlockStatus, BlockSummary, Builder, Error, MerkleProof, OutputStatus, Tx, TxStatus};

#[cfg(feature = "async-tor")]
// #[derive(Debug, Clone)]
pub struct AsyncTorClient {
    /// The URL of the Esplora Server.
    url: String,
    /// The inner [`arti_client::TorClient`] to make HTTP requests over Tor network.
    client: TorClient<tor_rtcompat::PreferredRuntime>,
    /// Socket timeout.
    pub timeout: Option<u64>,
    /// HTTP headers to set on every request made to Esplora server.
    pub headers: HashMap<String, String>,
}

#[cfg(feature = "async-tor")]
impl AsyncTorClient {
    /// Build a [`TorClient`] with default [`TorClientConfig`].
    pub async fn create_tor_client() -> Result<TorClient<PreferredRuntime>, arti_client::Error> {
        let config = TorClientConfig::default();
        TorClient::create_bootstrapped(config).await
    }

    /// Build an async client from a builder
    pub async fn from_builder(builder: Builder) -> Result<Self, arti_client::Error> {
        let tor_client = Self::create_tor_client().await?.isolated_client();

        Ok(Self {
            url: builder.base_url,
            timeout: builder.timeout,
            headers: builder.headers,
            client: tor_client,
        })
    }

    /// Get the underlying base URL.
    pub fn url(&self) -> &str {
        &self.url
    }

    async fn hyper_request(
        &self,
        uri: &hyper::Uri,
        data_stream: impl AsyncRead + AsyncWrite + Unpin + Send + 'static,
    ) -> Result<Response<Incoming>, Error> {
        let io = hyper_util::rt::TokioIo::new(data_stream);
        let (mut sender, connection) = hyper::client::conn::http1::handshake(io).await.unwrap();

        tokio::task::spawn(async move {
            if let Err(_e) = connection.await {
                // panic!() // FIXME: (@leonardo) do not panic, return proper error!
            }
        });

        let mut request = Request::get(uri.path())
            .body(Empty::<Bytes>::new())
            .unwrap(); // TODO: (@leonardo) fix this unwrap

        let headers = request.headers_mut();
        headers.insert(HOST, HeaderValue::from_str(uri.host().unwrap()).unwrap());

        if !self.headers.is_empty() {
            for (key, val) in &self.headers {
                let header_name: HeaderName = HeaderName::from_str(key).unwrap();
                let header_value: HeaderValue = HeaderValue::from_str(val).unwrap();
                headers.insert(header_name, header_value);
            }
        }

        println!("{:?}", request);
        // TODO: (@leonardo) fix this unwrap
        let response = sender.send_request(request).await.unwrap();

        Ok(response)
    }

    /// Perform a raw HTTP GET request with the given URI `path`.
    async fn get_request(&self, uri: &str) -> Result<Response<Bytes>, Error> {
        let url = hyper::Uri::from_str(uri).unwrap(); // TODO: (@leonardo) fix this unwrap
        let host = url.host().unwrap().to_owned(); // TODO: (@leonardo) fix this unwrap

        let is_tls = match url.scheme_str() {
            Some("https") => true,
            Some("http") => false,
            Some(_unexpected_scheme) => {
                panic!() // FIXME: (@leonardo) do not panic, return proper error!
            }
            None => {
                panic!() // FIXME: (@leonardo) do not panic, return proper error!
            }
        };

        let port = url.port_u16().unwrap_or(match is_tls {
            true => 443,
            false => 80,
        });

        let anonymized_data_stream = self
            .client
            .connect((host.clone(), port))
            .await
            .map_err(Error::Arti)?;

        let response = match is_tls {
            false => {
                self.hyper_request(&url.clone(), anonymized_data_stream)
                    .await
            }
            true => {
                // let cx = tokio_native_tls::native_tls::TlsConnector::builder()
                //     .build()
                //     .unwrap();
                // let tls_connector = tokio_native_tls::TlsConnector::from(cx);
                // let mut tls_stream = tls_connector
                //     .connect(host, anonymized_data_stream)
                //     .await
                //     .unwrap();

                let webpki_roots = webpki_roots::TLS_SERVER_ROOTS.iter().cloned();
                let mut root_certs = tokio_rustls::rustls::RootCertStore::empty();
                root_certs.extend(webpki_roots);

                let tls_config = tokio_rustls::rustls::ClientConfig::builder()
                    .with_root_certificates(root_certs)
                    .with_no_client_auth();
                let tls_connector =
                    tokio_rustls::TlsConnector::from(std::sync::Arc::new(tls_config));

                let server_name = rustls_pki_types::ServerName::try_from(host.clone()).unwrap();

                let tls_stream = tls_connector
                    .connect(server_name, anonymized_data_stream)
                    .await
                    .unwrap();
                self.hyper_request(&url.clone(), tls_stream).await
            }
        };

        let (parts, body) = response.unwrap().into_parts();
        let body = body.collect().await.unwrap().to_bytes();
        let response = Response::from_parts(parts, body);

        Ok(response) // TODO: fix unwrap
    }

    async fn get_response<T: Decodable>(&self, path: &str) -> Result<T, Error> {
        let url = format!("{}{}", self.url, path);
        let response = self.get_request(&url).await.unwrap();

        println!("{:?}", response);
        match response.status().is_success() {
            true => Ok(deserialize::<T>(&response.into_body()).unwrap()),
            false => Err(Error::HttpResponse {
                status: response.status().as_u16(),
                message: str::from_utf8(response.body()).unwrap().to_string(),
            }),
        }
    }

    /// Make an HTTP GET request to given URL, deserializing to `Option<T>`.
    ///
    /// It uses [`AsyncTorClient::get_response`] internally.
    ///
    /// See [`AsyncTorClient::get_response`] above for full documentation.
    async fn get_opt_response<T: Decodable>(&self, path: &str) -> Result<Option<T>, Error> {
        match self.get_response::<T>(path).await {
            Ok(res) => Ok(Some(res)),
            Err(Error::HttpResponse { status, message }) => match status {
                404 => Ok(None),
                _ => Err(Error::HttpResponse { status, message }),
            },
            Err(e) => Err(e),
        }
    }

    /// Make an HTTP GET request to given URL, deserializing to any `T` that
    /// implements [`serde::de::DeserializeOwned`].
    ///
    /// It should be used when requesting Esplora endpoints that have a specific
    /// defined API, mostly defined in [`crate::api`].
    ///
    /// # Errors
    ///
    /// This function will return an error either from the HTTP client, or the
    /// [`serde::de::DeserializeOwned`] deserialization.
    async fn get_response_json<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
    ) -> Result<T, Error> {
        let url = format!("{}{}", self.url, path);
        let response = self.get_request(&url).await.unwrap();

        match response.status().is_success() {
            true => {
                let body = response.into_body();
                let json = serde_json::from_slice::<T>(&body).unwrap();
                Ok(json)
            }
            false => Err(Error::HttpResponse {
                status: response.status().as_u16(),
                message: str::from_utf8(response.body()).unwrap().to_string(),
            }),
        }
    }

    /// Make an HTTP GET request to given URL, deserializing to `Option<T>`.
    ///
    /// It uses [`AsyncEsploraClient::get_response_json`] internally.
    ///
    /// See [`AsyncEsploraClient::get_response_json`] above for full
    /// documentation.
    async fn get_opt_response_json<T: serde::de::DeserializeOwned>(
        &self,
        url: &str,
    ) -> Result<Option<T>, Error> {
        match self.get_response_json(url).await {
            Ok(res) => Ok(Some(res)),
            Err(Error::HttpResponse { status, message }) => match status {
                404 => Ok(None),
                _ => Err(Error::HttpResponse { status, message }),
            },
            Err(e) => Err(e),
        }
    }

    /// Make an HTTP GET request to given URL, deserializing to any `T` that
    /// implement [`bitcoin::consensus::Decodable`] from Hex, [`Vec<u8>`].
    ///
    /// It should be used when requesting Esplora endpoints that can be directly
    /// deserialized to native `rust-bitcoin` types, which implements
    /// [`bitcoin::consensus::Decodable`] from Hex, `Vec<&u8>`.
    ///
    /// # Errors
    ///
    /// This function will return an error either from the HTTP client, or the
    /// [`bitcoin::consensus::Decodable`] deserialization.
    async fn get_response_hex<T: Decodable>(&self, path: &str) -> Result<T, Error> {
        let url = format!("{}{}", self.url, path);
        let response = self.get_request(&url).await?;

        match response.status().is_success() {
            true => {
                let hex_str = response.into_body().as_hex().to_string();
                let hex_vec = Vec::from_hex(&hex_str)?;
                Ok(deserialize(&hex_vec)?)
            }
            false => Err(Error::HttpResponse {
                status: response.status().as_u16(),
                message: str::from_utf8(response.body()).unwrap().to_string(),
            }),
        }
    }

    /// Make an HTTP GET request to given URL, deserializing to `Option<T>`.
    ///
    /// It uses [`AsyncEsploraClient::get_response_hex`] internally.
    ///
    /// See [`AsyncEsploraClient::get_response_hex`] above for full
    /// documentation.
    async fn get_opt_response_hex<T: Decodable>(&self, path: &str) -> Result<Option<T>, Error> {
        match self.get_response_hex(path).await {
            Ok(res) => Ok(Some(res)),
            Err(Error::HttpResponse { status, message }) => match status {
                404 => Ok(None),
                _ => Err(Error::HttpResponse { status, message }),
            },
            Err(e) => Err(e),
        }
    }

    /// Make an HTTP GET request to given URL, deserializing to `String`.
    ///
    /// It should be used when requesting Esplora endpoints that can return
    /// `String` formatted data that can be parsed downstream.
    ///
    /// # Errors
    ///
    /// This function will return an error either from the HTTP client.
    async fn get_response_text(&self, path: &str) -> Result<String, Error> {
        let url = format!("{}{}", self.url, path);
        let response = self.get_request(&url).await.unwrap();

        match response.status().is_success() {
            true => Ok(str::from_utf8(response.body()).unwrap().to_string()),
            false => Err(Error::HttpResponse {
                status: response.status().as_u16(),
                message: str::from_utf8(response.body()).unwrap().to_string(),
            }),
        }
    }

    /// Make an HTTP GET request to given URL, deserializing to `Option<T>`.
    ///
    /// It uses [`AsyncEsploraClient::get_response_text`] internally.
    ///
    /// See [`AsyncEsploraClient::get_response_text`] above for full
    /// documentation.
    async fn get_opt_response_text(&self, path: &str) -> Result<Option<String>, Error> {
        match self.get_response_text(path).await {
            Ok(s) => Ok(Some(s)),
            Err(Error::HttpResponse { status, message }) => match status {
                404 => Ok(None),
                _ => Err(Error::HttpResponse { status, message }),
            },
            Err(e) => Err(e),
        }
    }

    /// Make an HTTP POST request to given URL, serializing from any `T` that
    /// implement [`bitcoin::consensus::Encodable`].
    ///
    /// It should be used when requesting Esplora endpoints that expected a
    /// native bitcoin type serialized with [`bitcoin::consensus::Encodable`].
    ///
    /// # Errors
    ///
    /// This function will return an error either from the HTTP client, or the
    /// [`bitcoin::consensus::Encodable`] serialization.
    // async fn post_request_hex<T: Encodable>(&self, path: &str, body: T) -> Result<(), Error> {
    //     // let url = format!("{}{}", self.url, path);
    //     // let body = serialize::<T>(&body).to_lower_hex_string();

    //     // let response = self.client.post(url).body(body).send().await?;

    //     // match response.status().is_success() {
    //     //     true => Ok(()),
    //     //     false => Err(Error::HttpResponse {
    //     //         status: response.status().as_u16(),
    //     //         message: str::from_utf8(response.body()).unwrap().to_string(),
    //     //     }),
    //     // }

    //     todo!()
    // }

    /// Get a [`Transaction`] option given its [`Txid`]
    pub async fn get_tx(&self, txid: &Txid) -> Result<Option<Transaction>, Error> {
        self.get_opt_response(&format!("/tx/{txid}/raw")).await
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
        match self
            .get_opt_response_text(&format!("/block/{block_hash}/txid/{index}"))
            .await?
        {
            Some(s) => Ok(Some(Txid::from_str(&s).map_err(Error::HexToArray)?)),
            None => Ok(None),
        }
    }

    /// Get the status of a [`Transaction`] given its [`Txid`].
    pub async fn get_tx_status(&self, txid: &Txid) -> Result<TxStatus, Error> {
        self.get_response_json(&format!("/tx/{txid}/status")).await
    }

    /// Get transaction info given it's [`Txid`].
    pub async fn get_tx_info(&self, txid: &Txid) -> Result<Option<Tx>, Error> {
        self.get_opt_response_json(&format!("/tx/{txid}")).await
    }

    /// Get a [`BlockHeader`] given a particular block hash.
    pub async fn get_header_by_hash(&self, block_hash: &BlockHash) -> Result<BlockHeader, Error> {
        self.get_response_hex(&format!("/block/{block_hash}/header"))
            .await
    }

    /// Get the [`BlockStatus`] given a particular [`BlockHash`].
    pub async fn get_block_status(&self, block_hash: &BlockHash) -> Result<BlockStatus, Error> {
        self.get_response_json(&format!("/block/{block_hash}/status"))
            .await
    }

    /// Get a [`Block`] given a particular [`BlockHash`].
    pub async fn get_block_by_hash(&self, block_hash: &BlockHash) -> Result<Option<Block>, Error> {
        self.get_opt_response(&format!("/block/{block_hash}/raw"))
            .await
    }

    /// Get a merkle inclusion proof for a [`Transaction`] with the given
    /// [`Txid`].
    pub async fn get_merkle_proof(&self, tx_hash: &Txid) -> Result<Option<MerkleProof>, Error> {
        self.get_opt_response_json(&format!("/tx/{tx_hash}/merkle-proof"))
            .await
    }

    /// Get a [`MerkleBlock`] inclusion proof for a [`Transaction`] with the
    /// given [`Txid`].
    pub async fn get_merkle_block(&self, tx_hash: &Txid) -> Result<Option<MerkleBlock>, Error> {
        self.get_opt_response_hex(&format!("/tx/{tx_hash}/merkleblock-proof"))
            .await
    }

    /// Get the spending status of an output given a [`Txid`] and the output
    /// index.
    pub async fn get_output_status(
        &self,
        txid: &Txid,
        index: u64,
    ) -> Result<Option<OutputStatus>, Error> {
        self.get_opt_response_json(&format!("/tx/{txid}/outspend/{index}"))
            .await
    }

    // /// Broadcast a [`Transaction`] to Esplora
    // pub async fn broadcast(&self, transaction: &Transaction) -> Result<(), Error> {
    //     self.post_request_hex("/tx", transaction).await
    // }

    /// Get the current height of the blockchain tip
    pub async fn get_height(&self) -> Result<u32, Error> {
        self.get_response_text("/blocks/tip/height")
            .await
            .map(|height| u32::from_str(&height).map_err(Error::Parsing))?
    }

    /// Get the [`BlockHash`] of the current blockchain tip.
    pub async fn get_tip_hash(&self) -> Result<BlockHash, Error> {
        self.get_response_text("/blocks/tip/hash")
            .await
            .map(|block_hash| BlockHash::from_str(&block_hash).map_err(Error::HexToArray))?
    }

    /// Get the [`BlockHash`] of a specific block height
    pub async fn get_block_hash(&self, block_height: u32) -> Result<BlockHash, Error> {
        self.get_response_text(&format!("/block-height/{block_height}"))
            .await
            .map(|block_hash| BlockHash::from_str(&block_hash).map_err(Error::HexToArray))?
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
        let path = match last_seen {
            Some(last_seen) => format!("/scripthash/{:x}/txs/chain/{}", script_hash, last_seen),
            None => format!("/scripthash/{:x}/txs", script_hash),
        };

        self.get_response_json(&path).await
    }

    /// Get an map where the key is the confirmation target (in number of
    /// blocks) and the value is the estimated feerate (in sat/vB).
    pub async fn get_fee_estimates(&self) -> Result<HashMap<u16, f64>, Error> {
        self.get_response_json("/fee-estimates").await
    }

    /// Gets some recent block summaries starting at the tip or at `height` if
    /// provided.
    ///
    /// The maximum number of summaries returned depends on the backend itself:
    /// esplora returns `10` while [mempool.space](https://mempool.space/docs/api) returns `15`.
    pub async fn get_blocks(&self, height: Option<u32>) -> Result<Vec<BlockSummary>, Error> {
        let path = match height {
            Some(height) => format!("/blocks/{height}"),
            None => "/blocks".to_string(),
        };
        self.get_response_json(&path).await
    }
}
