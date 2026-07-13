// ============================================================================
// Core types shared across all API domains
// ============================================================================

export interface SystemStatus {
  status: string
  version: string
  uptime: string
  uptime_seconds: number
  targets: number
  alerts: number
  events: number
  hostname: string
  cpu_usage: number
  memory_usage: number
  disk_usage: number
  open_connections: number
  active_sessions: number
  request_rate: number
  error_rate: number
}

export interface Target {
  id: string
  target_id: string
  name: string
  host: string
  target_type: string
  target_subtype: string
  description?: string | null
  environment?: string
  status: string
  check_interval_seconds: number
  labels: Record<string, string>
  tags?: Record<string, string>
  ssh_user: string
  created_at: string
  last_checked: string | null
  is_active: boolean
  ssl_enabled?: boolean
  org_id: string
  port: number
  error: string | null
  agent_id?: string | null
}

export interface ServerStats {
  target_id: string
  host: string
  hostname: string
  os: string
  kernel: string
  cpu_cores: number
  cpu_usage_percent: number
  cpu_model: string
  load_avg_1: number
  load_avg_5: number
  load_avg_15: number
  memory_total_mb: number
  memory_used_mb: number
  memory_free_mb: number
  memory_usage_percent: number
  swap_total_mb: number
  swap_used_mb: number
  disk_total_gb: number
  disk_used_gb: number
  disk_free_gb: number
  disk_usage_percent: number
  uptime_seconds: number
  uptime_human: string
  network_rx_bytes: number
  network_tx_bytes: number
  process_count: number
  logged_in_users: number
  top_cpu_processes: { pid: number; name: string; cpu: number; memory: number }[]
  disks: { filesystem: string; mount: string; total_gb: number; used_gb: number; free_gb: number; usage_percent: number; fs_type: string }[]
  nfs_mounts: { remote: string; mount: string; total_gb: number; used_gb: number; free_gb: number; usage_percent: number }[]
  network_interfaces: { name: string; rx_bytes: number; tx_bytes: number; rx_packets: number; tx_packets: number }[]
  collected_at: string
  reachable?: boolean
  error?: string | null
}

export interface MetricPoint {
  timestamp: string
  target_id: string
  metric_name: string
  value: number
  labels: Record<string, string>
}

export interface Alert {
  id: string
  alert_id: string
  target_id: string
  target_name: string
  rule_id: string
  severity: string
  message: string
  status: string
  metric_value: number | null
  threshold: number | null
  created_at: string
  triggered_at: string | null
  acknowledged_at: string | null
  resolved_at: string | null
}

export interface AlertRule {
  id: string
  name: string
  metric_name: string
  operator: string
  threshold: number
  severity: string
  duration_secs: number
  enabled: boolean
  description: string
  labels: Record<string, string>
  annotations: Record<string, string>
}

export interface LogEntry {
  id: string
  timestamp: string
  source: string
  level: string
  message: string
  target_id: string
}

export interface AppEvent {
  id: string
  event_type: string
  source: string
  target_id: string | null
  severity: string
  message: string
  payload: string | null
  data: Record<string, unknown>
  timestamp: string
}

export interface EventStats {
  total_published: number
  subscribers: number
  events_per_second: number
  metrics_collected: number
  threats_detected: number
  logs_ingested: number
  by_type: Record<string, number>
  by_severity: Record<string, number>
}

export interface DrilldownResult {
  metric: string
  target_id: string
  data: MetricPoint[]
  anomalies: unknown[]
  correlations: unknown[]
}

export interface SearchResult {
  result_type: string
  id: string
  title: string
  subtitle: string
  url: string
  score: number
}

// Network types (sniffnet-inspired)
export interface NetworkCapture {
  is_running: boolean
  interface_name: string
  started_at: string | null
  total_packets: number
  packets_total: number
  total_bytes: number
  unique_hosts: number
  packets_per_second: number
  bytes_per_second: number
  tcp_count: number
  udp_count: number
  icmp_count: number
  arp_count: number
  protocols: { name: string; count: number; protocol?: string; packets?: number; percentage?: number }[]
  top_connections: NetworkConnection[]
}

export interface NetworkConnection {
  src_ip: string
  src_port: number
  dst_ip: string
  dst_port: number
  protocol: string
  bytes_sent: number
  bytes_recv: number
  packets_sent: number
  packets_recv: number
  service: string
  country_code: string
  is_favorite: boolean
  is_blacklisted: boolean
}

