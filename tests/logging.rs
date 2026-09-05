// SPDX-License-Identifier: MIT OR Apache-2.0

//! # Tests for Structured Logging

#![cfg(all(feature = "log", any(feature = "blocking", feature = "async")))]

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::time::timeout;

use bitcoin::{absolute, transaction, Transaction, Txid};
use esplora_client::{Builder, Error};
use tracing::field::{Field, Visit};
use tracing::instrument::WithSubscriber;
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Level, Metadata, Subscriber};

#[derive(Debug)]
struct LoggedEvent {
    level: Level,
    fields: BTreeMap<String, String>,
}

impl Visit for LoggedEvent {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        self.fields
            .insert(field.name().into(), format!("{value:?}"));
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        self.fields.insert(field.name().into(), value.into());
    }
}

// Scoped subscribers keep tests independent even when the harness runs them in parallel.
#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<LoggedEvent>>>);

impl Subscriber for Capture {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        metadata.target().starts_with("esplora_client::")
    }

    fn new_span(&self, _: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }

    fn record(&self, _: &Id, _: &Record<'_>) {}
    fn record_follows_from(&self, _: &Id, _: &Id) {}
    fn enter(&self, _: &Id) {}
    fn exit(&self, _: &Id) {}

    fn event(&self, event: &Event<'_>) {
        let mut logged = LoggedEvent {
            level: *event.metadata().level(),
            fields: BTreeMap::new(),
        };
        event.record(&mut logged);
        self.0.lock().unwrap().push(logged);
    }
}

fn private_builder() -> Builder {
    Builder::new("http://url-user:url-secret@localhost/base-secret")
        .proxy("http://proxy-user:proxy-secret@localhost:9050")
        .header("Authorization", "header-secret")
        .timeout(Duration::from_secs(3))
        .max_retries(2)
}

fn assert_build(capture: &Capture, client: &str) {
    let events = capture.0.lock().unwrap();
    assert_eq!(events.len(), 1, "client construction must emit one event");
    let event = &events[0];
    assert_eq!(event.level, Level::DEBUG);
    assert_eq!(event.fields["event"], "client_build");
    assert_eq!(event.fields["client"], client);
    assert_eq!(event.fields["max_retries"], "2");
    assert_eq!(event.fields["timeout"], "Some(3s)");
    assert_eq!(event.fields["proxy_configured"], "true");
    assert_eq!(event.fields["header_count"], "1");
    let output = format!("{events:?}");
    for secret in [
        "url-user",
        "url-secret",
        "base-secret",
        "proxy-user",
        "proxy-secret",
        "header-secret",
    ] {
        assert!(!output.contains(secret), "leaked {secret}");
    }
}

#[cfg(feature = "blocking")]
#[test]
fn test_blocking_construction_logs_safe_configuration() {
    let capture = Capture::default();
    tracing::subscriber::with_default(capture.clone(), || {
        private_builder().build_blocking();
    });
    assert_build(&capture, "blocking");
}

#[cfg(feature = "async")]
struct TestSleeper;

#[cfg(feature = "async")]
impl esplora_client::Sleeper for TestSleeper {
    type Sleep = tokio::time::Sleep;

    fn sleep(duration: Duration) -> Self::Sleep {
        tokio::time::sleep(duration)
    }
}

#[cfg(feature = "async")]
#[test]
fn test_async_construction_logs_safe_configuration() {
    let capture = Capture::default();
    tracing::subscriber::with_default(capture.clone(), || {
        private_builder()
            .max_connections(4)
            .build_async_with_sleeper::<TestSleeper>()
            .unwrap();
    });
    assert_build(&capture, "async");
    assert_eq!(capture.0.lock().unwrap()[0].fields["max_connections"], "4");
}

#[derive(Clone, Copy)]
enum Mode {
    #[cfg(feature = "blocking")]
    Blocking,
    #[cfg(feature = "async")]
    Async,
}

fn modes() -> Vec<Mode> {
    vec![
        #[cfg(feature = "blocking")]
        Mode::Blocking,
        #[cfg(feature = "async")]
        Mode::Async,
    ]
}

impl Mode {
    fn name(self) -> &'static str {
        match self {
            #[cfg(feature = "blocking")]
            Self::Blocking => "blocking",
            #[cfg(feature = "async")]
            Self::Async => "async",
        }
    }

    fn build(self, url: &str, retries: usize) -> Client {
        let builder = Builder::new(url)
            .timeout(Duration::from_secs(3))
            .header("Authorization", "header-secret")
            .max_retries(retries);
        match self {
            #[cfg(feature = "blocking")]
            Self::Blocking => Client::Blocking(builder.build_blocking()),
            #[cfg(feature = "async")]
            Self::Async => Client::Async(builder.build_async_with_sleeper().unwrap()),
        }
    }
}

enum Client {
    #[cfg(feature = "blocking")]
    Blocking(esplora_client::BlockingClient),
    #[cfg(feature = "async")]
    Async(esplora_client::AsyncClient<TestSleeper>),
}

impl Client {
    async fn height(&self) -> Result<u32, Error> {
        match self {
            #[cfg(feature = "blocking")]
            Self::Blocking(client) => client.get_height(),
            #[cfg(feature = "async")]
            Self::Async(client) => client.get_height().await,
        }
    }

