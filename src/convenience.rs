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

pub async fn execute<SuccessResponse, ErrorResponse, MakerBuilder, CheckDone, FutCheckDone>(
    make_builder: MakerBuilder,
    check_done: CheckDone,
    try_count: u8,
    retry_duration: Duration,
) -> Result<SuccessResponse, Error<ErrorResponse>>
where
    MakerBuilder: Fn(u8) -> RequestBuilder,
    CheckDone: Fn(Result<Response, reqwest::Error>) -> FutCheckDone,
    FutCheckDone: Future<Output = Result<SuccessResponse, (RetryType, ErrorResponse)>>,
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
