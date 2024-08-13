use crate::{client::Client, proto, Result};
use tracing::debug;

/// Request to obtain the current topology of the cluster the gateway is part of.
#[derive(Debug)]
pub struct TopologyBuilder(Client);

impl TopologyBuilder {
    /// Create a new topology request builder.
    pub fn new(client: Client) -> Self {
        TopologyBuilder(client)
    }

    /// Send a topology request to the configured gateway.
    #[tracing::instrument(skip(self), name = "topology", err)]
    pub async fn send(mut self) -> Result<TopologyResponse> {
        let req = proto::TopologyRequest {};
        debug!(?req, "sending request");

        let res = self
            .0
            .gateway_client
            .topology(tonic::Request::new(req))
            .await?;
        Ok(TopologyResponse(res.into_inner()))
    }
}

/// The current topology of the cluster
#[derive(Debug)]
pub struct TopologyResponse(proto::TopologyResponse);

impl TopologyResponse {
    /// List of brokers part of this cluster
    pub fn brokers(&self) -> Vec<BrokerInfo> {
        self.0
            .brokers
            .iter()
            .map(|proto| BrokerInfo(proto.clone()))
            .collect()
    }

    /// How many nodes are in the cluster.
    pub fn cluster_size(&self) -> u32 {
        self.0.cluster_size as u32
    }

    /// How many partitions are spread across the cluster.
    pub fn partitions_count(&self) -> u32 {
        self.0.partitions_count as u32
    }

    /// Configured replication factor for this cluster.
    pub fn replication_factor(&self) -> u32 {
        self.0.replication_factor as u32
    }

    /// gateway version
    pub fn gateway_version(&self) -> &str {
        &self.0.gateway_version
    }
}

/// Zeebe broker info
#[derive(Debug)]
pub struct BrokerInfo(proto::BrokerInfo);

impl BrokerInfo {
    /// Unique (within a cluster) node ID for the broker.
    pub fn node_id(&self) -> i32 {
        self.0.node_id
    }

    /// Hostname of the broker.
    pub fn host(&self) -> &str {
        &self.0.host
    }

    /// Port for the broker.
    pub fn port(&self) -> u32 {
        self.0.port as u32
    }

    /// List of partitions managed or replicated on this broker.
    pub fn partitions(&self) -> Vec<Partition> {
        self.0
            .partitions
            .iter()
            .map(|proto| Partition(*proto))
            .collect()
    }

    /// Broker version.
    pub fn version(&self) -> &str {
        &self.0.version
    }
}

/// Zeebe partition.
#[derive(Debug)]
pub struct Partition(proto::Partition);

impl Partition {
    /// the unique ID of this partition
    pub fn partition_id(&self) -> i32 {
        self.0.partition_id
    }

    /// The role of the broker for this partition.
    pub fn role(&self) -> i32 {
        self.0.role
    }

    /// The health of this partition
    pub fn health(&self) -> i32 {
        self.0.health
    }
}
