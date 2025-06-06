use std::time::Duration;

use twapi_v2::{api::get_2_users_me, oauth10a::OAuthAuthentication};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let auth = OAuthAuthentication::new(
        std::env::var("CONSUMER_KEY").unwrap_or_default(),
        std::env::var("CONSUMER_SECRET").unwrap_or_default(),
        std::env::var("ACCESS_KEY").unwrap_or_default(),
        std::env::var("ACCESS_SECRET").unwrap_or_default(),
    );
    let result = reqwest_retry::execute(
        |_| {
            let api = get_2_users_me::Api::all();
            api.build(&auth)
        },
        |response| async move {
            let Ok(response) = response else {
                return Err(false); // Retry on failure
            };
            if response.status().is_success() {
                match response.json::<get_2_users_me::Response>().await {
                    Ok(user) => Ok(user),
                    Err(_) => Err(false), // Retry on failure
                }
            } else if response.status().is_client_error() {
                Err(true)
            } else {
                Err(false) // Retry on failure
            }
        },
        3,
        Duration::from_secs(2), // Retry duration
    )
    .await?;
    println!("Result: {:?}", result);
    Ok(())
}
