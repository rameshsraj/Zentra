import { fetchJSON } from './_client'
import type {
  Dashboard, DashboardExport, Annotation, Playlist,
  QueryHistoryEntry, FrequentQuery,
} from './types'

// Dashboards (template variables, export/import)
export const getDashboards = () => fetchJSON<Dashboard[]>('/dashboards')
export const createDashboard = (data: Partial<Dashboard>) =>
  fetchJSON<Dashboard>('/dashboards', { method: 'POST', body: JSON.stringify(data) })
export const getDashboard = (id: string) => fetchJSON<Dashboard>(`/dashboards/${id}`)
export const deleteDashboard = (id: string) =>
  fetchJSON<void>(`/dashboards/${id}`, { method: 'DELETE' })
export const exportDashboard = (id: string) => fetchJSON<DashboardExport>(`/dashboards/${id}/export`)
export const importDashboard = (data: DashboardExport) =>
  fetchJSON<Dashboard>('/dashboards/import', { method: 'POST', body: JSON.stringify(data) })

// Annotations
export const getAnnotations = (params?: Record<string, string>) => {
  const qs = params ? '?' + new URLSearchParams(params).toString() : ''
  return fetchJSON<Annotation[]>(`/annotations${qs}`)
}
export const createAnnotation = (data: Partial<Annotation>) =>
  fetchJSON<Annotation>('/annotations', { method: 'POST', body: JSON.stringify(data) })
export const updateAnnotation = (id: string, data: Partial<Annotation>) =>
  fetchJSON<Annotation>(`/annotations/${id}`, { method: 'PUT', body: JSON.stringify(data) })
export const deleteAnnotation = (id: string) =>
  fetchJSON<void>(`/annotations/${id}`, { method: 'DELETE' })

// P13: Dashboard Playlists + Bookmarks
export const getPlaylists = () => fetchJSON<{ playlists: Playlist[]; count: number }>('/dashboards/playlists')
export const createPlaylist = (data: { name: string; interval_seconds?: number; dashboard_ids: string[] }) =>
  fetchJSON<{ playlist_id: string }>('/dashboards/playlists', { method: 'POST', body: JSON.stringify(data) })
export const updatePlaylist = (id: string, data: { name: string; interval_seconds?: number; dashboard_ids: string[] }) =>
  fetchJSON<{ playlist_id: string }>(`/dashboards/playlists/${id}`, { method: 'PUT', body: JSON.stringify(data) })
export const deletePlaylist = (id: string) =>
  fetchJSON<void>(`/dashboards/playlists/${id}`, { method: 'DELETE' })
export const playlistNext = (id: string) =>
  fetchJSON<{ dashboard_id: string; index: number; interval_seconds: number }>(`/dashboards/playlists/${id}/next`)
export const bookmarkDashboard = (id: string) =>
  fetchJSON<{ bookmarked: boolean }>(`/dashboards/${id}/bookmark`, { method: 'POST' })
export const unbookmarkDashboard = (id: string) =>
  fetchJSON<{ bookmarked: boolean }>(`/dashboards/${id}/bookmark`, { method: 'DELETE' })
export const getBookmarkedDashboards = () =>
  fetchJSON<{ bookmarked_dashboard_ids: string[]; count: number }>('/dashboards/bookmarked')

// P20: Rich Query History
export const updateQueryComment = (id: string, comment: string) =>
  fetchJSON<{ updated: boolean }>(`/query-history/${id}/comment`, { method: 'PUT', body: JSON.stringify({ comment }) })
export const getStarredQueries = () =>
  fetchJSON<QueryHistoryEntry[]>('/query-history/starred')
export const getFrequentQueries = () =>
  fetchJSON<{ frequent_queries: FrequentQuery[]; count: number }>('/query-history/frequent')

// SW P01: Entity-Scoped Dashboards & Templates
export const getDashboardsForEntity = (entityType: string, entityScope?: string) => {
  const params = new URLSearchParams({ entity_type: entityType })
  if (entityScope) params.set('entity_scope', entityScope)
  return fetchJSON<{ dashboards: Dashboard[]; count: number; template?: Record<string, unknown> }>(`/dashboards/for-entity?${params}`)
}
export const getDashboardTemplates = () =>
  fetchJSON<{ id: string; entity_type: string; name: string; description?: string; panels: unknown; is_builtin: boolean }[]>('/dashboard-templates')
export const instantiateTemplate = (entityType: string, data: { entity_scope?: string; name?: string }) =>
  fetchJSON<{ dashboard_id: string; name: string; entity_type: string; status: string }>(`/dashboard-templates/${entityType}/instantiate`, { method: 'POST', body: JSON.stringify(data) })

// P18: Snapshot Render
export const renderSnapshot = (key: string) =>
  fetchJSON<Record<string, unknown>>(`/snapshots/${key}/render`)