export interface NetworkHost {
  ip: string
  hostname: string | null
  country_code: string
  asn: string | null
  total_bytes: number
  total_packets: number
  connections: number
  services: string[]
  first_seen: string
  last_seen: string
  is_favorite: boolean
  is_blacklisted: boolean
}

export interface TrafficDataPoint {
  timestamp: string
  bytes_in: number
  bytes_out: number
  packets_in: number
  packets_out: number
}

// S4: Real capture stats types
export interface NetworkStats {
  total_bytes_in: number
  total_bytes_out: number
  total_packets_in: number
  total_packets_out: number
  unique_src_hosts: number
  unique_dst_hosts: number
  active_connections: number
  top_protocols: { name: string; bytes: number; packets: number }[]
  top_services: { name: string; bytes: number; connections: number }[]
  top_countries: { code: string; name: string; bytes: number }[]
  capture_duration_secs: number
  bps_in: number
  bps_out: number
}

export interface PaginatedConnections {
  connections: NetworkConnection[]
  total: number
  page: number
  page_size: number
  total_pages: number
}

// S5: PCAP types
export interface PcapExportStatus {
  exporting: boolean
  file_name: string
  started_at: string | null
}

export interface PcapImportResult {
  imported: boolean
  packet_count: number
  file_name: string
  duration_secs: number
  mode: string
}

// S7: Favorites types
export interface NetworkFavoriteItem {
  id: string
  favorite_type: string
  value: Record<string, unknown>
  created_at: string
}

// S8: Blacklist types
export interface BlacklistEntryType {
  id: string
  ip: string
  reason: string | null
  source: string
  created_at: string
}

// S9: Network Notification types
export interface NotificationRule {
  id: string
  rule_type: string
  config: Record<string, unknown>
  is_active: boolean
  created_at: string
}

export interface NotificationLogEntry {
  id: string
  rule_id: string
  event_type: string
  details: Record<string, unknown>
  timestamp: string
}

// Deep analysis types (Sniffnet-inspired)
export interface DeepTrafficSnapshot {
  total_packets: number
  total_bytes: number
  protocol_counts: Record<string, number>
  service_counts: Record<string, number>
  icmp_type_counts: Record<string, number>
  top_talkers: DeepHostEntry[]
  threat_hits: number
  reserved_hits: number
  unique_hosts: number
  unique_connections: number
  snapshot_epoch_ms: number
}

export interface DeepHostEntry {
  ip: string
  rdns_hostname: string
  asn: string
  country_code: string
  country_name: string
  service: string
  inbound_packets: number
  outbound_packets: number
  inbound_bytes: number
  outbound_bytes: number
  is_threat_listed: boolean
  is_reserved: boolean
  is_favorite: boolean
  active_connections: number
  first_seen: number
  last_seen: number
}

export interface ServiceEntry {
  service: string
  count: number
  percentage: number
}

export interface IcmpEntry {
  icmp_type: string
  count: number
}

export interface ThreatListStatus {
  loaded: boolean
  entry_count: number
  source: string
  hits_total: number
}

// AI / Chat types
export interface ChatMessage {
  id: string
  role: 'user' | 'assistant'
  content: string
  timestamp: string
  tool_used?: string
  tool_result?: Record<string, unknown>
}

export interface ChatResponse {
  text: string
  intent: string
  entities: Record<string, string>
  tool_used?: string
  tool_result?: Record<string, unknown>
  timestamp: string
}

export interface AIInsight {
  id: string
  category: string
  title: string
  description: string
  severity: string
  confidence: number
  metric_data?: Record<string, number>
  recommendation: string
  created_at: string
}

// RCA types
export interface RCAResult {
  id: string
  source: string
  total_errors: number
  unique_errors: number
  errors: RCAError[]
  analyzed_at: string
}

export interface RCAError {
  message: string
  count: number
  severity: string
  pattern: string
  solutions: RCASolution[]
}

export interface RCASolution {
  title: string
  description: string
  source: string
  url: string
  confidence: number
}

// Log Source types
export interface LogSource {
  id: string
  name: string
  source_type: string
  host: string
  port: number
  path: string
  status: 'active' | 'error' | 'paused'
  last_ingested: string | null
  log_count: number
  created_at: string
}

// Predictions / Predictive Alerting Engine
export interface Forecast {
  forecast_id: string
  metric_name: string
  target_id: string
  resource_id: string
  current_value: number
  predicted_value: number
  threshold: number
  hours_to_threshold: number | null
  rate_per_hour: number
  confidence: number
  severity: string
  description: string
  created_at: string
}

