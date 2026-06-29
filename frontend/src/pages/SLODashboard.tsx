import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { fetchJSON } from '@/lib/api'
import { Target, Zap, Plus, Trash2, ChevronDown } from 'lucide-react'

// ─── API helpers ─────────────────────────────────────────────────────────────

interface ServiceRed {
  service_name: string
  total_requests: number
  error_count: number
  error_rate_pct: number
  p50_ms: number | null
  p95_ms: number | null
  p99_ms: number | null
  req_per_sec: number
}

interface SloTarget {
  id: string
  service_name: string
  metric: string
  threshold: number
  window_days: number
}

interface SloServicesResp { services: ServiceRed[]; count: number }
interface SloTargetsResp  { targets: SloTarget[]; count: number }
interface SloBudget {
  service_name: string
  window_days: number
  total_requests: number
  error_rate_pct: number
  availability_pct: number
  slo_target_availability_pct: number
  budget_seconds_remaining: number
  burn_rate: number
  slo_status: 'ok' | 'breached'
}

const getSloServices = (window: string) =>
  fetchJSON<SloServicesResp>(`/slo/services?window=${window}`)
const getSloTargets  = () => fetchJSON<SloTargetsResp>('/slo/targets')
const getSlobudget   = (svc: string) => fetchJSON<SloBudget>(`/slo/budget/${svc}`)
const createTarget   = (d: { service_name: string; metric: string; threshold: number; window_days: number }) =>
  fetchJSON('/slo/targets', { method: 'POST', body: JSON.stringify(d) })
const deleteTarget   = (id: string) =>
  fetchJSON(`/slo/targets/${id}`, { method: 'DELETE' })

// ─── Helpers ──────────────────────────────────────────────────────────────────

const fmtSecs = (s: number) => {
  if (s >= 3600) return `${(s / 3600).toFixed(1)}h`
  if (s >= 60)   return `${Math.floor(s / 60)}m`
  return `${s}s`
}

// ─── Component ────────────────────────────────────────────────────────────────

