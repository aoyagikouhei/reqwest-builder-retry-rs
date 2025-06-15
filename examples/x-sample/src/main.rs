use std::time::Duration;

use reqwest_builder_retry::{
    RetryType,
    convenience::check_status_code,
    reqwest::{Error, Response, StatusCode},
};
use tracing::Level;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::{Registry, filter::Targets, layer::SubscriberExt};
use twapi_v2::{api::get_2_users_me, oauth10a::OAuthAuthentication};

pub fn setup_tracing(name: &str) {
    let formatting_layer = BunyanFormattingLayer::new(name.into(), std::io::stdout);
    let filter = Targets::new()
        .with_target(name, Level::TRACE)
        .with_target("twapi_v2", Level::TRACE);

    let subscriber = Registry::default()
        .with(filter)
        .with(JsonStorageLayer)
        .with(formatting_layer);
    tracing::subscriber::set_global_default(subscriber).unwrap();
}
#[derive(Debug)]
pub struct ResponseError {
    pub status: StatusCode,
    pub message: String,
}

async fn check_done<T>(
    response: Result<Response, Error>,
    retryable_status_codes: &[StatusCode],
) -> Result<T, (RetryType, ResponseError)>
where
    T: serde::de::DeserializeOwned,
{
    let response = response.map_err(|err| {
        (
            RetryType::Retry,
            ResponseError {
                status: StatusCode::IM_A_TEAPOT, // エラーだとステータスコードが無いので適当に設定
                message: err.to_string(),
            },
        )
    })?;

    let status_code = response.status();
    if let Some(retry_type) = check_status_code(status_code, retryable_status_codes).await {
        return Err((
            retry_type,
            ResponseError {
                status: status_code,
                message: "Non-success status code".to_string(),
            },
        ));
    }

    let text = response.text().await.unwrap_or_else(|_| "".to_string());
    match serde_json::from_str::<T>(&text) {
        Ok(result) => Ok(result),
        Err(err) => Err((
            RetryType::Retry,
            ResponseError {
                status: status_code,
                message: err.to_string(),
            },
        )),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    setup_tracing("x_sample");
    tracing::trace!("start");

    let auth = OAuthAuthentication::new(
        std::env::var("CONSUMER_KEY").unwrap_or_default(),
        std::env::var("CONSUMER_SECRET").unwrap_or_default(),
        std::env::var("ACCESS_KEY").unwrap_or_default(),
        std::env::var("ACCESS_SECRET").unwrap_or_default(),
    );

    let handle = tokio::spawn({
        async move {
            let result = reqwest_builder_retry::convenience::execute(
                |_| {
                    let api = get_2_users_me::Api::open();
                    // APIの実行には必ずタイムアウトをつけましょう
                    let builder = api.build(&auth).timeout(Duration::from_secs(3));
                    // リクエストのログ
                    tracing::trace!(?builder, "api request");
                    builder
                },
                |response| {
                    check_done::<get_2_users_me::Response>(
                        response,
                        &[StatusCode::TOO_MANY_REQUESTS, StatusCode::FORBIDDEN],
                    )
                },
                3,
                Duration::from_secs(2),
            )
            .await;
            println!("Result: {:?}", result);
        }
    });

    handle.await?;

    Ok(())
}
