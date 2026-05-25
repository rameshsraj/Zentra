//! Multi-protocol flow receiver: NetFlow v5, v9, IPFIX, sFlow on configurable UDP ports.

use tokio::net::UdpSocket;
use tracing::{info, warn, error};
use std::sync::Arc;
use sqlx::PgPool;
use super::{
    flow_aggregator::FlowAggregator,
    netflow_v5, netflow_v9, ipfix, sflow,
};

pub struct FlowServerConfig {
    pub netflow_v5_bind: String,   // default "0.0.0.0:2055"
    pub netflow_v9_bind: String,   // default "0.0.0.0:2056"
    pub ipfix_bind: String,        // default "0.0.0.0:4739"
    pub sflow_bind: String,        // default "0.0.0.0:6343"
}

impl Default for FlowServerConfig {
    fn default() -> Self {
        Self {
            netflow_v5_bind: "0.0.0.0:2055".into(),
            netflow_v9_bind: "0.0.0.0:2056".into(),
            ipfix_bind: "0.0.0.0:4739".into(),
            sflow_bind: "0.0.0.0:6343".into(),
        }
    }
}

pub struct FlowServer {
    config: FlowServerConfig,
    aggregator: Arc<FlowAggregator>,
    v9_cache: netflow_v9::TemplateCache,
}

impl FlowServer {
    pub fn new(pool: PgPool, config: FlowServerConfig) -> Self {
        let aggregator = Arc::new(FlowAggregator::new(pool));
        let v9_cache = netflow_v9::new_template_cache();
        Self { config, aggregator, v9_cache }
    }

    pub async fn run(self) {
        let agg = self.aggregator.clone();
        // Start flush loop (every 30 seconds)
        tokio::spawn({
            let agg2 = agg.clone();
            async move { Arc::clone(&agg2).run_flush_loop(30).await; }
        });

        let v9_cache = self.v9_cache.clone();

        // Spawn listener for each protocol
        let (v5_agg, v9_agg, ipfix_agg, sflow_agg) = (agg.clone(), agg.clone(), agg.clone(), agg.clone());
        let (v9_cache_v9, v9_cache_ipfix) = (v9_cache.clone(), v9_cache.clone());

        let v5_bind = self.config.netflow_v5_bind.clone();
        let v9_bind = self.config.netflow_v9_bind.clone();
        let ipfix_bind = self.config.ipfix_bind.clone();
        let sflow_bind = self.config.sflow_bind.clone();

        let tasks = vec![
            tokio::spawn(async move { listen_v5(&v5_bind, v5_agg).await; }),
            tokio::spawn(async move { listen_v9(&v9_bind, v9_agg, v9_cache_v9).await; }),
            tokio::spawn(async move { listen_ipfix(&ipfix_bind, ipfix_agg, v9_cache_ipfix).await; }),
            tokio::spawn(async move { listen_sflow(&sflow_bind, sflow_agg).await; }),
        ];

        // Wait for any listener to complete (they run forever unless error)
        for task in tasks {
            let _ = task.await;
        }
    }
}

async fn listen_v5(bind: &str, agg: Arc<FlowAggregator>) {
    let sock = match UdpSocket::bind(bind).await {
        Ok(s) => { info!(addr = %bind, "NetFlow v5 listener started"); s }
        Err(e) => { error!(addr = %bind, error = %e, "Failed to bind NetFlow v5 socket"); return; }
    };
    let mut buf = vec![0u8; 65535];
    loop {
        match sock.recv_from(&mut buf).await {
            Ok((len, peer)) => {
                let records = netflow_v5::parse(&buf[..len]);
                let peer_ip = peer.ip().to_string();
                for rec in records {
                    agg.push(rec, &peer_ip);
                }
            }
            Err(e) => warn!(error = %e, "NetFlow v5 recv error"),
        }
    }
}

async fn listen_v9(bind: &str, agg: Arc<FlowAggregator>, cache: netflow_v9::TemplateCache) {
    let sock = match UdpSocket::bind(bind).await {
        Ok(s) => { info!(addr = %bind, "NetFlow v9 listener started"); s }
        Err(e) => { error!(addr = %bind, error = %e, "Failed to bind NetFlow v9 socket"); return; }
    };
    let mut buf = vec![0u8; 65535];
    loop {
        match sock.recv_from(&mut buf).await {
            Ok((len, peer)) => {
                let peer_ip = peer.ip().to_string();
                let records = netflow_v9::parse(&buf[..len], &peer_ip, &cache);
                for rec in records {
                    agg.push(rec, &peer_ip);
                }
            }
            Err(e) => warn!(error = %e, "NetFlow v9 recv error"),
        }
    }
}

async fn listen_ipfix(bind: &str, agg: Arc<FlowAggregator>, cache: netflow_v9::TemplateCache) {
    let sock = match UdpSocket::bind(bind).await {
        Ok(s) => { info!(addr = %bind, "IPFIX listener started"); s }
        Err(e) => { error!(addr = %bind, error = %e, "Failed to bind IPFIX socket"); return; }
    };
    let mut buf = vec![0u8; 65535];
    loop {
        match sock.recv_from(&mut buf).await {
            Ok((len, peer)) => {
                let peer_ip = peer.ip().to_string();
                let records = ipfix::parse(&buf[..len], &peer_ip, &cache);
                for rec in records {
                    agg.push(rec, &peer_ip);
                }
            }
            Err(e) => warn!(error = %e, "IPFIX recv error"),
        }
    }
}

async fn listen_sflow(bind: &str, agg: Arc<FlowAggregator>) {
    let sock = match UdpSocket::bind(bind).await {
        Ok(s) => { info!(addr = %bind, "sFlow listener started"); s }
        Err(e) => { error!(addr = %bind, error = %e, "Failed to bind sFlow socket"); return; }
    };
    let mut buf = vec![0u8; 65535];
    loop {
        match sock.recv_from(&mut buf).await {
            Ok((len, peer)) => {
                let peer_ip = peer.ip().to_string();
                let records = sflow::parse(&buf[..len], &peer_ip);
                for rec in records {
                    agg.push(rec, &peer_ip);
                }
            }
            Err(e) => warn!(error = %e, "sFlow recv error"),
        }
    }
}
