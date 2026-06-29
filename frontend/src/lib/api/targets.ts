import { fetchJSON } from './_client'
import type {
  Target, ServerStats, ScrapeTarget, ScrapeHealthSummary, ScrapeRecordingRule,
  DiscoveryProvider, DiscoveredTarget, CollectorInfo, CollectorChart,
  DiscoveryRule, DiscoveredItem,
  TargetMetricsHistoryResponse, TargetMetricsLatestResponse, MetricsCatalogResponse,
} from './types'

export interface CreateTargetInput {
  name: string
  target_type?: string
  target_subtype: string
  host: string
  port?: number
  description?: string | null
  environment?: string
  ssl_enabled?: boolean
  is_active?: boolean
  username?: string
  password?: string
  database_name?: string
  check_interval_seconds?: number
  tags?: Record<string, string>
  agent_id?: string
}

// Targets
export const getTargets = () => fetchJSON<Target[]>('/targets')
export const createTarget = (data: CreateTargetInput) =>
  fetchJSON<Target>('/targets', { method: 'POST', body: JSON.stringify(data) })
export const deleteTarget = (id: string) =>
  fetchJSON<void>(`/targets/${id}`, { method: 'DELETE' })
export const verifySsh = (host: string, username?: string) =>
  fetchJSON<{ reachable: boolean; host: string; hostname: string; message: string }>('/targets/verify-ssh', {
    method: 'POST',
    body: JSON.stringify({ host, username }),
  })
export const setupSsh = (host: string, username: string, password: string) =>
  fetchJSON<{ success: boolean; message: string; host: string; username: string }>('/targets/setup-ssh', {
    method: 'POST',
    body: JSON.stringify({ host, username, password }),
  })
export const testConnection = (id: string) =>
  fetchJSON<{ success: boolean; message: string }>(`/targets/${id}/test`, { method: 'POST' })
export const testDbConnection = (data: {
  host: string; port: number; target_subtype?: string;
  username?: string; password?: string; database_name?: string;
}) =>
  fetchJSON<{ reachable: boolean; latency_ms: number; error?: string; detail?: string }>(
    '/targets/test-db-connection',
    { method: 'POST', body: JSON.stringify(data) },
  )
export const getTargetStats = (id: string) =>
  fetchJSON<ServerStats>(`/targets/${id}/stats`)

export interface CrawledComponent {
  component_type: string
  subtype: string
  name: string
  port: number
  version: string
  status: string
  detail: string
}
export interface CrawlResult {
  target_id: string
  host: string
  hostname: string
  components: CrawledComponent[]
  crawl_duration_ms: number
}
export const crawlServer = (id: string) =>
  fetchJSON<CrawlResult>(`/targets/${id}/crawl`, { method: 'POST' })

// Unified Target Metrics History (all component types)
export const getTargetMetricsHistory = (id: string, params?: {
  from?: string; to?: string; metric?: string; limit?: number
}) => {
  const qs = new URLSearchParams(
    Object.entries(params ?? {}).filter(([, v]) => v !== undefined).map(([k, v]) => [k, String(v)])
  ).toString()
  return fetchJSON<TargetMetricsHistoryResponse>(`/targets/${id}/metrics/history${qs ? '?' + qs : ''}`)
}
export const getTargetMetricsLatest = (id: string) =>
  fetchJSON<TargetMetricsLatestResponse>(`/targets/${id}/metrics/latest`)
export const getMetricsCatalog = () =>
  fetchJSON<MetricsCatalogResponse>('/metrics/catalog')

// Scrape Targets (Prometheus-compatible)
export const getScrapeTargets = () => fetchJSON<ScrapeTarget[]>('/scrape/targets')
export const createScrapeTarget = (data: Partial<ScrapeTarget>) =>
  fetchJSON<ScrapeTarget>('/scrape/targets', { method: 'POST', body: JSON.stringify(data) })
