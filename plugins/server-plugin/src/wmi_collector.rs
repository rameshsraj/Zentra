//! Windows agentless monitoring via WinRM + WQL (WMI queries over HTTP).
//! No agent installation on Windows hosts — uses built-in WS-Management protocol.
//! Transport: HTTP port 5985 (or HTTPS 5986)

use zentra_core::models::metric::Metric;
use zentra_core::models::target::Target;
use reqwest::Client;
use tracing::{debug, warn};

fn srv(name: &str, value: f64, resource: &str, target: &Target) -> Metric {
    Metric::new(name, value, resource)
        .with_context(target.org_id, target.target_id, "server-plugin")
        .with_resource_type("server")
}

pub struct WmiCollector;

impl WmiCollector {
    /// Send a WQL query via WinRM (WS-Management SOAP/HTTP).
    /// Returns the raw SOAP response body.
    async fn winrm_query(client: &Client, host: &str, port: u16, user: &str, pass: &str, wql: &str) -> Option<String> {
        let url = format!("http://{}:{}/wsman", host, port);
        let soap = format!(r#"<?xml version="1.0" encoding="UTF-8"?>
<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
            xmlns:a="http://schemas.xmlsoap.org/ws/2004/08/addressing"
            xmlns:w="http://schemas.dmtf.org/wbem/wsman/1/wsman.xsd"
            xmlns:p="http://schemas.microsoft.com/wbem/wsman/1/wsman.xsd"
            xmlns:e="http://schemas.xmlsoap.org/ws/2004/08/eventing"
            xmlns:n="http://schemas.xmlsoap.org/ws/2004/09/enumeration"
            xmlns:b="http://schemas.dmtf.org/wbem/wsman/1/cimbinding.xsd">
  <s:Header>
    <a:To>{url}</a:To>
    <w:ResourceURI s:mustUnderstand="true">http://schemas.microsoft.com/wbem/wsman/1/wmi/root/cimv2/*</w:ResourceURI>
    <a:ReplyTo><a:Address s:mustUnderstand="true">http://schemas.xmlsoap.org/ws/2004/08/addressing/role/anonymous</a:Address></a:ReplyTo>
    <a:Action s:mustUnderstand="true">http://schemas.xmlsoap.org/ws/2004/09/enumeration/Enumerate</a:Action>
    <w:MaxEnvelopeSize s:mustUnderstand="true">512000</w:MaxEnvelopeSize>
    <a:MessageID>uuid:1</a:MessageID>
    <w:Locale xml:lang="en-US" s:mustUnderstand="false"/>
    <p:DataLocale xml:lang="en-US" s:mustUnderstand="false"/>
    <w:OperationTimeout>PT30S</w:OperationTimeout>
    <w:OptionSet><w:Option Name="MaxElements">200</w:Option></w:OptionSet>
  </s:Header>
  <s:Body>
    <n:Enumerate>
      <w:Filter Dialect="http://schemas.microsoft.com/wbem/wsman/1/WQL">{wql}</w:Filter>
    </n:Enumerate>
  </s:Body>
</s:Envelope>"#, url = url, wql = wql);

        let resp = client
            .post(&url)
            .basic_auth(user, Some(pass))
            .header("Content-Type", "application/soap+xml;charset=UTF-8")
            .header("User-Agent", "Zentra-WinRM/1.0")
            .body(soap)
            .send()
            .await
            .ok()?;

        resp.text().await.ok()
    }

    /// Extract a named XML element value from SOAP response.
    /// Simple text extraction — not a full XML parser.
    fn extract_xml_value(xml: &str, tag: &str) -> Option<f64> {
        let open = format!("<{}", tag);
        let close = format!("</{}>", tag);
        let start = xml.find(&open)?;
        let tag_end = xml[start..].find('>')? + start + 1;
        let end = xml.find(&close)?;
        xml[tag_end..end].trim().parse::<f64>().ok()
    }

    fn extract_xml_str<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
        let open = format!("<{}", tag);
        let close = format!("</{}>", tag);
        let start = xml.find(&open)?;
        let tag_end = xml[start..].find('>')? + start + 1;
        let end = xml.find(&close)?;
        Some(xml[tag_end..end].trim())
    }

