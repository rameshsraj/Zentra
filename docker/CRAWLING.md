# Zentra Scraping / Crawling Architecture

This document describes how Zentra implements metric scraping and target
discovery, based on the `zentra-scrape` crate. It is intended as a reference
for simulating the same pattern in another application.

---

## Overview

Zentra uses a Prometheus-compatible pull model for metric collection.

```
Target List
(static / DNS-SD / K8s-SD)
        |
        v
  TargetDiscovery       <-- discovers what to scrape
        |
        v
     Scraper             <-- fetches /metrics from each target in parallel
        |
        v
  Parser (Prometheus     <-- parses text exposition format into samples
  text format)
        |
        v
  Recording Rules        <-- optional pre-aggregation before storage
        |
        v
  Ingestion Pipeline     <-- writes metrics to the database
```

---

## 1. Target Discovery (`target_discovery.rs`)

Targets are endpoints that expose a `/metrics` HTTP endpoint in Prometheus text
format.

### ScrapeTarget

Every target is represented as:

```rust
pub struct ScrapeTarget {
    pub target_id: Uuid,      // unique ID per target
    pub org_id:    Uuid,      // which tenant owns it
    pub host:      String,    // hostname or IP
    pub port:      u16,       // port (e.g., 9090, 9100)
    pub scheme:    Option<String>,       // "http" or "https"
    pub metrics_path: Option<String>,   // default: /metrics
    pub job_name:  Option<String>,      // logical group (e.g., "node_exporter")
    pub scrape_interval_seconds: Option<u32>,
    pub labels:    HashMap<String, String>, // key/value metadata tags
    pub enabled:   bool,
    pub discovered_by: String,   // "static", "dns_sd", "kubernetes_sd"
}
```

### Discovery Providers

Zentra supports three discovery methods, all implementing the same trait:

```rust
pub trait DiscoveryProvider: Send + Sync {
    fn name(&self) -> &str;
    fn discover(&self) -> Vec<ScrapeTarget>;
}
```

#### a) Static Discovery

The simplest method. Targets are defined manually (e.g., from the Zentra UI or
config file) and returned as-is.

```rust
let targets = vec![
    ScrapeTarget::new(org_id, "10.0.0.1", 9100)
        .with_job("node_exporter")
        .with_path("/metrics")
        .with_label("env", "production"),
];
let sd = StaticDiscovery::new(targets);
```

**Use case:** Any manually registered server, database, or device.

#### b) DNS Service Discovery (DNS-SD)

Queries DNS SRV records to find targets dynamically. For example, a record
`_prometheus._tcp.internal` resolves to a list of hosts and ports.

```
DNS SRV record: _prometheus._tcp.example.com
  -> srv-linux-01.internal:9100
  -> srv-linux-02.internal:9100
```

The implementation queries these records and creates a `ScrapeTarget` for each
resolved host/port. Currently a stub in Zentra -- ready to be filled with a DNS
library call (e.g., `trust-dns-resolver`).

#### c) Kubernetes Service Discovery (K8s-SD)

Queries the Kubernetes API server for pods/services annotated with:

```yaml
annotations:
  prometheus.io/scrape: "true"
  prometheus.io/port:   "9090"
  prometheus.io/path:   "/metrics"
```

Zentra reads the pod IP, port, and path from these annotations and creates a
target for each matching pod. Currently a stub -- ready for a `kube` client
integration.

#### Discovery Manager

All providers are registered with a `TargetDiscovery` manager that runs them
all and returns a merged list:

```rust
let mut discovery = TargetDiscovery::new();
discovery.add_provider(Box::new(StaticDiscovery::new(manual_targets)));
discovery.add_provider(Box::new(DnsServiceDiscovery::new(...)));

let all_targets = discovery.discover_all();
```

---

## 2. Scraper (`scraper.rs`)

The scraper fetches the `/metrics` endpoint from every target using an async
HTTP client (`reqwest`).

### Configuration

```rust
pub struct ScrapeConfig {
    pub scrape_interval:  Duration,   // default: 15s
    pub scrape_timeout:   Duration,   // default: 10s
    pub honor_labels:     bool,       // preserve source labels
    pub honor_timestamps: bool,       // use source timestamps if present
    pub metrics_path:     String,     // default: /metrics
    pub scheme:           String,     // default: http
}
```

### Scraping a Single Target

```rust
let result = scraper.scrape_target(&target).await;
```

Internally:
1. Builds URL: `{scheme}://{host}:{port}{metrics_path}`
2. Makes HTTP GET with `User-Agent: Zentra-Scraper/0.1`
3. On HTTP 200: passes body to parser
4. On error: records target as `Down`, returns empty samples
5. Records `TargetHealth` (Up / Down / Unknown) in a shared map

### Parallel Scraping

All targets are scraped concurrently using `futures::future::join_all`:

```rust
let results = scraper.scrape_all(&all_targets).await;
```

Each `ScrapeResult` contains:

```rust
pub struct ScrapeResult {
    pub target_id:          Uuid,
    pub url:                String,
    pub health:             TargetHealth,   // Up / Down / Unknown
    pub samples:            Vec<PrometheusSample>,
    pub metrics:            Vec<Metric>,    // converted for ingestion
    pub scrape_duration_ms: f64,
    pub sample_count:       usize,
    pub error:              Option<String>,
    pub scraped_at:         DateTime<Utc>,
}
```

### Sample to Metric Conversion

After parsing, each `PrometheusSample` is converted to a Zentra `Metric`:

