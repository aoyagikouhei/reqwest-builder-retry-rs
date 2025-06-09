use std::time::Duration;

use reqwest_builder_retry::RetryType;
use twapi_v2::{api::get_2_users_me, oauth10a::OAuthAuthentication};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let auth = OAuthAuthentication::new(
        std::env::var("CONSUMER_KEY").unwrap_or_default(),
        std::env::var("CONSUMER_SECRET").unwrap_or_default(),
        std::env::var("ACCESS_KEY").unwrap_or_default(),
        std::env::var("ACCESS_SECRET").unwrap_or_default(),
    );
    let result = reqwest_builder_retry::convenience::execute(
        |_| {
            let api = get_2_users_me::Api::all();
            api.build(&auth).timeout(Duration::from_secs(3))
        },
        |response| async move {
            let Ok(response) = response else {
                return Err(RetryType::Retry);
            };
            if response.status().is_success() {
                match response.json::<get_2_users_me::Response>().await {
                    Ok(user) => Ok(user),
                    Err(_) => Err(RetryType::Retry),
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
        },
        3,
        Duration::from_secs(2),
    )
    .await?;
    println!("Result: {:?}", result);
    Ok(())
}
