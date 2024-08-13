use crate::{client::Client, proto, Error, Result};
use tracing::{debug, trace};

/// Configuration to publish a message.
#[derive(Debug)]
pub struct PublishMessageBuilder {
    client: Client,
    name: Option<String>,
    correlation_key: Option<String>,
    time_to_live: Option<u64>,
    message_id: Option<String>,
    variables: Option<serde_json::Value>,
    tenant_id: Option<String>,
}

impl PublishMessageBuilder {
    /// Create a new publish message builder.
    pub fn new(client: Client) -> Self {
        PublishMessageBuilder {
            client,
            name: None,
            correlation_key: None,
            time_to_live: None,
            message_id: None,
            variables: None,
            tenant_id: None,
        }
    }

    /// Set the name of the message.
    pub fn with_name<T: Into<String>>(self, name: T) -> Self {
        PublishMessageBuilder {
            name: Some(name.into()),
            ..self
        }
    }

    /// Set the correlation key of the message.
    pub fn with_correlation_key<T: Into<String>>(self, correlation_key: T) -> Self {
        PublishMessageBuilder {
            correlation_key: Some(correlation_key.into()),
            ..self
        }
    }

    /// Set how long the message should be buffered on the broker, in milliseconds
    pub fn with_time_to_live(self, ttl: u64) -> Self {
        PublishMessageBuilder {
            time_to_live: Some(ttl),
            ..self
        }
    }

    /// Set the unique ID of the message; can be omitted. only useful to ensure only
    /// one message with the given ID will ever be published (during its lifetime)
    pub fn with_message_id<T: Into<String>>(self, message_id: T) -> Self {
        PublishMessageBuilder {
            message_id: Some(message_id.into()),
            ..self
        }
    }

    /// Set the JSON document representing the variables in the message.
    pub fn with_variables<T: Into<serde_json::Value>>(self, variables: T) -> Self {
        PublishMessageBuilder {
            variables: Some(variables.into()),
            ..self
        }
    }

    /// Set the tenant ID of the message.
    pub fn with_tenant_id<T: Into<String>>(self, tenant_id: T) -> Self {
        PublishMessageBuilder {
            tenant_id: Some(tenant_id.into()),
            ..self
        }
    }

    /// Submit the publish message request.
    #[tracing::instrument(skip(self), name = "publish_message", err)]
    pub async fn send(mut self) -> Result<PublishMessageResponse> {
        if self.name.is_none() {
            return Err(Error::InvalidParameters("`name` must be set"));
        }
        let req = proto::PublishMessageRequest {
            name: self.name.unwrap(),
            correlation_key: self.correlation_key.unwrap_or_default(),
            time_to_live: self.time_to_live.unwrap_or_default() as i64,
            message_id: self.message_id.unwrap_or_default(),
            variables: self
                .variables
                .map_or(String::new(), |vars| vars.to_string()),
            tenant_id: self.tenant_id.unwrap_or_default(),
        };

        debug!(name = ?req.name, "publishing message:");
        trace!(?req, "request:");
        let res = self
            .client
            .gateway_client
            .publish_message(tonic::Request::new(req))
            .await?;

        Ok(PublishMessageResponse(res.into_inner()))
    }
}

/// The publish message data.
#[derive(Debug)]
pub struct PublishMessageResponse(proto::PublishMessageResponse);

impl PublishMessageResponse {
    /// The unique ID of the message that was published
    pub fn key(&self) -> i64 {
        self.0.key
    }
}

/// Configuration to resolve an incident.
#[derive(Debug)]
pub struct ResolveIncidentBuilder {
    client: Client,
    incident_key: Option<i64>,
    operation_reference: Option<u64>,
}

impl ResolveIncidentBuilder {
    /// Create a new resolve incident builder.
    pub fn new(client: Client) -> Self {
        ResolveIncidentBuilder {
            client,
            incident_key: None,
            operation_reference: None,
        }
    }

    /// Set the unique ID of the incident to resolve.
    pub fn with_incident_key(self, incident_key: i64) -> Self {
        ResolveIncidentBuilder {
            incident_key: Some(incident_key),
            ..self
        }
    }

    /// Set a reference key chosen by the user and will be part of all records resulted from this operation
    pub fn with_operation_reference(self, operation_reference: u64) -> Self {
        ResolveIncidentBuilder {
            operation_reference: Some(operation_reference),
            ..self
        }
    }

    /// Submit the resolve incident request.
    #[tracing::instrument(skip(self), name = "resolve_incident", err)]
    pub async fn send(mut self) -> Result<ResolveIncidentResponse> {
        if self.incident_key.is_none() {
            return Err(Error::InvalidParameters("`incident_key` must be set"));
        }
        let req = proto::ResolveIncidentRequest {
            incident_key: self.incident_key.unwrap(),
            operation_reference: self.operation_reference,
        };

        debug!(?req, "sending request:");
        let res = self
            .client
            .gateway_client
            .resolve_incident(tonic::Request::new(req))
            .await?;

        Ok(ResolveIncidentResponse(res.into_inner()))
    }
}

/// The resolve incident data.
#[derive(Debug)]
pub struct ResolveIncidentResponse(proto::ResolveIncidentResponse);
