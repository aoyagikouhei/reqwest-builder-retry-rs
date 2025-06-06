use crate::error::Error;
use rand::{Rng, SeedableRng, rngs::StdRng};
use reqwest::{RequestBuilder, Response};
use std::future::Future;
use std::time::Duration;

pub mod error;
pub use reqwest;

async fn default_sleeper(duration: Duration) {
    tokio::time::sleep(duration).await;
}

fn default_jitter() -> Duration {
    let mut rng = StdRng::from_os_rng();
    Duration::from_millis(rng.random_range(0..100))
}

pub async fn execute<T, F, G, Fut>(
    make_builder: F,
    check_done: G,
    try_count: u8,
    retry_duration: Duration,
) -> Result<T, Error>
where
    F: Fn(u8) -> RequestBuilder,
    G: Fn(Response) -> Fut,
    Fut: Future<Output = Result<T, bool>> + Send + 'static,
{
    execute_raw(
        make_builder,
        check_done,
        try_count,
        retry_duration,
        default_jitter,
        default_sleeper,
    )
    .await
}

pub async fn execute_raw<T, F, G, H, I, FutG, FutI>(
    make_builder: F,
    check_done: G,
    try_count: u8,
    retry_duration: Duration,
    get_jitter: H,
    sleeper: I,
) -> Result<T, Error>
where
    F: Fn(u8) -> RequestBuilder,
    G: Fn(Response) -> FutG,
    H: Fn() -> Duration,
    I: Fn(Duration) -> FutI,
    FutG: Future<Output = Result<T, bool>> + Send + 'static,
    FutI: Future<Output = ()> + Send + 'static,
{
    // 指定回数実行する
    for i in 0..try_count {
        let builder = make_builder(i);
        let response = builder.send().await?;
        let retry_duration = if i == try_count - 1 {
            calc_retry_duration(retry_duration, get_jitter(), i as u32)
        } else {
            // 最後の試行では、スリープしない
            Duration::ZERO
        };
        match check_done(response).await {
            Ok(result) => return Ok(result),
            Err(stop_flag) => {
                if stop_flag {
                    return Err(Error::Stop);
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
    use reqwest::Client;

    fn get_try_count(json: &serde_json::Value) -> u8 {
        json["headers"]["Try-Count"]
            .as_str()
            .and_then(|s| s.parse::<u8>().ok())
            .unwrap_or(0)
    }

    fn get_jitter() -> Duration {
        // Jitter duration can be a fixed value or a random value.
        // Here we use a fixed value for simplicity.
        Duration::from_millis(100)
    }

    async fn sleeper(_duration: Duration) {
        //tokio::time::sleep(duration).await;
    }

    #[tokio::test]
    async fn test_stop() {
        let client = Client::new();
        let make_builder = |i: u8| {
            client
                .get("https://httpbin.org/get")
                .header("Try-Count", i.to_string())
        };
        let check_done = |response: Response| async move {
            if response.status().is_success() {
                let json = match response.json::<serde_json::Value>().await {
                    Ok(json) => json,
                    Err(_) => return Err(false),
                };
                let try_count = get_try_count(&json);
                if try_count == 0 { Err(true) } else { Ok(json) }
            } else {
                Err(false)
            }
        };

        match execute_raw(
            make_builder,
            check_done,
            3,
            Duration::from_secs(1),
            get_jitter,
            sleeper,
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
        let client = Client::new();
        let make_builder = |i: u8| {
            client
                .get("https://httpbin.org/get")
                .header("Try-Count", i.to_string())
        };
        let check_done = |response: Response| async move {
            if response.status().is_success() {
                let json = match response.json::<serde_json::Value>().await {
                    Ok(json) => json,
                    Err(_) => return Err(false),
                };
                let try_count = get_try_count(&json);
                if try_count < 4 {
                    Err(false) // continue retrying
                } else {
                    Ok(json)
                }
            } else {
                Err(false)
            }
        };

        match execute_raw(
            make_builder,
            check_done,
            3,
            Duration::from_secs(1),
            get_jitter,
            sleeper,
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
        let client = Client::new();
        let make_builder = |i: u8| {
            client
                .get("https://httpbin.org/get")
                .header("Try-Count", i.to_string())
        };
        let check_done = |response: Response| async move {
            if response.status().is_success() {
                match response.json::<serde_json::Value>().await {
                    Ok(json) => Ok(json),
                    Err(_) => return Err(false),
                }
            } else {
                Err(false)
            }
        };

        match execute_raw(
            make_builder,
            check_done,
            3,
            Duration::from_secs(1),
            get_jitter,
            sleeper,
        )
        .await
        {
            Ok(result) => {
                assert!(result.is_object()); // 簡単なチェック
            }
            Err(e) => {
                panic!("Test failed: {:?}", e);
            }
        }
    }
}
