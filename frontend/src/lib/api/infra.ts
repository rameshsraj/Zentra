import { fetchJSON } from './_client'
import type {
  MonitoringTemplate, TemplateAssignment,
  HealthCheck, HealthCheckResult, HealthCheckWithStatus, HealthCheckSummary, MatrixRow, UptimeStat,
  MaintenanceWindow, SLAReport, SLASummary,
  HostDependency, DependencyTreeNode, FlappingCheck,
  Deployment, DeploymentRunbook,
} from './types'

// Monitoring Templates (Zabbix-inspired)
export const getMonitoringTemplates = (category?: string) =>
  fetchJSON<MonitoringTemplate[]>(`/monitoring-templates${category ? `?category=${category}` : ''}`)
export const createMonitoringTemplate = (data: Partial<MonitoringTemplate>) =>
  fetchJSON<MonitoringTemplate>('/monitoring-templates', { method: 'POST', body: JSON.stringify(data) })
export const getMonitoringTemplate = (id: string) => fetchJSON<MonitoringTemplate>(`/monitoring-templates/${id}`)
export const updateMonitoringTemplate = (id: string, data: Partial<MonitoringTemplate>) =>
  fetchJSON<MonitoringTemplate>(`/monitoring-templates/${id}`, { method: 'PUT', body: JSON.stringify(data) })
export const deleteMonitoringTemplate = (id: string) =>
  fetchJSON<any>(`/monitoring-templates/${id}`, { method: 'DELETE' })
export const assignTemplate = (id: string, data: { target_id: string; target_name?: string; macro_overrides?: Record<string, any> }) =>
  fetchJSON<TemplateAssignment>(`/monitoring-templates/${id}/assign`, { method: 'POST', body: JSON.stringify(data) })
export const unassignTemplate = (templateId: string, targetId: string) =>
  fetchJSON<any>(`/monitoring-templates/${templateId}/unassign/${targetId}`, { method: 'DELETE' })
export const getTemplateAssignments = () => fetchJSON<TemplateAssignment[]>('/monitoring-templates/assignments')
export const importTemplate = (tpl: MonitoringTemplate) =>
  fetchJSON<MonitoringTemplate>('/monitoring-templates/import', { method: 'POST', body: JSON.stringify(tpl) })
export const exportTemplate = (id: string) => fetchJSON<MonitoringTemplate>(`/monitoring-templates/${id}/export`)
export const getTemplateCategories = () => fetchJSON<any[]>('/monitoring-templates/categories')
export const importTemplateBundle = (templates: any[]) =>
  fetchJSON<any>('/monitoring-templates/import-bundle', { method: 'POST', body: JSON.stringify(templates) })

// Health Checks (Icinga2-inspired)
export const getHealthChecks = (params?: Record<string, string>) => {
  const qs = params ? '?' + new URLSearchParams(params).toString() : ''
  return fetchJSON<HealthCheckWithStatus[]>(`/healthchecks${qs}`)
}
export const createHealthCheck = (data: { name: string; check_type: string; target_host: string; config?: any; interval_secs?: number; group?: string }) =>
  fetchJSON<HealthCheck>('/healthchecks', { method: 'POST', body: JSON.stringify(data) })
export const getHealthCheckDetail = (id: string) => fetchJSON<{ check: HealthCheckWithStatus; history: HealthCheckResult[] }>(`/healthchecks/${id}`)
export const updateHealthCheck = (id: string, data: any) =>
  fetchJSON<HealthCheck>(`/healthchecks/${id}`, { method: 'PUT', body: JSON.stringify(data) })
export const deleteHealthCheck = (id: string) => fetchJSON<any>(`/healthchecks/${id}`, { method: 'DELETE' })
export const runHealthCheck = (id: string) => fetchJSON<HealthCheckResult>(`/healthchecks/${id}/run`, { method: 'POST' })
export const getHealthCheckHistory = (id: string, params?: Record<string, string>) => {
  const qs = params ? '?' + new URLSearchParams(params).toString() : ''
  return fetchJSON<{ total: number; results: HealthCheckResult[] }>(`/healthchecks/${id}/history${qs}`)
}
export const getHealthCheckSummary = () => fetchJSON<HealthCheckSummary>('/healthchecks/summary')
export const getHealthCheckMatrix = () => fetchJSON<MatrixRow[]>('/healthchecks/matrix')
export const getHealthCheckGroups = () => fetchJSON<string[]>('/healthchecks/groups')
export const getHealthCheckUptime = () => fetchJSON<UptimeStat[]>('/healthchecks/uptime')
export const bulkCreateHealthChecks = (checks: any[]) =>
  fetchJSON<HealthCheck[]>('/healthchecks/bulk', { method: 'POST', body: JSON.stringify({ checks }) })

