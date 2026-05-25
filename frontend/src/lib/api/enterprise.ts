import { fetchJSON } from './_client'
import type {
  License, LicenseCompliance,
  ItsmConfig, ItsmTicket,
  SsoProvider, SsoSession,
  ComplianceFramework, ComplianceScan, ScanFinding,
  Tenant, TenantUsage,
  HaClusterStatus,
  OncallSchedule, OncallShift, CurrentOncall,
  StorageArray, StorageVolume, StorageReplication, StorageMetricsHistoryResponse, StorageMetricsLatest,
  CloudAccount, FlowSummaryRow, FlowExporter, AnomalyRecord,
  Runbook, RunbookExecution,
} from './types'

// License Management
export const getLicenses = () => fetchJSON<License[]>('/admin/licenses')
export const createLicense = (data: Partial<License>) =>
  fetchJSON<License>('/admin/licenses', { method: 'POST', body: JSON.stringify(data) })
export const getLicense = (id: string) => fetchJSON<License>(`/admin/licenses/${id}`)
export const updateLicense = (id: string, data: Partial<License>) =>
  fetchJSON<License>(`/admin/licenses/${id}`, { method: 'PUT', body: JSON.stringify(data) })
export const deleteLicense = (id: string) =>
  fetchJSON<void>(`/admin/licenses/${id}`, { method: 'DELETE' })
export const getLicenseCompliance = () => fetchJSON<LicenseCompliance>('/admin/licenses/compliance')
export const getExpiringLicenses = () => fetchJSON<License[]>('/admin/licenses/expiring')

// ITSM Integration (Phase 5)
export const getItsmConfigs = (provider?: string) => {
  const qs = provider ? `?provider=${encodeURIComponent(provider)}` : ''
  return fetchJSON<ItsmConfig[]>(`/itsm/configs${qs}`)
}
export const createItsmConfig = (data: any) => fetchJSON<ItsmConfig>('/itsm/configs', { method: 'POST', body: JSON.stringify(data) })
export const deleteItsmConfig = (id: string) => fetchJSON<any>(`/itsm/configs/${id}`, { method: 'DELETE' })
export const testItsmConfig = (id: string) => fetchJSON<any>(`/itsm/configs/${id}/test`, { method: 'POST' })
export const getItsmTickets = () => fetchJSON<ItsmTicket[]>('/itsm/tickets')
export const createItsmTicket = (data: any) => fetchJSON<ItsmTicket>('/itsm/tickets', { method: 'POST', body: JSON.stringify(data) })
export const getItsmSummary = () => fetchJSON<any>('/itsm/summary')

// SSO / SAML / OIDC / LDAP (Phase 5.2)
export const getSsoProviders = () => fetchJSON<SsoProvider[]>('/sso/providers')
export const getSsoProvider = (id: string) => fetchJSON<SsoProvider>(`/sso/providers/${id}`)
export const createSsoProvider = (data: any) => fetchJSON<SsoProvider>('/sso/providers', { method: 'POST', body: JSON.stringify(data) })
export const updateSsoProvider = (id: string, data: any) => fetchJSON<SsoProvider>(`/sso/providers/${id}`, { method: 'PUT', body: JSON.stringify(data) })
export const deleteSsoProvider = (id: string) => fetchJSON<any>(`/sso/providers/${id}`, { method: 'DELETE' })
export const testSsoProvider = (id: string) => fetchJSON<any>(`/sso/providers/${id}/test`, { method: 'POST' })
export const toggleSsoProvider = (id: string) => fetchJSON<SsoProvider>(`/sso/providers/${id}/toggle`, { method: 'POST' })
export const getSsoSessions = () => fetchJSON<SsoSession[]>('/sso/sessions')
export const getSsoSummary = () => fetchJSON<any>('/sso/summary')

// CIS Compliance Scanner (Phase 5.3)
export const getComplianceFrameworks = () => fetchJSON<ComplianceFramework[]>('/compliance/frameworks')
export const getComplianceScans = (params?: { framework_id?: string; target_id?: string; status?: string }) => {
  const qs = new URLSearchParams(Object.entries(params ?? {}).filter(([, v]) => v).map(([k, v]) => [k, v!])).toString()
  return fetchJSON<ComplianceScan[]>(`/compliance/scans${qs ? '?' + qs : ''}`)
}
export const triggerComplianceScan = (data: { framework_id: string; target_id: string }) =>
  fetchJSON<ComplianceScan>('/compliance/scans', { method: 'POST', body: JSON.stringify(data) })