    pub async fn collect(target: &Target) -> Vec<Metric> {
        let mut metrics = Vec::new();

        let host = &target.host;
        let port = if target.port > 0 { target.port as u16 } else { 5985 };
        let (user, pass) = target.credentials.as_ref()
            .map(|c| (
                c.username.clone().unwrap_or_else(|| "Administrator".into()),
                c.password_encrypted.clone().unwrap_or_default(),
            ))
            .unwrap_or_else(|| ("Administrator".into(), String::new()));

        let client = match Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
        {
            Ok(c) => c,
            Err(e) => {
                warn!(target = %target.name, err = %e, "WMI client build error");
                return metrics;
            }
        };

        // ── CPU ────────────────────────────────────────────────────────────
        let wql_cpu = "SELECT Name,NumberOfCores,NumberOfLogicalProcessors,LoadPercentage,CurrentClockSpeed FROM Win32_Processor";
        if let Some(xml) = Self::winrm_query(&client, host, port, &user, &pass, wql_cpu).await {
            if let Some(load) = Self::extract_xml_value(&xml, "LoadPercentage") {
                metrics.push(srv("server.windows.cpu_load_percent", load, "cpu", target).with_unit("percent"));
            }
            if let Some(cores) = Self::extract_xml_value(&xml, "NumberOfCores") {
                metrics.push(srv("server.windows.cpu_cores", cores, "cpu", target));
            }
            if let Some(logical) = Self::extract_xml_value(&xml, "NumberOfLogicalProcessors") {
                metrics.push(srv("server.windows.cpu_logical_processors", logical, "cpu", target));
            }
            if let Some(speed) = Self::extract_xml_value(&xml, "CurrentClockSpeed") {
                metrics.push(srv("server.windows.cpu_clock_mhz", speed, "cpu", target).with_unit("mhz"));
            }
        }

        // ── Memory ────────────────────────────────────────────────────────
        let wql_mem = "SELECT AvailableBytes,CommittedBytes,PercentCommittedMemoryInUse,PageFaultsPersec FROM Win32_PerfFormattedData_PerfOS_Memory";
        if let Some(xml) = Self::winrm_query(&client, host, port, &user, &pass, wql_mem).await {
            if let Some(avail) = Self::extract_xml_value(&xml, "AvailableBytes") {
                metrics.push(srv("server.windows.mem_available_bytes", avail, "memory", target).with_unit("bytes"));
            }
            if let Some(committed) = Self::extract_xml_value(&xml, "CommittedBytes") {
                metrics.push(srv("server.windows.mem_committed_bytes", committed, "memory", target).with_unit("bytes"));
            }
            if let Some(pct) = Self::extract_xml_value(&xml, "PercentCommittedMemoryInUse") {
                metrics.push(srv("server.windows.mem_committed_percent", pct, "memory", target).with_unit("percent"));
            }
            if let Some(pf) = Self::extract_xml_value(&xml, "PageFaultsPersec") {
                metrics.push(srv("server.windows.mem_page_faults_per_sec", pf, "memory", target));
            }
        }

        // ── Disk ──────────────────────────────────────────────────────────
        let wql_disk = "SELECT DeviceID,Size,FreeSpace,FileSystem FROM Win32_LogicalDisk WHERE DriveType=3";
        if let Some(xml) = Self::winrm_query(&client, host, port, &user, &pass, wql_disk).await {
            // Parse multiple disk entries (each wrapped in item elements)
            for item in xml.split("<p:Win32_LogicalDisk>").skip(1) {
                let drive = Self::extract_xml_str(item, "p:DeviceID").unwrap_or("?").to_string();
                if let Some(size) = Self::extract_xml_value(item, "p:Size") {
                    metrics.push(srv("server.windows.disk_size_bytes", size, &drive, target)
                        .with_tag("drive", &drive).with_unit("bytes"));
                }
                if let (Some(size), Some(free)) = (
                    Self::extract_xml_value(item, "p:Size"),
                    Self::extract_xml_value(item, "p:FreeSpace"),
                ) {
                    let used = size - free;
                    let pct = if size > 0.0 { (used / size) * 100.0 } else { 0.0 };
                    metrics.push(srv("server.windows.disk_used_percent", pct, &drive, target)
                        .with_tag("drive", &drive).with_unit("percent"));
                    metrics.push(srv("server.windows.disk_free_bytes", free, &drive, target)
                        .with_tag("drive", &drive).with_unit("bytes"));
                }
            }
        }

        // ── Disk Performance ──────────────────────────────────────────────
        let wql_diskperf = "SELECT Name,AvgDiskQueueLength,DiskReadsPerSec,DiskWritesPerSec,PercentDiskTime FROM Win32_PerfFormattedData_PerfDisk_LogicalDisk";
        if let Some(xml) = Self::winrm_query(&client, host, port, &user, &pass, wql_diskperf).await {
            for item in xml.split("<p:Win32_PerfFormattedData_PerfDisk_LogicalDisk>").skip(1) {
                let name = Self::extract_xml_str(item, "p:Name").unwrap_or("_Total").to_string();
                if name == "_Total" { continue; }

                if let Some(q) = Self::extract_xml_value(item, "p:AvgDiskQueueLength") {
                    metrics.push(srv("server.windows.disk_queue_depth", q, &name, target).with_tag("drive", &name));
                }
                if let Some(r) = Self::extract_xml_value(item, "p:DiskReadsPerSec") {
                    metrics.push(srv("server.windows.disk_reads_per_sec", r, &name, target).with_tag("drive", &name));
                }
                if let Some(w) = Self::extract_xml_value(item, "p:DiskWritesPerSec") {
                    metrics.push(srv("server.windows.disk_writes_per_sec", w, &name, target).with_tag("drive", &name));
                }
                if let Some(pct) = Self::extract_xml_value(item, "p:PercentDiskTime") {
                    metrics.push(srv("server.windows.disk_util_percent", pct, &name, target)
                        .with_tag("drive", &name).with_unit("percent"));
                }
            }
        }

        // ── Network interfaces ────────────────────────────────────────────
        let wql_net = "SELECT Name,BytesReceivedPerSec,BytesSentPerSec,PacketsReceivedErrors,PacketsOutboundErrors FROM Win32_PerfFormattedData_Tcpip_NetworkInterface";
        if let Some(xml) = Self::winrm_query(&client, host, port, &user, &pass, wql_net).await {
            for item in xml.split("<p:Win32_PerfFormattedData_Tcpip_NetworkInterface>").skip(1) {
                let name = Self::extract_xml_str(item, "p:Name").unwrap_or("unknown").to_string();
                if let Some(rx) = Self::extract_xml_value(item, "p:BytesReceivedPerSec") {
                    metrics.push(srv("server.windows.nic_rx_bytes_per_sec", rx, &name, target).with_tag("nic", &name));
                }
                if let Some(tx) = Self::extract_xml_value(item, "p:BytesSentPerSec") {
                    metrics.push(srv("server.windows.nic_tx_bytes_per_sec", tx, &name, target).with_tag("nic", &name));
                }
                if let Some(rx_err) = Self::extract_xml_value(item, "p:PacketsReceivedErrors") {
                    metrics.push(srv("server.windows.nic_rx_errors", rx_err, &name, target).with_tag("nic", &name));
                }
                if let Some(tx_err) = Self::extract_xml_value(item, "p:PacketsOutboundErrors") {
                    metrics.push(srv("server.windows.nic_tx_errors", tx_err, &name, target).with_tag("nic", &name));
                }
            }
        }

        // ── Services health ───────────────────────────────────────────────
        let wql_svc = "SELECT Name,State,StartMode FROM Win32_Service WHERE StartMode='Auto'";
        if let Some(xml) = Self::winrm_query(&client, host, port, &user, &pass, wql_svc).await {
            let mut running = 0.0f64;
            let mut stopped = 0.0f64;
            for item in xml.split("<p:Win32_Service>").skip(1) {
                let state = Self::extract_xml_str(item, "p:State").unwrap_or("Unknown");
                if state == "Running" { running += 1.0; } else { stopped += 1.0; }
            }
            metrics.push(srv("server.windows.services_running", running, "services", target));
            metrics.push(srv("server.windows.services_stopped_auto", stopped, "services", target));
        }

        debug!(target = %target.name, metrics = metrics.len(), "WMI collection complete");
        metrics
    }
}
