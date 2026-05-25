import { useQuery } from '@tanstack/react-query'
import { fetchJSON } from '@/lib/api/_client'
import PageHeader from '@/components/PageHeader'
import { cn } from '@/lib/utils'
import {
  Brain, Zap, DollarSign, Clock, AlertTriangle,
  BarChart3, TrendingUp,
} from 'lucide-react'

interface LlmOverview {
  total_requests: number
  total_tokens: number
  total_cost: number
  avg_latency_ms: number
  error_rate: number
  models: number
}

interface LlmModel {
  model: string
  gen_ai_system: string
  request_count: number
  avg_latency_ms: number
  total_input_tokens: number
  total_output_tokens: number
  error_rate: number
}

interface LlmModelsResponse {
  models: LlmModel[]
}

interface CostPoint {
  bucket: string
  cost: number
  tokens: number
}

interface CostTrendResponse {
  trend: CostPoint[]
}

interface LatencyBucket {
  range: string
  count: number
}

interface LatencyResponse {
  distribution: LatencyBucket[]
}

const getLlmOverview = () => fetchJSON<LlmOverview>('/llm/overview')
const getLlmModels = () => fetchJSON<LlmModelsResponse>('/llm/models')
const getCostTrend = () => fetchJSON<CostTrendResponse>('/llm/cost-trend')
const getLatencyDist = () => fetchJSON<LatencyResponse>('/llm/latency-distribution')

