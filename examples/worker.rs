use serde_json::json;
use tracing_subscriber::EnvFilter;
use zeebe::{Client, Job};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Create a zeebe client
    let client = Client::default();

    tracing::info!("Waiting for client to auth");

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    tracing::info!("Deploying process");

    // Deploy a process
    client
        .deploy_process()
        .with_resource_file("examples/workflows/order-process.bpmn")
        .send()
        .await?;

    tracing::info!("Creating process instance");

    // Create a new process instance
    client
        .create_process_instance()
        .with_bpmn_process_id("order-process")
        .with_latest_version()
        .with_variables(json!({"orderId": 31243}))
        .send()
        .await?;

    tracing::info!("Launching worker");

    // Process the instance with a worker
    client
        .job_worker()
        .with_job_type("payment-service")
        .with_handler(handle_job)
        .run()
        .await?;

    Ok(())
}

async fn handle_job(client: Client, job: Job) {
    tracing::info!("working on job!");

    // payment processing work...

    let _ = client.complete_job().with_job_key(job.key()).send().await;
}
