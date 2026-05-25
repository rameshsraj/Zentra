import { useState, useCallback, useEffect } from 'react'
import { useQueryClient } from '@tanstack/react-query'
import Sparkline from '@/components/charts/Sparkline'
import PageHeader from '@/components/PageHeader'
import { fetchJSON } from '@/lib/api'
import {
  Bell, Plus, Trash2, Play, Pause,
  Clock, Copy, ChevronDown, ChevronUp, Library, Shield,
} from 'lucide-react'
import { cn } from '@/lib/utils'

const spark = (base: number, _v?: number, len = 20) =>
  Array.from({ length: len }, (_, i) => Math.max(0, base + Math.sin(base * 0.1 + i * 0.5) * (base * 0.05)))

interface AlertRule {
  id: string
  name: string
  expr: string
  severity: 'critical' | 'warning' | 'info'
  for_duration: string
  group: string
  interval: string
  labels: Record<string, string>
  annotations: Record<string, string>
  enabled: boolean
  last_eval: string
  eval_time_ms: number
  state: 'inactive' | 'pending' | 'firing'
  fires_count: number
}

const alertTemplates = [
  { name: 'High CPU Usage', expr: 'avg(cpu_usage_percent) > 90', severity: 'critical' as const, for_duration: '5m', group: 'Infrastructure' },
  { name: 'High Memory Usage', expr: 'avg(memory_usage_percent) > 85', severity: 'critical' as const, for_duration: '5m', group: 'Infrastructure' },
  { name: 'Disk Space Low', expr: 'disk_usage_percent > 90', severity: 'warning' as const, for_duration: '10m', group: 'Infrastructure' },
  { name: 'Service Down', expr: 'up == 0', severity: 'critical' as const, for_duration: '1m', group: 'Availability' },
  { name: 'High Error Rate', expr: 'rate(http_errors_total[5m]) > 10', severity: 'warning' as const, for_duration: '5m', group: 'Application' },
  { name: 'Slow Response Time', expr: 'avg(http_response_time_ms) > 2000', severity: 'warning' as const, for_duration: '5m', group: 'Application' },
  { name: 'Certificate Expiring', expr: 'ssl_cert_days_remaining < 30', severity: 'warning' as const, for_duration: '1h', group: 'Security' },
  { name: 'Network Packet Loss', expr: 'packet_loss_percent > 5', severity: 'warning' as const, for_duration: '5m', group: 'Network' },
  { name: 'Database Connection Pool', expr: 'db_active_connections / db_max_connections > 0.8', severity: 'warning' as const, for_duration: '5m', group: 'Database' },
  { name: 'High Swap Usage', expr: 'swap_usage_percent > 50', severity: 'info' as const, for_duration: '15m', group: 'Infrastructure' },
]

const mockRules: AlertRule[] = []

const sevColor = (s: string) =>
  s === 'critical' ? 'text-red-400 bg-red-500/10' : s === 'warning' ? 'text-amber-400 bg-amber-500/10' : 'text-blue-400 bg-blue-500/10'

const stateColor = (s: string) =>
  s === 'firing' ? 'text-red-400 bg-red-500/10' : s === 'pending' ? 'text-amber-400 bg-amber-500/10' : 'text-slate-400 bg-white/5'

type Tab = 'rules' | 'groups' | 'templates' | 'library'