export interface AnomalyEvent {
  anomaly_id: string
  metric_name: string
  target_id: string
  resource_id: string
  value: number
  baseline_mean: number
  baseline_stddev: number
  z_score: number
  severity: string
  direction: string
  description: string
  detected_at: string
}

export interface TrendAlert {
  alert_id: string
  alert_type: string
  severity: string
  title: string
  description: string
  recommendation: string
  metric_name: string
  target_id: string
  created_at: string
}

export interface PredictionSummary {
  active_anomalies: number
  active_forecasts: number
  active_trend_alerts: number
  models_trained: number
  metrics_monitored: number
  last_evaluation: string
}

// Annotations
export interface Annotation {
  annotation_id: string
  org_id: string
  title: string
  text: string
  annotation_type: string
  tags: string[]
  target_id: string | null
  dashboard_id: string | null
  panel_id: string | null
  epoch_start: number
  epoch_end: number | null
  created_by: string | null
  created_at: string
  updated_at: string
}

// Scrape Targets (Prometheus-compatible)
export interface ScrapeTarget {
  target_id: string
  org_id: string
  host: string
  port: number
  scheme: string
  metrics_path: string
  job_name: string | null
  scrape_interval_seconds: number
  enabled: boolean
  discovered_by: string
  health: string
  sample_count: number
  scrape_duration_ms: number | null
  last_error: string | null
  last_scrape: string | null
}

export interface ScrapeHealthSummary {
  total_targets: number
  targets_up: number
  targets_down: number
  targets_unknown: number
  avg_scrape_duration_ms: number
  total_samples_last_scrape: number
}

export interface ScrapeRecordingRule {
  rule_id: string
  name: string
  source_metric: string
  output_metric: string
  aggregation: string
  group_by: string[]
  enabled: boolean
}

// Dashboards
export interface DashboardPanel {
  panel_id: string
  title: string
  panel_type: string
  metric_query: string
  width: number
  height: number
  position_x: number
  position_y: number
  options: Record<string, unknown>
}

export interface TemplateVariable {
  name: string
  label: string | null
  variable_type: string
  query: string | null
  options: string[]
  current_value: string | null
  multi: boolean
  include_all: boolean
}

export interface Dashboard {
  dashboard_id: string
  org_id: string
  name: string
  description: string | null
  panels: DashboardPanel[]
  variables: TemplateVariable[]
  version: number
  created_by: string | null
  created_at: string
  updated_at: string
}

export interface DashboardExport {
  Zentra_version: string
  export_format: string
  dashboard: Dashboard
}

// License Management
export interface License {
  license_id: string
  org_id: string
  name: string
  vendor: string
  product: string
  serial_number: string | null
  license_type: string
  total_seats: number
  assigned_seats: number
  available_seats: number
  cost_per_seat: number | null
  purchase_date: string | null
  expiration_date: string | null
  status: string
  notes: string | null
  created_at: string
  updated_at: string
}

export interface LicenseCompliance {
  total_licenses: number
  compliant: number
  over_deployed: number
  expiring_soon: number
  expired: number
  total_cost: number
  compliance_rate: number
}

// Notification Types
export interface NotificationChannel {
  id: string
  type: string
  name: string
  enabled: boolean
  min_severity: string
  settings: Record<string, string>
}

export interface NotificationHistoryItem {
  id: string
  channel_type: string
  channel_name: string
  notification_title: string
  severity: string
  status: string
  response_code: number | null
  dispatched_at: string
}

// MCP Types
export interface McpToolDef {
  name: string
  description: string
  category: string
  parameters: { name: string; param_type: string; required: boolean; description: string }[]
}

export interface McpToolCallResponse {
  tool: string
  success: boolean
  data: Record<string, unknown>
  summary: string
  timestamp: string
  execution_ms: number
}

export interface McpChatResponse {
  text: string
  mode_used: string
  tool_calls: McpToolCallResponse[]
  suggestions: string[]
  timestamp: string
}

// Playlists + Bookmarks
export interface Playlist {
  playlist_id: string; name: string; interval_seconds: number
  dashboard_ids: string[]; current_index: number; created_at: string
}

// Collectors
export interface CollectorInfo {
  collector_id: string; name: string; collector_type: string; status: string
  host: string | null; port: number | null; enabled: boolean; metric_count: number
  last_collection: string | null; last_error: string | null
  config: Record<string, unknown>; created_at: string
}

