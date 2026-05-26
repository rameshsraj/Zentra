import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { fetchJSON } from '@/lib/api'
import PageHeader from '@/components/PageHeader'
import LoadingSkeleton from '@/components/LoadingSkeleton'
import { BarChart3, AlertTriangle, Clock, Activity } from 'lucide-react'

interface SpanMetric {
  group: string
  request_count: number
  error_count: number
  error_rate_percent: number
  avg_ms: number
  p50_ms: number
  p95_ms: number
  p99_ms: number
}

const WINDOWS = ['5m', '15m', '1h', '6h', '24h']
const GROUP_BY = [
  { key: 'service', label: 'Service' },
  { key: 'method', label: 'HTTP Method' },
  { key: 'db', label: 'DB System' },
]

const getSpanMetrics = (window: string, groupBy: string) =>
  fetchJSON<{ metrics: SpanMetric[]; window: string; group_by: string; total_requests: number }>(
    `/otel/span-metrics?window=${window}&group_by=${groupBy}`
  )

function errColor(rate: number) {
  if (rate > 5) return 'text-red-400'
  if (rate > 1) return 'text-amber-400'
  return 'text-emerald-400'
}

function errBg(rate: number) {
  if (rate > 5) return 'bg-red-500/10 border-red-500/20'
  if (rate > 1) return 'bg-amber-500/10 border-amber-500/20'
  return 'bg-emerald-500/10 border-emerald-500/20'
}

export default function OTelSpanMetricsPage() {
  const [window, setWindow] = useState('1h')
  const [groupBy, setGroupBy] = useState('service')

  const { data, isLoading, refetch } = useQuery({
    queryKey: ['otel-span-metrics', window, groupBy],
    queryFn: () => getSpanMetrics(window, groupBy),
    refetchInterval: 15_000,
  })

  const metrics = data?.metrics ?? []

  return (
    <div className="space-y-6">
      <PageHeader
        title="Span Metrics (RED)"
        subtitle="Rate, Error, Duration — derived automatically from trace spans"
        onRefresh={() => refetch()}
      />

      {/* Controls */}
      <div className="flex items-center gap-3">
        <div className="flex bg-white/5 border border-white/10 rounded-xl overflow-hidden">
          {WINDOWS.map(w => (
            <button key={w} onClick={() => setWindow(w)}
              className={`px-3 py-2 text-xs font-medium transition-colors ${window === w ? 'bg-brand-600 text-white' : 'text-slate-400 hover:text-white'}`}>
              {w}
            </button>
          ))}
        </div>
        <div className="flex bg-white/5 border border-white/10 rounded-xl overflow-hidden">
          {GROUP_BY.map(g => (
            <button key={g.key} onClick={() => setGroupBy(g.key)}
              className={`px-3 py-2 text-xs font-medium transition-colors ${groupBy === g.key ? 'bg-brand-600 text-white' : 'text-slate-400 hover:text-white'}`}>
              {g.label}
            </button>
          ))}
        </div>
        <span className="ml-auto text-xs text-slate-500">{data?.total_requests?.toLocaleString() ?? 0} total requests</span>
      </div>

      {isLoading && <LoadingSkeleton variant="card" count={3} />}

      {!isLoading && metrics.length === 0 && (
        <div className="bg-white/5 border border-white/10 rounded-xl p-12 text-center">
          <BarChart3 className="w-12 h-12 text-slate-600 mx-auto mb-3" />
          <h3 className="text-lg font-bold text-white mb-1">No span metrics yet</h3>
          <p className="text-sm text-slate-400">Send trace spans via OTLP to see RED metrics here.</p>
        </div>
      )}

      {/* RED Cards */}
      <div className="space-y-3">
        {metrics.map(m => (
          <div key={m.group} className="bg-white/5 border border-white/10 rounded-xl p-4">
            <div className="flex items-center gap-3 mb-3">
              <h3 className="text-sm font-bold text-white">{m.group || '(unknown)'}</h3>
            </div>
            <div className="grid grid-cols-3 gap-4">
              {/* Rate */}
              <div className="space-y-1">
                <div className="text-[10px] text-slate-500 uppercase tracking-wider font-medium flex items-center gap-1">
                  <Activity className="w-3 h-3" /> Rate
                </div>
                <div className="text-lg font-bold text-white">{m.request_count.toLocaleString()}</div>
                <div className="text-xs text-slate-400">{(m.request_count / (window === '5m' ? 300 : window === '15m' ? 900 : window === '6h' ? 21600 : window === '24h' ? 86400 : 3600)).toFixed(1)} req/s</div>
              </div>
              {/* Error */}
              <div className="space-y-1">
                <div className="text-[10px] text-slate-500 uppercase tracking-wider font-medium flex items-center gap-1">
                  <AlertTriangle className="w-3 h-3" /> Error
                </div>
                <div className={`text-lg font-bold ${errColor(m.error_rate_percent)}`}>{m.error_rate_percent.toFixed(1)}%</div>
                <div className={`text-xs px-2 py-0.5 rounded border inline-block ${errBg(m.error_rate_percent)}`}>
                  {m.error_count} errors
                </div>
              </div>
              {/* Duration */}
              <div className="space-y-1.5">
                <div className="text-[10px] text-slate-500 uppercase tracking-wider font-medium flex items-center gap-1">
                  <Clock className="w-3 h-3" /> Duration
                </div>
                <div className="space-y-1">
                  {[
                    { label: 'p50', value: m.p50_ms, max: m.p99_ms },
                    { label: 'p95', value: m.p95_ms, max: m.p99_ms },
                    { label: 'p99', value: m.p99_ms, max: m.p99_ms },
                  ].map(p => (
                    <div key={p.label} className="flex items-center gap-2">
                      <span className="text-[10px] text-slate-500 w-6">{p.label}</span>
                      <div className="flex-1 h-1.5 bg-slate-800 rounded-full overflow-hidden">
                        <div className="h-full bg-brand-500/60 rounded-full" style={{ width: `${p.max > 0 ? (p.value / p.max) * 100 : 0}%` }} />
                      </div>
                      <span className="text-[10px] font-mono text-slate-400 w-14 text-right">{p.value?.toFixed(0) ?? 0}ms</span>
                    </div>
                  ))}
                  <div className="text-[10px] text-slate-500">avg {m.avg_ms?.toFixed(1) ?? 0}ms</div>
                </div>
              </div>
            </div>
          </div>
        ))}
      </div>
    </div>
  )
}
