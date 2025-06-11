use std::time::Duration;

use reqwest_builder_retry::{
    reqwest::{Error, Response}, RetryResult, RetryType
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

async fn check_done<T>(response: Result<Response, Error>) -> Result<RetryResult<T>, RetryType>
where
    T: serde::de::DeserializeOwned,
{
    tracing::trace!(?response, "api response");
    match response {
        Ok(response) => {
            if response.status().is_success() {
                let status_code = response.status();
                let headers = response.headers().clone();
                match response.json::<T>().await {
                    Ok(result) => Ok(RetryResult{result, status_code, headers}),
                    Err(_err)  =>  Err(RetryType::Retry)
                }
            } else if response.status().is_client_error() {
                // Xは403の時はリトライで回復することがある
                if response.status().as_u16() == 403 {
                    Err(RetryType::Retry)
                } else {
                    Err(RetryType::Stop)
                }
            } else {
                Err(RetryType::Retry)
            }
        }
        Err(_) => Err(RetryType::Retry),
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
    let result = reqwest_builder_retry::convenience::execute(
        |_| {
            let api = get_2_users_me::Api::open();
            // APIの実行には必ずタイムアウトをつけましょう
            let builder = api.build(&auth).timeout(Duration::from_secs(3));
            // リクエストのログ
            tracing::trace!(?builder, "api request");
            builder
        },
        check_done::<get_2_users_me::Response>,
        3,
        Duration::from_secs(2),
    )
    .await?;
    println!("Result: {:?}", result);
    Ok(())
}
