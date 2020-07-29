use zeebe::{Client, ClientConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let broker_addr = "http://0.0.0.0:26500".to_string();
    let mut client = Client::from_config(ClientConfig::with_endpoints(vec![broker_addr]))?;
    let topology = client.topology().send().await?;

    dbg!(topology);

    Ok(())
}
