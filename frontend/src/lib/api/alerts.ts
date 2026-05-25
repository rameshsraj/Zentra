import { fetchJSON } from './_client'
import type {
  Alert, AlertRule, AlertGroupData, InhibitionRule,
  MuteTiming, TimeInterval, BacktestResult,
  NotificationChannel, NotificationHistoryItem,
} from './types'

// Alerts
export const getAlerts = () => fetchJSON<Alert[]>('/alerts')
export const acknowledgeAlert = (id: string) =>
  fetchJSON<void>(`/alerts/${id}/acknowledge`, { method: 'PUT' })
export const resolveAlert = (id: string) =>
  fetchJSON<void>(`/alerts/${id}/resolve`, { method: 'PUT' })
export const getAlertRules = () => fetchJSON<AlertRule[]>('/alert-rules')

// P19: Alert Grouping + Inhibition
export const getAlertGroups = (groupBy?: string) =>
  fetchJSON<{ groups: AlertGroupData[]; total_groups: number }>(`/alerts/groups${groupBy ? `?group_by=${groupBy}` : ''}`)
export const getInhibitions = () =>
  fetchJSON<{ inhibitions: InhibitionRule[]; count: number }>('/alerts/inhibitions')
export const createInhibition = (data: { name: string; source_match: Record<string, string>; target_match: Record<string, string>; equal_labels: string[] }) =>
  fetchJSON<{ id: string }>('/alerts/inhibitions', { method: 'POST', body: JSON.stringify(data) })
export const deleteInhibition = (id: string) =>
  fetchJSON<void>(`/alerts/inhibitions/${id}`, { method: 'DELETE' })

// P10: Mute Timings + Alert Backtesting
export const getMuteTimings = () =>
  fetchJSON<{ mute_timings: MuteTiming[]; count: number }>('/notifications/mute-timings')
export const createMuteTiming = (data: { name: string; time_intervals: TimeInterval[] }) =>
  fetchJSON<{ id: string }>('/notifications/mute-timings', { method: 'POST', body: JSON.stringify(data) })
export const deleteMuteTiming = (id: string) =>
  fetchJSON<void>(`/notifications/mute-timings/${id}`, { method: 'DELETE' })
export const backtestAlert = (data: {
  metric_name: string; operator: string; threshold: number
  start: string; end: string; step_seconds?: number
}) => fetchJSON<BacktestResult>('/alerts/backtest', { method: 'POST', body: JSON.stringify(data) })

// Notifications
export const getNotificationChannels = () => fetchJSON<NotificationChannel[]>('/notifications/channels')
export const testNotificationChannel = (channelId: string) =>
  fetchJSON<{ channel_id: string; success: boolean; message: string; response_time_ms: number }>(
    `/notifications/channels/${channelId}/test`, { method: 'POST' },
  )
export const getNotificationHistory = () => fetchJSON<NotificationHistoryItem[]>('/notifications/history')