export const getComplianceScan = (id: string) => fetchJSON<ComplianceScan>(`/compliance/scans/${id}`)
export const getComplianceFindings = (params?: { scan_id?: string; status?: string; severity?: string }) => {
  const qs = new URLSearchParams(Object.entries(params ?? {}).filter(([, v]) => v).map(([k, v]) => [k, v!])).toString()
  return fetchJSON<ScanFinding[]>(`/compliance/findings${qs ? '?' + qs : ''}`)
}
export const getComplianceSummary = () => fetchJSON<any>('/compliance/summary')

// Multi-Tenancy / MSP Mode (Phase 5.4)
export const getTenants = (params?: { status?: string; plan?: string }) => {
  const qs = new URLSearchParams(Object.entries(params ?? {}).filter(([, v]) => v).map(([k, v]) => [k, v!])).toString()
  return fetchJSON<Tenant[]>(`/tenants${qs ? '?' + qs : ''}`)
}
export const getTenant = (id: string) => fetchJSON<Tenant>(`/tenants/${id}`)
export const createTenant = (data: any) => fetchJSON<Tenant>('/tenants', { method: 'POST', body: JSON.stringify(data) })
export const updateTenant = (id: string, data: any) => fetchJSON<Tenant>(`/tenants/${id}`, { method: 'PUT', body: JSON.stringify(data) })
export const deleteTenant = (id: string) => fetchJSON<any>(`/tenants/${id}`, { method: 'DELETE' })
export const suspendTenant = (id: string) => fetchJSON<Tenant>(`/tenants/${id}/suspend`, { method: 'POST' })
export const activateTenant = (id: string) => fetchJSON<Tenant>(`/tenants/${id}/activate`, { method: 'POST' })
export const getTenantUsage = (id: string) => fetchJSON<TenantUsage>(`/tenants/${id}/usage`)
export const getTenantsSummary = () => fetchJSON<any>('/tenants/summary')

// HA Cluster Status (Phase 5.5)
export const getHaClusterStatus = () => fetchJSON<HaClusterStatus>('/ha/cluster')
export const getHaHealth = () => fetchJSON<any>('/ha/health')
export const getFailoverHistory = () => fetchJSON<any[]>('/ha/failovers')

// On-Call Management (Phase 4.4)
export const getOncallSchedules = () => fetchJSON<OncallSchedule[]>('/oncall/schedules')
export const getOncallSchedule = (id: string) => fetchJSON<OncallSchedule>(`/oncall/schedules/${id}`)
export const createOncallSchedule = (data: any) => fetchJSON<OncallSchedule>('/oncall/schedules', { method: 'POST', body: JSON.stringify(data) })
export const deleteOncallSchedule = (id: string) => fetchJSON<any>(`/oncall/schedules/${id}`, { method: 'DELETE' })
export const getCurrentOncall = (scheduleId: string) => fetchJSON<CurrentOncall>(`/oncall/schedules/${scheduleId}/current`)
export const getOncallShifts = (scheduleId: string) => fetchJSON<OncallShift[]>(`/oncall/schedules/${scheduleId}/shifts`)
export const createOncallOverride = (scheduleId: string, layerId: string, data: any) =>
  fetchJSON<any>(`/oncall/schedules/${scheduleId}/layers/${layerId}/overrides`, { method: 'POST', body: JSON.stringify(data) })
export const getOncallSummary = () => fetchJSON<any>('/oncall/summary')

// Proxies + Escalation (V5 Prompt 12)
export const getProxies = () => fetchJSON<any[]>('/proxies')
export const registerProxy = (data: { name: string; address: string }) =>
  fetchJSON<any>('/proxies', { method: 'POST', body: JSON.stringify(data) })
export const getProxy = (id: string) => fetchJSON<any>(`/proxies/${id}`)
export const updateProxy = (id: string, data: any) =>
  fetchJSON<any>(`/proxies/${id}`, { method: 'PUT', body: JSON.stringify(data) })
export const deleteProxy = (id: string) => fetchJSON<any>(`/proxies/${id}`, { method: 'DELETE' })
export const getEscalationChains = () => fetchJSON<any[]>('/notifications/escalation-chains')
export const createEscalationChain = (data: any) =>
  fetchJSON<any>('/notifications/escalation-chains', { method: 'POST', body: JSON.stringify(data) })
