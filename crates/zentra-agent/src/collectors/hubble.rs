//! SW P06: Hubble gRPC Flow Collector for Zentra.
//!
//! Connects to Hubble Relay, subscribes to the flow stream, and persists
//! L4/L7 flow records into the netflow_records table.

use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, error};
use sqlx::PgPool;
use prost::Message;
use std::marker::Send;
use tonic::transport::Channel;
use tonic::client::Grpc;
use tonic::body::BoxBody;
use tonic::codegen::Body;
use tonic::Status;
use tonic::codec::ProstCodec;
use http::uri::PathAndQuery;

// ============================================================================
// Hubble Flow Proto Definitions (Hand-written prost-annotated structs)
// ============================================================================

#[derive(Clone, PartialEq, Message)]
pub struct GetFlowsRequest {
    #[prost(uint32, tag = "1")]
    pub number: u32,
    #[prost(bool, tag = "2")]
    pub follow: bool,
    // simplified: omitting filters for now
}

#[derive(Clone, PartialEq, Message)]
pub struct Flow {
    #[prost(message, optional, tag = "1")]
    pub time: Option<prost_types::Timestamp>,
    #[prost(string, tag = "2")]
    pub verdict: String, // FORWARDED, DROPPED, etc.
    #[prost(string, tag = "3")]
    pub drop_reason: String,
    #[prost(message, optional, tag = "4")]
    pub ethernet: Option<Ethernet>,
    #[prost(message, optional, tag = "5")]
    pub ip: Option<Ip>,
    #[prost(message, optional, tag = "8")]
    pub l4: Option<Layer4>,
    #[prost(message, optional, tag = "11")]
    pub l7: Option<Layer7>,
    #[prost(string, tag = "12")]
    pub source_names: String,
    #[prost(string, tag = "13")]
    pub destination_names: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct Ethernet {
    #[prost(string, tag = "1")]
    pub source: String,
    #[prost(string, tag = "2")]
    pub destination: String,
}

#[derive(Clone, PartialEq, Message)]
pub struct Ip {
    #[prost(string, tag = "1")]
    pub source: String,
    #[prost(string, tag = "2")]
    pub destination: String,
    #[prost(uint32, tag = "3")]
    pub ip_version: u32,
}

#[derive(Clone, PartialEq, Message)]
pub struct Layer4 {
    #[prost(oneof = "layer4::Protocol", tags = "1, 2, 3")]
    pub protocol: Option<layer4::Protocol>,
}

pub mod layer4 {
    use prost::Message;
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Protocol {
        #[prost(message, tag = "1")]
        TCP(Tcp),
        #[prost(message, tag = "2")]
        UDP(Udp),
        #[prost(message, tag = "3")]
        ICMPv4(Icmpv4),
    }

    #[derive(Clone, PartialEq, Message)]
    pub struct Tcp {
        #[prost(uint32, tag = "1")]
        pub source_port: u32,
        #[prost(uint32, tag = "2")]
        pub destination_port: u32,
    }

    #[derive(Clone, PartialEq, Message)]
    pub struct Udp {
        #[prost(uint32, tag = "1")]
        pub source_port: u32,
        #[prost(uint32, tag = "2")]
        pub destination_port: u32,
    }

    #[derive(Clone, PartialEq, Message)]
    pub struct Icmpv4 {
        #[prost(uint32, tag = "1")]
        pub type_code: u32,
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct Layer7 {
    #[prost(string, tag = "1")]
    pub type_text: String, // HTTP, DNS, Kafka
    #[prost(oneof = "layer7::Record", tags = "100, 101, 102")]
    pub record: Option<layer7::Record>,
}

pub mod layer7 {
    use prost::Message;
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum Record {
        #[prost(message, tag = "100")]
        Http(Http),
        #[prost(message, tag = "101")]
        Dns(Dns),
        #[prost(message, tag = "102")]
        Kafka(Kafka),
    }

    #[derive(Clone, PartialEq, Message)]
    pub struct Http {
        #[prost(string, tag = "1")]
        pub method: String,
        #[prost(string, tag = "2")]
        pub url: String,
        #[prost(uint32, tag = "3")]
        pub status: u32,
    }

    #[derive(Clone, PartialEq, Message)]
    pub struct Dns {
        #[prost(string, tag = "1")]
        pub query: String,
    }

    #[derive(Clone, PartialEq, Message)]
    pub struct Kafka {
        #[prost(string, tag = "1")]
        pub topic: String,
        #[prost(string, tag = "2")]
        pub operation: String,
    }
}

#[derive(Clone, PartialEq, Message)]
pub struct GetFlowsResponse {
    #[prost(oneof = "get_flows_response::ResponseType", tags = "1, 2, 3")]
    pub response_type: Option<get_flows_response::ResponseType>,
}

pub mod get_flows_response {
    use super::Flow;
    #[derive(Clone, PartialEq, ::prost::Oneof)]
    pub enum ResponseType {
        #[prost(message, tag = "1")]
        Flow(Flow),
        #[prost(message, tag = "2")]
        NodeStatus(NodeStatusEvent),
        #[prost(message, tag = "3")]
        LostEvents(LostEvent),
    }

    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct NodeStatusEvent {
        #[prost(string, tag = "1")]
        pub state: String,
    }

