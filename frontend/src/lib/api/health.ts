import { fetchJSON } from './_client'
import type { SystemStatus } from './types'

// Health
export const getHealth = () => fetchJSON<{ status: string }>('/health')

// System
export const getSystemStatus = () => fetchJSON<SystemStatus>('/system/status')
export const getDbHealth = () => fetchJSON<{ database: string }>('/system/db')

// App Mode (always live)
export interface AppModeResponse { mode: 'live'; description: string }
export const getAppMode = () => fetchJSON<AppModeResponse>('/config/mode')

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

// License Status
export interface LicenseStatus {
  is_licensed: boolean
  trial_expired: boolean
  customer_name: string
  onboarded_at: string | null
  trial_duration_days: number
  days_remaining: number
  max_targets: number
  features: {
    enable_oracle: boolean
    enable_ebpf_network: boolean
    enable_ai_observability: boolean
    enable_otel: boolean
    allowed_menu_items: string[]
  }
}
export const getLicenseStatus = () => fetchJSON<LicenseStatus>('/system/license-status')

// Data Audit (V5 Prompt 19)
export const getDataSourceAudit = () => fetchJSON<any>('/audit/data-sources')