    async fn broadcast(&self, tx: &Transaction) -> Result<Txid, Error> {
        match self {
            #[cfg(feature = "blocking")]
            Self::Blocking(client) => client.broadcast(tx),
            #[cfg(feature = "async")]
            Self::Async(client) => client.broadcast(tx).await,
        }
    }
}

// Real HTTP responses exercise both transports. Bound the entire exchange so missing
// requests or incomplete headers/bodies fail instead of hanging the test process.
type RequestBodies = Vec<Vec<u8>>;

async fn server(responses: &[(u16, &str)]) -> (String, tokio::task::JoinHandle<RequestBodies>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let responses: Vec<_> = responses
        .iter()
        .map(|(code, body)| (*code, body.to_string()))
        .collect();
    let handle = tokio::spawn(async move {
        timeout(Duration::from_secs(5), async move {
            let mut requests = Vec::new();
            for (status, body) in responses {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut reader = BufReader::new(&mut stream);
                let mut request_line = String::new();
                reader.read_line(&mut request_line).await.unwrap();
                let mut content_length = 0;
                loop {
                    let mut line = String::new();
                    assert_ne!(reader.read_line(&mut line).await.unwrap(), 0);
                    if line == "\r\n" {
                        break;
                    }
                    if let Some((name, value)) = line.split_once(':') {
                        if name.eq_ignore_ascii_case("content-length") {
                            content_length = value.trim().parse().unwrap();
                        }
                    }
                }
                let mut request_body = vec![0; content_length];
                reader.read_exact(&mut request_body).await.unwrap();
                requests.push(request_body);
                let response = format!(
                    "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                stream.write_all(response.as_bytes()).await.unwrap();
            }
            requests
        }).await.expect("client did not complete expected HTTP exchanges")
    });
    (url, handle)
}

fn assert_lifecycle(
    capture: &Capture,
    mode: Mode,
    method: &str,
    path: &str,
    expected: &[(&str, usize, Option<u16>, Option<u64>)],
) {
    let captured = capture.0.lock().unwrap();
    let events: Vec<_> = captured
        .iter()
        .filter(|event| event.fields["event"] != "client_build")
        .collect();
    assert_eq!(events.len(), expected.len(), "{captured:?}");
    for (event, (kind, attempt, status, delay)) in events.iter().zip(expected) {
        assert_eq!(event.fields["event"], *kind);
        assert_eq!(event.fields["client"], mode.name());
        assert_eq!(event.fields["method"], method);
        assert_eq!(event.fields["path"], path);
        assert_eq!(event.fields["attempt"], attempt.to_string());
        assert_eq!(
            event.fields.get("status"),
            status.map(|value| value.to_string()).as_ref()
        );
        assert_eq!(
            event.fields.get("delay_ms"),
            delay.map(|value| value.to_string()).as_ref()
        );
        assert_eq!(
            event.level,
            if *kind == "retry" {
                Level::DEBUG
            } else {
                Level::TRACE
            }
        );
    }
    let output = format!("{captured:?}");
    assert!(!output.contains("header-secret"));
    assert!(!output.contains("response-secret"));
    assert!(!output.contains("base-path-secret"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_get_logs_request_response_and_retry() {
    for mode in modes() {
        let (url, server) = server(&[(503, "response-secret"), (200, "42")]).await;
        let capture = Capture::default();
        assert_eq!(
            async {
                mode.build(&format!("{url}/base-path-secret"), 1)
                    .height()
                    .await
            }
            .with_subscriber(capture.clone())
            .await
            .unwrap(),
            42
        );
        assert_eq!(server.await.unwrap().len(), 2);
        assert_lifecycle(
            &capture,
            mode,
            "GET",
            "/blocks/tip/height",
            &[
                ("request", 1, None, None),
                ("response", 1, Some(503), None),
                ("retry", 1, Some(503), Some(256)),
                ("request", 2, None, None),
                ("response", 2, Some(200), None),
            ],
        );
    }
}

fn transaction() -> Transaction {
    Transaction {
        version: transaction::Version::TWO,
        lock_time: absolute::LockTime::ZERO,
        input: vec![],
        output: vec![],
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_post_logs_metadata_without_bodies_or_retries() {
    let txid = "0000000000000000000000000000000000000000000000000000000000000000";
    let tx = transaction();
    for mode in modes() {
        for (status, body) in [(200, txid), (503, "response-secret")] {
            let (url, server) = server(&[(status, body)]).await;
            let capture = Capture::default();
            let result = async { mode.build(&url, 2).broadcast(&tx).await }
                .with_subscriber(capture.clone())
                .await;
            if status == 200 {
                assert_eq!(result.unwrap().to_string(), txid);
            } else {
                assert!(matches!(
                    result,
                    Err(Error::HttpResponse { status: 503, .. })
                ));
            }
            let requests = server.await.unwrap();
            assert_eq!(requests.len(), 1);
            let body = String::from_utf8(requests[0].clone()).unwrap();
            assert!(!body.is_empty());
            assert!(!format!("{:?}", capture.0.lock().unwrap()).contains(&body));
            assert_lifecycle(
                &capture,
                mode,
                "POST",
                "/tx",
                &[
                    ("request", 1, None, None),
                    ("response", 1, Some(status), None),
                ],
            );
        }
    }
}
