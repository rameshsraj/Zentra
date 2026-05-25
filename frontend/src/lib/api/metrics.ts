import { fetchJSON } from './_client'
import type {
  MetricPoint, Forecast, AnomalyEvent, TrendAlert, PredictionSummary,
  AnomalyScore, MLModelStatus, ClusterInfo,
} from './types'

// Metrics
export const getMetrics = (params: Record<string, string>) => {
  const qs = new URLSearchParams(params).toString()
  return fetchJSON<MetricPoint[]>(`/metrics?${qs}`)
}

// Predictions / Predictive Alerting Engine
export const getForecasts = () => fetchJSON<Forecast[]>('/predictions/forecasts')
export const getAnomalies = () => fetchJSON<AnomalyEvent[]>('/predictions/anomalies')
export const getTrendAlerts = () => fetchJSON<TrendAlert[]>('/predictions/trend-alerts')
export const getPredictionSummary = () => fetchJSON<PredictionSummary>('/predictions/summary')

// P12: Prometheus-Compatible Label/Metadata APIs
export const getLabels = () => fetchJSON<{ status: string; data: string[] }>('/v1/labels')
export const getLabelValues = (name: string) => fetchJSON<{ status: string; data: string[] }>(`/v1/label/${name}/values`)
export const getMetricMetadata = () => fetchJSON<{ status: string; data: Record<string, unknown> }>('/v1/metadata')
export const querySeries = (selector?: string) => {
  const params = new URLSearchParams()
  if (selector) params.set('match[]', selector)
  return fetchJSON<{ status: string; data: unknown[] }>(`/v1/series?${params}`)
}
export const getPromTargets = () => fetchJSON<{ status: string; data: { activeTargets: unknown[]; droppedTargets: unknown[] } }>('/v1/targets')

// P6: K-means ML endpoints
export const getAnomalyScores = () => fetchJSON<AnomalyScore[]>('/ai/anomalies')
export const getMLModelStatus = () => fetchJSON<MLModelStatus>('/ai/ml/status')
export const getMLClusters = () => fetchJSON<ClusterInfo[]>('/ai/ml/clusters')
export const triggerMLRetrain = () => fetchJSON<any>('/ai/ml/train', { method: 'POST' })

// P5: Prometheus-compatible query endpoints
export const promInstantQuery = (query: string, time?: string) => {
  const params = new URLSearchParams({ query })
  if (time) params.set('time', time)
  return fetchJSON<any>(`/v1/query?${params}`)
}
export const promRangeQuery = (query: string, start: string, end: string, step: string) =>
  fetchJSON<any>(`/v1/query_range?${new URLSearchParams({ query, start, end, step })}`)
