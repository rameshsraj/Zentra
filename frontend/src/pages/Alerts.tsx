import { useState, useEffect } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { getAlerts, acknowledgeAlert, resolveAlert, fetchJSON } from '@/lib/api'
import MetricAreaChart from '@/components/charts/MetricAreaChart'
import PageHeader from '@/components/PageHeader'
import { Bell, CheckCircle, Eye, Filter, X, ChevronDown, ChevronRight, Layers, ShieldOff, Plus, Trash2 } from 'lucide-react'
import LoadingSkeleton from '@/components/LoadingSkeleton'
import { cn } from '@/lib/utils'
import { useNavigate } from 'react-router-dom'

interface InhibitionRule { id: string; source_match: Record<string,string>; target_match: Record<string,string>; equal: string[] }

export default function AlertsPage() {
  const qc = useQueryClient()
  const navigate = useNavigate()
  const { data: alerts, isLoading } = useQuery({ queryKey: ['alerts'], queryFn: getAlerts })

  const [search, setSearch] = useState('')
  const [severityFilter, setSeverityFilter] = useState<string[]>([])
  const [statusFilter, setStatusFilter] = useState<string[]>([])
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [viewMode, setViewMode] = useState<'flat' | 'grouped'>('flat')
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set())
  const [showInhibitions, setShowInhibitions] = useState(false)
  const [inhibitions, setInhibitions] = useState<InhibitionRule[]>([])

  useEffect(() => {
    fetchJSON<any>('/alerts/inhibitions').then((res: any) => {
      setInhibitions(Array.isArray(res) ? res : res?.rules ?? [])
    }).catch(() => {})
  }, [])

  const ackMutation = useMutation({
    mutationFn: acknowledgeAlert,
    onSuccess: () => qc.invalidateQueries({ queryKey: ['alerts'] }),
  })
  const resolveMutation = useMutation({
    mutationFn: resolveAlert,
    onSuccess: () => qc.invalidateQueries({ queryKey: ['alerts'] }),
  })

  const filtered = (alerts ?? []).filter(a => {
    if (search && !a.message.toLowerCase().includes(search.toLowerCase())) return false
    if (severityFilter.length && !severityFilter.includes(a.severity)) return false
    if (statusFilter.length && !statusFilter.includes(a.status)) return false
    return true
  })

  const active = (alerts ?? []).filter(a => a.status === 'triggered')
  const acknowledged = (alerts ?? []).filter(a => a.status === 'acknowledged')
  const resolved = (alerts ?? []).filter(a => a.status === 'resolved')
  const selected = selectedId ? (alerts ?? []).find(a => a.alert_id === selectedId) : null

  const toggleFilter = (arr: string[], val: string, setter: (v: string[]) => void) => {
    setter(arr.includes(val) ? arr.filter(v => v !== val) : [...arr, val])
  }

  return (
    <div className="space-y-6">
      <PageHeader
        title="Alert Dashboard"
        subtitle={`${active.length} active \u00b7 ${acknowledged.length} acknowledged \u00b7 ${resolved.length} resolved`}
        onRefresh={() => qc.invalidateQueries({ queryKey: ['alerts'] })}
        onSearch={setSearch}
        searchPlaceholder="Search alerts..."
        showAddTarget
        onAddTarget={() => navigate('/rules')}
        addTargetLabel="Create Alert Rule"
      />

      {/* KPI + Trend */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
        <div className="lg:col-span-4 grid grid-cols-2 gap-3">
          {[
            { label: 'Active', value: String(active.length), color: 'text-red-400', bg: 'bg-red-500/10 border-red-500/20' },
            { label: 'Acknowledged', value: String(acknowledged.length), color: 'text-amber-400', bg: 'bg-amber-500/10 border-amber-500/20' },
            { label: 'Resolved', value: String(resolved.length), color: 'text-emerald-400', bg: 'bg-emerald-500/10 border-emerald-500/20' },
            { label: 'Total', value: String((alerts ?? []).length), color: 'text-white', bg: 'bg-white/5 border-white/10' },
          ].map((kpi, i) => (
            <div key={i} className={`border rounded-xl p-4 backdrop-blur-sm ${kpi.bg}`}>
              <div className="text-[10px] text-slate-400 uppercase tracking-wider mb-1">{kpi.label}</div>
              <div className={`text-3xl font-bold font-mono ${kpi.color}`}>{kpi.value}</div>
            </div>
          ))}
        </div>
        <div className="lg:col-span-8">
          <div className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-5 h-full">
            <h3 className="text-sm font-bold text-white uppercase tracking-wider mb-3 flex items-center gap-2">
              <Bell className="w-4 h-4 text-amber-400" />
              Alert Trend (24h)
            </h3>
            <MetricAreaChart
              data={(alerts ?? []).reduce<{ time: string; alerts: number }[]>((acc, a) => {
                const h = new Date((a as any).triggered_at ?? (a as any).created_at ?? Date.now()).getHours()
                const t = `${String(h).padStart(2, '0')}:00`
                const existing = acc.find(x => x.time === t)
                if (existing) existing.alerts += 1
                else acc.push({ time: t, alerts: 1 })
                return acc
              }, []).sort((a, b) => a.time.localeCompare(b.time))}
              dataKey="alerts" color="#f59e0b" height={140}
            />
          </div>
        </div>
      </div>


      {/* Filters */}
      <div className="flex flex-wrap items-center gap-3" role="toolbar" aria-label="Alert filters">
        <div className="flex items-center gap-1.5">
          <Filter className="w-3.5 h-3.5 text-slate-500" />
          {['critical', 'warning', 'info'].map(sev => (
            <button
              key={sev}
              onClick={() => toggleFilter(severityFilter, sev, setSeverityFilter)}
              aria-pressed={severityFilter.includes(sev)}
              aria-label={`Filter by ${sev} severity`}
              className={`px-2.5 py-1 text-[10px] rounded-lg border transition-colors ${
                severityFilter.includes(sev)
                  ? sev === 'critical' ? 'bg-red-500/20 text-red-400 border-red-500/30' :
                    sev === 'warning' ? 'bg-amber-500/20 text-amber-400 border-amber-500/30' :
                    'bg-blue-500/20 text-blue-400 border-blue-500/30'
                  : 'bg-white/5 text-slate-500 border-white/10 hover:text-white'
              }`}
            >
              {sev}
            </button>
          ))}
          <span className="w-px h-4 bg-white/10 mx-1" />
          {['triggered', 'acknowledged', 'resolved'].map(st => (
            <button
              key={st}
              onClick={() => toggleFilter(statusFilter, st, setStatusFilter)}
              aria-pressed={statusFilter.includes(st)}
              aria-label={`Filter by ${st} status`}
              className={`px-2.5 py-1 text-[10px] rounded-lg border transition-colors ${
                statusFilter.includes(st)
                  ? 'bg-brand-600/20 text-brand-400 border-brand-500/30'
                  : 'bg-white/5 text-slate-500 border-white/10 hover:text-white'
              }`}
            >
              {st}
            </button>
          ))}
        </div>
        <div className="ml-auto flex items-center gap-2">
          <button onClick={() => setViewMode(viewMode === 'flat' ? 'grouped' : 'flat')} className={cn('flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs border transition-colors', viewMode === 'grouped' ? 'bg-cyan-600/20 text-cyan-400 border-cyan-500/30' : 'bg-white/5 text-slate-400 border-white/10 hover:text-white')}>
            <Layers className="w-3 h-3" /> {viewMode === 'grouped' ? 'Grouped' : 'Flat'}
          </button>
          <button onClick={() => setShowInhibitions(!showInhibitions)} className={cn('flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs border transition-colors', showInhibitions ? 'bg-purple-600/20 text-purple-400 border-purple-500/30' : 'bg-white/5 text-slate-400 border-white/10 hover:text-white')}>
            <ShieldOff className="w-3 h-3" /> Inhibitions ({inhibitions.length})
          </button>
        </div>
      </div>

      {/* Inhibition Rules Panel */}
      {showInhibitions && (
        <div className="bg-white/5 border border-white/10 rounded-xl p-5 space-y-3">
          <div className="flex items-center justify-between">
            <h3 className="text-sm font-bold text-white flex items-center gap-2"><ShieldOff className="w-4 h-4 text-purple-400" /> Inhibition Rules</h3>
            <button className="flex items-center gap-1 px-3 py-1.5 bg-purple-600/20 text-purple-400 rounded text-xs hover:bg-purple-600/30"><Plus className="w-3 h-3" /> Add Rule</button>
          </div>
          {inhibitions.length === 0 ? (
            <p className="text-xs text-slate-500">No inhibition rules. Inhibition rules suppress target alerts when source alerts are firing with matching labels.</p>
          ) : (
            <div className="space-y-2">
              {inhibitions.map(rule => (
                <div key={rule.id} className="flex items-center gap-3 bg-slate-800 rounded-lg p-3 border border-white/5">
                  <div className="flex-1">
                    <div className="text-xs text-slate-300">Source: <span className="text-red-400 font-mono">{JSON.stringify(rule.source_match)}</span></div>
                    <div className="text-xs text-slate-300">Target: <span className="text-amber-400 font-mono">{JSON.stringify(rule.target_match)}</span></div>
                    <div className="text-[10px] text-slate-500">Equal labels: {rule.equal.join(', ')}</div>
                  </div>
                  <button onClick={() => { fetch(`/api/alerts/inhibitions/${rule.id}`, { method: 'DELETE' }).then(() => setInhibitions(prev => prev.filter(r => r.id !== rule.id))) }} className="p-1.5 rounded hover:bg-red-600/20 text-slate-500 hover:text-red-400"><Trash2 className="w-3.5 h-3.5" /></button>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* Alert Table + Detail Panel */}
      <div className="flex gap-6">
        <div className={`flex-1 bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl overflow-hidden ${selected ? 'max-w-[60%]' : ''}`}>
          {isLoading ? (
            <LoadingSkeleton variant="table" count={5} />
          ) : filtered.length === 0 ? (
            <div className="text-center py-12">
              <Bell className="w-12 h-12 text-slate-600 mx-auto mb-3" />
              <p className="text-slate-400">No alerts match your filters</p>
            </div>
          ) : (
            <div className="overflow-x-auto">
              {viewMode === 'grouped' ? (
                /* Grouped View */
                <div className="divide-y divide-white/5">
                  {Object.entries(
                    filtered.reduce<Record<string, typeof filtered>>((groups, alert) => {
                      const key = alert.severity
                      if (!groups[key]) groups[key] = []
                      groups[key].push(alert)
                      return groups
                    }, {})
                  ).map(([groupKey, groupAlerts]) => {
                    const isExp = expandedGroups.has(groupKey)
                    return (
                      <div key={groupKey}>
                        <button
                          onClick={() => setExpandedGroups(prev => {
                            const next = new Set(prev)
                            if (next.has(groupKey)) next.delete(groupKey); else next.add(groupKey)
                            return next
                          })}
                          className="w-full flex items-center gap-2 px-4 py-3 hover:bg-white/5 transition-colors"
                        >
                          {isExp ? <ChevronDown className="w-4 h-4 text-slate-400" /> : <ChevronRight className="w-4 h-4 text-slate-400" />}
                          <span className={cn('px-2 py-0.5 rounded-full text-[10px] font-semibold border',
                            groupKey === 'critical' ? 'bg-red-500/20 text-red-400 border-red-500/30' :
                            groupKey === 'warning' ? 'bg-amber-500/20 text-amber-400 border-amber-500/30' :
                            'bg-blue-500/20 text-blue-400 border-blue-500/30'
                          )}>{groupKey}</span>
                          <span className="text-xs text-slate-400">{groupAlerts.length} alerts</span>
                        </button>
                        {isExp && groupAlerts.map(alert => (
                          <div
                            key={alert.alert_id}
                            onClick={() => setSelectedId(alert.alert_id === selectedId ? null : alert.alert_id)}
                            className={cn('flex items-center gap-4 px-8 py-2 text-xs cursor-pointer hover:bg-white/5 border-t border-white/5', alert.alert_id === selectedId && 'bg-brand-600/10')}
                          >
                            <span className="text-slate-300 truncate flex-1">{alert.message}</span>
                            <span className="font-mono text-slate-500">{(alert.metric_value ?? 0).toFixed(1)}</span>
                            <span className={cn('px-2 py-0.5 rounded text-[10px] font-semibold',
                              alert.status === 'triggered' ? 'bg-red-500/10 text-red-400' :
                              alert.status === 'acknowledged' ? 'bg-amber-500/10 text-amber-400' :
                              'bg-emerald-500/10 text-emerald-400'
                            )}>{alert.status}</span>
                          </div>
                        ))}
                      </div>
                    )
                  })}
                </div>
              ) : (
                /* Flat View */
                <table className="w-full text-sm">
                  <thead>
                    <tr className="bg-white/5">
                      {['Severity', 'Message', 'Value', 'Status', 'Time', 'Actions'].map(h => (
                        <th key={h} className="px-4 py-3 text-left text-xs font-semibold text-slate-400 uppercase tracking-wider">{h}</th>
                      ))}
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-white/5">
                    {filtered.map(alert => (
                      <tr
                        key={alert.alert_id}
                        onClick={() => setSelectedId(alert.alert_id === selectedId ? null : alert.alert_id)}
                        className={`hover:bg-white/5 transition-colors cursor-pointer ${alert.alert_id === selectedId ? 'bg-brand-600/10' : ''}`}
                      >
                        <td className="px-4 py-2.5">
                          <span className={`px-2 py-0.5 rounded-full text-[10px] font-semibold border ${
                            alert.severity === 'critical' ? 'bg-red-500/20 text-red-400 border-red-500/30' :
                            alert.severity === 'warning' ? 'bg-amber-500/20 text-amber-400 border-amber-500/30' :
                            'bg-blue-500/20 text-blue-400 border-blue-500/30'
                          }`}>{alert.severity}</span>
                        </td>
                        <td className="px-4 py-2.5 text-xs text-slate-300 max-w-xs truncate">{alert.message}</td>
                        <td className="px-4 py-2.5 font-mono text-xs text-slate-400">
                          {(alert.metric_value ?? 0).toFixed(1)} / {(alert.threshold ?? 0).toFixed(1)}
                        </td>
                        <td className="px-4 py-2.5">
                          <span className={`px-2 py-0.5 rounded text-[10px] font-semibold ${
                            alert.status === 'triggered' ? 'bg-red-500/10 text-red-400' :
                            alert.status === 'acknowledged' ? 'bg-amber-500/10 text-amber-400' :
                            'bg-emerald-500/10 text-emerald-400'
                          }`}>{alert.status}</span>
                        </td>
                        <td className="px-4 py-2.5 font-mono text-[10px] text-slate-500">
                          {new Date(alert.triggered_at ?? alert.created_at).toLocaleString()}
                        </td>
                        <td className="px-4 py-2.5">
                          <div className="flex gap-1" onClick={e => e.stopPropagation()}>
                            {alert.status === 'triggered' && (
                              <button onClick={() => ackMutation.mutate(alert.alert_id)} className="p-1 rounded hover:bg-white/10 text-slate-500 hover:text-amber-400 transition-colors" title="Acknowledge">
                                <Eye className="w-3.5 h-3.5" />
                              </button>
                            )}
                            {alert.status !== 'resolved' && (
                              <button onClick={() => resolveMutation.mutate(alert.alert_id)} className="p-1 rounded hover:bg-white/10 text-slate-500 hover:text-emerald-400 transition-colors" title="Resolve">
                                <CheckCircle className="w-3.5 h-3.5" />
                              </button>
                            )}
                          </div>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>
          )}
        </div>

        {/* Detail Panel */}
        {selected && (
          <div className="w-[40%] bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-5">
            <div className="flex items-center justify-between mb-4">
              <h3 className="text-sm font-bold text-white uppercase tracking-wider">Alert Details</h3>
              <button onClick={() => setSelectedId(null)} className="p-1 rounded hover:bg-white/10 text-slate-500 hover:text-white">
                <X className="w-4 h-4" />
              </button>
            </div>
            <div className="space-y-4">
              <div>
                <div className="text-[10px] text-slate-500 uppercase mb-1">Message</div>
                <div className="text-sm text-white">{selected.message}</div>
              </div>
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <div className="text-[10px] text-slate-500 uppercase mb-1">Severity</div>
                  <span className={`px-2.5 py-0.5 rounded-full text-xs font-semibold border ${
                    selected.severity === 'critical' ? 'bg-red-500/20 text-red-400 border-red-500/30' :
                    selected.severity === 'warning' ? 'bg-amber-500/20 text-amber-400 border-amber-500/30' :
                    'bg-blue-500/20 text-blue-400 border-blue-500/30'
                  }`}>{selected.severity}</span>
                </div>
                <div>
                  <div className="text-[10px] text-slate-500 uppercase mb-1">Status</div>
                  <span className={`px-2.5 py-0.5 rounded text-xs font-semibold ${
                    selected.status === 'triggered' ? 'bg-red-500/10 text-red-400' :
                    selected.status === 'acknowledged' ? 'bg-amber-500/10 text-amber-400' :
                    'bg-emerald-500/10 text-emerald-400'
                  }`}>{selected.status}</span>
                </div>
              </div>
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <div className="text-[10px] text-slate-500 uppercase mb-1">Current Value</div>
                  <div className="text-lg font-mono font-bold text-red-400">{(selected.metric_value ?? 0).toFixed(2)}</div>
                </div>
                <div>
                  <div className="text-[10px] text-slate-500 uppercase mb-1">Threshold</div>
                  <div className="text-lg font-mono font-bold text-slate-300">{(selected.threshold ?? 0).toFixed(2)}</div>
                </div>
              </div>
              <div>
                <div className="text-[10px] text-slate-500 uppercase mb-1">Triggered At</div>
                <div className="text-xs font-mono text-slate-300">{new Date(selected.triggered_at ?? selected.created_at).toLocaleString()}</div>
              </div>
              <div className="flex gap-2 pt-2 border-t border-white/10">
                {selected.status === 'triggered' && (
                  <button
                    onClick={() => ackMutation.mutate(selected.alert_id)}
                    className="flex-1 px-3 py-2 text-xs font-medium bg-amber-500/20 text-amber-400 border border-amber-500/30 rounded-lg hover:bg-amber-500/30 transition-colors"
                  >
                    Acknowledge
                  </button>
                )}
                {selected.status !== 'resolved' && (
                  <button
                    onClick={() => resolveMutation.mutate(selected.alert_id)}
                    className="flex-1 px-3 py-2 text-xs font-medium bg-emerald-500/20 text-emerald-400 border border-emerald-500/30 rounded-lg hover:bg-emerald-500/30 transition-colors"
                  >
                    Resolve
                  </button>
                )}
              </div>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
