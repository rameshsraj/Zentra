import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { fetchJSON } from '@/lib/api'
import PageHeader from '@/components/PageHeader'
import LoadingSkeleton from '@/components/LoadingSkeleton'
import { Bell, Plus, Play, CheckCircle, AlertTriangle, XCircle, Info } from 'lucide-react'

interface AlertRule {
  id: string; name: string; signal: string; metric: string
  condition: { operator: string }; threshold: number
  window_seconds: number; severity: string; description: string
  enabled: boolean; is_default?: boolean; created_at?: string
}

interface Evaluation {
  rule: string; metric: string; current: number; threshold: number; status: string
}

const getRules = () => fetchJSON<{ rules: AlertRule[]; count: number }>('/otel/alert-rules')
const evalRules = () => fetchJSON<{ evaluations: Evaluation[]; firing_count: number }>('/otel/alert-rules/evaluate')
const createRule = (data: Record<string, unknown>) =>
  fetchJSON<{ id: string; created: boolean }>('/otel/alert-rules', { method: 'POST', body: JSON.stringify(data) })

const sevIcon = (s: string) => {
  switch (s) {
    case 'critical': return <XCircle className="w-3.5 h-3.5 text-red-400" />
    case 'warning': return <AlertTriangle className="w-3.5 h-3.5 text-amber-400" />
    case 'info': return <Info className="w-3.5 h-3.5 text-blue-400" />
    default: return <Bell className="w-3.5 h-3.5 text-slate-400" />
  }
}

const sevBg = (s: string) => {
  switch (s) {
    case 'critical': return 'bg-red-500/10 border-red-500/20 text-red-400'
    case 'warning': return 'bg-amber-500/10 border-amber-500/20 text-amber-400'
    case 'info': return 'bg-blue-500/10 border-blue-500/20 text-blue-400'
    default: return 'bg-slate-500/10 border-slate-500/20 text-slate-400'
  }
}

