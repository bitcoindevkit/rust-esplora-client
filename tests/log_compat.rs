// SPDX-License-Identifier: MIT OR Apache-2.0

//! # Tests for `log` Compatibility
//!
//! A separate test executable isolates the process-global logger from tracing subscribers.
#![cfg(any(feature = "blocking", feature = "async"))]

use std::sync::Mutex;

struct Logger(Mutex<Vec<(log::Level, String)>>);

impl log::Log for Logger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.target().starts_with("esplora_client::")
    }

    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata()) {
            self.0
                .lock()
                .unwrap()
                .push((record.level(), record.args().to_string()));
        }
    }

    fn flush(&self) {}
}

static LOGGER: Logger = Logger(Mutex::new(Vec::new()));

#[cfg(feature = "async")]
struct Sleeper;

#[cfg(feature = "async")]
impl esplora_client::Sleeper for Sleeper {
    type Sleep = tokio::time::Sleep;

    fn sleep(duration: std::time::Duration) -> Self::Sleep {
        tokio::time::sleep(duration)
    }
}

#[tokio::test]
async fn test_logger_receives_events_only_when_feature_is_enabled() {
    log::set_logger(&LOGGER).unwrap();
    log::set_max_level(log::LevelFilter::Trace);
    // Fail deterministically during send, without making a network connection.
    let builder = esplora_client::Builder::new("http://invalid\0host")
        .header("Authorization", "header-secret");
    #[cfg(feature = "blocking")]
    {
        assert!(builder.clone().build_blocking().get_height().is_err());
        assert_records();
    }
    #[cfg(feature = "async")]
    {
        assert!(builder
            .build_async_with_sleeper::<Sleeper>()
            .unwrap()
            .get_height()
            .await
            .is_err());
        assert_records();
    }
}

fn assert_records() {
    let mut records = LOGGER.0.lock().unwrap();
    if cfg!(feature = "log") {
        assert_eq!(
            records.len(),
            2,
            "expected construction and request records: {records:?}"
        );
        assert_eq!(records[0].0, log::Level::Debug);
        assert!(records[0].1.contains("client_build"));
        assert_eq!(records[1].0, log::Level::Trace);
        assert!(records[1].1.contains("/blocks/tip/height"));
        for (_, message) in records.iter() {
            assert!(!message.contains("header-secret"));
            assert!(!message.contains("invalid"));
        }
    } else {
        assert!(
            records.is_empty(),
            "disabled logging emitted records: {records:?}"
        );
    }
    records.clear();
}
