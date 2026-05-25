import { fetchJSON } from './_client'
import type { OracleConnection } from './types'

// Oracle Connection Management (V5 Prompt 14 + P1)
export const getOracleConnections = () => fetchJSON<OracleConnection[]>('/oracle/connections')
export const createOracleConnection = (data: any) =>
  fetchJSON<any>('/oracle/connections', { method: 'POST', body: JSON.stringify(data) })
export const updateOracleConnection = (id: string, data: any) =>
  fetchJSON<any>(`/oracle/connections/${id}`, { method: 'PUT', body: JSON.stringify(data) })
export const deleteOracleConnection = (id: string) =>
  fetchJSON<any>(`/oracle/connections/${id}`, { method: 'DELETE' })
export const testOracleConnection = (id: string) =>
  fetchJSON<any>(`/oracle/connections/${id}/test`, { method: 'POST' })
export const getOracleConnectionHealth = (id: string) =>
  fetchJSON<any>(`/oracle/connections/${id}/health`)

// Oracle Metrics (V5 Prompt 15)
export const getOracleMetrics = () => fetchJSON<any>('/oracle/metrics')
export const createOracleMetric = (data: any) =>
  fetchJSON<any>('/oracle/metrics', { method: 'POST', body: JSON.stringify(data) })
export const getOracleMetric = (id: string) => fetchJSON<any>(`/oracle/metrics/${id}`)
export const updateOracleMetric = (id: string, data: any) =>
  fetchJSON<any>(`/oracle/metrics/${id}`, { method: 'PUT', body: JSON.stringify(data) })
export const deleteOracleMetric = (id: string) =>
  fetchJSON<any>(`/oracle/metrics/${id}`, { method: 'DELETE' })
export const testOracleMetric = (id: string) =>
  fetchJSON<any>(`/oracle/metrics/${id}/test`, { method: 'POST' })
export const reloadOracleMetrics = () =>
  fetchJSON<any>('/oracle/metrics/reload', { method: 'POST' })

// Oracle Alert Log (V5 Prompt 16)
export const getOracleAlertLog = (params?: Record<string, string>) => {
  const qs = params ? '?' + new URLSearchParams(params).toString() : ''
  return fetchJSON<any>(`/oracle/alertlog${qs}`)
}
export const getOracleAlertLogStats = (connectionId?: string) => {
  const qs = connectionId ? `?connection_id=${connectionId}` : ''
  return fetchJSON<any>(`/oracle/alertlog/stats${qs}`)
}

// Oracle Scrape + TxEventQ (V5 Prompt 17-18)
export const getOracleScrapeStatus = () => fetchJSON<any>('/oracle/scrape/status')
export const triggerOracleScrape = () => fetchJSON<any>('/oracle/scrape/trigger', { method: 'POST' })
export const getOracleScrapeMetrics = () => fetchJSON<any>('/oracle/scrape/metrics')
export const startOracleScheduler = () => fetchJSON<any>('/oracle/scheduler/start', { method: 'POST' })
export const stopOracleScheduler = () => fetchJSON<any>('/oracle/scheduler/stop', { method: 'POST' })
export const getOracleTxEventQueues = () => fetchJSON<any>('/oracle/txeventq')
export const getOracleTxEventQueueDetail = (name: string) => fetchJSON<any>(`/oracle/txeventq/${name}`)
export const getOracleTxEventQueueStats = () => fetchJSON<any>('/oracle/txeventq/stats')