export default function LLMObservability() {
  const { data: overview } = useQuery({ queryKey: ['llm-overview'], queryFn: getLlmOverview, refetchInterval: 30000 })
  const { data: modelsData } = useQuery({ queryKey: ['llm-models'], queryFn: getLlmModels, refetchInterval: 30000 })
  const { data: costData } = useQuery({ queryKey: ['llm-cost-trend'], queryFn: getCostTrend, refetchInterval: 60000 })
  const { data: latencyData } = useQuery({ queryKey: ['llm-latency'], queryFn: getLatencyDist, refetchInterval: 60000 })

  const models = modelsData?.models ?? []
  const costTrend = costData?.trend ?? []
  const latencyDist = latencyData?.distribution ?? []

  return (
    <div className="space-y-6">
      <PageHeader
        title="AI / LLM Observability"
        subtitle="Monitor GenAI workloads — latency, token usage, cost trends, and model performance via OpenTelemetry semantic conventions"
      />

      {/* Overview cards */}
      <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-4">
        <StatCard icon={Zap} label="Total Requests" value={overview?.total_requests ?? 0} format="number" color="cyan" />
        <StatCard icon={BarChart3} label="Total Tokens" value={overview?.total_tokens ?? 0} format="number" color="blue" />
        <StatCard icon={DollarSign} label="Est. Cost" value={overview?.total_cost ?? 0} format="currency" color="emerald" />
        <StatCard icon={Clock} label="Avg Latency" value={overview?.avg_latency_ms ?? 0} format="ms" color="amber" />
        <StatCard icon={AlertTriangle} label="Error Rate" value={overview?.error_rate ?? 0} format="percent" color="red" />
        <StatCard icon={Brain} label="Models" value={overview?.models ?? 0} format="number" color="purple" />
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Models table */}
        <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-5">
          <h3 className="text-sm font-semibold text-white flex items-center gap-2 mb-4">
            <Brain className="w-4 h-4 text-purple-400" />
            Model Breakdown
          </h3>
          {models.length === 0 ? (
            <p className="text-xs text-slate-500 text-center py-6">No GenAI spans detected yet. Send traces with gen_ai.* attributes.</p>
          ) : (
            <div className="overflow-x-auto">
              <table className="w-full text-xs">
                <thead>
                  <tr className="border-b border-slate-700 text-slate-400">
                    <th className="text-left px-3 py-2">Model</th>
                    <th className="text-left px-3 py-2">System</th>
                    <th className="text-right px-3 py-2">Requests</th>
                    <th className="text-right px-3 py-2">Avg Latency</th>
                    <th className="text-right px-3 py-2">Tokens</th>
                  </tr>
                </thead>
                <tbody>
                  {models.map(m => (
                    <tr key={m.model} className="border-b border-slate-700/50 hover:bg-slate-700/20">
                      <td className="px-3 py-2 text-white font-medium">{m.model}</td>
                      <td className="px-3 py-2">
                        <span className="px-1.5 py-0.5 bg-purple-900/30 text-purple-300 rounded text-[10px]">{m.gen_ai_system}</span>
                      </td>
                      <td className="px-3 py-2 text-right font-mono text-slate-300">{m.request_count.toLocaleString()}</td>
                      <td className="px-3 py-2 text-right font-mono text-slate-300">{m.avg_latency_ms.toFixed(0)}ms</td>
                      <td className="px-3 py-2 text-right font-mono text-slate-300">
                        {(m.total_input_tokens + m.total_output_tokens).toLocaleString()}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </div>

        {/* Cost trend */}
        <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-5">
          <h3 className="text-sm font-semibold text-white flex items-center gap-2 mb-4">
            <TrendingUp className="w-4 h-4 text-emerald-400" />
            Cost Trend (Hourly)
          </h3>
          {costTrend.length === 0 ? (
            <p className="text-xs text-slate-500 text-center py-6">No cost data available yet.</p>
          ) : (
            <div>
              <div className="flex items-end gap-px h-32">
                {costTrend.map((p, i) => {
                  const maxCost = Math.max(1, ...costTrend.map(c => c.cost))
                  return (
                    <div
                      key={i}
                      className="flex-1 bg-emerald-500/50 hover:bg-emerald-400/70 rounded-t transition"
                      style={{ height: `${(p.cost / maxCost) * 100}%`, minHeight: '2px' }}
                      title={`${new Date(p.bucket).toLocaleString()}: $${p.cost.toFixed(4)} (${p.tokens} tokens)`}
                    />
                  )
                })}
              </div>
              <div className="flex justify-between text-[10px] text-slate-500 mt-2">
                <span>{costTrend.length > 0 ? new Date(costTrend[0].bucket).toLocaleDateString() : ''}</span>
                <span>{costTrend.length > 0 ? new Date(costTrend[costTrend.length - 1].bucket).toLocaleDateString() : ''}</span>
              </div>
            </div>
          )}
        </div>

        {/* Latency distribution */}
        <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-5">
          <h3 className="text-sm font-semibold text-white flex items-center gap-2 mb-4">
            <Clock className="w-4 h-4 text-amber-400" />
            Latency Distribution
          </h3>
          {latencyDist.length === 0 ? (
            <p className="text-xs text-slate-500 text-center py-6">No latency data available yet.</p>
          ) : (
            <div className="space-y-2">
              {latencyDist.map(b => {
                const maxCount = Math.max(1, ...latencyDist.map(d => d.count))
                return (
                  <div key={b.range} className="flex items-center gap-3">
                    <span className="text-xs text-slate-400 w-20 text-right font-mono">{b.range}</span>
                    <div className="flex-1 h-5 bg-slate-700 rounded overflow-hidden">
                      <div
                        className="h-full bg-amber-500/60 rounded transition-all"
                        style={{ width: `${(b.count / maxCount) * 100}%` }}
                      />
                    </div>
                    <span className="text-xs text-slate-500 w-12 text-right font-mono">{b.count}</span>
                  </div>
                )
              })}
            </div>
          )}
        </div>

        {/* Token efficiency */}
        <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-5">
          <h3 className="text-sm font-semibold text-white flex items-center gap-2 mb-4">
            <BarChart3 className="w-4 h-4 text-blue-400" />
            Token Usage by Model
          </h3>
          {models.length === 0 ? (
            <p className="text-xs text-slate-500 text-center py-6">No token data available.</p>
          ) : (
            <div className="space-y-3">
              {models.map(m => {
                const total = m.total_input_tokens + m.total_output_tokens
                const inputPct = total > 0 ? (m.total_input_tokens / total) * 100 : 50
                return (
                  <div key={m.model}>
                    <div className="flex justify-between text-xs mb-1">
                      <span className="text-white">{m.model}</span>
                      <span className="text-slate-400">{total.toLocaleString()} tokens</span>
                    </div>
                    <div className="h-3 bg-slate-700 rounded-full overflow-hidden flex">
                      <div className="bg-blue-500/70 h-full" style={{ width: `${inputPct}%` }} title={`Input: ${m.total_input_tokens.toLocaleString()}`} />
                      <div className="bg-cyan-500/70 h-full flex-1" title={`Output: ${m.total_output_tokens.toLocaleString()}`} />
                    </div>
                    <div className="flex justify-between text-[10px] text-slate-500 mt-0.5">
                      <span>Input: {m.total_input_tokens.toLocaleString()}</span>
                      <span>Output: {m.total_output_tokens.toLocaleString()}</span>
                    </div>
                  </div>
                )
              })}
            </div>
          )}
        </div>
      </div>
    </div>
  )
}

function StatCard({ icon: Icon, label, value, format, color }: {
  icon: React.ElementType; label: string; value: number; format: 'number' | 'currency' | 'ms' | 'percent'; color: string
}) {
  let display: string
  switch (format) {
    case 'currency': display = `$${value.toFixed(2)}`; break
    case 'ms': display = `${value.toFixed(0)}ms`; break
    case 'percent': display = `${value.toFixed(1)}%`; break
    default: display = value.toLocaleString()
  }

  return (
    <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-4">
      <div className="flex items-center gap-2 mb-2">
        <Icon className={cn('w-4 h-4', `text-${color}-400`)} />
        <span className="text-[10px] text-slate-400 uppercase tracking-wide">{label}</span>
      </div>
      <div className="text-xl font-bold text-white">{display}</div>
    </div>
  )
}
