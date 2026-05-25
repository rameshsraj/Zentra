import { fetchJSON } from './_client'
import type { LogEntry, LogSource, LogQueryResult, LogStreamInfo, LogAlert, LogAnalyticsData } from './types'

// Logs
export const searchLogs = (params: Record<string, string>) => {
  const qs = new URLSearchParams(params).toString()
  return fetchJSON<LogEntry[]>(`/logs?${qs}`)
}

// Log Sources
export const getLogSources = () => fetchJSON<LogSource[]>('/log-sources')
export const createLogSource = (data: Partial<LogSource>) =>
  fetchJSON<LogSource>('/log-sources', { method: 'POST', body: JSON.stringify(data) })
export const deleteLogSource = (id: string) =>
  fetchJSON<void>(`/log-sources/${id}`, { method: 'DELETE' })

// Log Engine (P10-P11)
export const queryLogs = (params: Record<string, string>) => { const qs = '?' + new URLSearchParams(params).toString(); return fetchJSON<LogQueryResult>(`/logs/query${qs}`) }
export const getLogLabels = () => fetchJSON<string[]>('/logs/labels')
export const getLogLabelValues = (name: string) => fetchJSON<string[]>(`/logs/label/${name}/values`)
export const getLogStreams = () => fetchJSON<LogStreamInfo[]>('/logs/streams')
export const getLogVolume = () => fetchJSON<any[]>('/logs/volume')
export const ingestLogs = (data: any) => fetchJSON<any>('/logs/ingest', { method: 'POST', body: JSON.stringify(data) })
export const collectLogs = (data: any) => fetchJSON<any>('/logs/collect', { method: 'POST', body: JSON.stringify(data) })
export const exportLogs = (params?: Record<string, string>) => { const qs = params ? '?' + new URLSearchParams(params).toString() : ''; return fetchJSON<any>(`/logs/export${qs}`) }
export const getLogAlerts = () => fetchJSON<LogAlert[]>('/log-alerts')
export const createLogAlert = (data: any) => fetchJSON<LogAlert>('/log-alerts', { method: 'POST', body: JSON.stringify(data) })
export const updateLogAlert = (id: string, data: any) => fetchJSON<LogAlert>(`/log-alerts/${id}`, { method: 'PUT', body: JSON.stringify(data) })
export const deleteLogAlert = (id: string) => fetchJSON<any>(`/log-alerts/${id}`, { method: 'DELETE' })
export const getLogAnalytics = () => fetchJSON<LogAnalyticsData>('/logs/analytics')

/** Open a Server-Sent Events live tail stream. Returns an EventSource. */
export const openLogTail = (query = '{}', limit = 50): EventSource => {
  const qs = new URLSearchParams({ query, limit: String(limit) })
  return new EventSource(`/api/logs/tail?${qs}`)
}

// Correlation
export const logsForMetric = (params: Record<string, string>) => {
  const qs = new URLSearchParams(params).toString()
  return fetchJSON<any>(`/correlation/logs-for-metric?${qs}`)
}
export const metricsForLog = (params: Record<string, string>) => {
  const qs = new URLSearchParams(params).toString()
  return fetchJSON<any>(`/correlation/metrics-for-log?${qs}`)
}