- `metric_name` -> metric name
- `value` -> float value
- `instance` or `job` label -> resource identifier
- All labels -> copied as tags
- `org_id`, `target_id`, source `"zentra-scrape"` -> context metadata

---

## 3. Parser (`parser.rs`)

Parses the Prometheus text exposition format:

```
# HELP http_requests_total Total HTTP requests.
# TYPE http_requests_total counter
http_requests_total{method="GET",code="200"} 1027 1395066363000
http_requests_total{method="POST",code="500"} 3
go_goroutines 42
```

### Rules

- Lines starting with `#` are metadata (HELP or TYPE comments) -- skipped for
  sample parsing
- Each data line is: `metric_name{label="value",...} <value> [timestamp]`
- Labels are optional
- Timestamp (milliseconds epoch) is optional
- Special float values handled: `+Inf`, `-Inf`, `NaN`

### Output

```rust
pub struct PrometheusSample {
    pub metric_name: String,
    pub labels:      HashMap<String, String>,
    pub value:       f64,
    pub timestamp:   Option<i64>,
}
```

Metadata (HELP/TYPE) is parsed separately via `parse_metadata()` and returns
`MetricFamily` structs with name, help string, and type (counter/gauge/
histogram/summary/untyped).

---

## 4. Recording Rules (`recording_rules.rs`)

Recording rules pre-aggregate scraped samples before they reach the ingestion
pipeline. This reduces write volume and pre-computes expensive queries.

### Rule Definition

```rust
pub struct ScrapeRecordingRule {
    pub name:           String,
    pub source_metric:  String,   // input metric name to match
    pub output_metric:  String,   // synthetic metric to produce
    pub aggregation:    ScrapeAggregation,
    pub group_by:       Vec<String>,  // label keys to group by
    pub enabled:        bool,
}
```

### Aggregations

| Operation | Description |
|-----------|-------------|
| `Sum`     | Total of all matching values |
| `Avg`     | Mean of all matching values |
| `Min`     | Lowest value |
| `Max`     | Highest value |
| `Count`   | Number of matching samples |
| `Rate`    | (last - first) / (count - 1) -- simple per-sample rate |

### Example

```rust
// Sum all HTTP requests across jobs, group by job label
let rule = ScrapeRecordingRule::new(
    "total_http_requests",
    "http_requests_total",
    "recording:http_requests:sum",
    ScrapeAggregation::Sum,
).with_group_by(vec!["job".into()]);

let synthetic_metrics = evaluate_rules(&[rule], &samples, org_id, target_id);
```

---

## 5. End-to-End Flow

```
Every scrape_interval (default 15s):

1. TargetDiscovery::discover_all()
   -> returns Vec<ScrapeTarget>

2. Scraper::scrape_all(&targets)
   -> HTTP GET {scheme}://{host}:{port}{path} for each target (parallel)
   -> parse_prometheus_text(body) -> Vec<PrometheusSample>
   -> convert_samples() -> Vec<Metric>

3. evaluate_rules(&recording_rules, &samples)
   -> Vec<Metric> (synthetic aggregates)

4. Ingestion pipeline receives:
   - Raw metrics from step 2
   - Synthetic metrics from step 3
   -> Written to TimescaleDB
```

---

## 6. Simulating This in Another Application

To replicate this pattern, you need:

### Minimum Components

| Component | What it does | Technology options |
|-----------|-------------|-------------------|
| Target list | Stores host/port/path/job metadata | DB table, YAML config, etcd |
| HTTP fetcher | GET /metrics from each target | reqwest (Rust), httpx (Python), axios (Node) |
| Text parser | Parse Prometheus exposition format | Custom or prometheus_client library |
| Scheduler | Run every N seconds | tokio interval (Rust), APScheduler (Python), setInterval (Node) |
| Storage writer | Write parsed samples to DB | TimescaleDB, InfluxDB, Prometheus remote_write |

### Minimal Python Equivalent

```python
import httpx
import time
import re

def fetch_metrics(host, port, path="/metrics"):
    url = f"http://{host}:{port}{path}"
    resp = httpx.get(url, timeout=10)
    resp.raise_for_status()
    return parse_prometheus_text(resp.text)

def parse_prometheus_text(body):
    samples = []
    for line in body.splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        # metric_name{labels} value [timestamp]
        m = re.match(r'^([^\{]+)(\{[^\}]*\})?\s+([^\s]+)(\s+\d+)?$', line)
        if m:
            samples.append({
                "name":   m.group(1).strip(),
                "labels": m.group(2) or "",
                "value":  float(m.group(3)),
            })
    return samples

# Discovery: static list
targets = [
    {"host": "10.100.0.11", "port": 9100, "job": "node_exporter"},
    {"host": "10.100.0.12", "port": 9100, "job": "node_exporter"},
]

while True:
    for t in targets:
        samples = fetch_metrics(t["host"], t["port"])
        # write samples to your DB here
        print(f"{t['host']}: {len(samples)} samples")
    time.sleep(15)
```

---

## Key Design Decisions in Zentra

- **Pull model** -- Zentra pulls from targets, targets do not push. This matches
  Prometheus conventions.
- **Parallel scraping** -- All targets scraped concurrently per interval.
- **Per-target health tracking** -- `TargetHealth` (Up/Down/Unknown) tracked
  independently for each target.
- **Pluggable discovery** -- New discovery providers (e.g., Consul SD, EC2 SD)
  can be added without changing the scraper.
- **Recording rules at scrape time** -- Aggregates computed before storage,
  reducing DB write amplification for high-cardinality metrics.
- **Prometheus compatibility** -- Uses standard text format, so any
  Prometheus exporter works as a target out of the box.
