use std::{future::Future, time::Duration};

#[cfg(feature = "async")]
use async_std::task::sleep;
#[cfg(feature = "async")]
use reqwest::{RequestBuilder, Response as ReqwestResponse, StatusCode};

#[cfg(feature = "blocking")]
use ureq::{Error as UReqError, Request, Response as UReqResponse};

const SECOND: u64 = 1_000;
const BASE_BACKOFF_MS: u64 = 5_000;
const DEFAULT_MAX_RETRIES: u32 = 5;

pub const RETRY_TOO_MANY_REQUEST_ONLY: [StatusCode; 1] = [StatusCode::TOO_MANY_REQUESTS];

pub trait AsyncRetryable<R, E = crate::Error> {
    fn exec_with_retry(
        self,
        retryable_error_codes: Vec<StatusCode>,
        max_retries: Option<u32>,
    ) -> impl Future<Output = Result<R, E>>;
}

pub trait SyncRetryable<R, E = crate::Error> {
    fn exec_with_retry(
        self,
        retryable_error_codes: Vec<StatusCode>,
        max_retries: Option<u32>,
    ) -> Result<R, E>;
}

fn compute_backoff(retry_after: Option<&str>, backoff: Duration) -> Duration {
    match retry_after {
        // If response has a Retry-After header, parse it and use it as backoff
        Some(retry_after) => {
            return Duration::from_millis(
                retry_after
                    .parse::<u64>()
                    .map(|seconds| seconds * SECOND)
                    .unwrap_or(BASE_BACKOFF_MS),
            )
        }
        // Else, double previous backoff
        _ => return backoff * 2,
    }
}

#[cfg(feature = "async")]
impl AsyncRetryable<ReqwestResponse> for RequestBuilder {
    async fn exec_with_retry(
        self,
        retryable_error_codes: Vec<StatusCode>,
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
                    if retryable_error_codes.contains(&response.status())
                        && retry_count < max_retries =>
                {
                    backoff = compute_backoff(
                        response
                            .headers()
                            .get("Retry-After")
                            .map(|retry| retry.to_str().unwrap_or("0")),
                        backoff,
                    );

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
        retryable_error_codes: Vec<StatusCode>,
        max_retries: Option<u32>,
    ) -> Result<UReqResponse, UReqError> {
        let mut backoff = Duration::from_millis(BASE_BACKOFF_MS);
        let mut retry_count = 0;

        let max_retries = match max_retries {
            Some(max_retries) => max_retries,
            None => DEFAULT_MAX_RETRIES,
        };

        let retryable_error_codes = retryable_error_codes
            .into_iter()
            .map(|code| code.as_u16())
            .collect::<Vec<_>>();

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
