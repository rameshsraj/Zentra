import { useEffect, useMemo, useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import {
  Activity,
  AlertTriangle,
  CheckCircle,
  Clock,
  Database,
  Globe,
  Layers,
  Server,
  TrendingUp,
  XCircle,
} from 'lucide-react'
import { getMiddlewareDashboard, getMiddlewareHistory, getMiddlewareInstances, getMiddlewareStatus, type MiddlewareServiceStatus } from '@/lib/api'
import { Area, AreaChart, ResponsiveContainer, Tooltip, XAxis, YAxis } from 'recharts'

const SERVICE_ICONS: Record<string, any> = { nginx: Globe, apache: Globe, haproxy: Layers }
const SERVICE_COLORS: Record<string, string> = { nginx: 'text-emerald-400', apache: 'text-rose-400', haproxy: 'text-cyan-400' }
const SERVICE_ACCENTS: Record<string, string> = { nginx: 'border-emerald-500/30 bg-emerald-500/10', apache: 'border-rose-500/30 bg-rose-500/10', haproxy: 'border-cyan-500/30 bg-cyan-500/10' }
const STATUS_DOT: Record<string, string> = { healthy: 'bg-emerald-400', warning: 'bg-amber-400', critical: 'bg-rose-400', unknown: 'bg-slate-500' }
const CHART_COLORS = ['#3b82f6', '#10b981', '#f59e0b']

function formatMetricName(key: string) {
  return key.replace(/_/g, ' ')
}

function formatMetricValue(value: unknown) {
  if (typeof value === 'number') return value % 1 === 0 ? value.toLocaleString() : value.toFixed(2)
  return String(value ?? '-')
}

function getHistoryMetricKeys(history: any[]) {
  if (history.length === 0) return []
  return Object.keys(history[0])
    .filter(k => k !== 'timestamp' && typeof history[0][k] === 'number')
    .slice(0, 3)
}

function fallbackServiceId(type: string) {
  return `${type}-001`
}

function isHealthy(status?: string) {
  return status === 'healthy' || status === 'connected'
}

function isWarning(status?: string) {
  return status === 'warning'
}

function isCritical(status?: string) {
  return status === 'critical' || status === 'unreachable'
}

export default function MiddlewareMonitoringPage() {
  const [selectedService, setSelectedService] = useState<{ type: string; id: string } | null>(null)
  const [detailTab, setDetailTab] = useState<'status' | 'history'>('status')
  const { data: dashboard = [] } = useQuery({ queryKey: ['middleware-dashboard'], queryFn: getMiddlewareDashboard })
  const { data: instances = [] } = useQuery({ queryKey: ['middleware-instances', selectedService?.type], queryFn: () => selectedService ? getMiddlewareInstances(selectedService.type) : Promise.resolve([]), enabled: !!selectedService })
  const { data: status } = useQuery({ queryKey: ['middleware-status', selectedService?.type, selectedService?.id], queryFn: () => selectedService ? getMiddlewareStatus(selectedService.type, selectedService.id) : null, enabled: !!selectedService })
  const { data: history = [], isLoading: historyLoading } = useQuery({ queryKey: ['middleware-history', selectedService?.type, selectedService?.id], queryFn: () => selectedService ? getMiddlewareHistory(selectedService.type, selectedService.id) : [], enabled: !!selectedService, refetchInterval: 30_000 })

  const serviceTypes = useMemo(() => [...new Set(dashboard.map((s: MiddlewareServiceStatus) => s.service_type))], [dashboard])
  const selectedSummary = dashboard.find((s: MiddlewareServiceStatus) => s.service_type === selectedService?.type)
  const selectedInstance = instances.find((inst: any) => inst.id === selectedService?.id)
  const historyMetricKeys = getHistoryMetricKeys(history)
  const metricEntries = Object.entries(status ?? {}).filter(([, v]) => typeof v !== 'object' || Array.isArray(v)).slice(0, 9)

  const health = useMemo(() => ({
    healthy: dashboard.filter((s: MiddlewareServiceStatus) => isHealthy(s.status)).length,
    warning: dashboard.filter((s: MiddlewareServiceStatus) => isWarning(s.status)).length,
    critical: dashboard.filter((s: MiddlewareServiceStatus) => isCritical(s.status)).length,
    unknown: dashboard.filter((s: MiddlewareServiceStatus) => !isHealthy(s.status) && !isWarning(s.status) && !isCritical(s.status)).length,
  }), [dashboard])

  useEffect(() => {
    if (!selectedService && serviceTypes.length > 0) {
      const nextType = serviceTypes[0]
      setSelectedService({ type: nextType, id: fallbackServiceId(nextType) })
    }
  }, [selectedService, serviceTypes])

  useEffect(() => {
    if (!selectedService || instances.length === 0) return
    if (!instances.some((inst: any) => inst.id === selectedService.id)) {
      setSelectedService({ type: selectedService.type, id: instances[0].id })
    }
  }, [instances, selectedService])

  const selectServiceType = (type: string) => {
    const existing = selectedService?.type === type && selectedService?.id ? selectedService.id : fallbackServiceId(type)
    setSelectedService({ type, id: existing })
    setDetailTab('status')
  }

  const selectServiceInstance = (id: string) => {
    if (!selectedService) return
    setSelectedService({ type: selectedService.type, id })
    setDetailTab('status')
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-col gap-1">
        <h1 className="text-2xl font-bold text-white flex items-center gap-2"><Server className="w-6 h-6 text-blue-400" /> Middleware Monitoring</h1>
        <p className="text-sm text-slate-400">Service health, request posture, and runtime telemetry across middleware targets</p>
      </div>

      <div className="grid grid-cols-2 md:grid-cols-3 xl:grid-cols-6 gap-3">
        {[
          { label: 'Services', value: dashboard.length, icon: Database, color: 'text-blue-400' },
          { label: 'Healthy', value: health.healthy, icon: CheckCircle, color: 'text-emerald-400' },
          { label: 'Warning', value: health.warning, icon: AlertTriangle, color: 'text-amber-400' },
          { label: 'Critical', value: health.critical, icon: XCircle, color: 'text-rose-400' },
          { label: 'Unknown', value: health.unknown, icon: Clock, color: 'text-slate-400' },
          { label: 'Types', value: serviceTypes.length, icon: Layers, color: 'text-cyan-400' },
        ].map(kpi => (
          <div key={kpi.label} className="bg-white/5 backdrop-blur-sm border border-white/10 p-3 rounded-xl">
            <div className="flex items-center gap-1.5 mb-1">
              <kpi.icon className={`w-3 h-3 ${kpi.color}`} />
              <span className="text-[10px] text-slate-400 uppercase tracking-wider">{kpi.label}</span>
            </div>
            <div className={`text-2xl font-bold font-mono ${kpi.color}`}>{kpi.value}</div>
          </div>
        ))}
      </div>

      <div className="grid grid-cols-1 xl:grid-cols-[minmax(0,1fr)_360px] gap-4">
        <div className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-5">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-sm font-bold text-white uppercase tracking-wider">Middleware Service Overview</h2>
            <span className="text-xs text-slate-500">{dashboard.length} monitored services</span>
          </div>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
            {dashboard.map((s: MiddlewareServiceStatus, i: number) => {
              const Icon = SERVICE_ICONS[s.service_type] || Activity
              const color = SERVICE_COLORS[s.service_type] || 'text-slate-400'
              const isSelected = selectedService?.type === s.service_type
              return (
                <button key={i} onClick={() => selectServiceType(s.service_type)}
                  className={`rounded-xl p-4 border text-left transition-colors min-h-[150px] ${isSelected ? 'border-blue-500/70 bg-blue-500/10 ring-1 ring-blue-500/30' : 'border-white/10 bg-slate-950/20 hover:bg-white/[0.08]'}`}>
                  <div className="flex items-center justify-between mb-3">
                    <div className={`p-2 rounded-lg border ${SERVICE_ACCENTS[s.service_type] || 'border-white/10 bg-white/5'}`}><Icon className={`w-4 h-4 ${color}`} /></div>
                    <div className="flex items-center gap-2"><span className="text-xs text-slate-500 capitalize">{s.status}</span><div className={`w-2 h-2 rounded-full ${STATUS_DOT[s.status] || STATUS_DOT.unknown}`} /></div>
                  </div>
                  <h3 className="text-white font-semibold text-sm leading-snug line-clamp-2">{s.name}</h3>
                  <p className="text-slate-400 text-xs mt-1 font-mono">{s.host}:{s.port}</p>
                  <div className="grid grid-cols-2 gap-2 mt-3">
                    <div className="bg-black/20 rounded-lg p-2"><p className="text-[10px] text-slate-500">Signal</p><p className="text-xs text-slate-200 truncate">{s.key_metric_1}</p></div>
                    <div className="bg-black/20 rounded-lg p-2"><p className="text-[10px] text-slate-500">Check</p><p className="text-xs text-slate-200 truncate">{s.key_metric_2}</p></div>
                  </div>
                </button>
              )
            })}
          </div>
        </div>

        <div className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-5">
          <h2 className="text-sm font-bold text-white uppercase tracking-wider mb-4">Scope</h2>
          <div className="space-y-4">
            <div>
              <label className="block text-xs font-medium text-slate-400 mb-1.5">Service</label>
              <select
                value={selectedService?.type ?? ''}
                onChange={e => selectServiceType(e.target.value)}
                className="w-full bg-slate-950/60 border border-white/10 rounded-lg px-3 py-2.5 text-sm text-white focus:outline-none focus:ring-2 focus:ring-blue-500/50 [&>option]:bg-slate-900"
              >
                {serviceTypes.map(t => <option key={t} value={t}>{t.toUpperCase()}</option>)}
              </select>
            </div>
            <div>
              <label className="block text-xs font-medium text-slate-400 mb-1.5">Instance</label>
              <select
                value={selectedService?.id ?? ''}
                onChange={e => selectServiceInstance(e.target.value)}
                className="w-full bg-slate-950/60 border border-white/10 rounded-lg px-3 py-2.5 text-sm text-white focus:outline-none focus:ring-2 focus:ring-blue-500/50 [&>option]:bg-slate-900"
              >
                {instances.length > 0 ? instances.map((inst: any) => (
                  <option key={inst.id} value={inst.id}>{inst.name ?? inst.id} - {inst.host}:{inst.port}</option>
                )) : selectedService ? (
                  <option value={selectedService.id}>{selectedSummary?.name ?? selectedService.id} - {selectedSummary?.host}:{selectedSummary?.port}</option>
                ) : null}
              </select>
            </div>
            <div className="grid grid-cols-2 gap-2">
              {(['status', 'history'] as const).map(t => (
                <button key={t} onClick={() => setDetailTab(t)} className={`px-3 py-2.5 rounded-lg text-sm ${detailTab === t ? 'bg-blue-600 text-white' : 'bg-white/5 text-slate-300 hover:bg-white/10'}`}>{t === 'status' ? 'Status' : 'History'}</button>
              ))}
            </div>
          </div>
        </div>
      </div>

      {selectedService && status && (
        <div className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-5">
          <div className="flex flex-col gap-2 lg:flex-row lg:items-center lg:justify-between mb-5">
            <div>
              <h2 className="text-lg font-bold text-white capitalize">{selectedService.type} - {selectedInstance?.name ?? selectedSummary?.name ?? selectedService.id}</h2>
              <p className="text-xs text-slate-400 font-mono mt-1">{selectedInstance ? `${selectedInstance.host}:${selectedInstance.port}` : selectedSummary ? `${selectedSummary.host}:${selectedSummary.port}` : selectedService.id}</p>
            </div>
            <div className="inline-flex items-center gap-2 rounded-full border border-white/10 bg-slate-950/40 px-3 py-1.5 text-xs text-slate-300 w-fit">
              <div className={`w-2 h-2 rounded-full ${STATUS_DOT[selectedSummary?.status ?? 'unknown'] || STATUS_DOT.unknown}`} />
              <span className="capitalize">{selectedSummary?.status ?? 'unknown'}</span>
            </div>
          </div>

          {detailTab === 'status' && (
            <div className="space-y-4">
              <div className="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-3 gap-3">
                {metricEntries.map(([k, v]) => (
                  <div key={k} className="bg-slate-950/50 border border-white/5 rounded-xl p-4 min-h-[74px]">
                    <p className="text-slate-500 text-xs capitalize">{formatMetricName(k)}</p>
                    <p className="text-white font-mono text-lg mt-1">{formatMetricValue(v)}</p>
                  </div>
                ))}
              </div>

              <div className="grid grid-cols-1 xl:grid-cols-3 gap-3">
                {historyMetricKeys.length > 0 ? historyMetricKeys.map((key, i) => (
                  <div key={key} className="bg-slate-950/50 border border-white/5 rounded-xl p-4 min-h-[230px]">
                    <div className="flex items-center justify-between mb-3">
                      <p className="text-slate-300 text-sm font-semibold capitalize">{formatMetricName(key)}</p>
                      <TrendingUp className="w-4 h-4 text-slate-500" />
                    </div>
                    <div className="h-40">
                      <ResponsiveContainer>
                        <AreaChart data={history} margin={{ top: 8, right: 8, left: 0, bottom: 0 }}>
                          <XAxis dataKey="timestamp" tickFormatter={(t: string) => new Date(t).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })} tick={{ fill: '#94a3b8', fontSize: 10 }} />
                          <YAxis tick={{ fill: '#94a3b8', fontSize: 10 }} width={34} />
                          <Tooltip contentStyle={{ backgroundColor: '#1f2937', border: '1px solid #374151', borderRadius: '0.5rem' }} labelStyle={{ color: '#9ca3af' }} labelFormatter={(l: string) => new Date(l).toLocaleString()} />
                          <Area type="monotone" dataKey={key} stroke={CHART_COLORS[i]} fill={`${CHART_COLORS[i]}24`} strokeWidth={2} dot={false} />
                        </AreaChart>
                      </ResponsiveContainer>
                    </div>
                  </div>
                )) : (
                  <div className="xl:col-span-3 bg-slate-950/50 border border-white/5 rounded-xl p-8 min-h-[220px] flex flex-col items-center justify-center text-center">
                    <div className="p-3 rounded-xl bg-white/5 border border-white/10 mb-3"><TrendingUp className="w-7 h-7 text-slate-500" /></div>
                    <p className="text-sm font-semibold text-slate-200">{historyLoading ? 'Loading charts' : 'No chart history available'}</p>
                    <p className="text-xs text-slate-500 mt-1">Metrics will appear here when performance history is returned by the API.</p>
                  </div>
                )}
              </div>
            </div>
          )}

          {detailTab === 'history' && historyMetricKeys.length > 0 && (
            <div className="h-80 bg-slate-950/50 border border-white/5 rounded-xl p-4">
              <ResponsiveContainer>
                <AreaChart data={history} margin={{ top: 8, right: 12, left: 0, bottom: 0 }}>
                  <XAxis dataKey="timestamp" tickFormatter={(t: string) => new Date(t).toLocaleTimeString()} tick={{ fill: '#94a3b8', fontSize: 11 }} />
                  <YAxis tick={{ fill: '#94a3b8', fontSize: 11 }} />
                  <Tooltip contentStyle={{ backgroundColor: '#1f2937', border: '1px solid #374151', borderRadius: '0.5rem' }} labelStyle={{ color: '#9ca3af' }} />
                  {historyMetricKeys.map((k, i) => (
                    <Area key={k} type="monotone" dataKey={k} stroke={CHART_COLORS[i]} fill={`${CHART_COLORS[i]}24`} strokeWidth={2} dot={false} />
                  ))}
                </AreaChart>
              </ResponsiveContainer>
            </div>
          )}

          {detailTab === 'history' && historyMetricKeys.length === 0 && (
            <div className="bg-slate-950/50 border border-white/5 rounded-xl p-8 min-h-[220px] flex flex-col items-center justify-center text-center">
              <div className="p-3 rounded-xl bg-white/5 border border-white/10 mb-3"><TrendingUp className="w-7 h-7 text-slate-500" /></div>
              <p className="text-sm font-semibold text-slate-200">{historyLoading ? 'Loading history' : 'No history available'}</p>
              <p className="text-xs text-slate-500 mt-1">This service has status metrics, but the API has not returned time-series samples yet.</p>
            </div>
          )}
        </div>
      )}
    </div>
  )
}