export interface CollectorChart {
  chart_id: string; title: string; chart_type: string; metrics: string[]; unit: string
}

// Traces
export interface TraceSummary {
  trace_id: string; root_service: string; root_operation: string
  total_duration_us: number; span_count: number; has_errors: boolean
  services: string[]; started_at: string
}

export interface TraceSpan {
  trace_id: string; span_id: string; parent_span_id: string | null
  operation_name: string; service_name: string; start_time: string
  duration_us: number; status: string; tags: Record<string, string>
  events: { name: string; timestamp: string; attributes: Record<string, string> }[]
}

// Alert Grouping + Inhibition
export interface AlertGroupData {
  group_key: string; labels: Record<string, unknown>
  alerts: unknown[]; count: number; most_recent: string | null
}

export interface InhibitionRule {
  id: string; name: string; source_match: Record<string, string>
  target_match: Record<string, string>; equal_labels: string[]
  enabled: boolean; created_at: string
}

// Mute Timings
export interface MuteTiming {
  id: string; name: string; time_intervals: TimeInterval[]
  created_at: string; updated_at: string
}

export interface TimeInterval {
  weekdays?: number[]; months?: number[]; days_of_month?: number[]
  start_time?: string; end_time?: string; location?: string
}

export interface BacktestResult {
  metric_name: string; threshold: number; operator: string
  total_points: number; violations: { timestamp: string; value: number; would_fire: boolean }[]
  violation_count: number; violation_pct: number
  evaluated_from: string; evaluated_to: string
}

// Query History
export interface QueryHistoryEntry {
  query_id: string; user_id: string; datasource_type: string
  query_text: string; starred: boolean; comment: string | null
  execution_time_ms: number | null; created_at: string
}

export interface FrequentQuery {
  query_text: string; datasource_type: string
  execution_count: number; last_run: string
}

// ML / K-means
export interface AnomalyScore {
  metric_name: string; host: string; anomaly_score: number; is_anomalous: boolean
  cluster_id: number; distance_from_centroid: number; timestamp: string
}
export interface MLModelStatus {
  model_name: string; status: string; last_trained: string; training_window: string
  num_clusters: number; total_metrics_tracked: number; anomaly_threshold: number
  metrics_above_threshold: number; host_anomaly_rate: number
}
export interface ClusterInfo { cluster_id: number; centroid: number[]; member_count: number; avg_distance: number; label: string }

// Service Discovery
export interface DiscoveryProvider {
  id: string; name: string; provider_type: string; config: Record<string, unknown>
  enabled: boolean; last_sync: string; targets_discovered: number; status: string; error: string | null
}
export interface DiscoveredTarget {
  id: string; provider_id: string; address: string; labels: Record<string, string>
  health: string; last_scrape: string; scrape_duration_ms: number; discovered_at: string
}

// Network Interfaces / Protocol / Sockets
export interface NetworkInterface {
  name: string; ip_address: string; mac_address: string; speed_mbps: number
  mtu: number; state: string; rx_bytes: number; tx_bytes: number
  rx_packets: number; tx_packets: number; rx_errors: number; tx_errors: number
  rx_dropped: number; tx_dropped: number
}
export interface ProtocolStat {
  protocol: string; connections: number; bytes_in: number; bytes_out: number
  packets_in: number; packets_out: number; errors: number; retransmits: number; rtt_avg_ms: number
}
export interface SocketStats { tcp_states: { state: string; count: number; recv_q: number; send_q: number }[]; summary: Record<string, number> }

// Container Awareness
export interface ContainerNetInfo {
  container_id: string; container_name: string; image: string; namespace: string
  ip_address: string; connections: number; rx_bytes: number; tx_bytes: number; state: string
}

// Monitoring Templates
export interface TemplateMacro { key: string; default: string; description: string }
export interface TemplateCheck { name: string; check_type: string; config: Record<string, any>; interval_secs: number; warning_threshold: string | null; critical_threshold: string | null }
export interface TemplateAlert { name: string; expression: string; severity: string; message: string }

export interface MonitoringTemplate {
  id: string; name: string; category: string; description: string; version: string
  parent_template: string | null; macros: TemplateMacro[]; checks: TemplateCheck[]
  alerts: TemplateAlert[]; metrics: string[]; dashboards: string[]
  created_at: string; updated_at: string
}

export interface TemplateAssignment {
  id: string; template_id: string; template_name: string
  target_id: string; target_name: string
  macro_overrides: Record<string, any>; assigned_at: string
}