export default function SLODashboardPage() {
  const qc = useQueryClient()
  const [window, setWindow] = useState('1h')
  const [selectedService, setSelectedService] = useState<string | null>(null)
  const [showAddTarget, setShowAddTarget] = useState(false)
  const [newSvc, setNewSvc] = useState('')
  const [newMetric, setNewMetric] = useState('error_rate')
  const [newThreshold, setNewThreshold] = useState<number>(1)
  const [newWindowDays, setNewWindowDays] = useState(30)

  const { data: svcData, isLoading: svcsLoading } = useQuery({
    queryKey: ['slo-services', window],
    queryFn: () => getSloServices(window),
    refetchInterval: 30_000,
  })
  const { data: targetsData } = useQuery({ queryKey: ['slo-targets'], queryFn: getSloTargets })
  const { data: budget } = useQuery({
    queryKey: ['slo-budget', selectedService],
    queryFn: () => getSlobudget(selectedService!),
    enabled: !!selectedService,
  })

  const createMut = useMutation({
    mutationFn: () => createTarget({ service_name: newSvc, metric: newMetric, threshold: newThreshold, window_days: newWindowDays }),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['slo-targets'] }); setShowAddTarget(false); setNewSvc('') },
  })
  const deleteMut = useMutation({
    mutationFn: (id: string) => deleteTarget(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['slo-targets'] }),
  })

  const services = svcData?.services ?? []
  const targets  = targetsData?.targets ?? []

  const sloMap: Record<string, SloTarget[]> = {}
  for (const t of targets) {
    if (!sloMap[t.service_name]) sloMap[t.service_name] = []
    sloMap[t.service_name].push(t)
  }

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white flex items-center gap-2">
            <Target className="w-6 h-6 text-violet-400" />
            SLO / SLI Dashboard
          </h1>
          <p className="text-sm text-slate-400 mt-0.5">Service Level Objectives from OTEL RED metrics</p>
        </div>
        <div className="flex items-center gap-2">
          {/* Window picker */}
          <select
            value={window}
            onChange={e => setWindow(e.target.value)}
            className="bg-white/5 border border-white/10 rounded-lg px-3 py-1.5 text-sm text-white"
          >
            {['15m','1h','6h','24h'].map(w => <option key={w} value={w}>{w}</option>)}
          </select>
          <button
            onClick={() => setShowAddTarget(s => !s)}
            className="flex items-center gap-1.5 px-3 py-1.5 bg-violet-600 hover:bg-violet-500 text-white rounded-lg text-sm transition-colors"
          >
            <Plus className="w-4 h-4" /> Add SLO Target
          </button>
        </div>
      </div>

      {/* Add SLO Target form */}
      {showAddTarget && (
        <div className="bg-white/5 border border-white/10 rounded-xl p-4 grid grid-cols-2 md:grid-cols-5 gap-3 items-end">
          <div>
            <label className="text-xs text-slate-400 mb-1 block">Service</label>
            <input
              value={newSvc} onChange={e => setNewSvc(e.target.value)}
              placeholder="e.g. checkout"
              className="w-full bg-white/5 border border-white/10 rounded px-2 py-1.5 text-xs text-white"
            />
          </div>
          <div>
            <label className="text-xs text-slate-400 mb-1 block">Metric</label>
            <select value={newMetric} onChange={e => setNewMetric(e.target.value)}
              className="w-full bg-white/5 border border-white/10 rounded px-2 py-1.5 text-xs text-white">
              <option value="error_rate">Error Rate (%)</option>
              <option value="p99_latency">P99 Latency (ms)</option>
              <option value="p95_latency">P95 Latency (ms)</option>
              <option value="availability">Availability (%)</option>
            </select>
          </div>
          <div>
            <label className="text-xs text-slate-400 mb-1 block">Threshold</label>
            <input
              type="number" value={newThreshold} onChange={e => setNewThreshold(Number(e.target.value))}
              className="w-full bg-white/5 border border-white/10 rounded px-2 py-1.5 text-xs text-white"
            />
          </div>
          <div>
            <label className="text-xs text-slate-400 mb-1 block">Window (days)</label>
            <input
              type="number" value={newWindowDays} onChange={e => setNewWindowDays(Number(e.target.value))}
              className="w-full bg-white/5 border border-white/10 rounded px-2 py-1.5 text-xs text-white"
            />
          </div>
          <button
            onClick={() => createMut.mutate()}
            disabled={!newSvc || createMut.isPending}
            className="px-4 py-1.5 bg-violet-600 hover:bg-violet-500 disabled:opacity-40 text-white rounded text-xs transition-colors"
          >
            Save
          </button>
        </div>
      )}

      {/* Service RED metrics table */}
      <div className="bg-white/5 border border-white/10 rounded-xl overflow-hidden">
        <div className="px-4 py-3 border-b border-white/10 flex items-center justify-between">
          <h2 className="text-sm font-semibold text-white">Service RED Metrics</h2>
          <span className="text-xs text-slate-500">{services.length} services · window: {window}</span>
        </div>
        {svcsLoading ? (
          <div className="px-4 py-8 text-center text-sm text-slate-500">Loading…</div>
        ) : services.length === 0 ? (
          <div className="px-4 py-8 text-center text-sm text-slate-500">
            No trace data found for window <strong>{window}</strong>. Send OTEL spans to populate this table.
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-white/10 text-xs text-slate-400 uppercase tracking-wider">
                  <th className="px-4 py-2 text-left">Service</th>
                  <th className="px-4 py-2 text-right">Req/s</th>
                  <th className="px-4 py-2 text-right">Errors</th>
                  <th className="px-4 py-2 text-right">Error Rate</th>
                  <th className="px-4 py-2 text-right">P50</th>
                  <th className="px-4 py-2 text-right">P95</th>
                  <th className="px-4 py-2 text-right">P99</th>
                  <th className="px-4 py-2 text-right">SLO Targets</th>
                  <th className="px-4 py-2 text-right">Budget</th>
                </tr>
              </thead>
              <tbody>
                {services.map(svc => {
                  const tgts = sloMap[svc.service_name] ?? []
                  const isErrory = svc.error_rate_pct > 5
                  return (
                    <tr
                      key={svc.service_name}
                      onClick={() => setSelectedService(s => s === svc.service_name ? null : svc.service_name)}
                      className="border-b border-white/5 hover:bg-white/5 cursor-pointer transition-colors"
                    >
                      <td className="px-4 py-3 font-medium text-white">
                        <div className="flex items-center gap-2">
                          <div className={`w-2 h-2 rounded-full ${isErrory ? 'bg-red-500' : 'bg-emerald-500'}`} />
                          {svc.service_name}
                        </div>
                      </td>
                      <td className="px-4 py-3 text-right text-slate-300 font-mono text-xs">{svc.req_per_sec}</td>
                      <td className="px-4 py-3 text-right text-slate-300 font-mono text-xs">{svc.error_count}</td>
                      <td className="px-4 py-3 text-right font-mono text-xs">
                        <span className={isErrory ? 'text-red-400' : 'text-emerald-400'}>
                          {svc.error_rate_pct.toFixed(2)}%
                        </span>
                      </td>
                      <td className="px-4 py-3 text-right text-slate-400 font-mono text-xs">{svc.p50_ms?.toFixed(1) ?? '—'}ms</td>
                      <td className="px-4 py-3 text-right text-slate-400 font-mono text-xs">{svc.p95_ms?.toFixed(1) ?? '—'}ms</td>
                      <td className="px-4 py-3 text-right font-mono text-xs">
                        <span className={svc.p99_ms && svc.p99_ms > 1000 ? 'text-amber-400' : 'text-slate-300'}>
                          {svc.p99_ms?.toFixed(1) ?? '—'}ms
                        </span>
                      </td>
                      <td className="px-4 py-3 text-right text-xs text-slate-400">{tgts.length > 0 ? `${tgts.length} target${tgts.length > 1 ? 's' : ''}` : '—'}</td>
                      <td className="px-4 py-3 text-right">
                        <ChevronDown className={`w-3 h-3 text-slate-400 transition-transform ${selectedService === svc.service_name ? 'rotate-180' : ''}`} />
                      </td>
                    </tr>
                  )
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Budget detail panel */}
      {selectedService && budget && (
        <div className="bg-white/5 border border-white/10 rounded-xl p-5 space-y-4">
          <div className="flex items-center justify-between">
            <h3 className="font-semibold text-white flex items-center gap-2">
              <Zap className={`w-4 h-4 ${budget.slo_status === 'ok' ? 'text-emerald-400' : 'text-red-400'}`} />
              Error Budget — <span className="text-violet-300">{selectedService}</span>
            </h3>
            <span className={`text-xs px-2 py-0.5 rounded-full font-medium ${
              budget.slo_status === 'ok' ? 'bg-emerald-500/20 text-emerald-300' : 'bg-red-500/20 text-red-300'
            }`}>
              {budget.slo_status === 'ok' ? '✓ SLO Met' : '✗ SLO Breached'}
            </span>
          </div>

          <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
            {[
              { label: 'Availability', value: `${budget.availability_pct.toFixed(3)}%`, target: `Target: ${budget.slo_target_availability_pct}%`, ok: budget.availability_pct >= budget.slo_target_availability_pct },
              { label: 'Error Rate', value: `${budget.error_rate_pct.toFixed(2)}%`, target: `${budget.total_requests} total requests`, ok: budget.error_rate_pct < 5 },
              { label: 'Budget Remaining', value: fmtSecs(budget.budget_seconds_remaining), target: `Window: ${budget.window_days}d`, ok: budget.budget_seconds_remaining > 0 },
              { label: 'Burn Rate', value: `${budget.burn_rate.toFixed(2)}×`, target: budget.burn_rate <= 1 ? 'Normal' : 'Burning fast!', ok: budget.burn_rate <= 1 },
            ].map((m, i) => (
              <div key={i} className="bg-white/5 rounded-lg p-3">
                <div className="text-[10px] text-slate-400 uppercase tracking-wider mb-1">{m.label}</div>
                <div className={`text-lg font-bold font-mono ${m.ok ? 'text-emerald-400' : 'text-red-400'}`}>{m.value}</div>
                <div className="text-[10px] text-slate-500 mt-0.5">{m.target}</div>
              </div>
            ))}
          </div>

          {/* Budget bar */}
          <div>
            <div className="flex justify-between text-xs text-slate-400 mb-1">
              <span>Error Budget Burn</span>
              <span>{Math.min(budget.burn_rate * 100, 100).toFixed(1)}% used</span>
            </div>
            <div className="h-2 bg-white/10 rounded-full overflow-hidden">
              <div
                className={`h-full rounded-full transition-all ${budget.slo_status === 'ok' ? 'bg-emerald-500' : 'bg-red-500'}`}
                style={{ width: `${Math.min(budget.burn_rate * 100, 100)}%` }}
              />
            </div>
          </div>
        </div>
      )}

      {/* SLO Targets table */}
      {targets.length > 0 && (
        <div className="bg-white/5 border border-white/10 rounded-xl overflow-hidden">
          <div className="px-4 py-3 border-b border-white/10">
            <h2 className="text-sm font-semibold text-white">Configured SLO Targets</h2>
          </div>
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-white/10 text-xs text-slate-400 uppercase">
                <th className="px-4 py-2 text-left">Service</th>
                <th className="px-4 py-2 text-left">Metric</th>
                <th className="px-4 py-2 text-right">Threshold</th>
                <th className="px-4 py-2 text-right">Window</th>
                <th className="px-4 py-2 text-right"></th>
              </tr>
            </thead>
            <tbody>
              {targets.map(t => (
                <tr key={t.id} className="border-b border-white/5 hover:bg-white/5">
                  <td className="px-4 py-2 text-white">{t.service_name}</td>
                  <td className="px-4 py-2 text-slate-300">{t.metric}</td>
                  <td className="px-4 py-2 text-right text-slate-300 font-mono">{t.threshold}</td>
                  <td className="px-4 py-2 text-right text-slate-400">{t.window_days}d</td>
                  <td className="px-4 py-2 text-right">
                    <button onClick={() => deleteMut.mutate(t.id)} className="text-slate-500 hover:text-red-400 transition-colors">
                      <Trash2 className="w-3.5 h-3.5" />
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}
