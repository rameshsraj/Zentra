import { useState, useMemo, useEffect } from 'react'
import { useQuery, useMutation } from '@tanstack/react-query'
import { searchTraces, getTrace, getTraceServices, getTraceOperations, fetchJSON } from '@/lib/api'
import type { TraceSpan } from '@/lib/api/types'
import PageHeader from '@/components/PageHeader'
import LoadingSkeleton from '@/components/LoadingSkeleton'
import { Search, ChevronRight, AlertTriangle, CheckCircle, Layers, X, Code, Info } from 'lucide-react'
import { cn } from '@/lib/utils'

interface TraceQlResult {
  valid: boolean
  error?: {
    position: number
    message: string
  }
}

function fmtDuration(us: number) {
  if (us < 1000) return `${us}µs`
  if (us < 1_000_000) return `${(us / 1000).toFixed(1)}ms`
  return `${(us / 1_000_000).toFixed(2)}s`
}

function relTime(iso: string) {
  const d = Date.now() - new Date(iso).getTime()
  if (d < 60_000) return `${Math.floor(d / 1000)}s ago`
  if (d < 3_600_000) return `${Math.floor(d / 60_000)}m ago`
  if (d < 86_400_000) return `${Math.floor(d / 3_600_000)}h ago`
  return `${Math.floor(d / 86_400_000)}d ago`
}

const statusColor = (s: string) =>
  s === 'error' ? 'text-red-400' : s === 'ok' ? 'text-emerald-400' : 'text-slate-400'
const statusBg = (s: string) =>
  s === 'error' ? 'bg-red-500/10 border-red-500/20' : s === 'ok' ? 'bg-emerald-500/10 border-emerald-500/20' : 'bg-slate-500/10 border-slate-500/20'

