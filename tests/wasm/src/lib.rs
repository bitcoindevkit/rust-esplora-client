#![cfg(target_arch = "wasm32")]

use core::future::Ready;
use core::time::Duration;
use std::sync::atomic::{AtomicUsize, Ordering};

use esplora_client::absolute::LockTime;
use esplora_client::transaction::Version;
use esplora_client::{
    Amount, Builder, Error, OutPoint, ScriptBuf, Sequence, Sleeper, Transaction, TxIn, TxOut,
    Witness,
};
use wasm_bindgen_test::wasm_bindgen_test;

fn base_url() -> &'static str {
    option_env!("ESPLORA_WASM_TEST_URL").expect("ESPLORA_WASM_TEST_URL must be set")
}

/// WASM has no default sleeper, so the tests provide their own.
struct NoopSleeper;

impl Sleeper for NoopSleeper {
    type Sleep = Ready<()>;

    fn sleep(_: Duration) -> Self::Sleep {
        core::future::ready(())
    }
}

static SLEEPS: AtomicUsize = AtomicUsize::new(0);

struct CountingSleeper;

impl Sleeper for CountingSleeper {
    type Sleep = Ready<()>;

    fn sleep(_: Duration) -> Self::Sleep {
        SLEEPS.fetch_add(1, Ordering::SeqCst);
        core::future::ready(())
    }
}

#[wasm_bindgen_test(async)]
async fn get_height() {
    let client = Builder::new(base_url())
        .build_async_with_sleeper::<NoopSleeper>()
        .expect("build client");

    assert_eq!(client.get_height().await.expect("get height"), 101);
}

#[wasm_bindgen_test(async)]
async fn get_fee_estimates() {
    let client = Builder::new(base_url())
        .build_async_with_sleeper::<NoopSleeper>()
        .expect("build client");

    let fees = client.get_fee_estimates().await.expect("get fee estimates");
    assert_eq!(fees.len(), 2);
    assert_eq!(fees.get(&1).expect("target 1").to_sat_per_kwu(), 375);
    assert_eq!(fees.get(&2).expect("target 2").to_sat_per_kwu(), 625);
}

#[wasm_bindgen_test(async)]
async fn retries_retryable_response() {
    SLEEPS.store(0, Ordering::SeqCst);

    let url = format!("{}/flaky", base_url());
    let client = Builder::new(&url)
        .max_retries(1)
        .build_async_with_sleeper::<CountingSleeper>()
        .expect("build client");

    assert_eq!(
        client.get_height().await.expect("get height after retry"),
        101
    );
    assert_eq!(SLEEPS.load(Ordering::SeqCst), 1);
}

/// Minimum viable transaction; the fixture only checks the body is hex.
fn test_transaction() -> Transaction {
    Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint::null(),
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![TxOut {
            value: Amount::from_sat(1000),
            script_pubkey: ScriptBuf::new(),
        }],
    }
}

#[wasm_bindgen_test(async)]
async fn broadcasts_transaction() {
    let client = Builder::new(base_url())
        .build_async_with_sleeper::<NoopSleeper>()
        .expect("build client");

    let txid = client
        .broadcast(&test_transaction())
        .await
        .expect("broadcast");
    assert_eq!(
        txid.to_string(),
        "4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b"
    );
}

#[wasm_bindgen_test(async)]
async fn surfaces_non_retryable_response_without_retry() {
    SLEEPS.store(0, Ordering::SeqCst);

    let url = format!("{}/no-such-endpoint", base_url());
    let client = Builder::new(&url)
        .max_retries(2)
        .build_async_with_sleeper::<CountingSleeper>()
        .expect("build client");

    match client.get_height().await {
        Err(Error::HttpResponse {
            status: 404,
            message,
        }) => {
            assert_eq!(message, "not found");
        }
        other => panic!("expected 404 `Error::HttpResponse`, got {other:?}"),
    }
    assert_eq!(SLEEPS.load(Ordering::SeqCst), 0);
}

#[wasm_bindgen_test(async)]
async fn applies_timeout() {
    let url = format!("{}/slow", base_url());
    let client = Builder::new(&url)
        .timeout(Duration::from_millis(100))
        .build_async_with_sleeper::<NoopSleeper>()
        .expect("build client");

    let error = client
        .get_height()
        .await
        .expect_err("slow request should time out");
    assert!(
        error.to_string().to_lowercase().contains("timeout"),
        "{error}"
    );
}
