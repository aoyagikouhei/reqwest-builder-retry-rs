use crate::error::Error;
use reqwest::{RequestBuilder, Response};
use std::future::Future;
use std::time::Duration;

pub mod error;
pub use reqwest;

#[cfg(feature = "convenience")]
pub mod convenience;

pub enum RetryType {
    Stop,
    Retry,
}

pub async fn execute<T, F, G, H, I, FutG, FutI>(
    make_builder: F,
    check_done: G,
    try_count: u8,
    retry_duration: Duration,
    get_jitter: H,
    sleeper: I,
) -> Result<T, Error>
where
    F: Fn(u8) -> RequestBuilder,
    G: Fn(Result<Response, reqwest::Error>) -> FutG,
    H: Fn() -> Duration,
    I: Fn(Duration) -> FutI,
    FutG: Future<Output = Result<T, RetryType>> + Send + 'static,
    FutI: Future<Output = ()> + Send + 'static,
{
    // 指定回数実行する
    for i in 0..try_count {
        let builder = make_builder(i);
        let response = builder.send().await;
        let retry_duration = if i == try_count - 1 {
            calc_retry_duration(retry_duration, get_jitter(), i as u32)
        } else {
            // 最後の試行では、スリープしない
            Duration::ZERO
        };
        match check_done(response).await {
            Ok(result) => return Ok(result),
            Err(retry_type) => {
                match retry_type {
                    RetryType::Retry => {
                        // continue retrying
                    }
                    RetryType::Stop => {
                        return Err(Error::Stop);
                    }
                }
            }
        }
        if retry_duration > Duration::ZERO {
            sleeper(retry_duration).await;
        }
    }
    Err(Error::TryOver)
}

fn calc_retry_duration(
    retry_duration: Duration,
    jitter_duration: Duration,
    try_count: u32,
) -> Duration {
    // exponential backoff
    // 0の時1回、1の時2回、2の時4回、3の時8回
    let retry_count = 2u64.pow(try_count) as u32;
    retry_duration * retry_count + jitter_duration
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_builder_for_test(i: u8) -> RequestBuilder {
        reqwest::Client::new()
            .get("https://httpbin.org/get")
            .header("Try-Count", i.to_string())
    }

    #[tokio::test]
    async fn test_stop() {
        match execute(
            make_builder_for_test,
            |_| async move { Err::<serde_json::Value, RetryType>(RetryType::Stop) },
            3,
            Duration::from_secs(1),
            || Duration::from_millis(100),
            |_| async move {},
        )
        .await
        {
            Err(Error::Stop) => {}
            _ => {
                panic!("Test failed: Expected TryOver error.");
            }
        }
    }

    #[tokio::test]
    async fn test_over_try() {
        match execute(
            make_builder_for_test,
            |_| async move { Err::<serde_json::Value, RetryType>(RetryType::Retry) },
            1,
            Duration::from_secs(1),
            || Duration::from_millis(100),
            |_| async move {},
        )
        .await
        {
            Err(Error::TryOver) => {}
            _ => {
                panic!("Test failed: Expected TryOver error.");
            }
        }
    }

    #[tokio::test]
    async fn test_success() {
        let check_done = |response: Result<Response, _>| async move {
            let Ok(response) = response else {
                return Err(RetryType::Retry); // Retry on failure
            };
            if !response.status().is_success() {
                return Err(RetryType::Retry); // Retry on failure
            }
            let Ok(json) = response.json::<serde_json::Value>().await else {
                return Err(RetryType::Retry);
            };
            Ok(json)
        };

        match execute(
            make_builder_for_test,
            check_done,
            3,
            Duration::from_secs(1),
            || Duration::from_millis(100),
            |_| async move {},
        )
        .await
        {
            Ok(result) => {
                assert!(result.is_object());
            }
            Err(e) => {
                panic!("Test failed: {:?}", e);
            }
        }
    }
}
