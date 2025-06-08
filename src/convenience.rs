use std::time::Duration;

use rand::{Rng, SeedableRng, rngs::StdRng};
use reqwest::{RequestBuilder, Response};

use crate::{RetryType, error::Error};

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
    G: Fn(Result<Response, reqwest::Error>) -> Fut,
    Fut: Future<Output = Result<T, RetryType>> + Send + 'static,
{
    crate::execute(
        make_builder,
        check_done,
        try_count,
        retry_duration,
        default_jitter,
        default_sleeper,
    )
    .await
}
