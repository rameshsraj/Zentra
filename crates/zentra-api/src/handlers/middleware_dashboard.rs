use axum::{extract::State, Json};
use serde::Serialize;
use chrono::Utc;
use crate::state::AppState;
use crate::handlers::{
    middleware_nginx, middleware_apache, middleware_elasticsearch,
    middleware_haproxy, middleware_tomcat, middleware_phpfpm,
    middleware_rabbitmq, middleware_kafka, middleware_memcached,
    middleware_redis, middleware_docker, middleware_kubernetes,
};

use crate::error::ApiError;

#[derive(Debug, Clone, Serialize)]
pub struct MiddlewareServiceStatus {
    pub service_type: String,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub status: String,
    pub key_metric_1: String,
    pub key_metric_2: String,
    pub last_checked: String,
}

pub async fn dashboard(
    State(_s): State<AppState>,
) -> Result<Json<Vec<MiddlewareServiceStatus>>, ApiError> {
    let mut services = Vec::new();
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build().unwrap_or_default();
    let now = Utc::now().to_rfc3339();

    // Nginx instances
    {
        let insts = middleware_nginx::live_instances().read().clone();
        for inst in &insts {
            let url = format!("http://{}:{}{}", inst.host, inst.port, inst.stub_status_path);
            let (status, m1, m2) = match client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    let body = resp.text().await.unwrap_or_default();
                    let mut active: u64 = 0; let mut requests: u64 = 0;
                    for line in body.lines() {
                        let line = line.trim();
                        if line.starts_with("Active connections:") {
                            active = line.split(':').nth(1).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
                        } else if let Ok(vals) = line.split_whitespace().map(|v| v.parse::<u64>()).collect::<Result<Vec<u64>,_>>() {
                            if vals.len() == 3 { requests = vals[2]; }
                        }
                    }
                    ("healthy".into(), format!("{} active conns", active), format!("{} total requests", requests))
                }
                _ => ("unknown".into(), "Pending check".into(), "".into()),
            };
            services.push(MiddlewareServiceStatus {
                service_type: "nginx".into(), name: inst.name.clone(),
                host: inst.host.clone(), port: inst.port,
                status, key_metric_1: m1, key_metric_2: m2,
                last_checked: now.clone(),
            });
        }
    }

    // Apache instances
    {
        let insts = middleware_apache::live_instances().read().clone();
        for inst in &insts {
            let url = format!("http://{}:{}/server-status?auto", inst.host, inst.port);
            let (status, m1, m2) = match client.get(&url).send().await {
                Ok(resp) if resp.status().is_success() => {
                    let body = resp.text().await.unwrap_or_default();
                    let mut busy: u64 = 0; let mut rps: f64 = 0.0;
                    for line in body.lines() {
                        if let Some((k, v)) = line.split_once(':') {
                            match k.trim() {
                                "BusyWorkers" => busy = v.trim().parse().unwrap_or(0),
                                "ReqPerSec" => rps = v.trim().parse().unwrap_or(0.0),
                                _ => {}
                            }
                        }
                    }
                    ("healthy".into(), format!("{} busy workers", busy), format!("{:.2} req/s", rps))
                }
                _ => ("unknown".into(), "Pending check".into(), "".into()),
            };
            services.push(MiddlewareServiceStatus {
                service_type: "apache".into(), name: inst.name.clone(),
                host: inst.host.clone(), port: inst.port,
                status, key_metric_1: m1, key_metric_2: m2,
                last_checked: now.clone(),
            });
        }
    }

    // Elasticsearch instances
    for inst in middleware_elasticsearch::live_instances().read().iter() {
        services.push(MiddlewareServiceStatus {
            service_type: "elasticsearch".into(), name: inst.name.clone(),
            host: inst.host.clone(), port: inst.port,
            status: inst.status.clone(), key_metric_1: "Cluster node".into(), key_metric_2: "".into(),
            last_checked: now.clone(),
        });
    }

    // HAProxy instances
    for inst in middleware_haproxy::live_instances().read().iter() {
        services.push(MiddlewareServiceStatus {
            service_type: "haproxy".into(), name: inst.name.clone(),
            host: inst.host.clone(), port: inst.port,
            status: inst.status.clone(), key_metric_1: "Load balancer".into(), key_metric_2: "".into(),
            last_checked: now.clone(),
        });
    }

    // Tomcat instances
    for inst in middleware_tomcat::live_instances().read().iter() {
        services.push(MiddlewareServiceStatus {
            service_type: "tomcat".into(), name: inst.name.clone(),
            host: inst.host.clone(), port: inst.port,
            status: inst.status.clone(), key_metric_1: "App server".into(), key_metric_2: "".into(),
            last_checked: now.clone(),
        });
    }

    // PHP-FPM instances
    for inst in middleware_phpfpm::live_instances().read().iter() {
        services.push(MiddlewareServiceStatus {
            service_type: "phpfpm".into(), name: inst.name.clone(),
            host: inst.host.clone(), port: inst.port,
            status: inst.status.clone(), key_metric_1: "Process manager".into(), key_metric_2: "".into(),
            last_checked: now.clone(),
        });
    }

    // RabbitMQ instances
    for inst in middleware_rabbitmq::live_instances().read().iter() {
        services.push(MiddlewareServiceStatus {
            service_type: "rabbitmq".into(), name: inst.name.clone(),
            host: inst.host.clone(), port: inst.port,
            status: inst.status.clone(), key_metric_1: "Message broker".into(), key_metric_2: "".into(),
            last_checked: now.clone(),
        });
    }

    // Kafka instances
    for inst in middleware_kafka::live_instances().read().iter() {
        services.push(MiddlewareServiceStatus {
            service_type: "kafka".into(), name: inst.name.clone(),
            host: inst.host.clone(), port: inst.port,
            status: inst.status.clone(), key_metric_1: "Stream broker".into(), key_metric_2: "".into(),
            last_checked: now.clone(),
        });
    }

    // Redis instances
    for inst in middleware_redis::live_instances().read().iter() {
        services.push(MiddlewareServiceStatus {
            service_type: "redis".into(), name: inst.name.clone(),
            host: inst.host.clone(), port: inst.port,
            status: inst.status.clone(), key_metric_1: "Cache store".into(), key_metric_2: "".into(),
            last_checked: now.clone(),
        });
    }

    // Memcached instances
    for inst in middleware_memcached::live_instances().read().iter() {
        services.push(MiddlewareServiceStatus {
            service_type: "memcached".into(), name: inst.name.clone(),
            host: inst.host.clone(), port: inst.port,
            status: inst.status.clone(), key_metric_1: "Cache store".into(), key_metric_2: "".into(),
            last_checked: now.clone(),
        });
    }

    // Docker instances
    for inst in middleware_docker::live_instances().read().iter() {
        services.push(MiddlewareServiceStatus {
            service_type: "docker".into(), name: inst.name.clone(),
            host: inst.host.clone(), port: inst.port,
            status: inst.status.clone(), key_metric_1: "Container runtime".into(), key_metric_2: "".into(),
            last_checked: now.clone(),
        });
    }

    // Kubernetes instances
    for inst in middleware_kubernetes::live_instances().read().iter() {
        services.push(MiddlewareServiceStatus {
            service_type: "kubernetes".into(), name: inst.name.clone(),
            host: inst.host.clone(), port: inst.port,
            status: inst.status.clone(), key_metric_1: "Orchestrator".into(), key_metric_2: "".into(),
            last_checked: now.clone(),
        });
    }

    Ok(Json(services))
}