// Health Checks
export interface HealthCheck {
  id: string; name: string; check_type: string; target_host: string
  config: Record<string, any>; interval_secs: number; timeout_secs: number
  warning_threshold: string | null; critical_threshold: string | null
  enabled: boolean; group: string | null; parent_check_id: string | null
  created_at: string; updated_at: string
}

export interface HealthCheckResult {
  id: string; check_id: string; status: string; response_time_ms: number
  output: string; details: Record<string, any>; checked_at: string
}

export interface HealthCheckWithStatus {
  check: HealthCheck; last_result: HealthCheckResult | null
  uptime_24h: number; uptime_7d: number; uptime_30d: number
  avg_response_ms: number; is_flapping: boolean; state_since: string
}

export interface HealthCheckSummary {
  total_checks: number; ok: number; warning: number; critical: number; unknown: number
  checks: HealthCheckWithStatus[]
}

export interface MatrixCell {
  check_id: string; check_name: string; check_type: string; status: string; response_time_ms: number
}
export interface MatrixRow { host: string; checks: MatrixCell[] }

export interface UptimeStat {
  check_id: string; check_name: string; target_host: string
  uptime_24h: number; uptime_7d: number; uptime_30d: number
  avg_response_ms: number; total_checks_24h: number; failed_checks_24h: number
}

// Maintenance Windows
export interface MaintenanceWindow {
  id: string; name: string; scope_type: string; scope_targets: string[]
  schedule_type: string; start_time: string; end_time: string
  recurrence: string | null; suppress_alerts: boolean; created_by: string
  created_at: string; is_active: boolean
}

// SLA Reports
export interface SLAReport {
  check_id: string; check_name: string; target_host: string; sla_target_pct: number
  uptime_24h: number; uptime_7d: number; uptime_30d: number; uptime_90d: number
  total_outages_30d: number; total_downtime_mins_30d: number; longest_outage_mins: number
  compliance: boolean; outages: { started_at: string; ended_at: string | null; duration_mins: number; reason: string }[]
}
export interface SLASummary { total_checks: number; compliant: number; violated: number; avg_uptime_30d: number; worst_check: string | null; worst_uptime: number }

// Dependencies + Flapping
export interface HostDependency { id: string; parent_target_id: string; child_target_id: string; dependency_type: string; description: string; created_at: string }
export interface DependencyTreeNode { target_id: string; target_name: string; status: string; children: DependencyTreeNode[] }
export interface FlappingCheck { check_id: string; check_name: string; target_host: string; transition_count: number; window_minutes: number; current_status: string; acknowledged: boolean; flapping_since: string }

// Middleware
export interface MiddlewareServiceStatus { service_type: string; name: string; host: string; port: number; status: string; key_metric_1: string; key_metric_2: string; last_checked: string }

// Deployments
export interface Deployment { id: string; name: string; version: string; environment: string; deployed_by: string; status: string; started_at: string; completed_at: string | null; duration_secs: number | null; target_ids: string[]; checklist: any[]; pre_deploy_results: any[]; post_deploy_results: any[]; rollback_triggered: boolean; notes: string; runbook_id: string | null }
export interface DeploymentRunbook { id: string; name: string; content_markdown: string; linked_environment: string | null; created_at: string; updated_at: string }

// Log Engine
export interface LogQueryResult { streams: { labels: Record<string, string>; entries: { timestamp: string; line: string; fields?: Record<string, string> }[] }[]; total_entries: number }
export interface LogStreamInfo { labels: Record<string, string>; fingerprint: string; entry_count: number; bytes: number; last_entry_at: number | null }
export interface LogAlert { id: string; name: string; query: string; threshold: number; comparison: string; severity: string; enabled: boolean; last_fired: string | null; created_at: string }
export interface LogAnalyticsData { total_streams: number; total_entries: number; total_bytes: number; ingestion_rate_lines_per_sec: number; ingestion_rate_bytes_per_sec: number; volume_by_source: { source: string; lines: number; bytes: number }[]; top_error_messages: { message: string; count: number; last_seen: string }[]; error_rate_trend: { timestamp: string; count: number; bytes: number }[] }

// Discovery Rules
export interface DiscoveryRule { id: string; name: string; rule_type: string; target_id: string; interval_secs: number; enabled: boolean; config: any; auto_create_checks: boolean; template_id: string | null; last_run: string | null; discovered_items: DiscoveredItem[] }
export interface DiscoveredItem { key: string; name: string; item_type: string; properties: any; discovered_at: string; check_id: string | null }

