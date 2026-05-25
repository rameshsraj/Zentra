import { useState } from 'react'
import { useQuery, useQueryClient, useMutation } from '@tanstack/react-query'
import {
  getRunbooks, getRunbookStats, getRunbookExecutions,
  executeRunbook, toggleRunbook, deleteRunbook, approveRunbookExecution,
} from '@/lib/api'
import PageHeader from '@/components/PageHeader'
import LiveModeButton from '@/components/LiveModeButton'
import {
  Play, Pause, Trash2, CheckCircle, XCircle, Clock,
  ChevronDown, ChevronUp, Cpu, Database, HardDrive, Network, Shield,
  Server, RotateCcw, ThumbsUp, ThumbsDown,
} from 'lucide-react'

const categoryIcons: Record<string, typeof Server> = {
  disk: HardDrive, cpu: Cpu, memory: Cpu, database: Database,
  network: Network, service: Server, security: Shield,
}

const statusColors: Record<string, string> = {
  completed: 'text-emerald-400 bg-emerald-500/10 border-emerald-500/20',
  running: 'text-blue-400 bg-blue-500/10 border-blue-500/20',
  failed: 'text-red-400 bg-red-500/10 border-red-500/20',
  pending_approval: 'text-amber-400 bg-amber-500/10 border-amber-500/20',
  cancelled: 'text-slate-400 bg-slate-500/10 border-slate-500/20',
}

const severityColors: Record<string, string> = {
  critical: 'text-red-400 bg-red-500/10 border-red-500/20',
  warning: 'text-amber-400 bg-amber-500/10 border-amber-500/20',
  info: 'text-blue-400 bg-blue-500/10 border-blue-500/20',
}

