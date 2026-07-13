import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Server, Activity, Globe, Layers, TrendingUp } from 'lucide-react'
import { getMiddlewareDashboard, getMiddlewareInstances, getMiddlewareStatus, getMiddlewareHistory, type MiddlewareServiceStatus } from '@/lib/api'
import { AreaChart, Area, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts'

const SERVICE_ICONS: Record<string, any> = { nginx: Globe, apache: Globe, haproxy: Layers }
const SERVICE_COLORS: Record<string, string> = { nginx: 'text-green-400', apache: 'text-red-400', haproxy: 'text-cyan-400' }
const STATUS_COLORS: Record<string, string> = { healthy: 'bg-green-500', warning: 'bg-yellow-500', critical: 'bg-red-500', unknown: 'bg-gray-500' }
const CHART_COLORS = ['#3b82f6', '#10b981', '#f59e0b']

function formatMetricName(key: string) {
  return key.replace(/_/g, ' ')
}

function getHistoryMetricKeys(history: any[]) {
  if (history.length === 0) return []
  return Object.keys(history[0])
    .filter(k => k !== 'timestamp' && typeof history[0][k] === 'number')
    .slice(0, 3)
}

export default function MiddlewareMonitoringPage() {
  const [selectedService, setSelectedService] = useState<{ type: string; id: string } | null>(null)
  const [detailTab, setDetailTab] = useState<'status' | 'history'>('status')
  const { data: dashboard = [] } = useQuery({ queryKey: ['middleware-dashboard'], queryFn: getMiddlewareDashboard })
  const { data: instances = [] } = useQuery({ queryKey: ['middleware-instances', selectedService?.type], queryFn: () => selectedService ? getMiddlewareInstances(selectedService.type) : Promise.resolve([]), enabled: !!selectedService })
  const { data: status } = useQuery({ queryKey: ['middleware-status', selectedService?.type, selectedService?.id], queryFn: () => selectedService ? getMiddlewareStatus(selectedService.type, selectedService.id) : null, enabled: !!selectedService })
  const { data: history = [], isLoading: historyLoading } = useQuery({ queryKey: ['middleware-history', selectedService?.type, selectedService?.id], queryFn: () => selectedService ? getMiddlewareHistory(selectedService.type, selectedService.id) : [], enabled: !!selectedService, refetchInterval: 30_000 })

  const serviceTypes = [...new Set(dashboard.map((s: MiddlewareServiceStatus) => s.service_type))]
  const historyMetricKeys = getHistoryMetricKeys(history)

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold text-white flex items-center gap-2"><Server className="w-6 h-6 text-blue-400" /> Middleware Monitoring</h1>

      <div className="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-4 gap-4">
        {dashboard.map((s: MiddlewareServiceStatus, i: number) => {
          const Icon = SERVICE_ICONS[s.service_type] || Activity
          const color = SERVICE_COLORS[s.service_type] || 'text-gray-400'
          return (
            <button key={i} onClick={() => { const inst = instances.length > 0 ? instances[0] : null; setSelectedService({ type: s.service_type, id: inst?.id || `${s.service_type}-001` }) }}
              className="bg-gray-800 rounded-lg p-4 border border-gray-700 hover:border-gray-500 text-left transition-colors min-h-[148px]">
              <div className="flex items-center justify-between mb-2">
                <Icon className={`w-5 h-5 ${color}`} />
                <div className={`w-2 h-2 rounded-full ${STATUS_COLORS[s.status] || 'bg-gray-500'}`} />
              </div>
              <h3 className="text-white font-medium text-sm leading-snug">{s.name}</h3>
              <p className="text-gray-400 text-xs mt-1">{s.host}:{s.port}</p>
              <div className="mt-2 space-y-0.5">
                <p className="text-gray-300 text-xs">{s.key_metric_1}</p>
                <p className="text-gray-300 text-xs">{s.key_metric_2}</p>
              </div>
            </button>
          )
        })}
      </div>

      <div className="flex gap-2 flex-wrap">
        {serviceTypes.map(t => {
          const Icon = SERVICE_ICONS[t] || Activity
          return <button key={t} onClick={() => setSelectedService({ type: t, id: `${t}-001` })} className={`flex items-center gap-1.5 px-3 py-1.5 rounded text-sm transition-colors ${selectedService?.type === t ? 'bg-blue-600 text-white' : 'bg-gray-700 text-gray-300 hover:bg-gray-600'}`}><Icon className="w-3.5 h-3.5" />{t}</button>
        })}
      </div>

      {selectedService && status && (
        <div className="bg-gray-800 rounded-lg border border-gray-700 p-6">
          <div className="flex items-center justify-between mb-4">
            <h2 className="text-lg font-bold text-white capitalize">{selectedService.type} - {selectedService.id}</h2>
            <div className="flex gap-2">
              {(['status', 'history'] as const).map(t => (
                <button key={t} onClick={() => setDetailTab(t)} className={`px-3 py-1.5 rounded text-sm ${detailTab === t ? 'bg-blue-600 text-white' : 'bg-gray-700 text-gray-300'}`}>{t === 'status' ? 'Status' : 'History'}</button>
              ))}
            </div>
          </div>

          {detailTab === 'status' && (
            <div className="space-y-4">
              <div className="grid grid-cols-1 sm:grid-cols-2 xl:grid-cols-3 gap-3">
                {Object.entries(status).filter(([k]) => typeof (status as any)[k] !== 'object' || Array.isArray((status as any)[k])).map(([k, v]) => (
                  <div key={k} className="bg-gray-900 rounded p-3 min-h-[64px]">
                    <p className="text-gray-500 text-xs">{formatMetricName(k)}</p>
                    <p className="text-white font-mono text-sm mt-1">{typeof v === 'number' ? (v % 1 === 0 ? v.toLocaleString() : (v as number).toFixed(2)) : String(v)}</p>
                  </div>
                ))}
              </div>

              <div className="grid grid-cols-1 xl:grid-cols-3 gap-3">
                {historyMetricKeys.length > 0 ? historyMetricKeys.map((key, i) => (
                  <div key={key} className="bg-gray-900 rounded p-4 min-h-[220px]">
                    <div className="flex items-center justify-between mb-3">
                      <p className="text-gray-300 text-sm font-medium capitalize">{formatMetricName(key)}</p>
                      <TrendingUp className="w-4 h-4 text-gray-500" />
                    </div>
                    <div className="h-40">
                      <ResponsiveContainer>
                        <AreaChart data={history} margin={{ top: 8, right: 8, left: 0, bottom: 0 }}>
                          <XAxis dataKey="timestamp" tickFormatter={(t: string) => new Date(t).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })} tick={{ fill: '#9ca3af', fontSize: 10 }} />
                          <YAxis tick={{ fill: '#9ca3af', fontSize: 10 }} width={34} />
                          <Tooltip contentStyle={{ backgroundColor: '#1f2937', border: '1px solid #374151', borderRadius: '0.5rem' }} labelStyle={{ color: '#9ca3af' }} labelFormatter={(l: string) => new Date(l).toLocaleString()} />
                          <Area type="monotone" dataKey={key} stroke={CHART_COLORS[i]} fill={`${CHART_COLORS[i]}24`} strokeWidth={2} dot={false} />
                        </AreaChart>
                      </ResponsiveContainer>
                    </div>
                  </div>
                )) : (
                  <div className="xl:col-span-3 bg-gray-900 rounded p-6 min-h-[180px] flex flex-col items-center justify-center text-center">
                    <TrendingUp className="w-8 h-8 text-gray-600 mb-3" />
                    <p className="text-sm font-medium text-gray-300">{historyLoading ? 'Loading charts' : 'No chart history available'}</p>
                    <p className="text-xs text-gray-500 mt-1">Metrics will appear here when performance history is returned by the API.</p>
                  </div>
                )}
              </div>
            </div>
          )}

          {detailTab === 'history' && historyMetricKeys.length > 0 && (
            <div className="h-72 bg-gray-900 rounded p-4">
              <ResponsiveContainer>
                <AreaChart data={history} margin={{ top: 8, right: 12, left: 0, bottom: 0 }}>
                  <XAxis dataKey="timestamp" tickFormatter={(t: string) => new Date(t).toLocaleTimeString()} tick={{ fill: '#9ca3af', fontSize: 11 }} />
                  <YAxis tick={{ fill: '#9ca3af', fontSize: 11 }} />
                  <Tooltip contentStyle={{ backgroundColor: '#1f2937', border: '1px solid #374151', borderRadius: '0.5rem' }} labelStyle={{ color: '#9ca3af' }} />
                  {historyMetricKeys.map((k, i) => (
                    <Area key={k} type="monotone" dataKey={k} stroke={CHART_COLORS[i]} fill={`${CHART_COLORS[i]}24`} strokeWidth={2} dot={false} />
                  ))}
                </AreaChart>
              </ResponsiveContainer>
            </div>
          )}

          {detailTab === 'history' && historyMetricKeys.length === 0 && (
            <div className="bg-gray-900 rounded p-6 min-h-[180px] flex flex-col items-center justify-center text-center">
              <TrendingUp className="w-8 h-8 text-gray-600 mb-3" />
              <p className="text-sm font-medium text-gray-300">{historyLoading ? 'Loading history' : 'No history available'}</p>
              <p className="text-xs text-gray-500 mt-1">This service has status metrics, but the API has not returned time-series samples yet.</p>
            </div>
          )}
        </div>
      )}
    </div>
  )
}
