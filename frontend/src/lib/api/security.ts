import { fetchJSON } from './_client'
import type { IdsRuleItem, IdsAlertItem, IdsStats } from './types'

// IDS/IPS (V5 Prompt 6)
export const getIdsRules = (params?: Record<string, string>) => {
  const qs = params ? '?' + new URLSearchParams(params).toString() : ''
  return fetchJSON<{ rules: IdsRuleItem[]; total: number }>(`/security/ids/rules${qs}`)
}
export const createIdsRule = (data: any) =>
  fetchJSON<any>('/security/ids/rules', { method: 'POST', body: JSON.stringify(data) })
export const updateIdsRule = (id: string, data: any) =>
  fetchJSON<any>(`/security/ids/rules/${id}`, { method: 'PUT', body: JSON.stringify(data) })
export const deleteIdsRule = (id: string) =>
  fetchJSON<any>(`/security/ids/rules/${id}`, { method: 'DELETE' })
export const bulkIdsRules = (action: string, rule_ids: string[]) =>
  fetchJSON<any>('/security/ids/rules/bulk', { method: 'POST', body: JSON.stringify({ action, rule_ids }) })
export const importIdsRules = (rules_text: string, source?: string) =>
  fetchJSON<any>('/security/ids/rules/import', { method: 'POST', body: JSON.stringify({ rules_text, source }) })
export const getIdsAlerts = (params?: Record<string, string>) => {
  const qs = params ? '?' + new URLSearchParams(params).toString() : ''
  return fetchJSON<{ alerts: IdsAlertItem[]; total: number }>(`/security/ids/alerts${qs}`)
}
export const getIdsAlert = (id: string) => fetchJSON<any>(`/security/ids/alerts/${id}`)
export const updateIdsAlertStatus = (id: string, status: string) =>
  fetchJSON<any>(`/security/ids/alerts/${id}/status`, { method: 'PUT', body: JSON.stringify({ status }) })
export const getIdsStats = () => fetchJSON<IdsStats>('/security/ids/stats')
export const ingestEve = (events: any[]) =>
  fetchJSON<any>('/security/ids/ingest', { method: 'POST', body: JSON.stringify({ events }) })

// Network Traffic Analysis (V5 Prompt 7)
export const getConnections = (params?: Record<string, string>) => {
  const qs = params ? '?' + new URLSearchParams(params).toString() : ''
  return fetchJSON<{ connections: any[]; total: number }>(`/security/connections${qs}`)
}
export const getConnection = (id: string) => fetchJSON<any>(`/security/connections/${id}`)
export const getConnectionStats = () => fetchJSON<any>('/security/connections/stats')
export const getDnsLogs = (params?: Record<string, string>) => {
  const qs = params ? '?' + new URLSearchParams(params).toString() : ''
  return fetchJSON<{ entries: any[]; total: number }>(`/security/dns${qs}`)
}
export const getDnsStats = () => fetchJSON<any>('/security/dns/stats')
export const getHttpLogs = (params?: Record<string, string>) => {
  const qs = params ? '?' + new URLSearchParams(params).toString() : ''
  return fetchJSON<{ entries: any[]; total: number }>(`/security/http${qs}`)
}
export const getHttpStats = () => fetchJSON<any>('/security/http/stats')
export const getSslLogs = (params?: Record<string, string>) => {
  const qs = params ? '?' + new URLSearchParams(params).toString() : ''
  return fetchJSON<{ entries: any[]; total: number }>(`/security/ssl${qs}`)
}
export const getSslStats = () => fetchJSON<any>('/security/ssl/stats')
export const getSshLogs = (params?: Record<string, string>) => {
  const qs = params ? '?' + new URLSearchParams(params).toString() : ''
  return fetchJSON<{ entries: any[]; total: number }>(`/security/ssh${qs}`)
}

// SOC Workflow (V5 Prompt 8)
export const getSocCases = (params?: Record<string, string>) => {
  const qs = params ? '?' + new URLSearchParams(params).toString() : ''
  return fetchJSON<{ cases: any[]; total: number }>(`/security/cases${qs}`)
}
export const createSocCase = (data: any) =>
  fetchJSON<any>('/security/cases', { method: 'POST', body: JSON.stringify(data) })