export const deleteEscalationChain = (id: string) =>
  fetchJSON<any>(`/notifications/escalation-chains/${id}`, { method: 'DELETE' })
export const getOnCallSchedules = () => fetchJSON<any[]>('/notifications/on-call')
export const createOnCallSchedule = (data: any) =>
  fetchJSON<any>('/notifications/on-call', { method: 'POST', body: JSON.stringify(data) })

// Enterprise Storage Arrays (Phase 1.4)
export const getStorageArrays = (params?: { array_type?: string; status?: string }) => {
  const qs = new URLSearchParams(Object.entries(params ?? {}).filter(([, v]) => v).map(([k, v]) => [k, v!])).toString()
  return fetchJSON<StorageArray[]>(`/storage-arrays${qs ? '?' + qs : ''}`)
}
export const getStorageArray = (id: string) => fetchJSON<StorageArray>(`/storage-arrays/${id}`)
export const getStorageVolumes = (arrayId: string) => fetchJSON<StorageVolume[]>(`/storage-arrays/${arrayId}/volumes`)
export const getStorageReplications = () => fetchJSON<StorageReplication[]>('/storage-arrays/replications')
export const getStorageSummary = () => fetchJSON<any>('/storage-arrays/summary')

export const getStorageArrayMetricsHistory = (id: string, params?: {
  from?: string; to?: string; rollup?: 'raw' | 'hourly' | 'daily'; limit?: number
}) => {
  const qs = new URLSearchParams(
    Object.entries(params ?? {}).filter(([, v]) => v !== undefined).map(([k, v]) => [k, String(v)])
  ).toString()
  return fetchJSON<StorageMetricsHistoryResponse>(`/storage-arrays/${id}/metrics/history${qs ? '?' + qs : ''}`)
}

export const getStorageArrayMetricsLatest = (id: string) =>
  fetchJSON<StorageMetricsLatest>(`/storage-arrays/${id}/metrics/latest`)

// Cloud Monitoring (Phase 2)
export const getCloudAccounts = () => fetchJSON<CloudAccount[]>('/cloud/accounts')
export const createCloudAccount = (data: {
  name: string; provider: string; account_id?: string; region?: string; credentials: Record<string, unknown>
}) => fetchJSON<CloudAccount>('/cloud/accounts', { method: 'POST', body: JSON.stringify(data) })
export const deleteCloudAccount = (id: string) => fetchJSON<void>(`/cloud/accounts/${id}`, { method: 'DELETE' })
export const validateCloudAccount = (id: string) => fetchJSON<{ message: string; account_id: string }>(`/cloud/accounts/${id}/validate`, { method: 'POST' })
export const getCloudAccountMetrics = (id: string, metric: string, hours = 1) =>
  fetchJSON<any>(`/cloud/accounts/${id}/metrics?metric=${encodeURIComponent(metric)}&hours=${hours}`)
export const getCloudSummary = () => fetchJSON<any>('/cloud/summary')

// NetFlow / IPFIX / sFlow (Phase 3)
export const getNetflowTopTalkers = (params?: { from?: string; to?: string; limit?: number; src_ip?: string; dst_ip?: string; protocol?: string }) => {
  const qs = new URLSearchParams()
  if (params?.from) qs.set('from', params.from)
  if (params?.to) qs.set('to', params.to)
  if (params?.limit) qs.set('limit', String(params.limit))
  if (params?.src_ip) qs.set('src_ip', params.src_ip)
  if (params?.dst_ip) qs.set('dst_ip', params.dst_ip)
  if (params?.protocol) qs.set('protocol', params.protocol)
  return fetchJSON<FlowSummaryRow[]>(`/netflow/top-talkers?${qs}`)
}
export const getNetflowStats = () => fetchJSON<any>('/netflow/stats')
export const getNetflowExporters = () => fetchJSON<FlowExporter[]>('/netflow/exporters')
export const getNetflowProtocolBreakdown = (hours = 1) => fetchJSON<any[]>(`/netflow/protocol-breakdown?hours=${hours}`)
export const getNetflowTimeseries = (hours = 6, bucket = '5m') => fetchJSON<any[]>(`/netflow/timeseries?hours=${hours}&bucket=${bucket}`)

