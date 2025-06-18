use std::time::Duration;

use tracing::Level;
use tracing_bunyan_formatter::{BunyanFormattingLayer, JsonStorageLayer};
use tracing_subscriber::{Registry, filter::Targets, layer::SubscriberExt};
use twapi_v2::{api::get_2_users_me, oauth10a::OAuthAuthentication, reqwest::StatusCode};

// Tracingの準備
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

    // スレッドで利用可能化チェック
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
                    // レスポンスのログ
                    tracing::trace!(?response, "api response");
                    // レスポンスのチェック
                    reqwest_builder_retry::convenience::json::check_done::<get_2_users_me::Response>(
                        response,
                        &[StatusCode::TOO_MANY_REQUESTS, StatusCode::FORBIDDEN],
                    )
                },
                3,                      // トライ回数
                Duration::from_secs(2), // リトライ間隔
            )
            .await;
            println!("Result: {:?}", result);
        }
    });

    handle.await?;

    Ok(())
}
