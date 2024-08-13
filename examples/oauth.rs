//! Zeebe OAuth example
//!
//! In order to run the example call:
//!
//! ```sh
//! export ZEEBE_ADDRESS='[Zeebe API]'
//! export ZEEBE_CLIENT_ID='[Client ID]'
//! export ZEEBE_CLIENT_SECRET='[Client Secret]'
//! export ZEEBE_AUTHORIZATION_SERVER_URL='[OAuth API]'
//!
//! cargo run --example oauth
//! ```
use zeebe::Client;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter("zeebe=trace")
        .init();

    let client = Client::from_env()?;
    client.auth_initialized().await?;
    let topology = client.topology().send().await?;

    dbg!(topology);

    Ok(())
}