export default function RunbooksPage() {
  const qc = useQueryClient()
  const [search, setSearch] = useState('')
  const [categoryFilter, setCategoryFilter] = useState('')
  const [activeTab, setActiveTab] = useState<'runbooks' | 'executions'>('runbooks')
  const [expandedId, setExpandedId] = useState<string | null>(null)

  const { data: runbooks } = useQuery({ queryKey: ['runbooks'], queryFn: () => getRunbooks() })
  const { data: stats } = useQuery({ queryKey: ['runbook-stats'], queryFn: getRunbookStats })
  const { data: executions } = useQuery({ queryKey: ['runbook-executions'], queryFn: getRunbookExecutions })

  const executeMut = useMutation({
    mutationFn: (id: string) => executeRunbook(id),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['runbooks'] }); qc.invalidateQueries({ queryKey: ['runbook-executions'] }) },
  })
  const toggleMut = useMutation({
    mutationFn: (id: string) => toggleRunbook(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['runbooks'] }),
  })
  const deleteMut = useMutation({
    mutationFn: (id: string) => deleteRunbook(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['runbooks'] }),
  })
  const approveMut = useMutation({
    mutationFn: ({ id, approved, comment }: { id: string; approved: boolean; comment?: string }) =>
      approveRunbookExecution(id, approved, comment),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['runbook-executions'] }),
  })

  const filtered = (runbooks ?? []).filter(r => {
    if (search && !r.name.toLowerCase().includes(search.toLowerCase())) return false
    if (categoryFilter && r.category !== categoryFilter) return false
    return true
  })

  const categories = [...new Set((runbooks ?? []).map(r => r.category))]

  return (
    <div className="space-y-6">
      <PageHeader
        title="Remediation Runbooks"
        subtitle={`${stats?.total_runbooks ?? 0} runbooks · ${stats?.enabled_runbooks ?? 0} active · ${stats?.pending_approval ?? 0} pending approval`}
        onRefresh={() => { qc.invalidateQueries({ queryKey: ['runbooks'] }); qc.invalidateQueries({ queryKey: ['runbook-executions'] }) }}
        onSearch={setSearch}
        searchPlaceholder="Search runbooks..."
      />

      {/* Vertical Side Menu */}
      <div className="flex gap-6">
        <div className="w-48 flex-shrink-0 space-y-1">
          {(['runbooks', 'executions'] as const).map(tab => (
            <button
              key={tab}
              onClick={() => setActiveTab(tab)}
              className={`flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-lg transition-colors w-full text-left capitalize ${
                activeTab === tab ? 'bg-brand-600/20 text-brand-400 border border-brand-500/30' : 'bg-white/5 text-slate-400 hover:text-white border border-transparent hover:border-white/10'
              }`}
            >
              {tab}
              {tab === 'executions' && (stats?.pending_approval ?? 0) > 0 && (
                <span className="ml-auto px-1.5 py-0.5 text-[10px] bg-amber-500 text-white rounded-full">
                  {stats.pending_approval}
                </span>
              )}
            </button>
          ))}
        </div>

        {/* Right side content */}
        <div className="flex-1 space-y-6">
          {/* KPI strip */}
          <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
            {[
              { label: 'Total Runbooks', value: stats?.total_runbooks ?? 0, color: 'text-white' },
              { label: 'Executions', value: stats?.total_executions ?? 0, color: 'text-blue-400' },
              { label: 'Success Rate', value: `${(stats?.success_rate ?? 0).toFixed(0)}%`, color: 'text-emerald-400' },
              { label: 'Pending Approval', value: stats?.pending_approval ?? 0, color: 'text-amber-400' },
            ].map((kpi, i) => (
              <div key={i} className="bg-white/5 border border-white/10 rounded-xl p-4">
                <div className="text-[10px] text-slate-400 uppercase tracking-wider mb-1">{kpi.label}</div>
                <div className={`text-2xl font-bold font-mono ${kpi.color}`}>{kpi.value}</div>
              </div>
            ))}
          </div>

      {activeTab === 'runbooks' && (
        <>
          {/* Category filters */}
          <div className="flex flex-wrap gap-1.5">
            {['', ...categories].map(cat => (
              <button
                key={cat}
                onClick={() => setCategoryFilter(cat)}
                className={`px-3 py-1.5 text-[10px] rounded-lg border transition-colors uppercase ${
                  categoryFilter === cat
                    ? 'bg-brand-600/20 text-brand-400 border-brand-500/30'
                    : 'bg-white/5 text-slate-500 border-white/10 hover:text-white'
                }`}
              >
                {cat || 'All'}
              </button>
            ))}
          </div>

          <div className="space-y-2">
            {filtered.map(rb => {
              const Icon = categoryIcons[rb.category] || Server
              const expanded = expandedId === rb.id
              const successRate = rb.run_count > 0 ? Math.round((rb.success_count / rb.run_count) * 100) : null

              return (
                <div key={rb.id} className="bg-white/5 border border-white/10 rounded-xl overflow-hidden">
                  <div className="p-4 flex items-center gap-4">
                    <div className={`w-10 h-10 rounded-lg flex items-center justify-center flex-shrink-0 ${rb.enabled ? 'bg-brand-600/20' : 'bg-white/5'}`}>
                      <Icon className={`w-5 h-5 ${rb.enabled ? 'text-brand-400' : 'text-slate-600'}`} />
                    </div>

                    <div className="flex-1 min-w-0">
                      <div className="flex items-center gap-2 flex-wrap">
                        <span className="text-sm font-bold text-white">{rb.name}</span>
                        <span className={`px-2 py-0.5 rounded text-[10px] font-semibold border ${severityColors[rb.severity] ?? severityColors.info}`}>
                          {rb.severity}
                        </span>
                        <span className="px-2 py-0.5 rounded text-[10px] font-semibold bg-white/10 text-slate-400 border border-white/10 uppercase">
                          {rb.category}
                        </span>
                        {rb.requires_approval && (
                          <span className="px-2 py-0.5 rounded text-[10px] font-semibold bg-amber-500/10 text-amber-400 border border-amber-500/20">
                            needs approval
                          </span>
                        )}
                        {!rb.enabled && (
                          <span className="px-2 py-0.5 rounded text-[10px] font-semibold bg-slate-500/10 text-slate-500 border border-slate-500/20">
                            disabled
                          </span>
                        )}
                      </div>
                      <div className="text-xs text-slate-500 mt-0.5 truncate">{rb.description}</div>
                      <div className="flex items-center gap-4 mt-1 text-[10px] text-slate-600">
                        <span>{rb.steps.length} steps</span>
                        <span>{rb.run_count} runs</span>
                        {successRate !== null && <span className="text-emerald-500">{successRate}% success</span>}
                        {rb.last_run && <span>Last: {new Date(rb.last_run).toLocaleDateString()}</span>}
                      </div>
                    </div>

                    <div className="flex items-center gap-2 flex-shrink-0">
                      <LiveModeButton>
                        <button
                          onClick={() => executeMut.mutate(rb.id)}
                          disabled={executeMut.isPending}
                          className="flex items-center gap-1 px-3 py-1.5 text-xs bg-brand-600/20 text-brand-400 border border-brand-500/30 rounded-lg hover:bg-brand-600/30 transition-colors"
                          title="Run now"
                        >
                          <Play className="w-3 h-3" /> Run
                        </button>
                      </LiveModeButton>
                      <LiveModeButton>
                        <button
                          onClick={() => toggleMut.mutate(rb.id)}
                          className={`p-1.5 rounded hover:bg-white/10 transition-colors ${rb.enabled ? 'text-emerald-400' : 'text-slate-600'}`}
                          title={rb.enabled ? 'Disable' : 'Enable'}
                        >
                          {rb.enabled ? <CheckCircle className="w-4 h-4" /> : <Pause className="w-4 h-4" />}
                        </button>
                      </LiveModeButton>
                      <button
                        onClick={() => setExpandedId(expanded ? null : rb.id)}
                        className="p-1.5 rounded hover:bg-white/10 text-slate-500 transition-colors"
                      >
                        {expanded ? <ChevronUp className="w-4 h-4" /> : <ChevronDown className="w-4 h-4" />}
                      </button>
                      <LiveModeButton>
                        <button
                          onClick={() => { if (confirm(`Delete "${rb.name}"?`)) deleteMut.mutate(rb.id) }}
                          className="p-1.5 rounded hover:bg-white/10 text-slate-500 hover:text-red-400 transition-colors"
                        >
                          <Trash2 className="w-4 h-4" />
                        </button>
                      </LiveModeButton>
                    </div>
                  </div>

                  {expanded && (
                    <div className="border-t border-white/10 p-4 space-y-3">
                      <div className="text-[10px] text-slate-400 uppercase font-semibold tracking-wider">Steps</div>
                      <div className="space-y-2">
                        {rb.steps.map((step, i) => (
                          <div key={step.id} className="flex items-start gap-3 text-xs">
                            <div className="w-6 h-6 rounded-full bg-white/10 flex items-center justify-center text-[10px] text-slate-400 flex-shrink-0 mt-0.5">
                              {i + 1}
                            </div>
                            <div className="flex-1">
                              <div className="flex items-center gap-2">
                                <span className="font-semibold text-slate-300">{step.name}</span>
                                <span className="px-1.5 py-0.5 text-[9px] bg-white/5 text-slate-500 rounded uppercase">{step.step_type}</span>
                                <span className="text-slate-600 text-[10px]">{step.timeout_seconds}s timeout</span>
                              </div>
                              {step.command && (
                                <div className="mt-1 px-2 py-1 bg-slate-900 rounded font-mono text-[10px] text-slate-400 truncate">
                                  {step.command}
                                </div>
                              )}
                              {step.description && <div className="text-slate-600 mt-0.5">{step.description}</div>}
                            </div>
                          </div>
                        ))}
                      </div>
                      {rb.trigger_conditions.length > 0 && (
                        <div>
                          <div className="text-[10px] text-slate-400 uppercase font-semibold tracking-wider mb-1">Trigger Conditions</div>
                          <div className="flex flex-wrap gap-1.5">
                            {rb.trigger_conditions.map((cond, i) => (
                              <span key={i} className="px-2 py-1 text-[10px] bg-slate-900 text-slate-400 rounded font-mono">{cond}</span>
                            ))}
                          </div>
                        </div>
                      )}
                    </div>
                  )}
                </div>
              )
            })}
          </div>
        </>
      )}

      {activeTab === 'executions' && (
        <div className="space-y-2">
          {(executions ?? []).map(exec => (
            <div key={exec.id} className="bg-white/5 border border-white/10 rounded-xl p-4">
              <div className="flex items-center gap-4">
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 flex-wrap">
                    <span className="text-sm font-bold text-white">{exec.runbook_name}</span>
                    <span className={`px-2 py-0.5 rounded text-[10px] font-semibold border ${statusColors[exec.status] ?? statusColors.cancelled}`}>
                      {exec.status.replace('_', ' ')}
                    </span>
                  </div>
                  <div className="text-xs text-slate-500 mt-0.5">
                    Trigger: {exec.trigger} {exec.target && `· Target: ${exec.target}`}
                  </div>
                  <div className="text-[10px] text-slate-600 mt-0.5">
                    {new Date(exec.started_at).toLocaleString()}
                    {exec.completed_at && ` → ${new Date(exec.completed_at).toLocaleTimeString()}`}
                  </div>
                  {exec.summary && <div className="text-xs text-slate-400 mt-1">{exec.summary}</div>}
                </div>

                {exec.status === 'pending_approval' && (
                  <div className="flex gap-2">
                    <button
                      onClick={() => approveMut.mutate({ id: exec.id, approved: true, comment: 'Approved' })}
                      className="flex items-center gap-1 px-3 py-1.5 text-xs bg-emerald-500/20 text-emerald-400 border border-emerald-500/30 rounded-lg hover:bg-emerald-500/30 transition-colors"
                    >
                      <ThumbsUp className="w-3 h-3" /> Approve
                    </button>
                    <button
                      onClick={() => approveMut.mutate({ id: exec.id, approved: false, comment: 'Rejected' })}
                      className="flex items-center gap-1 px-3 py-1.5 text-xs bg-red-500/20 text-red-400 border border-red-500/30 rounded-lg hover:bg-red-500/30 transition-colors"
                    >
                      <ThumbsDown className="w-3 h-3" /> Reject
                    </button>
                  </div>
                )}
              </div>

              {exec.step_results.length > 0 && (
                <div className="mt-3 pt-3 border-t border-white/10">
                  <div className="flex gap-1.5 flex-wrap">
                    {exec.step_results.map((sr) => (
                      <div key={sr.step_id} className={`flex items-center gap-1 px-2 py-1 rounded text-[10px] ${
                        sr.status === 'success' ? 'bg-emerald-500/10 text-emerald-400' :
                        sr.status === 'failed' ? 'bg-red-500/10 text-red-400' :
                        sr.status === 'skipped' ? 'bg-slate-500/10 text-slate-500' :
                        'bg-white/5 text-slate-500'
                      }`}>
                        {sr.status === 'success' ? <CheckCircle className="w-3 h-3" /> :
                         sr.status === 'failed' ? <XCircle className="w-3 h-3" /> :
                         <Clock className="w-3 h-3" />}
                        {sr.step_name}
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>
          ))}

          {(executions ?? []).length === 0 && (
            <div className="bg-white/5 border border-white/10 rounded-xl text-center py-12">
              <RotateCcw className="w-12 h-12 text-slate-600 mx-auto mb-3" />
              <p className="text-slate-400">No executions yet</p>
              <p className="text-slate-500 text-xs mt-1">Run a runbook to see execution history</p>
            </div>
          )}
        </div>
      )}
        </div>
      </div>
    </div>
  )
}
