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
    RetryAfter(Duration),
}

pub async fn execute<
    SuccessResponse,
    ErrorResponse,
    MakerBuilder,
    CheckDone,
    JITTER,
    SLEEPER,
    FutCheckDone,
    FutSLEEPER,
>(
    make_builder: MakerBuilder,
    check_done: CheckDone,
    try_count: u8,
    retry_duration: Duration,
    jitter: JITTER,
    sleeper: SLEEPER,
) -> Result<SuccessResponse, Error<ErrorResponse>>
where
    MakerBuilder: Fn(u8) -> RequestBuilder,
    CheckDone: Fn(Result<Response, reqwest::Error>) -> FutCheckDone,
    JITTER: Fn() -> Duration,
    SLEEPER: Fn(Duration) -> FutSLEEPER,
    FutCheckDone: Future<Output = Result<SuccessResponse, (RetryType, ErrorResponse)>>
        + Send
        + Sync
        + 'static,
    FutSLEEPER: Future<Output = ()> + Send + Sync + 'static,
{
    // リトライ回数が0の時はエラーを返す
    if try_count == 0 {
        return Err(Error::NoTry);
    }

    // 指定回数実行する
    for i in 0..try_count {
        let builder = make_builder(i);
        let response = builder.send().await;
        let (next_retry_duration, error_response) = match check_done(response).await {
            Ok(result) => return Ok(result),
            Err((retry_type, err)) => {
                match retry_type {
                    RetryType::Retry => {
                        // リトライなので停止時間を計算する
                        (calc_retry_duration(retry_duration, jitter(), i as u32), err)
                    }
                    RetryType::RetryAfter(target_duration) => {
                        // リトライ後の待機時間を指定された時間に設定する
                        (target_duration, err)
                    }
                    RetryType::Stop => {
                        // 停止指示が来たのでエラーを返す
                        return Err(Error::Stop(err));
                    }
                }
            }
        };

        if i >= try_count - 1 {
            // 最後の実行であれば、エラーを返す
            return Err(Error::TryOver(error_response));
        }

        // リトライ時間が0以上なら待機する
        if next_retry_duration > Duration::ZERO {
            sleeper(next_retry_duration).await;
        }
    }

    // ここに到達することはないが、コンパイラの警告を避けるためにエラーを返す
    Err(Error::NoTry)
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
            |_| async move { Err::<serde_json::Value, (RetryType, ())>((RetryType::Stop, ())) },
            3,
            Duration::from_secs(1),
            || Duration::from_millis(100),
            |_| async move {},
        )
        .await
        {
            Err(Error::Stop(_)) => {}
            _ => {
                panic!("Test failed: Expected TryOver error.");
            }
        }
    }

    #[tokio::test]
    async fn test_over_try() {
        match execute(
            make_builder_for_test,
            |_| async move { Err::<serde_json::Value, (RetryType, ())>((RetryType::Retry, ())) },
            4,
            Duration::from_secs(2),
            || Duration::from_millis(100),
            |duration| async move { println!("Sleeping for {:?}", duration) },
        )
        .await
        {
            Err(Error::TryOver(_)) => {}
            _ => {
                panic!("Test failed: Expected TryOver error.");
            }
        }
    }

    #[tokio::test]
    async fn test_success() {
        let check_done = |response: Result<Response, _>| async move {
            let Ok(response) = response else {
                return Err((RetryType::Retry, ())); // Retry on failure
            };
            if !response.status().is_success() {
                return Err((RetryType::Retry, ())); // Retry on failure
            }
            let Ok(json) = response.json::<serde_json::Value>().await else {
                return Err((RetryType::Retry, ()));
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
            Ok(res) => {
                assert_eq!(res.is_object(), true);
            }
            Err(e) => {
                panic!("Test failed: {:?}", e);
            }
        }
    }
}
