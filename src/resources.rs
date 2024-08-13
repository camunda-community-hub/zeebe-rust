use crate::{proto, Client, Result};
use tracing::debug;

/// Deploys resource to Zeebe.
#[derive(Debug)]
pub struct DeployResourceBuilder {
    client: Client,
    resource_name: Option<String>,
    content: Vec<u8>,
    tenant_id: Option<String>,
}

impl DeployResourceBuilder {
    /// Create a new deploy resource builder.
    pub fn new(client: Client) -> Self {
        DeployResourceBuilder {
            client,
            resource_name: None,
            content: Vec::new(),
            tenant_id: None,
        }
    }

    /// Set tenant_id for the deployment.
    pub fn with_tenant_id<T: Into<String>>(self, tenant_id: T) -> Self {
        DeployResourceBuilder {
            tenant_id: Some(tenant_id.into()),
            ..self
        }
    }

    /// Set the name of the resource.
    pub fn with_resource_name<T: Into<String>>(self, resource_name: T) -> Self {
        DeployResourceBuilder {
            resource_name: Some(resource_name.into()),
            ..self
        }
    }

    /// Set the content of the resource.
    pub fn with_content<T: Into<Vec<u8>>>(self, content: T) -> Self {
        DeployResourceBuilder {
            content: content.into(),
            ..self
        }
    }

    /// Submit the deployment to the Zeebe brokers.
    #[tracing::instrument(skip(self), name = "deploy_resource", err)]
    pub async fn send(mut self) -> Result<proto::DeployResourceResponse> {
        debug!(tenant_id = ?self.tenant_id, resource_name = ?self.resource_name, "sending request");

        let res = self
            .client
            .gateway_client
            .deploy_resource(tonic::Request::new(proto::DeployResourceRequest {
                tenant_id: self.tenant_id.unwrap_or_default(),
                resources: vec![proto::Resource {
                    name: self.resource_name.unwrap_or_default(),
                    content: self.content,
                }],
            }))
            .await?
            .into_inner();
        Ok(res)
    }
}

/// Deletes resource from Zeebe.
#[derive(Debug)]
pub struct DeleteResourceBuilder {
    client: Client,
    resource_key: Option<i64>,
    operation_reference: Option<u64>,
}

impl DeleteResourceBuilder {
    /// Create a new delete resource builder.
    pub fn new(client: Client) -> Self {
        DeleteResourceBuilder {
            client,
            resource_key: None,
            operation_reference: None,
        }
    }

    /// Set the resource key to delete.
    pub fn with_resource_key(self, resource_key: i64) -> Self {
        DeleteResourceBuilder {
            resource_key: Some(resource_key),
            ..self
        }
    }

    /// Set the operation reference.
    pub fn with_operation_reference(self, operation_reference: u64) -> Self {
        DeleteResourceBuilder {
            operation_reference: Some(operation_reference),
            ..self
        }
    }

    /// Submit the delete request to the Zeebe brokers.
    #[tracing::instrument(skip(self), name = "delete_resource", err)]
    pub async fn send(mut self) -> Result<proto::DeleteResourceResponse> {
        let res = self
            .client
            .gateway_client
            .delete_resource(tonic::Request::new(proto::DeleteResourceRequest {
                resource_key: self.resource_key.unwrap_or_default(),
                operation_reference: self.operation_reference,
            }))
            .await?
            .into_inner();
        Ok(res)
    }
}
