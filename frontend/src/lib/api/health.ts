import { fetchJSON } from './_client'
import type { SystemStatus } from './types'

// Health
export const getHealth = () => fetchJSON<{ status: string }>('/health')

// System
export const getSystemStatus = () => fetchJSON<SystemStatus>('/system/status')
export const getDbHealth = () => fetchJSON<{ database: string }>('/system/db')

// Global Demo/Live Mode
export interface AppModeResponse { mode: 'demo' | 'live'; demo: boolean; description: string }
export const getAppMode = () => fetchJSON<AppModeResponse>('/config/mode')
export const setAppMode = (mode: 'demo' | 'live') =>
  fetchJSON<AppModeResponse>('/config/mode', { method: 'POST', body: JSON.stringify({ mode }) })

// P11: Config/Status API
export const getRuntimeInfo = () => fetchJSON<{ status: string; data: Record<string, unknown> }>('/v1/status/runtimeinfo')
export const getBuildInfo = () => fetchJSON<{ status: string; data: Record<string, unknown> }>('/v1/status/buildinfo')
export const getRunningConfig = () => fetchJSON<{ status: string; data: Record<string, unknown> }>('/v1/status/config')
export const getStatusFlags = () => fetchJSON<{ status: string; data: Record<string, unknown> }>('/v1/status/flags')
export const getTsdbStatus = () => fetchJSON<{ status: string; data: Record<string, unknown> }>('/v1/status/tsdb')
export const reloadConfig = () => fetchJSON<{ status: string; message: string }>('/v1/admin/reload', { method: 'POST' })

// Dashboard Overview (V5 Prompt 21)
export const getDashboardOverview = () => fetchJSON<any>('/dashboard/overview')
export const getDashboardHealthSummary = () => fetchJSON<any>('/dashboard/health-summary')
export const getDashboardTimeline = () => fetchJSON<any>('/dashboard/timeline')

// Data Audit (V5 Prompt 19)
export const getDataSourceAudit = () => fetchJSON<any>('/audit/data-sources')
export const getMockDataCheck = () => fetchJSON<any>('/audit/mock-data')