export default function AlertRulesPage() {
  const qc = useQueryClient()
  const [tab, setTab] = useState<Tab>('rules')
  const [expandedRule, setExpandedRule] = useState<string | null>(null)
  const [showCreate, setShowCreate] = useState(false)
  const handleRefresh = useCallback(() => qc.invalidateQueries({ queryKey: ['alerts'] }), [qc])

  const liveRules = mockRules
  const liveTemplates = alertTemplates
  const groups = [...new Set(liveRules.map(r => r.group))]
  const firingCount = liveRules.filter(r => r.state === 'firing').length
  const pendingCount = liveRules.filter(r => r.state === 'pending').length
  const enabledCount = liveRules.filter(r => r.enabled).length

  return (
    <div className="space-y-6">
      <PageHeader
        title="Alert Rules"
        subtitle={`${liveRules.length} rules \u00b7 ${groups.length} groups \u00b7 ${liveTemplates.length} templates`}
        onRefresh={handleRefresh}
        onSearch={() => {}}
        searchPlaceholder="Search rules..."
        showAddTarget
        addTargetLabel="Create Rule"
        onAddTarget={() => setShowCreate(!showCreate)}
      />

      {/* KPI Strip */}
      <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-3">
        {[
          { label: 'Total Rules', value: String(liveRules.length), color: 'text-blue-400', sub: `${enabledCount} enabled`, sparkData: spark(8, 2) },
          { label: 'Firing', value: String(firingCount), color: 'text-red-400', sub: 'Active alerts', sparkData: spark(firingCount, 1) },
          { label: 'Pending', value: String(pendingCount), color: 'text-amber-400', sub: 'Evaluating for duration', sparkData: spark(pendingCount, 1) },
          { label: 'Groups', value: String(groups.length), color: 'text-purple-400', sub: 'Rule groups', sparkData: spark(groups.length, 1) },
          { label: 'Templates', value: String(liveTemplates.length), color: 'text-cyan-400', sub: 'Available', sparkData: spark(10, 2) },
          { label: 'Avg Eval Time', value: enabledCount ? `${Math.round(liveRules.filter(r => r.enabled).reduce((a, r) => a + r.eval_time_ms, 0) / enabledCount)}ms` : 'â€”', color: 'text-emerald-400', sub: 'Last evaluation', sparkData: spark(12, 5) },
        ].map((kpi, i) => (
          <div key={i} className="bg-white/5 backdrop-blur-sm border border-white/10 p-3 rounded-xl">
            <div className="text-[10px] text-slate-400 uppercase tracking-wider mb-1">{kpi.label}</div>
            <div className={`text-xl font-bold font-mono ${kpi.color}`}>{kpi.value}</div>
            <div className="flex items-center justify-between mt-0.5">
              <span className="text-[10px] text-slate-500">{kpi.sub}</span>
              <Sparkline data={kpi.sparkData} width={48} height={16} color={kpi.color.replace('text-', '')} />
            </div>
          </div>
        ))}
      </div>

      {/* Tabs */}
      <div className="flex items-center gap-4 border-b border-white/10 pb-1">
        {(['rules', 'groups', 'templates', 'library'] as Tab[]).map(t => (
          <button key={t} onClick={() => setTab(t)}
            className={`text-sm pb-2 border-b-2 transition-colors capitalize ${tab === t ? 'text-blue-400 border-blue-400' : 'text-slate-400 border-transparent hover:text-slate-200'}`}
          >
            {t === 'rules' ? 'All Rules' : t === 'groups' ? 'Rule Groups' : t === 'templates' ? 'Templates' : 'Library'}
          </button>
        ))}
      </div>

      {/* Rules Tab */}
      {tab === 'rules' && (
        <div className="space-y-2">
          {liveRules.map(rule => (
            <div key={rule.id} className="bg-white/[0.02] border border-white/10 rounded-xl overflow-hidden">
              <button className="w-full flex items-center gap-3 px-4 py-3 text-left hover:bg-white/5 transition-colors"
                onClick={() => setExpandedRule(expandedRule === rule.id ? null : rule.id)}>
                <div className={`w-2 h-2 rounded-full ${rule.state === 'firing' ? 'bg-red-400 animate-pulse' : rule.state === 'pending' ? 'bg-amber-400 animate-pulse' : rule.enabled ? 'bg-emerald-400' : 'bg-slate-600'}`} />
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="text-sm text-white font-medium">{rule.name}</span>
                    <span className={`text-[10px] px-1.5 py-0.5 rounded capitalize ${sevColor(rule.severity)}`}>{rule.severity}</span>
                    <span className={`text-[10px] px-1.5 py-0.5 rounded capitalize ${stateColor(rule.state)}`}>{rule.state}</span>
                  </div>
                  <div className="text-[11px] text-slate-500 font-mono truncate mt-0.5">{rule.expr}</div>
                </div>
                <div className="text-xs text-slate-500 text-right shrink-0">
                  <div>{rule.group}</div>
                  <div className="font-mono">{rule.interval}</div>
                </div>
                {expandedRule === rule.id ? <ChevronUp className="w-4 h-4 text-slate-400" /> : <ChevronDown className="w-4 h-4 text-slate-400" />}
              </button>
              {expandedRule === rule.id && (
                <div className="border-t border-white/10 px-4 py-3 bg-white/[0.01] space-y-3">
                  <div className="grid grid-cols-2 md:grid-cols-4 gap-3 text-xs">
                    <div><span className="text-slate-500">For Duration</span><div className="text-white font-mono mt-0.5">{rule.for_duration}</div></div>
                    <div><span className="text-slate-500">Eval Interval</span><div className="text-white font-mono mt-0.5">{rule.interval}</div></div>
                    <div><span className="text-slate-500">Last Eval</span><div className="text-white font-mono mt-0.5">{rule.last_eval}</div></div>
                    <div><span className="text-slate-500">Eval Time</span><div className="text-white font-mono mt-0.5">{rule.eval_time_ms}ms</div></div>
                  </div>
                  <div className="text-xs">
                    <span className="text-slate-500">Labels: </span>
                    {Object.entries(rule.labels).map(([k, v]) => (
                      <span key={k} className="inline-block bg-blue-500/10 text-blue-400 px-1.5 py-0.5 rounded mr-1 font-mono">{k}={v}</span>
                    ))}
                  </div>
                  <div className="text-xs">
                    <span className="text-slate-500">Annotations: </span>
                    {Object.entries(rule.annotations).map(([k, v]) => (
                      <span key={k} className="inline-block bg-purple-500/10 text-purple-400 px-1.5 py-0.5 rounded mr-1">{k}: {v}</span>
                    ))}
                  </div>
                  <div className="flex gap-2 pt-1">
                    <button className="text-xs px-3 py-1 bg-white/5 border border-white/10 rounded hover:bg-white/10 text-slate-300 flex items-center gap-1">
                      {rule.enabled ? <><Pause className="w-3 h-3" /> Disable</> : <><Play className="w-3 h-3" /> Enable</>}
                    </button>
                    <button className="text-xs px-3 py-1 bg-white/5 border border-white/10 rounded hover:bg-white/10 text-slate-300 flex items-center gap-1">
                      <Copy className="w-3 h-3" /> Duplicate
                    </button>
                    <button className="text-xs px-3 py-1 bg-red-500/10 border border-red-500/20 rounded hover:bg-red-500/20 text-red-400 flex items-center gap-1">
                      <Trash2 className="w-3 h-3" /> Delete
                    </button>
                  </div>
                </div>
              )}
            </div>
          ))}
        </div>
      )}

      {/* Groups Tab */}
      {tab === 'groups' && (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {groups.map(group => {
            const groupRules = liveRules.filter(r => r.group === group)
            const firing = groupRules.filter(r => r.state === 'firing').length
            return (
              <div key={group} className="bg-white/[0.02] border border-white/10 rounded-xl p-4">
                <div className="flex items-center justify-between mb-3">
                  <h3 className="text-sm font-semibold text-white">{group}</h3>
                  <span className="text-xs text-slate-400">{groupRules.length} rules</span>
                </div>
                {firing > 0 && (
                  <div className="text-xs text-red-400 mb-2 flex items-center gap-1">
                    <Bell className="w-3 h-3" /> {firing} firing
                  </div>
                )}
                <div className="space-y-1.5">
                  {groupRules.map(r => (
                    <div key={r.id} className="flex items-center justify-between text-xs bg-white/5 rounded px-3 py-1.5">
                      <div className="flex items-center gap-2">
                        <div className={`w-1.5 h-1.5 rounded-full ${r.state === 'firing' ? 'bg-red-400' : r.state === 'pending' ? 'bg-amber-400' : 'bg-emerald-400'}`} />
                        <span className="text-slate-300">{r.name}</span>
                      </div>
                      <span className={`px-1.5 py-0.5 rounded capitalize ${sevColor(r.severity)} text-[10px]`}>{r.severity}</span>
                    </div>
                  ))}
                </div>
              </div>
            )
          })}
        </div>
      )}

      {/* Templates Tab */}
      {tab === 'templates' && (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
          {liveTemplates.map((tpl, i) => (
            <div key={i} className="bg-white/[0.02] border border-white/10 rounded-xl p-4 hover:bg-white/5 transition-colors">
              <div className="flex items-center justify-between mb-2">
                <h4 className="text-sm font-medium text-white">{tpl.name}</h4>
                <span className={`text-[10px] px-1.5 py-0.5 rounded capitalize ${sevColor(tpl.severity)}`}>{tpl.severity}</span>
              </div>
              <div className="text-[11px] font-mono text-slate-400 bg-black/20 rounded px-2 py-1.5 mb-2">{tpl.expr}</div>
              <div className="flex items-center justify-between text-xs">
                <div className="flex items-center gap-3 text-slate-500">
                  <span className="flex items-center gap-1"><Clock className="w-3 h-3" /> {tpl.for_duration}</span>
                  <span>{tpl.group}</span>
                </div>
                <button className="text-xs px-2 py-1 bg-blue-500/10 text-blue-400 rounded hover:bg-blue-500/20 flex items-center gap-1">
                  <Plus className="w-3 h-3" /> Use
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Create Rule Modal */}
      {showCreate && (
        <div className="fixed inset-0 z-50 bg-black/60 flex items-center justify-center p-4" onClick={() => setShowCreate(false)}>
          <div className="bg-[#0f1724] border border-white/10 rounded-2xl w-full max-w-lg p-6 space-y-4" onClick={e => e.stopPropagation()}>
            <h2 className="text-lg font-semibold text-white">Create Alert Rule</h2>
            <div className="space-y-3">
              <div>
                <label className="text-xs text-slate-400 block mb-1">Rule Name</label>
                <input type="text" className="w-full bg-white/5 border border-white/10 rounded px-3 py-2 text-sm text-white" placeholder="e.g. High CPU Usage" />
              </div>
              <div>
                <label className="text-xs text-slate-400 block mb-1">Expression (PromQL)</label>
                <textarea className="w-full bg-white/5 border border-white/10 rounded px-3 py-2 text-sm text-white font-mono h-20" placeholder="avg(cpu_usage_percent) > 90" />
              </div>
              <div className="grid grid-cols-3 gap-3">
                <div>
                  <label className="text-xs text-slate-400 block mb-1">Severity</label>
                  <select className="w-full bg-white/5 border border-white/10 rounded px-3 py-2 text-sm text-white">
                    <option>critical</option><option>warning</option><option>info</option>
                  </select>
                </div>
                <div>
                  <label className="text-xs text-slate-400 block mb-1">For Duration</label>
                  <input type="text" className="w-full bg-white/5 border border-white/10 rounded px-3 py-2 text-sm text-white" placeholder="5m" />
                </div>
                <div>
                  <label className="text-xs text-slate-400 block mb-1">Group</label>
                  <input type="text" className="w-full bg-white/5 border border-white/10 rounded px-3 py-2 text-sm text-white" placeholder="Infrastructure" />
                </div>
              </div>
            </div>
            <div className="flex justify-end gap-2 pt-2">
              <button onClick={() => setShowCreate(false)} className="px-4 py-2 text-sm text-slate-400 hover:text-white">Cancel</button>
              <button className="px-4 py-2 text-sm bg-blue-600 text-white rounded hover:bg-blue-500">Create Rule</button>
            </div>
          </div>
        </div>
      )}
      {/* P9: Alert Rules Library Tab */}
      {tab === 'library' && <AlertLibraryTab />}
    </div>
  )
}

function AlertLibraryTab() {
  const [library, setLibrary] = useState<any>(null)
  const [expandedCat, setExpandedCat] = useState<Set<string>>(new Set())

  useEffect(() => {
    fetchJSON<any>('/alerts/library').then(setLibrary).catch(() => {})
  }, [])

  if (!library) return <div className="text-sm text-slate-400 py-8 text-center">Loading library...</div>

  const categories: any[] = library.categories || []
  const totalAlerts = categories.reduce((a: number, c: any) => a + (c.alerts?.length || 0), 0)

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-4">
        <div className="flex items-center gap-2 text-sm text-slate-300">
          <Library className="w-4 h-4 text-purple-400" />
          <span className="font-bold text-white">{totalAlerts}</span> pre-built alert rules across <span className="font-bold text-white">{categories.length}</span> categories
        </div>
      </div>
      <div className="space-y-2">
        {categories.map((cat: any) => {
          const isExpanded = expandedCat.has(cat.name)
          return (
            <div key={cat.name} className="bg-white/5 border border-white/10 rounded-xl overflow-hidden">
              <button
                onClick={() => setExpandedCat(prev => {
                  const next = new Set(prev)
                  if (next.has(cat.name)) next.delete(cat.name); else next.add(cat.name)
                  return next
                })}
                className="w-full flex items-center gap-3 px-4 py-3 hover:bg-white/5 transition-colors"
              >
                {isExpanded ? <ChevronDown className="w-4 h-4 text-slate-400" /> : <ChevronUp className="w-4 h-4 text-slate-400 rotate-180" />}
                <Shield className="w-4 h-4 text-purple-400" />
                <span className="text-sm font-semibold text-white">{cat.name}</span>
                <span className="text-[10px] text-slate-500 bg-white/10 px-2 py-0.5 rounded-full">{cat.count || cat.alerts?.length || 0} rules</span>
              </button>
              {isExpanded && cat.alerts && (
                <div className="border-t border-white/5">
                  <table className="w-full text-sm">
                    <thead><tr className="bg-white/[0.03]">
                      {['Name', 'Expression', 'Severity', 'Duration', 'Action'].map(h => (
                        <th key={h} className="px-4 py-2 text-left text-[10px] font-semibold text-slate-400 uppercase">{h}</th>
                      ))}
                    </tr></thead>
                    <tbody className="divide-y divide-white/5">
                      {cat.alerts.map((a: any) => (
                        <tr key={a.id} className="hover:bg-white/5">
                          <td className="px-4 py-2 text-xs text-white font-medium">{a.name}</td>
                          <td className="px-4 py-2 text-[10px] font-mono text-cyan-400 max-w-xs truncate">{a.expr}</td>
                          <td className="px-4 py-2">
                            <span className={cn('px-1.5 py-0.5 rounded text-[10px] font-semibold',
                              a.severity === 'critical' ? 'bg-red-500/20 text-red-400' :
                              a.severity === 'warning' ? 'bg-amber-500/20 text-amber-400' :
                              'bg-blue-500/20 text-blue-400'
                            )}>{a.severity}</span>
                          </td>
                          <td className="px-4 py-2 text-xs text-slate-400 font-mono">{a.for_duration}</td>
                          <td className="px-4 py-2">
                            <button className="px-2 py-1 bg-brand-600/30 text-brand-400 rounded text-[10px] hover:bg-brand-600/50 flex items-center gap-1">
                              <Plus className="w-3 h-3" /> Enable
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
        })}
      </div>
    </div>
  )
}