export default function OTelAlertRulesPage() {
  const qc = useQueryClient()
  const [showCreate, setShowCreate] = useState(false)
  const [evalResults, setEvalResults] = useState<Evaluation[] | null>(null)
  const [evaluating, setEvaluating] = useState(false)

  const { data, isLoading, refetch } = useQuery({ queryKey: ['otel-alert-rules'], queryFn: getRules })
  const rules = data?.rules ?? []

  const mutation = useMutation({
    mutationFn: createRule,
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['otel-alert-rules'] }); setShowCreate(false) },
  })

  const handleEvaluate = async () => {
    setEvaluating(true)
    try {
      const result = await evalRules()
      setEvalResults(result.evaluations)
    } finally { setEvaluating(false) }
  }

  const [form, setForm] = useState({ name: '', signal: 'traces', metric: 'error_rate_percent', threshold: '5', window_seconds: '300', severity: 'warning', description: '' })

  return (
    <div className="space-y-6">
      <PageHeader title="Signal Alerts" subtitle="Define alert conditions on OTel signal data" onRefresh={() => refetch()} />

      {/* Action bar */}
      <div className="flex items-center gap-3">
        <button onClick={() => setShowCreate(!showCreate)}
          className="flex items-center gap-1.5 px-3 py-2 bg-brand-600 hover:bg-brand-700 rounded-xl text-xs font-medium transition-colors">
          <Plus className="w-3.5 h-3.5" /> New Rule
        </button>
        <button onClick={handleEvaluate} disabled={evaluating}
          className="flex items-center gap-1.5 px-3 py-2 bg-white/5 border border-white/10 rounded-xl text-xs text-slate-400 hover:text-white transition-colors disabled:opacity-50">
          <Play className="w-3.5 h-3.5" /> {evaluating ? 'Evaluating...' : 'Evaluate Now'}
        </button>
        <span className="ml-auto text-xs text-slate-500">{rules.length} rules</span>
      </div>

      {/* Create Form */}
      {showCreate && (
        <div className="bg-white/5 border border-white/10 rounded-xl p-4 space-y-3">
          <h3 className="text-sm font-bold text-white">Create Alert Rule</h3>
          <div className="grid grid-cols-3 gap-3">
            <input value={form.name} onChange={e => setForm(f => ({ ...f, name: e.target.value }))} placeholder="Rule name"
              className="px-3 py-2 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none" />
            <select value={form.signal} onChange={e => setForm(f => ({ ...f, signal: e.target.value }))}
              className="px-3 py-2 bg-white/5 border border-white/10 rounded-xl text-sm text-white focus:outline-none [&>option]:bg-slate-900">
              <option value="traces">Traces</option><option value="metrics">Metrics</option><option value="logs">Logs</option>
            </select>
            <select value={form.severity} onChange={e => setForm(f => ({ ...f, severity: e.target.value }))}
              className="px-3 py-2 bg-white/5 border border-white/10 rounded-xl text-sm text-white focus:outline-none [&>option]:bg-slate-900">
              <option value="info">Info</option><option value="warning">Warning</option><option value="critical">Critical</option>
            </select>
          </div>
          <div className="grid grid-cols-3 gap-3">
            <input value={form.threshold} onChange={e => setForm(f => ({ ...f, threshold: e.target.value }))} placeholder="Threshold" type="number"
              className="px-3 py-2 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none" />
            <input value={form.window_seconds} onChange={e => setForm(f => ({ ...f, window_seconds: e.target.value }))} placeholder="Window (seconds)" type="number"
              className="px-3 py-2 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none" />
            <input value={form.description} onChange={e => setForm(f => ({ ...f, description: e.target.value }))} placeholder="Description"
              className="px-3 py-2 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none" />
          </div>
          <div className="flex gap-2">
            <button onClick={() => mutation.mutate({ ...form, threshold: parseFloat(form.threshold), window_seconds: parseInt(form.window_seconds), condition: { operator: '>' } })}
              disabled={!form.name || mutation.isPending}
              className="px-4 py-2 bg-brand-600 hover:bg-brand-700 rounded-xl text-xs font-medium transition-colors disabled:opacity-50">
              {mutation.isPending ? 'Creating...' : 'Create'}
            </button>
            <button onClick={() => setShowCreate(false)} className="px-4 py-2 bg-white/5 border border-white/10 rounded-xl text-xs text-slate-400 hover:text-white transition-colors">Cancel</button>
          </div>
        </div>
      )}

      {/* Evaluation Results */}
      {evalResults && (
        <div className="bg-white/5 border border-white/10 rounded-xl p-4 space-y-2">
          <h3 className="text-sm font-bold text-white mb-2">Evaluation Results</h3>
          {evalResults.map((e, i) => (
            <div key={i} className={`flex items-center justify-between px-3 py-2 rounded-lg border ${e.status === 'FIRING' ? 'bg-red-500/10 border-red-500/20' : 'bg-emerald-500/10 border-emerald-500/20'}`}>
              <div className="flex items-center gap-2">
                {e.status === 'FIRING' ? <XCircle className="w-4 h-4 text-red-400" /> : <CheckCircle className="w-4 h-4 text-emerald-400" />}
                <span className="text-sm text-white">{e.rule}</span>
              </div>
              <div className="flex items-center gap-4 text-xs">
                <span className="text-slate-400">Current: <span className="font-mono text-white">{typeof e.current === 'number' ? e.current.toFixed(2) : e.current}</span></span>
                <span className="text-slate-400">Threshold: <span className="font-mono text-white">{e.threshold}</span></span>
                <span className={`font-bold ${e.status === 'FIRING' ? 'text-red-400' : 'text-emerald-400'}`}>{e.status}</span>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Rules Table */}
      {isLoading && <LoadingSkeleton variant="table" count={4} />}

      {!isLoading && rules.length > 0 && (
        <div className="bg-white/5 border border-white/10 rounded-xl overflow-hidden">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-white/10 text-left">
                <th className="px-4 py-3 text-xs font-medium text-slate-500 uppercase">Name</th>
                <th className="px-4 py-3 text-xs font-medium text-slate-500 uppercase">Signal</th>
                <th className="px-4 py-3 text-xs font-medium text-slate-500 uppercase">Condition</th>
                <th className="px-4 py-3 text-xs font-medium text-slate-500 uppercase">Window</th>
                <th className="px-4 py-3 text-xs font-medium text-slate-500 uppercase">Severity</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-white/5">
              {rules.map(r => (
                <tr key={r.id} className="hover:bg-white/5">
                  <td className="px-4 py-3">
                    <div className="text-white font-medium">{r.name}</div>
                    {r.description && <div className="text-xs text-slate-500 mt-0.5">{r.description}</div>}
                  </td>
                  <td className="px-4 py-3 text-slate-300">{r.signal}</td>
                  <td className="px-4 py-3 font-mono text-xs text-slate-300">{r.metric} {r.condition?.operator} {r.threshold}</td>
                  <td className="px-4 py-3 text-slate-400">{r.window_seconds}s</td>
                  <td className="px-4 py-3">
                    <span className={`inline-flex items-center gap-1 text-xs px-2 py-0.5 rounded border ${sevBg(r.severity)}`}>
                      {sevIcon(r.severity)} {r.severity}
                    </span>
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
