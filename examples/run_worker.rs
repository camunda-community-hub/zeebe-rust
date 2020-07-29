use std::collections::HashMap;
use zeebe::{Client, Job};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter("zeebe=trace")
        .init();

    // Create a zeebe client
    let mut client = Client::default();

    // Deploy a workflow
    client
        .deploy_workflow("examples/workflows/order-process.bpmn")
        .send()
        .await?;

    // Create a new workflow instance
    let mut variables = HashMap::new();
    variables.insert("orderId", "31243");
    client
        .create_workflow_instance()
        .with_bpmn_process_id("order-process")
        .with_latest_version()
        .with_variables(serde_json::to_value(variables)?)
        .send()
        .await?;

    // Process the workflow instance with a worker
    client
        .job_worker()
        .with_job_type("payment-service")
        .with_handler(handle_job)
        .spawn()
        .await?;

    Ok(())
}

async fn handle_job(_client: Client, _job: Job) -> zeebe::Result<()> {
    tracing::info!("working on job!");
    Ok(())
}