export const deleteScrapeTarget = (id: string) =>
  fetchJSON<void>(`/scrape/targets/${id}`, { method: 'DELETE' })
export const getScrapeHealth = () => fetchJSON<ScrapeHealthSummary>('/scrape/health')
export const getScrapeRecordingRules = () => fetchJSON<ScrapeRecordingRule[]>('/scrape/recording-rules')

// P4: Service Discovery
export const getDiscoveryProviders = () => fetchJSON<DiscoveryProvider[]>('/service-discovery/providers')
export const getDiscoveredTargets = () => fetchJSON<DiscoveredTarget[]>('/service-discovery/targets')
export const createDiscoveryProvider = (data: { name: string; provider_type: string; config: Record<string, unknown> }) =>
  fetchJSON<any>('/service-discovery/providers', { method: 'POST', body: JSON.stringify(data) })
export const deleteDiscoveryProvider = (id: string) =>
  fetchJSON<any>(`/service-discovery/providers/${id}`, { method: 'DELETE' })
export const syncDiscoveryProvider = (id: string) =>
  fetchJSON<any>(`/service-discovery/providers/${id}/sync`, { method: 'POST' })

// P14: Collectors API
export const getCollectors = () => fetchJSON<{ collectors: CollectorInfo[]; total: number; running: number; total_metrics: number }>('/collectors')
export const getCollectorMetrics = (name: string) => fetchJSON<{ metrics: Record<string, unknown> }>(`/collectors/${name}/metrics`)
export const getCollectorCharts = (name: string) => fetchJSON<{ charts: CollectorChart[] }>(`/collectors/${name}/charts`)
export const createCollector = (data: { name: string; collector_type: string; host?: string; port?: number }) =>
  fetchJSON<{ collector_id: string }>('/collectors', { method: 'POST', body: JSON.stringify(data) })
export const checkCollector = (name: string) =>
  fetchJSON<{ reachable: boolean; latency_ms: number }>(`/collectors/${name}/check`, { method: 'POST' })
export const deleteCollector = (name: string) =>
  fetchJSON<void>(`/collectors/${name}`, { method: 'DELETE' })

// Discovery Rules (P12)
export const getDiscoveryRules = () => fetchJSON<DiscoveryRule[]>('/discovery-rules')
export const createDiscoveryRule = (data: any) => fetchJSON<DiscoveryRule>('/discovery-rules', { method: 'POST', body: JSON.stringify(data) })
export const updateDiscoveryRule = (id: string, data: any) => fetchJSON<DiscoveryRule>(`/discovery-rules/${id}`, { method: 'PUT', body: JSON.stringify(data) })
export const deleteDiscoveryRule = (id: string) => fetchJSON<any>(`/discovery-rules/${id}`, { method: 'DELETE' })
export const runDiscoveryRule = (id: string) => fetchJSON<DiscoveryRule>(`/discovery-rules/${id}/run`, { method: 'POST' })
export const getDiscoveryRuleItems = (id: string) => fetchJSON<DiscoveredItem[]>(`/discovery-rules/${id}/items`)

// Advanced Discovery (V5 Prompt 10)
export const triggerDiscoveryScan = (data: { ip_range: string; scan_type?: string }) =>
  fetchJSON<any>('/discovery/scan', { method: 'POST', body: JSON.stringify(data) })
export const getDiscoveryScanResults = (scanId: string) => fetchJSON<any>(`/discovery/scan/${scanId}/results`)
export const autoRegisterHost = (data: { address: string; template_id?: string; labels?: any }) =>
  fetchJSON<any>('/discovery/auto-register', { method: 'POST', body: JSON.stringify(data) })
export const getLowLevelDiscovery = (targetId: string) => fetchJSON<any>(`/discovery/lld/${targetId}`)
export const snmpWalk = (data: { target: string; community?: string; oid?: string }) =>
  fetchJSON<any>('/discovery/snmp-walk', { method: 'POST', body: JSON.stringify(data) })
