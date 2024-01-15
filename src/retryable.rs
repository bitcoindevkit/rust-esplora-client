use std::{
    future::Future,
    time::{Duration, SystemTime},
};

#[cfg(test)]
use std::time::UNIX_EPOCH;

#[cfg(feature = "async")]
use async_std::task::sleep;
#[cfg(feature = "async")]
use reqwest::{RequestBuilder, Response as ReqwestResponse};

#[cfg(feature = "blocking")]
use ureq::{Error as UReqError, Request, Response as UReqResponse};

const SECOND: u64 = 1_000;
const BASE_BACKOFF_MS: u64 = 5_000;
const DEFAULT_MAX_RETRIES: u32 = 5;

pub const RETRY_TOO_MANY_REQUEST_ONLY: [u16; 2] = [
    429, // TOO_MANY_REQUESTS
    503, // SERVICE_UNAVAILABLE,
];

pub trait AsyncRetryable<R, E = crate::Error> {
    fn exec_with_retry(
        self,
        retryable_error_codes: Vec<u16>,
        max_retries: Option<u32>,
    ) -> impl Future<Output = Result<R, E>>;
}

pub trait SyncRetryable<R, E = crate::Error> {
    fn exec_with_retry(
        self,
        retryable_error_codes: Vec<u16>,
        max_retries: Option<u32>,
    ) -> Result<R, E>;
}

fn now() -> SystemTime {
    #[cfg(target_arch = "wasm32")]
    let current_time = instant::SystemTime::now();
    #[cfg(not(target_arch = "wasm32"))]
    let current_time = SystemTime::now();
    #[cfg(test)] // Mocks date to `Wed, 21 Oct 2015 07:28:00 GMT`
    let current_time = SystemTime::from(UNIX_EPOCH + Duration::from_secs(1445412480u64));

    current_time
}

pub(crate) fn compute_backoff(retry_after: Option<&str>, backoff: Duration) -> Duration {
    // If response has a Retry-After header, we parse it and use it as backoff
    let duration = retry_after.and_then(|retry_after| {
        if let Ok(date) = retry_after.parse::<httpdate::HttpDate>() {
            let retry_after_date = SystemTime::from(date);
            let current = now();

            if retry_after_date <= current {
                Some(Duration::from_secs(0))
            } else {
                retry_after_date.duration_since(now()).ok()
            }
        } else {
            retry_after
                .parse::<u64>()
                .map(|seconds| Duration::from_millis(seconds * SECOND))
                .ok()
        }
    });

    match duration {
        Some(duration) => duration,
        // If duration is None here, we might have no Retry-After or an invalid one. In any case, we fallback to doubling previous one
        None => backoff * 2,
    }
}

#[cfg(feature = "async")]
impl AsyncRetryable<ReqwestResponse> for RequestBuilder {
    async fn exec_with_retry(
        self,
        retryable_error_codes: Vec<u16>,
        max_retries: Option<u32>,
    ) -> Result<ReqwestResponse, crate::Error> {
        let mut backoff = Duration::from_millis(BASE_BACKOFF_MS);
        let mut retry_count = 0;

        let max_retries = match max_retries {
            Some(max_retries) => max_retries,
            None => DEFAULT_MAX_RETRIES,
        };

        loop {
            match self.try_clone().unwrap().send().await {
                Err(err) => return Err(crate::Error::Reqwest(err)),
                Ok(response)
                    if retryable_error_codes.contains(&response.status().as_u16())
                        && retry_count < max_retries =>
                {
                    let header_value = response
                        .headers()
                        .get("Retry-After")
                        .and_then(|retry| retry.to_str().ok());

                    backoff = compute_backoff(header_value, backoff);
                    retry_count += 1;

                    sleep(backoff).await;
                }
                Ok(response) => {
                    return response
                        .error_for_status()
                        .map_err(|error| crate::Error::Reqwest(error));
                }
            }
        }
    }
}

#[cfg(feature = "blocking")]
impl SyncRetryable<UReqResponse, UReqError> for Request {
    fn exec_with_retry(
        self,
        retryable_error_codes: Vec<u16>,
        max_retries: Option<u32>,
    ) -> Result<UReqResponse, UReqError> {
        let mut backoff = Duration::from_millis(BASE_BACKOFF_MS);
        let mut retry_count = 0;

        let max_retries = match max_retries {
            Some(max_retries) => max_retries,
            None => DEFAULT_MAX_RETRIES,
        };

        loop {
            match self.clone().call() {
                Err(ureq::Error::Status(code, resp)) => {
                    if retryable_error_codes.contains(&code) && retry_count < max_retries {
                        backoff = compute_backoff(resp.header("Retry-After"), backoff);
                        retry_count += 1;

                        std::thread::sleep(backoff);
                    }
                }
                Err(err) => return Err(err),
                Ok(response) => return Ok(response),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_double_backoff_when_no_retry_after() {
        let backoff = compute_backoff(None, Duration::from_secs(1));
        assert_eq!(backoff.as_secs(), 2);
    }

    #[test]
    fn should_parse_retry_after_date() {
        let backoff = compute_backoff(
            Some("Wed, 21 Oct 2015 09:28:00 GMT"),
            Duration::from_secs(1),
        );
        assert_eq!(backoff.as_secs(), 7200);
    }

    #[test]
    fn should_return_no_backoff_when_retry_after_is_past() {
        let backoff = compute_backoff(
            Some("Wed, 21 Oct 2015 06:28:00 GMT"),
            Duration::from_secs(1),
        );
        assert_eq!(backoff.as_secs(), 0);
    }

    #[test]
    fn should_parse_retry_after_timestamp() {
        let backoff = compute_backoff(Some("3600"), Duration::from_secs(1));
        assert_eq!(backoff.as_secs(), 3600);
    }

    #[test]
    fn should_double_backoff_when_retry_after_is_not_parseable() {
        let backoff = compute_backoff(Some("3600!"), Duration::from_secs(1));
        assert_eq!(backoff.as_secs(), 2);
    }
}