// Auth types
export interface LoginRequest { email: string; password: string; mfa_code?: string }
export interface LoginResponse {
  token: string
  refresh_token: string
  user: UserSummary
  expires_at: string
  mfa_required: boolean
}
export interface UserSummary {
  id: string
  email: string
  username: string
  display_name: string | null
  avatar_url: string | null
  is_active: boolean
  is_locked: boolean
  last_login: string | null
  roles: string[]
}
export interface UserProfile extends UserSummary {
  org_id: string
  timezone: string
  theme: string
  mfa_enabled: boolean
  password_changed_at: string | null
  created_at: string
  updated_at: string
  permissions: { resource: string; action: string }[]
  module_access: string[]
  menu_items: string[]
  is_admin: boolean
  is_super_admin: boolean
}
export interface UserSession { id: string; ip_address: string | null; user_agent: string | null; expires_at: string; created_at: string }
export interface MfaSetupResponse { secret: string; qr_uri: string }

// User Management
export interface UserListResponse { users: UserListItem[]; total: number; limit: number; offset: number }
export interface UserListItem {
  id: string; org_id: string; email: string; username: string
  display_name: string | null; avatar_url: string | null
  is_active: boolean; is_locked: boolean
  last_login: string | null; created_at: string; roles: string[]
}
export interface UserDetail extends UserListItem {
  timezone: string; theme: string; mfa_enabled: boolean
  password_changed_at: string | null; updated_at: string
  roles: any[]; groups: any[]
}
export interface CreateUserReq { email: string; username: string; password: string; display_name?: string; role_ids?: string[]; org_id?: string }
export interface UpdateUserReq { email?: string; username?: string; display_name?: string; avatar_url?: string; timezone?: string; theme?: string; is_active?: boolean }
export interface BulkUserActionReq { action: string; user_ids: string[] }

// User Groups
export interface UserGroupItem { id: string; org_id: string; name: string; description: string | null; member_count: number; created_at: string; updated_at: string }
export interface UserGroupDetail extends UserGroupItem { members: { id: string; email: string; username: string; display_name: string | null }[] }

// Roles & Permissions
export interface RoleItem { role_id: string; org_id: string; name: string; description: string | null; is_system: boolean; permission_count: number; created_at: string }
export interface RoleDetail extends RoleItem { permissions: PermissionItem[] }
export interface PermissionItem { permission_id: string; resource: string; action: string; description: string | null }

// Audit Log
export interface AuditLogResponse { entries: AuditEntry[]; total: number; limit: number; offset: number }
export interface AuditEntry {
  log_id: string; org_id: string; user_id: string | null; action: string
  resource_type: string; resource_id: string | null; details: any
  ip_address: string | null; user_agent: string | null; status: string; timestamp: string
}
export interface AuditStats { total_entries: number; actions_today: number; top_users: any[]; top_resources: any[] }

// API Tokens
export interface ApiTokenItem { id: string; name: string; key_prefix: string; enabled: boolean; expires_at: string | null; last_used_at: string | null; created_at: string }
export interface CreateTokenResponse { token: ApiTokenItem; raw_key: string; message: string }

// System Settings
export interface SystemSettingItem { key: string; value: any; description: string | null; updated_at: string }

// IDS/IPS
export interface IdsRuleItem {
  id: string; sid: number; category: string; severity: string
  message: string; rule_text: string; enabled: boolean; source: string; created_at: string
}
export interface IdsAlertItem {
  id: string; rule_id: string | null; timestamp: string
  src_ip: string | null; src_port: number | null; dest_ip: string | null; dest_port: number | null
  protocol: string | null; payload: string | null; severity: string; status: string; community_id: string | null
}
export interface IdsStats {
  total_rules: number; enabled_rules: number; total_alerts: number; alerts_24h: number
  severity_distribution: { severity: string; count: number }[]
  top_categories: { category: string; count: number }[]
  top_sources: { ip: string; count: number }[]
}

// Network Discovery
export interface DiscoveredDevice {
  ip: string
  hostname: string | null
  mac_address: string | null
  vendor: string | null
  device_type: string
  open_ports: number[]
  os_guess: string | null
  first_seen: string
  last_seen: string
  snmp_community: string | null
}