// Maintenance Windows (P3)
export const getMaintenanceWindows = (filter?: string) => {
  const qs = filter ? `?filter=${filter}` : ''
  return fetchJSON<MaintenanceWindow[]>(`/maintenance-windows${qs}`)
}
export const createMaintenanceWindow = (data: any) => fetchJSON<MaintenanceWindow>('/maintenance-windows', { method: 'POST', body: JSON.stringify(data) })
export const updateMaintenanceWindow = (id: string, data: any) => fetchJSON<MaintenanceWindow>(`/maintenance-windows/${id}`, { method: 'PUT', body: JSON.stringify(data) })
export const deleteMaintenanceWindow = (id: string) => fetchJSON<any>(`/maintenance-windows/${id}`, { method: 'DELETE' })
export const getActiveMaintenanceWindows = () => fetchJSON<MaintenanceWindow[]>('/maintenance-windows/active')
export const checkTargetMaintenance = (targetId: string) => fetchJSON<any>(`/maintenance-windows/check/${targetId}`)

// SLA Reports (P3)
export const getSLAReports = () => fetchJSON<SLAReport[]>('/sla/reports')
export const getSLAReportDetail = (checkId: string) => fetchJSON<SLAReport>(`/sla/reports/${checkId}`)
export const setSLATargets = (targets: { check_id: string; target_pct: number }[]) => fetchJSON<any>('/sla/targets', { method: 'PUT', body: JSON.stringify(targets) })
export const getSLASummary = () => fetchJSON<SLASummary>('/sla/summary')

// Dependencies + Flapping (P4)
export const getDependencies = () => fetchJSON<HostDependency[]>('/dependencies')
export const createDependency = (data: any) => fetchJSON<HostDependency>('/dependencies', { method: 'POST', body: JSON.stringify(data) })
export const deleteDependency = (id: string) => fetchJSON<any>(`/dependencies/${id}`, { method: 'DELETE' })
export const getDependencyTree = () => fetchJSON<DependencyTreeNode[]>('/dependencies/tree')
export const getFlappingChecks = () => fetchJSON<FlappingCheck[]>('/healthchecks/flapping')
export const acknowledgeFlapping = (id: string) => fetchJSON<any>(`/healthchecks/${id}/flapping/acknowledge`, { method: 'PUT' })

// Deployments (P9)
export const getDeployments = (params?: Record<string, string>) => { const qs = params ? '?' + new URLSearchParams(params).toString() : ''; return fetchJSON<Deployment[]>(`/deployments${qs}`) }
export const createDeployment = (data: any) => fetchJSON<Deployment>('/deployments', { method: 'POST', body: JSON.stringify(data) })
export const getDeployment = (id: string) => fetchJSON<Deployment>(`/deployments/${id}`)
export const updateDeployment = (id: string, data: any) => fetchJSON<Deployment>(`/deployments/${id}`, { method: 'PUT', body: JSON.stringify(data) })
export const deleteDeployment = (id: string) => fetchJSON<any>(`/deployments/${id}`, { method: 'DELETE' })
export const runPreCheck = (id: string) => fetchJSON<any[]>(`/deployments/${id}/pre-check`, { method: 'POST' })
export const runPostCheck = (id: string) => fetchJSON<any[]>(`/deployments/${id}/post-check`, { method: 'POST' })
export const rollbackDeployment = (id: string) => fetchJSON<Deployment>(`/deployments/${id}/rollback`, { method: 'POST' })
export const approveDeployment = (id: string) => fetchJSON<Deployment>(`/deployments/${id}/approve`, { method: 'POST' })
export const getDeploymentEnvironments = () => fetchJSON<any[]>('/deployments/environments')
export const getDeploymentRunbooks = () => fetchJSON<DeploymentRunbook[]>('/deployments/runbooks')
export const createDeploymentRunbook = (data: any) => fetchJSON<DeploymentRunbook>('/deployments/runbooks', { method: 'POST', body: JSON.stringify(data) })
export const updateDeploymentRunbook = (id: string, data: any) => fetchJSON<DeploymentRunbook>(`/deployments/runbooks/${id}`, { method: 'PUT', body: JSON.stringify(data) })
export const deleteDeploymentRunbook = (id: string) => fetchJSON<any>(`/deployments/runbooks/${id}`, { method: 'DELETE' })