export default function TracesPage() {
  const [query, setQuery] = useState<string>('{status=error}')
  const [limit, setLimit] = useState(50)
  const [selectedTraceId, setSelectedTraceId] = useState<string | null>(null)
  const [validation, setValidation] = useState<TraceQlResult | null>(null)

  const { data: servicesData } = useQuery({ queryKey: ['trace-services'], queryFn: getTraceServices })
  
  const { data, isLoading, refetch } = useQuery({
    queryKey: ['traces-query', query, limit],
    queryFn: () => fetchJSON<{ traces: any[], count: number }>('/query/traceql', {
      method: 'POST',
      body: JSON.stringify({ expr: query, limit })
    }),
    enabled: !!query && (!validation || validation.valid),
    refetchInterval: 15_000,
  })

  const { data: traceDetail, isLoading: detailLoading } = useQuery({
    queryKey: ['trace-detail', selectedTraceId],
    queryFn: () => getTrace(selectedTraceId!),
    enabled: !!selectedTraceId,
  })

  // Debounced validation
  useEffect(() => {
    const timer = setTimeout(async () => {
      if (!query.trim()) {
        setValidation({ valid: true });
        return;
      }
      try {
        const res = await fetchJSON<TraceQlResult>(`/query/traceql/validate?expr=${encodeURIComponent(query)}`);
        setValidation(res);
      } catch (e) {
        setValidation({ valid: false, error: { position: 0, message: 'Validation failed' } });
      }
    }, 500);
    return () => clearTimeout(timer);
  }, [query]);

  const traces = data?.traces ?? []
  const services = servicesData?.services ?? []

  const exampleQueries = [
    { label: 'Errors', val: '{status=error}' },
    { label: 'Slow > 1s', val: '{duration > 1s}' },
    { label: 'DB Calls', val: '{span.db.system != ""}' },
    { label: 'Order Service', val: '{service.name="order-service"}' },
  ]

  return (
    <div className="space-y-6">
      <PageHeader
        title="Distributed Traces"
        subtitle="Search and inspect trace spans using TraceQL query language"
        onRefresh={() => refetch()}
      />

      {/* TraceQL Search Bar */}
      <div className="space-y-2">
        <div className="relative group">
          <div className="absolute left-3 top-3 text-slate-500">
            <Code className="w-4 h-4" />
          </div>
          <textarea
            value={query}
            onChange={e => setQuery(e.target.value)}
            placeholder='{service.name="auth" && status=error}'
            className={cn(
              "w-full bg-slate-900 border border-slate-700 rounded-xl pl-10 pr-4 py-2.5 font-mono text-sm text-white focus:outline-none focus:ring-1 transition-all min-h-[44px] resize-none",
              validation?.valid === false ? "border-red-500/50 focus:ring-red-500/50" : "focus:border-brand-500 focus:ring-brand-500/50"
            )}
            rows={1}
            onKeyDown={e => {
              if (e.key === 'Enter' && !e.shiftKey) {
                e.preventDefault();
                refetch();
              }
            }}
          />
          {validation?.valid === false && validation.error && (
            <div className="mt-1 flex items-center gap-1.5 text-[11px] text-red-400 px-1">
              <AlertTriangle className="w-3 h-3" />
              <span>{validation.error.message} (at {validation.error.position})</span>
            </div>
          )}
        </div>

        <div className="flex items-center gap-2 overflow-x-auto pb-1 scrollbar-hide">
          <span className="text-[10px] text-slate-500 uppercase font-bold mr-1 flex-shrink-0">Examples:</span>
          {exampleQueries.map(q => (
            <button
              key={q.label}
              onClick={() => setQuery(q.val)}
              className="px-2 py-1 bg-white/5 border border-white/10 rounded-lg text-[10px] text-slate-400 hover:text-white hover:bg-white/10 transition-colors whitespace-nowrap"
            >
              {q.label}
            </button>
          ))}
          <div className="ml-auto flex items-center gap-2 bg-white/5 border border-white/10 rounded-lg px-2 py-1">
             <span className="text-[10px] text-slate-500">Limit</span>
             <select 
               value={limit} 
               onChange={e => setLimit(Number(e.target.value))}
               className="bg-transparent text-[10px] text-white focus:outline-none"
             >
               {[20, 50, 100, 200].map(n => <option key={n} value={n} className="bg-slate-900">{n}</option>)}
             </select>
          </div>
        </div>
      </div>

      {/* Trace List */}
      {isLoading && <LoadingSkeleton variant="table" count={5} />}

      {!isLoading && traces.length === 0 && (
        <div className="bg-white/5 border border-white/10 rounded-xl p-12 text-center">
          <Layers className="w-12 h-12 text-slate-600 mx-auto mb-3" />
          <h3 className="text-lg font-bold text-white mb-1">No traces found</h3>
          <p className="text-sm text-slate-400">Try adjusting your TraceQL query or time window.</p>
        </div>
      )}

      {!isLoading && traces.length > 0 && !selectedTraceId && (
        <div className="bg-white/5 border border-white/10 rounded-xl overflow-hidden">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-white/10 text-left">
                <th className="px-4 py-3 text-xs font-medium text-slate-500 uppercase tracking-wider">Trace ID</th>
                <th className="px-4 py-3 text-xs font-medium text-slate-500 uppercase tracking-wider">Service</th>
                <th className="px-4 py-3 text-xs font-medium text-slate-500 uppercase tracking-wider">Operation</th>
                <th className="px-4 py-3 text-xs font-medium text-slate-500 uppercase tracking-wider">Duration</th>
                <th className="px-4 py-3 text-xs font-medium text-slate-500 uppercase tracking-wider">Spans</th>
                <th className="px-4 py-3 text-xs font-medium text-slate-500 uppercase tracking-wider">Status</th>
                <th className="px-4 py-3 text-xs font-medium text-slate-500 uppercase tracking-wider">Time</th>
                <th className="px-4 py-3"></th>
              </tr>
            </thead>
            <tbody className="divide-y divide-white/5">
              {traces.map(t => (
                <tr
                  key={t.trace_id}
                  onClick={() => setSelectedTraceId(t.trace_id)}
                  className="hover:bg-white/5 cursor-pointer transition-colors"
                >
                  <td className="px-4 py-3 font-mono text-xs text-brand-400">{t.trace_id.slice(0, 16)}…</td>
                  <td className="px-4 py-3 text-white font-medium">{t.root_service}</td>
                  <td className="px-4 py-3 text-slate-300">{t.root_operation}</td>
                  <td className="px-4 py-3 font-mono text-slate-300">{fmtDuration(t.duration_ms * 1000)}</td>
                  <td className="px-4 py-3 text-slate-400">{t.span_count}</td>
                  <td className="px-4 py-3">
                    <span className={`inline-flex items-center gap-1 text-xs ${statusColor(t.has_errors ? 'error' : 'ok')}`}>
                      {t.has_errors ? <AlertTriangle className="w-3 h-3" /> : <CheckCircle className="w-3 h-3" />}
                      {t.has_errors ? 'Error' : 'OK'}
                    </span>
                  </td>
                  <td className="px-4 py-3 text-xs text-slate-500">{relTime(t.started_at)}</td>
                  <td className="px-4 py-3 text-slate-500"><ChevronRight className="w-4 h-4" /></td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* Trace Detail / Waterfall */}
      {selectedTraceId && (
        <TraceDetailPanel
          traceId={selectedTraceId}
          spans={traceDetail?.spans ?? []}
          loading={detailLoading}
          onClose={() => setSelectedTraceId(null)}
        />
      )}
    </div>
  )
}

// ─── Trace Detail Panel ───────────────────────────────────────────────────────

function TraceDetailPanel({ traceId, spans, loading, onClose }: {
  traceId: string
  spans: TraceSpan[]
  loading: boolean
  onClose: () => void
}) {
  const [selectedSpan, setSelectedSpan] = useState<TraceSpan | null>(null)

  const sorted = useMemo(() => {
    if (!spans.length) return []
    return [...spans].sort((a, b) => new Date(a.start_time).getTime() - new Date(b.start_time).getTime())
  }, [spans])

  const traceStart = sorted.length ? new Date(sorted[0].start_time).getTime() : 0
  const traceEnd = sorted.length
    ? Math.max(...sorted.map(s => new Date(s.start_time).getTime() + s.duration_us / 1000))
    : 0
  const totalMs = traceEnd - traceStart || 1

  return (
    <div className="bg-white/5 border border-white/10 rounded-xl overflow-hidden">
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-white/10">
        <div className="flex items-center gap-3">
          <button onClick={onClose} className="p-1 rounded hover:bg-white/10 text-slate-400"><X className="w-4 h-4" /></button>
          <h3 className="text-sm font-bold text-white">Trace Detail</h3>
          <span className="font-mono text-xs text-brand-400">{traceId.slice(0, 24)}…</span>
          <span className="text-xs text-slate-500">{spans.length} spans</span>
          {totalMs > 0 && <span className="text-xs text-slate-500">• {fmtDuration(totalMs * 1000)}</span>}
        </div>
      </div>

      {loading && <div className="p-6"><LoadingSkeleton variant="table" count={3} /></div>}

      {!loading && sorted.length === 0 && (
        <div className="p-8 text-center text-sm text-slate-500">No spans found for this trace.</div>
      )}

      {!loading && sorted.length > 0 && (
        <div className="flex divide-x divide-white/10">
          {/* Waterfall */}
          <div className="flex-1 overflow-auto max-h-[60vh]">
            {sorted.map(span => {
              const offset = (new Date(span.start_time).getTime() - traceStart) / totalMs * 100
              const width = Math.max((span.duration_us / 1000) / totalMs * 100, 1)
              const isSelected = selectedSpan?.span_id === span.span_id
              const depth = getDepth(span, sorted)
              return (
                <div
                  key={span.span_id}
                  onClick={() => setSelectedSpan(isSelected ? null : span)}
                  className={`flex items-center gap-2 px-3 py-1.5 cursor-pointer border-b border-white/5 transition-colors ${
                    isSelected ? 'bg-brand-500/10' : 'hover:bg-white/5'
                  }`}
                >
                  <div className="w-[200px] flex-shrink-0 truncate" style={{ paddingLeft: depth * 16 }}>
                    <span className="text-[11px] text-white font-medium">{span.service_name}</span>
                    <span className="text-[10px] text-slate-500 ml-1">{span.operation_name}</span>
                  </div>
                  <div className="flex-1 relative h-5 bg-slate-800/50 rounded-sm overflow-hidden">
                    <div
                      className={`absolute top-0.5 bottom-0.5 rounded-sm ${
                        span.status === 'error' ? 'bg-red-500/70' : 'bg-brand-500/60'
                      }`}
                      style={{ left: `${offset}%`, width: `${width}%`, minWidth: 4 }}
                    />
                  </div>
                  <div className="w-[70px] flex-shrink-0 text-right text-[11px] font-mono text-slate-400">
                    {fmtDuration(span.duration_us)}
                  </div>
                </div>
              )
            })}
          </div>

          {/* Span Detail Sidebar */}
          {selectedSpan && (
            <div className="w-[340px] flex-shrink-0 overflow-auto max-h-[60vh] p-4 space-y-4">
              <div>
                <h4 className="text-xs font-bold text-white uppercase tracking-wider mb-2">Span Info</h4>
                <div className="space-y-1.5 text-xs">
                  <Row label="Span ID" value={selectedSpan.span_id} />
                  <Row label="Parent" value={selectedSpan.parent_span_id || '(root)'} />
                  <Row label="Service" value={selectedSpan.service_name} />
                  <Row label="Operation" value={selectedSpan.operation_name} />
                  <Row label="Duration" value={fmtDuration(selectedSpan.duration_us)} />
                  <Row label="Status" value={selectedSpan.status} className={statusColor(selectedSpan.status)} />
                </div>
              </div>

              {Object.keys(selectedSpan.tags).length > 0 && (
                <div>
                  <h4 className="text-xs font-bold text-white uppercase tracking-wider mb-2">Attributes</h4>
                  <div className="space-y-1 text-xs">
                    {Object.entries(selectedSpan.tags).map(([k, v]) => (
                      <div key={k} className="flex justify-between gap-2">
                        <span className="text-slate-500 truncate">{k}</span>
                        <span className="text-slate-300 font-mono truncate text-right">{v}</span>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {selectedSpan.events.length > 0 && (
                <div>
                  <h4 className="text-xs font-bold text-white uppercase tracking-wider mb-2">Events</h4>
                  <div className="space-y-2">
                    {selectedSpan.events.map((e, i) => (
                      <div key={i} className={`text-xs p-2 rounded border ${statusBg('unset')}`}>
                        <div className="font-medium text-white">{e.name}</div>
                        <div className="text-slate-500 text-[10px]">{new Date(e.timestamp).toISOString()}</div>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>
          )}
        </div>
      )}
    </div>
  )
}

function Row({ label, value, className }: { label: string; value: string; className?: string }) {
  return (
    <div className="flex justify-between gap-2">
      <span className="text-slate-500">{label}</span>
      <span className={`font-mono truncate text-right ${className || 'text-slate-300'}`}>{value}</span>
    </div>
  )
}

function getDepth(span: TraceSpan, all: TraceSpan[]): number {
  let depth = 0
  let current = span
  while (current.parent_span_id) {
    const parent = all.find(s => s.span_id === current.parent_span_id)
    if (!parent) break
    depth++
    current = parent
  }
  return depth
}
