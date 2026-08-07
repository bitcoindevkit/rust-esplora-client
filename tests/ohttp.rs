// SPDX-License-Identifier: MIT OR Apache-2.0

//! # Integration Test for the Async OHTTP Client
//!
//! Spins up a minimal OHTTP relay and gateway, points an [`esplora_client::AsyncClient`]
//! configured with [`esplora_client::Builder::build_async_with_ohttp`] through them, and
//! verifies the result matches a direct (non-OHTTP) request.

#![allow(unused_imports)]
#![cfg(all(feature = "async-ohttp", feature = "tokio"))]

use std::io::Cursor;
use std::sync::Arc;

use bitcoin_ohttp as ohttp;
use bitreq::{Method as BitreqMethod, Request as BitreqRequest};
use http_body_util::{BodyExt, Full};
use hyper::body::{Bytes, Incoming};
use hyper::service::service_fn;
use hyper::{Method, Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

use esplora_client::Builder;

use testenv::TestEnv;

mod testenv;

fn find_free_port() -> u16 {
    let listener = std::net::TcpListener::bind("0.0.0.0:0").unwrap();
    listener.local_addr().unwrap().port()
}

/// Starts an OHTTP relay that forwards encapsulated requests to `gateway_url`.
async fn start_ohttp_relay(
    gateway_url: ohttp_relay::GatewayUri,
) -> (
    u16,
    tokio::task::JoinHandle<Result<(), Box<dyn std::error::Error + Send + Sync>>>,
) {
    let port = find_free_port();
    let relay = ohttp_relay::listen_tcp(port, gateway_url).await.unwrap();
    (port, relay)
}

/// Starts a minimal OHTTP gateway that decapsulates requests, forwards them to
/// `esplora_base_url`, and re-encapsulates the response.
async fn start_ohttp_gateway(esplora_base_url: String) -> (u16, tokio::task::JoinHandle<()>) {
    let port = find_free_port();
    let listener = TcpListener::bind(format!("0.0.0.0:{port}")).await.unwrap();
    let esplora_base_url = Arc::new(esplora_base_url);

    let handle = tokio::spawn(async move {
        let key_config = ohttp::KeyConfig::new(
            0,
            ohttp::hpke::Kem::K256Sha256,
            vec![ohttp::SymmetricSuite::new(
                ohttp::hpke::Kdf::HkdfSha256,
                ohttp::hpke::Aead::ChaCha20Poly1305,
            )],
        )
        .expect("valid key config");
        let server = Arc::new(ohttp::Server::new(key_config).expect("valid server"));

        loop {
            match listener.accept().await {
                Ok((stream, _)) => {
                    let io = TokioIo::new(stream);
                    let server = server.clone();
                    let esplora_base_url = esplora_base_url.clone();
                    let service = service_fn(move |req: Request<Incoming>| {
                        let server = server.clone();
                        let esplora_base_url = esplora_base_url.clone();
                        async move { handle_gateway_request(req, server, esplora_base_url).await }
                    });

                    tokio::spawn(async move {
                        if let Err(err) = hyper::server::conn::http1::Builder::new()
                            .serve_connection(io, service)
                            .await
                        {
                            eprintln!("Error serving connection: {err:?}");
                        }
                    });
                }
                Err(e) => {
                    eprintln!("Error accepting connection: {e:?}");
                    break;
                }
            }
        }
    });

    (port, handle)
}

async fn handle_gateway_request(
    req: Request<Incoming>,
    server: Arc<ohttp::Server>,
    esplora_base_url: Arc<String>,
) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let path = req.uri().path();
    if path == "/.well-known/ohttp-gateway" && req.method() == Method::GET {
        let key_config = server.config().encode().unwrap();
        return Ok(Response::builder()
            .status(200)
            .header("content-type", "application/ohttp-keys")
            .body(Full::new(Bytes::from(key_config)))
            .unwrap());
    }

    if path == "/.well-known/ohttp-gateway" && req.method() == Method::POST {
        let content_type_header = req
            .headers()
            .get("content-type")
            .expect("content-type header should be set by the client");
        assert_eq!(content_type_header, "message/ohttp-req");

        let ohttp_req_bytes = req.collect().await?.to_bytes();
        let (bhttp_body, response_ctx) = server.decapsulate(&ohttp_req_bytes).unwrap();
        let mut cursor = Cursor::new(bhttp_body);
        let inner: bhttp::Message =
            bhttp::Message::read_bhttp(&mut cursor).expect("valid bhttp message");
        let inner_path = String::from_utf8(inner.control().path().unwrap().to_vec()).unwrap();

        let mut forward =
            BitreqRequest::new(BitreqMethod::Get, format!("{esplora_base_url}{inner_path}"));
        for field in inner.header().iter() {
            forward = forward.with_header(
                String::from_utf8_lossy(field.name()).into_owned(),
                String::from_utf8_lossy(field.value()).into_owned(),
            );
        }
        let res = forward.send_async().await.unwrap();

        let status_code = bhttp::StatusCode::try_from(u16::try_from(res.status_code).unwrap())
            .expect("valid status code");
        let mut response_message = bhttp::Message::response(status_code);
        response_message.write_content(res.as_bytes());
        let mut bhttp_res = Vec::new();
        response_message
            .write_bhttp(bhttp::Mode::IndeterminateLength, &mut bhttp_res)
            .unwrap();
        let encapsulated_response = response_ctx.encapsulate(&bhttp_res).unwrap();

        return Ok(Response::builder()
            .status(200)
            .header("content-type", "message/ohttp-res")
            .body(Full::new(Bytes::copy_from_slice(&encapsulated_response)))
            .unwrap());
    }

    Ok(Response::builder()
        .status(404)
        .body(Full::new(Bytes::from("Not Found")))
        .unwrap())
}

#[tokio::test]
async fn test_ohttp_e2e() {
    let env = TestEnv::new();

    let block_hash = env.async_client.get_block_hash(1).await.unwrap();

    let esplora_base_url = format!("http://{}", env.esplora_url());
    let (gateway_port, _gateway_handle) = start_ohttp_gateway(esplora_base_url).await;
    let gateway_origin = format!("http://localhost:{gateway_port}");
    let (relay_port, _relay_handle) =
        start_ohttp_relay(gateway_origin.parse::<ohttp_relay::GatewayUri>().unwrap()).await;

    let gateway_url = format!("http://localhost:{gateway_port}/.well-known/ohttp-gateway");
    let relay_url = format!("http://localhost:{relay_port}");

    let ohttp_client = Builder::new(&format!("http://{}", env.esplora_url()))
        .build_async_with_ohttp(&relay_url, &gateway_url)
        .await
        .unwrap();

    let res = ohttp_client.get_block_hash(1).await.unwrap();
    assert_eq!(res, block_hash);
}