    #[derive(Clone, PartialEq, ::prost::Message)]
    pub struct LostEvent {
        #[prost(uint64, tag = "1")]
        pub num_events_lost: u64,
    }
}

pub struct ObserverClient<T> {
    inner: Grpc<T>,
}

impl<T> ObserverClient<T>
where
    T: tonic::client::GrpcService<BoxBody>,
    T::Error: Into<tonic::codegen::StdError>,
    T::ResponseBody: Body<Data = tonic::codegen::Bytes> + Send + 'static,
    <T::ResponseBody as Body>::Error: Into<tonic::codegen::StdError> + Send,
{
    pub fn new(inner: T) -> Self {
        let inner = Grpc::new(inner);
        Self { inner }
    }

    pub async fn get_flows(
        &mut self,
        request: impl tonic::IntoRequest<GetFlowsRequest>,
    ) -> Result<tonic::Response<tonic::codec::Streaming<GetFlowsResponse>>, Status> {
        self.inner.ready().await.map_err(|e| {
            Status::new(tonic::Code::Unknown, format!("Service was not ready: {}", e.into()))
        })?;
        let codec = ProstCodec::default();
        let path = PathAndQuery::from_static("/observer.Observer/GetFlows");
        self.inner.server_streaming(request.into_request(), path, codec).await
    }
}

// ============================================================================
// Hubble Collector Implementation
// ============================================================================

pub struct HubbleCollector {
    pool: PgPool,
    relay_addr: String,
    org_id: uuid::Uuid,
}

impl HubbleCollector {
    pub fn new(pool: PgPool, relay_addr: String, org_id: uuid::Uuid) -> Self {
        Self { pool, relay_addr, org_id }
    }

    pub async fn run(&self) {
        info!("Hubble Collector starting (relay: {})", self.relay_addr);
        
        loop {
            if let Err(e) = self.collect_stream().await {
                error!("Hubble stream error: {}. Reconnecting in 10s...", e);
                sleep(Duration::from_secs(10)).await;
            }
        }
    }

    async fn collect_stream(&self) -> Result<(), String> {
        use tonic::transport::Channel;
        
        info!("Connecting to Hubble Relay at {}...", self.relay_addr);
        
        let endpoint = format!("http://{}", self.relay_addr);
        let channel = Channel::from_shared(endpoint)
            .map_err(|e| format!("Invalid relay address: {}", e))?
            .connect()
            .await
            .map_err(|e| format!("Failed to connect to Hubble Relay: {}", e))?;

        let mut client = ObserverClient::new(channel);
        
        let request = GetFlowsRequest {
            number: 0, // 0 means follow
            follow: true,
        };

        info!("Subscribing to Hubble flow stream...");
        let mut stream = client.get_flows(request).await
            .map_err(|e| format!("gRPC request failed: {}", e))?
            .into_inner();
        
        info!("Hubble stream active");

        while let Some(res) = stream.message().await.map_err(|e| e.to_string())? {
            if let Some(resp_type) = res.response_type {
                match resp_type {
                    get_flows_response::ResponseType::Flow(flow) => {
                        if let Err(e) = self.persist_flow(flow).await {
                            error!("Failed to persist Hubble flow: {}", e);
                        }
                    },
                    get_flows_response::ResponseType::NodeStatus(node) => {
                        info!("Hubble node status: {}", node.state);
                    },
                    get_flows_response::ResponseType::LostEvents(lost) => {
                        error!("Hubble lost events: {}", lost.num_events_lost);
                    }
                }
            }
        }
        
        Ok(())
    }

    async fn persist_flow(&self, flow: Flow) -> Result<(), sqlx::Error> {
        // ... rest of persist_flow remains same ...
        let ts = flow.time.map(|t| {
            chrono::DateTime::from_timestamp(t.seconds, t.nanos as u32).unwrap_or_else(|| chrono::Utc::now())
        }).unwrap_or_else(|| chrono::Utc::now());

        let (src_ip, dst_ip) = flow.ip.map(|ip| (ip.source, ip.destination)).unwrap_or_default();
        let (src_port, dst_port) = match flow.l4.and_then(|l4| l4.protocol) {
            Some(layer4::Protocol::TCP(tcp)) => (tcp.source_port as i32, tcp.destination_port as i32),
            Some(layer4::Protocol::UDP(udp)) => (udp.source_port as i32, udp.destination_port as i32),
            _ => (0, 0),
        };

        let mut extra = serde_json::json!({
            "verdict": flow.verdict,
            "drop_reason": flow.drop_reason,
            "src_names": flow.source_names,
            "dst_names": flow.destination_names,
        });

        if let Some(l7) = flow.l7 {
            let mut l7_data = serde_json::json!({ "type": l7.type_text });
            match l7.record {
                Some(layer7::Record::Http(http)) => {
                    l7_data["method"] = serde_json::json!(http.method);
                    l7_data["url"] = serde_json::json!(http.url);
                    l7_data["status_code"] = serde_json::json!(http.status);
                },
                Some(layer7::Record::Dns(dns)) => {
                    l7_data["query"] = serde_json::json!(dns.query);
                },
                Some(layer7::Record::Kafka(kafka)) => {
                    l7_data["topic"] = serde_json::json!(kafka.topic);
                    l7_data["operation"] = serde_json::json!(kafka.operation);
                },
                None => {}
            }
            extra["l7"] = l7_data;
        }

        sqlx::query(
            r#"INSERT INTO netflow_records (org_id, received_at, src_ip, src_port, dst_ip, dst_port, flow_version, extra)
               VALUES ($1, $2, $3, $4, $5, $6, 100, $7)"#
        )
        .bind(self.org_id)
        .bind(ts)
        .bind(src_ip)
        .bind(src_port)
        .bind(dst_ip)
        .bind(dst_port)
        .bind(extra)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}