// Oracle
export interface OracleConnection {
  id: string
  name: string
  host: string
  port: number
  service_name: string
  username: string
  auth_mode: string
  role: string
  tns_admin_path: string | null
  pool_min: number
  pool_max: number
  pool_increment: number
  conn_max_lifetime_secs: number
  query_timeout_secs: number
  labels: Record<string, string>
  is_active: boolean
  status: string
  last_ping: string | null
  last_error: string | null
  created_at: string
  updated_at: string
}

// Cloud Monitoring
export interface CloudAccount {
  id: string
  name: string
  provider: string
  account_id: string | null
  region: string | null
  status: string
  last_sync: string | null
  resource_count: number | null
  created_at: string
}

// NetFlow
export interface FlowSummaryRow {
  src_ip: string
  dst_ip: string
  protocol_name: string | null
  total_bytes: number
  total_packets: number
  flow_count: number
}

export interface FlowExporter {
  ip: string
  hostname: string | null
  flow_type: string
  last_seen: string
  packets_rx: number
  flows_rx: number
}

// ML Anomaly Detection
export interface AnomalyRecord {
  id: string
  detected_at: string
  target_id: string | null
  target_name: string | null
  metric_name: string
  value: number
  expected_mean: number | null
  expected_stddev: number | null
  deviation_sigma: number | null
  detector: string
  severity: string
  acknowledged: boolean
  false_positive: boolean
}

// Runbooks
export interface RunbookStep {
  id: string; name: string; step_type: string; command?: string; url?: string
  method?: string; body?: string; wait_seconds?: number; timeout_seconds: number
  on_failure: string; description?: string
}
export interface Runbook {
  id: string; name: string; description: string; trigger_type: string
  trigger_conditions: string[]; category: string; severity: string
  steps: RunbookStep[]; timeout_minutes: number; requires_approval: boolean
  enabled: boolean; created_at: string; updated_at: string
  last_run?: string; run_count: number; success_count: number
}
export interface RunbookExecution {
  id: string; runbook_id: string; runbook_name: string; trigger: string
  target?: string; status: string; started_at: string; completed_at?: string
  step_results: any[]; summary?: string; approved_by?: string
}

// ITSM
export interface ItsmConfig {
  id: string; name: string; provider: string; enabled: boolean; url: string
  username?: string; api_key_masked?: string; project_key?: string; instance?: string
  routing_key?: string; field_mappings: any; created_at: string; last_sync?: string
  sync_status: string; tickets_created: number
}
export interface ItsmTicket {
  id: string; provider: string; external_id: string; external_url: string
  title: string; description: string; status: string; priority: string
  assignee?: string; alert_id?: string; created_at: string; updated_at: string; resolved_at?: string
}

// SSO
export interface SsoProvider {
  id: string; name: string; provider_type: string; enabled: boolean; icon?: string
  client_id?: string; client_secret_masked?: string
  authorization_url?: string; token_url?: string; userinfo_url?: string; jwks_url?: string
  scopes: string[]; entity_id?: string; sso_url?: string; slo_url?: string
  certificate_fingerprint?: string; metadata_url?: string
  ldap_host?: string; ldap_port?: number; ldap_base_dn?: string; ldap_bind_dn?: string
  ldap_user_filter?: string; ldap_group_filter?: string; ldap_tls: boolean
  attr_email: string; attr_name: string; attr_groups?: string
  group_sync_enabled: boolean; default_role: string; auto_provision: boolean
  created_at: string; last_auth?: string; logins_count: number
}
export interface SsoSession {
  id: string; provider_id: string; provider_name: string; user_email: string
  user_name: string; created_at: string; expires_at: string; ip_address: string; user_agent: string
}

// Compliance
export interface ComplianceFramework {
  id: string; name: string; version: string; category: string; total_controls: number; enabled: boolean; description: string
}
export interface ComplianceScan {
  id: string; framework_id: string; framework_name: string; target_id: string; target_name: string
  started_at: string; completed_at?: string; status: string
  pass_count: number; fail_count: number; warn_count: number; skip_count: number
  score: number; duration_secs?: number
}
export interface ScanFinding {
  id: string; scan_id: string; control_id: string; control_title: string; section: string
  status: string; severity: string; details: string; remediation: string; evidence?: string
}

