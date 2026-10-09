// SPDX-License-Identifier: MIT OR Apache-2.0

//! Integration test for the async OHTTP client.
//!
//! Points an [`esplora_client::AsyncClient`] configured with
//! [`esplora_client::Builder::build_async_with_ohttp`] through the relay and
//! gateway from [`ohttp_client::harness`], and verifies the result matches a
//! direct (non-OHTTP) request.

#![cfg(all(feature = "async-ohttp", feature = "tokio"))]

use ohttp_client::harness::TestHarness;

use esplora_client::Builder;

use testenv::TestEnv;

mod testenv;

#[tokio::test]
async fn test_ohttp_e2e() {
    let env = TestEnv::new();

    let block_hash = env.async_client.get_block_hash(1).await.unwrap();

    // `TestHarness::start` drives its relay with `Runtime::block_on`, which
    // panics if called on a runtime worker thread.
    let harness = tokio::task::spawn_blocking(TestHarness::start)
        .await
        .unwrap();

    // The harness serves `CONNECT` from a separate port: `ohttp-relay`'s own
    // bootstrap tunnel gates on an HTTPS-only gateway probe that the harness's
    // plain-HTTP gateway cannot satisfy. A deployed relay tunnels on its own
    // address, which is what the builder defaults to.
    let ohttp_client = Builder::new(env.async_client.url())
        .ohttp_bootstrap_proxy(harness.connect_proxy_url())
        .build_async_with_ohttp(harness.relay_url(), harness.gateway_url())
        .await
        .unwrap();

    let res = ohttp_client.get_block_hash(1).await.unwrap();
    assert_eq!(res, block_hash);
}