// ML Anomaly Detection (Phase 4)
export const getAnomalyDetections = (params?: {
  target_id?: string; metric_name?: string; severity?: string; detector?: string
  from?: string; to?: string; limit?: number
}) => {
  const qs = new URLSearchParams()
  if (params?.target_id) qs.set('target_id', params.target_id)
  if (params?.metric_name) qs.set('metric_name', params.metric_name)
  if (params?.severity) qs.set('severity', params.severity)
  if (params?.detector) qs.set('detector', params.detector)
  if (params?.from) qs.set('from', params.from)
  if (params?.to) qs.set('to', params.to)
  if (params?.limit) qs.set('limit', String(params.limit))
  return fetchJSON<AnomalyRecord[]>(`/anomalies?${qs}`)
}
export const getAnomalyStats = () => fetchJSON<any>('/anomalies/stats')
export const getTopAnomalousMetrics = (hours = 24, limit = 10) => fetchJSON<any[]>(`/anomalies/top-metrics?hours=${hours}&limit=${limit}`)
export const acknowledgeAnomaly = (id: string) => fetchJSON<void>(`/anomalies/${id}/acknowledge`, { method: 'PUT' })
export const submitAnomalyFeedback = (id: string, is_real: boolean, comment?: string) =>
  fetchJSON<void>(`/anomalies/${id}/feedback`, { method: 'POST', body: JSON.stringify({ is_real, comment }) })

// SNMP Traps (Phase 1)
export const getSnmpTraps = (params?: { severity?: string; limit?: number; offset?: number }) => {
  const qs = new URLSearchParams()
  if (params?.severity) qs.set('severity', params.severity)
  if (params?.limit) qs.set('limit', String(params.limit))
  if (params?.offset) qs.set('offset', String(params.offset))
  return fetchJSON<any[]>(`/snmp/traps?${qs}`)
}
export const getSnmpTrapStats = () => fetchJSON<any>('/snmp/traps/stats')
export const acknowledgeSnmpTrap = (id: string) => fetchJSON<void>(`/snmp/traps/${id}/acknowledge`, { method: 'PUT' })
export const getSnmpMibs = () => fetchJSON<any[]>('/snmp/mibs')
export const deleteSnmpMib = (id: string) => fetchJSON<void>(`/snmp/mibs/${id}`, { method: 'DELETE' })
export const resolveOid = (oid: string) => fetchJSON<any>(`/snmp/oid-resolve?oid=${encodeURIComponent(oid)}`)

// Automated Remediation Runbooks (Phase 4)
export const getRunbooks = (params?: { category?: string; trigger_type?: string; enabled?: boolean }) => {
  const qs = new URLSearchParams()
  if (params?.category) qs.set('category', params.category)
  if (params?.trigger_type) qs.set('trigger_type', params.trigger_type)
  if (params?.enabled !== undefined) qs.set('enabled', String(params.enabled))
  return fetchJSON<Runbook[]>(`/runbooks?${qs}`)
}
export const getRunbook = (id: string) => fetchJSON<Runbook>(`/runbooks/${id}`)
export const createRunbook = (data: any) => fetchJSON<Runbook>('/runbooks', { method: 'POST', body: JSON.stringify(data) })
export const updateRunbook = (id: string, data: any) => fetchJSON<Runbook>(`/runbooks/${id}`, { method: 'PUT', body: JSON.stringify(data) })
export const deleteRunbook = (id: string) => fetchJSON<any>(`/runbooks/${id}`, { method: 'DELETE' })
export const toggleRunbook = (id: string) => fetchJSON<Runbook>(`/runbooks/${id}/toggle`, { method: 'POST' })
export const executeRunbook = (id: string, data?: { target?: string; parameters?: Record<string, string> }) =>
  fetchJSON<RunbookExecution>(`/runbooks/${id}/execute`, { method: 'POST', body: JSON.stringify(data ?? {}) })
export const getRunbookExecutions = () => fetchJSON<RunbookExecution[]>('/runbooks/executions')
export const getRunbookExecution = (id: string) => fetchJSON<RunbookExecution>(`/runbooks/executions/${id}`)
export const approveRunbookExecution = (id: string, approved: boolean, comment?: string) =>
  fetchJSON<RunbookExecution>(`/runbooks/executions/${id}/approve`, { method: 'POST', body: JSON.stringify({ approved, comment }) })
export const getRunbookStats = () => fetchJSON<any>('/runbooks/stats')
