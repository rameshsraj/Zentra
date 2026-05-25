import { useState, useCallback, useEffect } from 'react'
import PageHeader from '@/components/PageHeader'
import Sparkline from '@/components/charts/Sparkline'
import LiveModeButton from '@/components/LiveModeButton'
import { fetchJSON, getEscalationChains, getOnCallSchedules, createEscalationChain, deleteEscalationChain, createOnCallSchedule } from '@/lib/api'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import {
  Mail, MessageSquare, Bell, Webhook, Send, Hash,
  CheckCircle, XCircle, Trash2, TestTube, Clock,
  ChevronDown, ChevronUp, Plus, VolumeX, Link2, Users,
} from 'lucide-react'
import { cn } from '@/lib/utils'

const spark = (base: number, _v?: number, len = 20) =>
  Array.from({ length: len }, (_, i) => Math.max(0, base + Math.sin(base * 0.1 + i * 0.5) * (base * 0.05)))

interface NotifChannel {
  id: string
  name: string
  type: 'email' | 'slack' | 'teams' | 'pagerduty' | 'webhook' | 'telegram' | 'discord' | 'syslog'
  enabled: boolean
  config_summary: string
  last_test: string
  last_test_ok: boolean
  notifications_sent_24h: number
  failures_24h: number
}

const channelIcon: Record<string, typeof Mail> = {
  email: Mail, slack: Hash, teams: MessageSquare, pagerduty: Bell,
  webhook: Webhook, telegram: Send, discord: MessageSquare, syslog: Clock,
}

const channels: NotifChannel[] = []

interface NotifPolicy {
  id: string
  name: string
  match_labels: Record<string, string>
  channels: string[]
  severity_filter: string[]
  mute_start?: string
  mute_end?: string
  repeat_interval: string
}

const policies: NotifPolicy[] = []

type Tab = 'channels' | 'policies' | 'history' | 'mute-timings' | 'escalation' | 'oncall'

const mockHistory: { time: string; channel: string; type: string; alert: string; severity: string; status: string }[] = []

