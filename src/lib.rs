use std::time::Duration;
use rand::{rngs::StdRng, Rng, SeedableRng};
use reqwest::{RequestBuilder, Response};
use std::future::Future;
use crate::error::Error;

pub mod error;
pub use reqwest;

pub async fn execute<T, F, G, Fut>(
    make_builder: F,
    check_done: G,
    try_count: u8,
    retry_duration: Duration,
) -> Result<T, Error>
where
    F: Fn(u8) -> RequestBuilder,
    G: Fn(Response, Duration) -> Fut,
    Fut: Future<Output = Result<T, bool>> + Send + 'static,
{
    // ランダム初期化
    let mut rng = StdRng::from_os_rng();

    // 指定回数実行する
    for i in 0..try_count {
        let builder = make_builder(i);
        let response = builder.send().await?;
        let retry_duration = if i == try_count - 1 {
            calc_retry_duration(retry_duration, i as u32, &mut rng)
        } else {
            // 最後の試行では、スリープしない
            Duration::ZERO
        };
        match check_done(response, retry_duration).await {
            Ok(result) => return Ok(result),
            Err(stop_flag) => {
                if stop_flag {
                    return Err(Error::Stop);
                }
            }
        }
    }
    Err(Error::TryOver)
}

fn calc_retry_duration(retry_duration: Duration, try_count: u32, rng: &mut StdRng) -> Duration {
    // Jistter
    let jitter = Duration::from_millis(rng.random_range(0..100));

    // exponential backoff
    // 0の時1回、1の時2回、2の時4回、3の時8回
    let retry_count = 2u64.pow(try_count) as u32;
    retry_duration * retry_count + jitter
}