// Organizations / Tenants
export interface Organization {
  org_id: string; name: string; slug: string; plan: string; status: string; owner_email: string
  created_at: string; updated_at: string; trial_ends_at?: string
  max_targets: number; max_users: number; max_dashboards: number; max_alert_rules: number
  retention_days: number; current_targets: number; current_users: number
  features: Record<string, any>; custom_branding: boolean; sso_enabled: boolean
  api_rate_limit: number; region: string; notes?: string
}
export type Tenant = Organization
export interface TenantUsage {
  org_id: string; org_name: string; targets_used: number; targets_limit: number
  users_used: number; users_limit: number; alerts_fired_30d: number
}

// HA
export interface ClusterNode {
  id: string; hostname: string; role: string; status: string; region: string; zone: string
  version: string; uptime_secs: number; last_heartbeat: string; cpu_usage: number
  memory_usage: number; connections: number; metrics_per_sec: number; logs_per_sec: number; is_current: boolean
}
export interface HaClusterStatus {
  cluster_id: string; cluster_name: string; health: string; leader_id: string; leader_hostname: string
  nodes: ClusterNode[]; quorum_size: number; quorum_met: boolean; total_nodes: number; healthy_nodes: number
  term: number; commit_index: number; last_applied: number; election_timeout_ms: number
  heartbeat_interval_ms: number; split_brain_detected: boolean; failovers_last_30d: number; last_failover?: string
}

// On-Call
export interface OncallMember {
  user_id: string; user_name: string; email: string; phone?: string; rotation_index: number
}
export interface TimeRestriction {
  start_day: string; end_day: string; start_time: string; end_time: string
}
export interface OncallLayer {
  id: string; name: string; order: number; members: OncallMember[]; restriction?: TimeRestriction
}
export interface OncallSchedule {
  id: string; name: string; rotation_type: string; shift_duration_hours: number
  handoff_time: string; timezone: string; enabled: boolean; team: string
  description?: string; created_at: string; layers: OncallLayer[]
}
export interface OncallShift {
  id: string; schedule_id: string; layer_id: string; user_id: string; user_name: string
  email: string; start_time: string; end_time: string; is_override: boolean; override_reason?: string
}
export interface CurrentOncall {
  schedule_id: string; schedule_name: string; primary: OncallMember; secondary?: OncallMember
  current_shift_start: string; current_shift_end: string; next_handoff: string
}

// Storage
export interface StorageArray {
  id: string; name: string; array_type: string; vendor: string; model: string; firmware: string
  ip: string; status: string; total_tb: number; used_tb: number; free_tb: number; utilization_pct: number
  iops: number; throughput_mb: number; latency_ms: number; data_reduction_ratio: number
  snapshot_count: number; volume_count: number; protocol: string
  replication_enabled: boolean; replication_lag_secs?: number; last_polled: string
  location: string; tags: string[]
}
export interface StorageVolume {
  id: string; array_id: string; name: string; size_tb: number; used_tb: number
  protocol: string; status: string; iops: number; latency_ms: number; snapshot_count: number; hosts: string[]
}
export interface StorageReplication {
  id: string; source_array_id: string; target_array_id: string; relationship_type: string
  status: string; lag_secs: number; last_transfer_size_gb: number; last_transfer_duration_secs: number
  transfer_rate_mb: number; last_updated: string
}
export interface StorageMetricPoint {
  timestamp: string; total_tb: number; used_tb: number; free_tb: number; utilization_pct: number
  iops: number; throughput_mb: number; latency_ms: number; data_reduction_ratio: number
  snapshot_count: number; volume_count: number; overall_health?: string; alert_count: number
}
export interface StorageMetricsHistoryResponse {
  target_id: string; from: string; to: string; rollup: string; count: number
  data: StorageMetricPoint[]
}
export interface StorageMetricsLatest {
  target_id: string; timestamp: string; total_tb: number; used_tb: number; free_tb: number
  utilization_pct: number; iops: number; throughput_mb: number; latency_ms: number
  data_reduction_ratio: number; snapshot_count: number; volume_count: number
  overall_health?: string; alert_count: number; raw_extends?: Record<string, string>
}

// Unified Target Metrics History (all component types)
export interface TargetMetricPoint {
  timestamp: string; metric_name: string; value: number; unit: string
}
export interface TargetMetricsHistoryResponse {
  target_id: string; from: string; to: string; count: number
  data: TargetMetricPoint[]
}
export interface TargetMetricsLatestResponse {
  target_id: string; metric_count: number
  metrics: Record<string, { value: number; unit: string; timestamp: string }>
}
export interface MetricsCatalogResponse {
  resource_types: string[]
  catalog: Record<string, { metric_name: string; unit: string; sample_count: number }[]>
}