export const getSocCase = (id: string) => fetchJSON<any>(`/security/cases/${id}`)
export const updateSocCase = (id: string, data: any) =>
  fetchJSON<any>(`/security/cases/${id}`, { method: 'PUT', body: JSON.stringify(data) })
export const addCaseEvent = (caseId: string, data: { event_type: string; content: any }) =>
  fetchJSON<any>(`/security/cases/${caseId}/events`, { method: 'POST', body: JSON.stringify(data) })
export const escalateAlert = (alertId: string) =>
  fetchJSON<any>(`/security/alerts/${alertId}/escalate`, { method: 'POST' })
export const huntSearch = (params?: Record<string, string>) => {
  const qs = params ? '?' + new URLSearchParams(params).toString() : ''
  return fetchJSON<{ results: any[]; total: number }>(`/security/hunt${qs}`)
}
export const getIocLists = () => fetchJSON<{ lists: any[] }>('/security/ioc')
export const addIocEntry = (data: { list_name: string; ioc_type: string; value: string; description?: string }) =>
  fetchJSON<any>('/security/ioc', { method: 'POST', body: JSON.stringify(data) })
export const deleteIocEntry = (id: string) => fetchJSON<any>(`/security/ioc/${id}`, { method: 'DELETE' })
export const checkIoc = (indicator: string) =>
  fetchJSON<any>('/security/ioc/check', { method: 'POST', body: JSON.stringify({ indicator }) })
export const threatIntelLookup = (indicator: string) => fetchJSON<any>(`/security/threat-intel/${encodeURIComponent(indicator)}`)

// Firewall + File Analysis (V5 Prompt 9)
export const getFirewallLogs = (params?: Record<string, string>) => {
  const qs = params ? '?' + new URLSearchParams(params).toString() : ''
  return fetchJSON<{ entries: any[]; total: number }>(`/security/firewall/logs${qs}`)
}
export const getFirewallRules = () => fetchJSON<{ rules: any[] }>('/security/firewall/rules')
export const getFirewallStats = () => fetchJSON<any>('/security/firewall/stats')
export const ingestFirewall = (entries: any[]) =>
  fetchJSON<any>('/security/firewall/ingest', { method: 'POST', body: JSON.stringify({ entries }) })

export const getScannedFiles = (params?: Record<string, string>) => {
  const qs = params ? '?' + new URLSearchParams(params).toString() : ''
  return fetchJSON<{ files: any[]; total: number }>(`/security/files${qs}`)
}
export const scanFile = (data: any) =>
  fetchJSON<any>('/security/files/scan', { method: 'POST', body: JSON.stringify(data) })
export const getScannedFile = (id: string) => fetchJSON<any>(`/security/files/${id}`)
export const getYaraRules = () => fetchJSON<{ rules: any[] }>('/security/files/rules')
export const createYaraRule = (data: any) =>
  fetchJSON<any>('/security/files/rules', { method: 'POST', body: JSON.stringify(data) })
export const getFileStats = () => fetchJSON<any>('/security/files/stats')

// LLM Security (V5 Prompt 13)
export const getLlmOverview = () => fetchJSON<any>('/llm-security/overview')
export const scanLlmPrompt = (data: { prompt: string; response?: string; model?: string }) =>
  fetchJSON<any>('/llm-security/scan', { method: 'POST', body: JSON.stringify(data) })
export const getLlmEvents = (params?: Record<string, string>) => {
  const qs = params ? '?' + new URLSearchParams(params).toString() : ''
  return fetchJSON<{ events: any[]; total: number }>(`/llm-security/events${qs}`)
}
export const getLlmEvent = (id: string) => fetchJSON<any>(`/llm-security/events/${id}`)
export const getLlmStats = () => fetchJSON<any>('/llm-security/stats')
export const getLlmRules = () => fetchJSON<any[]>('/llm-security/rules')
export const createLlmRule = (data: any) =>
  fetchJSON<any>('/llm-security/rules', { method: 'POST', body: JSON.stringify(data) })
export const getLlmModels = () => fetchJSON<any[]>('/llm-security/models')