export default function NotificationChannelsPage() {
  const [tab, setTab] = useState<Tab>('channels')
  const [expanded, setExpanded] = useState<string | null>(null)
  const handleRefresh = useCallback(() => {}, [])

  const liveChannels = channels
  const livePolicies = policies
  const liveHistory = mockHistory
  const totalSent = liveChannels.reduce((a, c) => a + c.notifications_sent_24h, 0)
  const totalFailed = liveChannels.reduce((a, c) => a + c.failures_24h, 0)
  const enabledCount = liveChannels.filter(c => c.enabled).length

  return (
    <div className="space-y-6">
      <PageHeader
        title="Notification Channels"
        subtitle={`${liveChannels.length} channels \u00b7 ${livePolicies.length} routing policies \u00b7 ${totalSent} sent (24h)`}
        onRefresh={handleRefresh}
        onSearch={() => {}}
        searchPlaceholder="Search channels..."
        showAddTarget
        onAddTarget={() => setTab('channels')}
        addTargetLabel="Add Channel"
      />

      {/* KPI Strip */}
      <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-3">
        {[
          { label: 'Channels', value: String(liveChannels.length), color: 'text-blue-400', sub: `${enabledCount} active`, sparkData: spark(8, 1) },
          { label: 'Sent (24h)', value: String(totalSent), color: 'text-emerald-400', sub: 'Notifications', sparkData: spark(totalSent, 20) },
          { label: 'Failed (24h)', value: String(totalFailed), color: totalFailed > 0 ? 'text-red-400' : 'text-emerald-400', sub: totalFailed > 0 ? 'Check webhook' : 'All OK', sparkData: spark(totalFailed, 2) },
          { label: 'Policies', value: String(livePolicies.length), color: 'text-purple-400', sub: 'Routing rules', sparkData: spark(5, 1) },
          { label: 'Delivery Rate', value: totalSent > 0 ? `${(((totalSent - totalFailed) / totalSent) * 100).toFixed(1)}%` : '100%', color: 'text-cyan-400', sub: 'Success rate', sparkData: spark(98, 2) },
          { label: 'Channel Types', value: String(new Set(liveChannels.map(c => c.type)).size), color: 'text-amber-400', sub: 'Integrations', sparkData: spark(8, 0) },
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
        {(['channels', 'policies', 'history', 'mute-timings', 'escalation', 'oncall'] as Tab[]).map(t => (
          <button key={t} onClick={() => setTab(t)}
            className={`text-sm pb-2 border-b-2 transition-colors capitalize ${tab === t ? 'text-blue-400 border-blue-400' : 'text-slate-400 border-transparent hover:text-slate-200'}`}
          >
            {t === 'channels' ? 'Channels' : t === 'policies' ? 'Routing Policies' : t === 'history' ? 'Send History' : t === 'mute-timings' ? 'Mute Timings' : t === 'escalation' ? 'Escalation Chains' : 'On-Call'}
          </button>
        ))}
      </div>

      {/* Channels Tab */}
      {tab === 'channels' && (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
          {liveChannels.map(ch => {
            const Icon = channelIcon[ch.type] || Bell
            return (
              <div key={ch.id} className={`bg-white/[0.02] border rounded-xl p-4 ${ch.enabled ? 'border-white/10' : 'border-white/5 opacity-60'}`}>
                <div className="flex items-center gap-3 mb-3">
                  <div className={`p-2 rounded-lg ${ch.enabled ? 'bg-blue-500/10' : 'bg-white/5'}`}>
                    <Icon className={`w-5 h-5 ${ch.enabled ? 'text-blue-400' : 'text-slate-500'}`} />
                  </div>
                  <div className="flex-1 min-w-0">
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-medium text-white">{ch.name}</span>
                      <span className="text-[10px] px-1.5 py-0.5 rounded bg-white/5 text-slate-400 uppercase">{ch.type}</span>
                    </div>
                    <div className="text-[11px] text-slate-500 font-mono truncate">{ch.config_summary}</div>
                  </div>
                  {ch.enabled
                    ? <CheckCircle className="w-4 h-4 text-emerald-400 shrink-0" />
                    : <XCircle className="w-4 h-4 text-slate-500 shrink-0" />
                  }
                </div>
                <div className="grid grid-cols-3 gap-2 text-xs">
                  <div className="bg-white/5 rounded px-2 py-1.5 text-center">
                    <div className="text-slate-400">Sent (24h)</div>
                    <div className="text-white font-mono">{ch.notifications_sent_24h}</div>
                  </div>
                  <div className="bg-white/5 rounded px-2 py-1.5 text-center">
                    <div className="text-slate-400">Failed</div>
                    <div className={`font-mono ${ch.failures_24h > 0 ? 'text-red-400' : 'text-emerald-400'}`}>{ch.failures_24h}</div>
                  </div>
                  <div className="bg-white/5 rounded px-2 py-1.5 text-center">
                    <div className="text-slate-400">Last Test</div>
                    <div className={`font-mono ${ch.last_test_ok ? 'text-emerald-400' : 'text-red-400'}`}>{ch.last_test_ok ? '✓' : '✗'} {ch.last_test}</div>
                  </div>
                </div>
                <div className="flex gap-2 mt-3">
                  <button className="text-xs px-3 py-1 bg-white/5 border border-white/10 rounded hover:bg-white/10 text-slate-300 flex items-center gap-1">
                    <TestTube className="w-3 h-3" /> Test
                  </button>
                  <button className="text-xs px-3 py-1 bg-red-500/10 border border-red-500/20 rounded hover:bg-red-500/20 text-red-400 flex items-center gap-1">
                    <Trash2 className="w-3 h-3" /> Remove
                  </button>
                </div>
              </div>
            )
          })}
        </div>
      )}

      {/* Routing Policies Tab */}
      {tab === 'policies' && (
        <div className="space-y-3">
          {livePolicies.map(policy => (
            <div key={policy.id} className="bg-white/[0.02] border border-white/10 rounded-xl overflow-hidden">
              <button className="w-full flex items-center gap-3 px-4 py-3 text-left hover:bg-white/5"
                onClick={() => setExpanded(expanded === policy.id ? null : policy.id)}>
                <div className="flex-1">
                  <div className="text-sm text-white font-medium">{policy.name}</div>
                  <div className="text-[11px] text-slate-500 mt-0.5">
                    Channels: {policy.channels.length > 0 ? policy.channels.join(', ') : 'Muted'} · Repeat: {policy.repeat_interval}
                  </div>
                </div>
                <div className="flex gap-1">
                  {policy.severity_filter.map(s => (
                    <span key={s} className={`text-[10px] px-1.5 py-0.5 rounded ${s === 'critical' ? 'bg-red-500/10 text-red-400' : s === 'warning' ? 'bg-amber-500/10 text-amber-400' : 'bg-blue-500/10 text-blue-400'}`}>{s}</span>
                  ))}
                </div>
                {expanded === policy.id ? <ChevronUp className="w-4 h-4 text-slate-400" /> : <ChevronDown className="w-4 h-4 text-slate-400" />}
              </button>
              {expanded === policy.id && (
                <div className="border-t border-white/10 px-4 py-3 bg-white/[0.01] text-xs space-y-2">
                  <div>
                    <span className="text-slate-500">Match Labels: </span>
                    {Object.keys(policy.match_labels).length > 0
                      ? Object.entries(policy.match_labels).map(([k, v]) => <span key={k} className="font-mono bg-blue-500/10 text-blue-400 px-1.5 py-0.5 rounded mr-1">{k}={v}</span>)
                      : <span className="text-slate-400">All alerts</span>
                    }
                  </div>
                  {policy.mute_start && <div className="text-amber-400">⏸ Muted {policy.mute_start} – {policy.mute_end}</div>}
                </div>
              )}
            </div>
          ))}
        </div>
      )}

      {/* History Tab */}
      {tab === 'history' && (
        <div className="bg-white/[0.02] border border-white/10 rounded-xl overflow-hidden">
          <table className="w-full text-xs">
            <thead>
              <tr className="text-slate-400 border-b border-white/10">
                <th className="text-left px-4 py-2">Time</th>
                <th className="text-left px-4 py-2">Channel</th>
                <th className="text-left px-4 py-2">Alert</th>
                <th className="text-center px-4 py-2">Severity</th>
                <th className="text-center px-4 py-2">Status</th>
              </tr>
            </thead>
            <tbody>
              {liveHistory.map((h, i) => (
                <tr key={i} className="border-b border-white/5 hover:bg-white/5">
                  <td className="px-4 py-2 font-mono text-slate-400">{h.time}</td>
                  <td className="px-4 py-2">
                    <span className="text-white">{h.channel}</span>
                    <span className="text-slate-500 ml-1 text-[10px] uppercase">({h.type})</span>
                  </td>
                  <td className="px-4 py-2 text-slate-300">{h.alert}</td>
                  <td className="px-4 py-2 text-center">
                    <span className={`px-1.5 py-0.5 rounded text-[10px] ${h.severity === 'critical' ? 'bg-red-500/10 text-red-400' : 'bg-amber-500/10 text-amber-400'}`}>{h.severity}</span>
                  </td>
                  <td className="px-4 py-2 text-center">
                    <span className={`px-1.5 py-0.5 rounded text-[10px] ${h.status === 'delivered' ? 'bg-emerald-500/10 text-emerald-400' : 'bg-red-500/10 text-red-400'}`}>{h.status}</span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
      {/* P10: Mute Timings Tab */}
      {tab === 'mute-timings' && <MuteTimingsTab />}
      {tab === 'escalation' && <EscalationChainsTab />}
      {tab === 'oncall' && <OnCallTab />}
    </div>
  )
}

function MuteTimingsTab() {
  const [mutings, setMutings] = useState<any[]>([])
  const [showAdd, setShowAdd] = useState(false)
  const [newName, setNewName] = useState('')
  const [newStart, setNewStart] = useState('22:00')
  const [newEnd, setNewEnd] = useState('07:00')
  const [newDays, setNewDays] = useState('mon,tue,wed,thu,fri')

  useEffect(() => {
    fetchJSON<any[]>('/notifications/mute-timings').then(setMutings).catch(() => {})
  }, [])

  const handleCreate = () => {
    fetchJSON<any>('/notifications/mute-timings', {
      method: 'POST',
      body: JSON.stringify({ name: newName, start_time: newStart, end_time: newEnd, weekdays: newDays }),
    }).then(() => {
      fetchJSON<any[]>('/notifications/mute-timings').then(setMutings).catch(() => {})
      setShowAdd(false)
      setNewName('')
    }).catch(() => {})
  }

  const handleDelete = (id: string) => {
    fetchJSON<any>(`/notifications/mute-timings/${id}`, { method: 'DELETE' }).then(() => {
      setMutings(prev => prev.filter(m => m.id !== id))
    }).catch(() => {})
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div className="text-sm text-slate-300 flex items-center gap-2">
          <VolumeX className="w-4 h-4 text-purple-400" />
          {mutings.length} mute timing{mutings.length !== 1 ? 's' : ''} configured
        </div>
        <LiveModeButton>
          <button onClick={() => setShowAdd(true)} className="flex items-center gap-1.5 px-3 py-1.5 bg-brand-600 hover:bg-brand-500 text-white rounded-lg text-xs font-medium">
            <Plus className="w-3.5 h-3.5" /> Add Mute Timing
          </button>
        </LiveModeButton>
      </div>

      {showAdd && (
        <div className="bg-white/5 border border-white/10 rounded-xl p-4 space-y-3">
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="text-[10px] text-slate-400 uppercase mb-1 block">Name</label>
              <input value={newName} onChange={e => setNewName(e.target.value)} className="w-full bg-white/5 border border-white/10 rounded px-3 py-2 text-xs text-white" placeholder="Weekend Mute" />
            </div>
            <div>
              <label className="text-[10px] text-slate-400 uppercase mb-1 block">Weekdays</label>
              <input value={newDays} onChange={e => setNewDays(e.target.value)} className="w-full bg-white/5 border border-white/10 rounded px-3 py-2 text-xs text-white" placeholder="mon,tue,wed" />
            </div>
            <div>
              <label className="text-[10px] text-slate-400 uppercase mb-1 block">Start Time</label>
              <input type="time" value={newStart} onChange={e => setNewStart(e.target.value)} className="w-full bg-white/5 border border-white/10 rounded px-3 py-2 text-xs text-white" />
            </div>
            <div>
              <label className="text-[10px] text-slate-400 uppercase mb-1 block">End Time</label>
              <input type="time" value={newEnd} onChange={e => setNewEnd(e.target.value)} className="w-full bg-white/5 border border-white/10 rounded px-3 py-2 text-xs text-white" />
            </div>
          </div>
          <div className="flex gap-2">
            <button onClick={() => setShowAdd(false)} className="px-3 py-1.5 text-xs text-slate-400 border border-white/10 rounded-lg">Cancel</button>
            <button onClick={handleCreate} disabled={!newName} className="px-3 py-1.5 text-xs text-white bg-brand-600 rounded-lg disabled:opacity-50">Create</button>
          </div>
        </div>
      )}

      <div className="space-y-2">
        {mutings.map((m: any) => (
          <div key={m.id} className="flex items-center gap-4 bg-white/5 border border-white/10 rounded-xl px-4 py-3">
            <VolumeX className="w-4 h-4 text-purple-400 shrink-0" />
            <div className="flex-1">
              <div className="text-sm font-medium text-white">{m.name}</div>
              <div className="text-[10px] text-slate-500">
                {m.start_time} — {m.end_time} | Days: {m.weekdays || 'all'}
              </div>
            </div>
            <span className={cn('px-2 py-0.5 rounded text-[10px] font-semibold', m.active ? 'bg-purple-500/20 text-purple-400' : 'bg-white/10 text-slate-400')}>
              {m.active ? 'Active' : 'Inactive'}
            </span>
            <button onClick={() => handleDelete(m.id)} className="p-1.5 text-slate-400 hover:text-red-400"><Trash2 className="w-3.5 h-3.5" /></button>
          </div>
        ))}
        {mutings.length === 0 && <div className="text-sm text-slate-500 text-center py-8">No mute timings configured</div>}
      </div>
    </div>
  )
}

// ═══════════════════════════════════════════════════════════════════════════
// Escalation Chains Tab (V5 Prompt 12)
// ═══════════════════════════════════════════════════════════════════════════
function EscalationChainsTab() {
  const qc = useQueryClient()
  const { data: chains = [] } = useQuery({ queryKey: ['escalation-chains'], queryFn: getEscalationChains })
  const [showAdd, setShowAdd] = useState(false)
  const [name, setName] = useState('')
  const [desc, setDesc] = useState('')

  const handleCreate = async () => {
    if (!name) return
    await createEscalationChain({
      name, description: desc || undefined,
      steps: [
        { step: 1, action: 'notify', channel_id: 'ch-002' },
        { step: 2, action: 'wait', wait_minutes: 15 },
        { step: 3, action: 'escalate', channel_id: 'ch-004' },
      ],
    })
    qc.invalidateQueries({ queryKey: ['escalation-chains'] })
    setShowAdd(false); setName(''); setDesc('')
  }
  const handleDelete = async (id: string) => { await deleteEscalationChain(id); qc.invalidateQueries({ queryKey: ['escalation-chains'] }) }

  const actionColor: Record<string, string> = { notify: 'text-blue-400 bg-blue-500/10', wait: 'text-yellow-400 bg-yellow-500/10', escalate: 'text-red-400 bg-red-500/10' }

  return (
    <div className="space-y-4">
      <div className="flex justify-between items-center">
        <h3 className="text-sm font-semibold text-slate-300 flex items-center gap-2"><Link2 className="w-4 h-4" /> Escalation Chains</h3>
        <LiveModeButton><button onClick={() => setShowAdd(true)} className="flex items-center gap-1 px-3 py-1.5 bg-blue-600 hover:bg-blue-700 text-white rounded-lg text-xs"><Plus className="w-3 h-3" /> New Chain</button></LiveModeButton>
      </div>

      {showAdd && (
        <div className="bg-white/5 border border-white/10 rounded-xl p-4 space-y-3">
          <input value={name} onChange={e => setName(e.target.value)} placeholder="Chain name" className="w-full bg-white/5 border border-white/10 rounded px-3 py-2 text-xs text-white" />
          <input value={desc} onChange={e => setDesc(e.target.value)} placeholder="Description (optional)" className="w-full bg-white/5 border border-white/10 rounded px-3 py-2 text-xs text-white" />
          <div className="flex gap-2">
            <button onClick={() => setShowAdd(false)} className="px-3 py-1.5 text-xs text-slate-400 border border-white/10 rounded-lg">Cancel</button>
            <button onClick={handleCreate} disabled={!name} className="px-3 py-1.5 text-xs text-white bg-blue-600 rounded-lg disabled:opacity-50">Create with default steps</button>
          </div>
        </div>
      )}

      <div className="space-y-3">
        {chains.map((c: any) => (
          <div key={c.id} className="bg-white/[0.02] border border-white/10 rounded-xl p-4">
            <div className="flex items-center justify-between mb-3">
              <div>
                <h4 className="text-sm font-medium text-white">{c.name}</h4>
                {c.description && <p className="text-[10px] text-slate-500">{c.description}</p>}
              </div>
              <div className="flex items-center gap-2">
                {c.repeat && <span className="text-[10px] px-2 py-0.5 rounded bg-purple-500/20 text-purple-400">Repeating</span>}
                <button onClick={() => handleDelete(c.id)} className="p-1 text-slate-400 hover:text-red-400"><Trash2 className="w-3.5 h-3.5" /></button>
              </div>
            </div>
            <div className="flex items-center gap-1 flex-wrap">
              {(c.steps || []).map((s: any, i: number) => (
                <div key={i} className={`flex items-center gap-1 px-2 py-1 rounded text-[10px] ${actionColor[s.action] || 'text-slate-400 bg-white/5'}`}>
                  <span className="font-semibold uppercase">{s.action}</span>
                  {s.channel_id && <span className="text-slate-400">→ {s.channel_id}</span>}
                  {s.wait_minutes && <span className="text-slate-400">{s.wait_minutes}min</span>}
                  {i < (c.steps || []).length - 1 && <span className="text-slate-600 ml-1">→</span>}
                </div>
              ))}
            </div>
          </div>
        ))}
        {chains.length === 0 && <div className="text-sm text-slate-500 text-center py-8">No escalation chains configured</div>}
      </div>
    </div>
  )
}

// ═══════════════════════════════════════════════════════════════════════════
// On-Call Schedules Tab (V5 Prompt 12)
// ═══════════════════════════════════════════════════════════════════════════
function OnCallTab() {
  const qc = useQueryClient()
  const { data: schedules = [] } = useQuery({ queryKey: ['oncall-schedules'], queryFn: getOnCallSchedules })
  const [showAdd, setShowAdd] = useState(false)
  const [name, setName] = useState('')

  const handleCreate = async () => {
    if (!name) return
    await createOnCallSchedule({
      name,
      users: [{ user_id: 'user-1', username: 'admin', order: 1 }],
      rotation_type: 'weekly',
    })
    qc.invalidateQueries({ queryKey: ['oncall-schedules'] })
    setShowAdd(false); setName('')
  }

  return (
    <div className="space-y-4">
      <div className="flex justify-between items-center">
        <h3 className="text-sm font-semibold text-slate-300 flex items-center gap-2"><Users className="w-4 h-4" /> On-Call Schedules</h3>
        <LiveModeButton><button onClick={() => setShowAdd(true)} className="flex items-center gap-1 px-3 py-1.5 bg-blue-600 hover:bg-blue-700 text-white rounded-lg text-xs"><Plus className="w-3 h-3" /> New Schedule</button></LiveModeButton>
      </div>

      {showAdd && (
        <div className="bg-white/5 border border-white/10 rounded-xl p-4 space-y-3">
          <input value={name} onChange={e => setName(e.target.value)} placeholder="Schedule name" className="w-full bg-white/5 border border-white/10 rounded px-3 py-2 text-xs text-white" />
          <div className="flex gap-2">
            <button onClick={() => setShowAdd(false)} className="px-3 py-1.5 text-xs text-slate-400 border border-white/10 rounded-lg">Cancel</button>
            <button onClick={handleCreate} disabled={!name} className="px-3 py-1.5 text-xs text-white bg-blue-600 rounded-lg disabled:opacity-50">Create</button>
          </div>
        </div>
      )}

      <div className="space-y-3">
        {schedules.map((s: any) => (
          <div key={s.id} className="bg-white/[0.02] border border-white/10 rounded-xl p-4">
            <div className="flex items-center justify-between mb-3">
              <div>
                <h4 className="text-sm font-medium text-white">{s.name}</h4>
                <p className="text-[10px] text-slate-500">{s.rotation_type} rotation · {s.time_zone} · {s.start_time}–{s.end_time}</p>
              </div>
              <span className="text-[10px] px-2 py-0.5 rounded bg-cyan-500/20 text-cyan-400">{(s.users || []).length} members</span>
            </div>
            <div className="flex gap-2 flex-wrap">
              {(s.users || []).map((u: any) => (
                <div key={u.user_id} className="flex items-center gap-1.5 px-2 py-1 rounded bg-white/5 text-[10px]">
                  <div className="w-5 h-5 rounded-full bg-blue-600/30 flex items-center justify-center text-[8px] text-blue-400 font-bold">{u.order}</div>
                  <span className="text-white">{u.username}</span>
                </div>
              ))}
            </div>
          </div>
        ))}
        {schedules.length === 0 && <div className="text-sm text-slate-500 text-center py-8">No on-call schedules configured</div>}
      </div>
    </div>
  )
}
