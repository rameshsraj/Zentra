import { useQuery } from '@tanstack/react-query'
import { fetchJSON } from '@/lib/api/_client'
import PageHeader from '@/components/PageHeader'
import { cn } from '@/lib/utils'
import {
  Server, ArrowRight, Activity, Clock, AlertTriangle,
  CheckCircle, Radio,
} from 'lucide-react'

interface Collector {
  name: string
  last_seen: string | null
  signals_forwarded: number
}

interface PipelineResponse {
  collectors: Collector[]
  flow: { from: string; to: string; protocol: string }[]
}

const getPipeline = () => fetchJSON<PipelineResponse>('/otel/pipeline')

export default function OTelPipeline() {
  const { data, isLoading } = useQuery({
    queryKey: ['otel-pipeline'],
    queryFn: getPipeline,
    refetchInterval: 30000,
  })

  const collectors = data?.collectors ?? []

  const isHealthy = (c: Collector) => {
    if (!c.last_seen) return false
    const diff = Date.now() - new Date(c.last_seen).getTime()
    return diff < 5 * 60 * 1000 // 5 minutes
  }

  return (
    <div className="space-y-6">
      <PageHeader
        title="Pipeline Topology"
        subtitle="Visualize the OpenTelemetry Collector pipeline — apps → collectors → Zentra"
      />

      {/* Flow diagram */}
      <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-8">
        <div className="flex items-center justify-center gap-4 flex-wrap">
          {/* Apps node */}
          <div className="bg-blue-900/30 border border-blue-700/40 rounded-xl px-6 py-4 text-center min-w-[140px]">
            <Activity className="w-6 h-6 text-blue-400 mx-auto mb-2" />
            <div className="text-sm font-medium text-blue-300">Applications</div>
            <div className="text-[10px] text-slate-500 mt-1">OTel SDKs</div>
          </div>

          <ArrowRight className="w-6 h-6 text-slate-600 flex-shrink-0" />

          {/* Collectors */}
          {collectors.length > 0 ? (
            <div className="flex flex-col gap-2">
              {collectors.map(c => (
                <div
                  key={c.name}
                  className={cn(
                    'border rounded-xl px-5 py-3 min-w-[180px]',
                    isHealthy(c)
                      ? 'bg-emerald-900/20 border-emerald-700/40'
                      : 'bg-red-900/20 border-red-700/40'
                  )}
                >
                  <div className="flex items-center gap-2 mb-1">
                    {isHealthy(c) ? (
                      <CheckCircle className="w-3.5 h-3.5 text-emerald-400" />
                    ) : (
                      <AlertTriangle className="w-3.5 h-3.5 text-red-400" />
                    )}
                    <span className="text-sm font-medium text-white truncate">{c.name}</span>
                  </div>
                  <div className="text-[10px] text-slate-400 space-y-0.5">
                    <div className="flex items-center gap-1">
                      <Radio className="w-2.5 h-2.5" />
                      {c.signals_forwarded.toLocaleString()} signals
                    </div>
                    <div className="flex items-center gap-1">
                      <Clock className="w-2.5 h-2.5" />
                      {c.last_seen ? new Date(c.last_seen).toLocaleTimeString() : 'Never'}
                    </div>
                  </div>
                </div>
              ))}
            </div>
          ) : (
            <div className="bg-slate-700/30 border border-slate-600 border-dashed rounded-xl px-6 py-4 text-center min-w-[180px]">
              <Server className="w-6 h-6 text-slate-500 mx-auto mb-2" />
              <div className="text-sm text-slate-400">No Collectors</div>
              <div className="text-[10px] text-slate-600 mt-1">
                {isLoading ? 'Loading...' : 'Direct SDK → Zentra'}
              </div>
            </div>
          )}

          <ArrowRight className="w-6 h-6 text-slate-600 flex-shrink-0" />

          {/* Zentra node */}
          <div className="bg-cyan-900/30 border border-cyan-700/40 rounded-xl px-6 py-4 text-center min-w-[140px]">
            <Server className="w-6 h-6 text-cyan-400 mx-auto mb-2" />
            <div className="text-sm font-medium text-cyan-300">Zentra</div>
            <div className="text-[10px] text-slate-500 mt-1">OTLP Backend</div>
          </div>
        </div>
      </div>

      {/* Collector detail cards */}
      {collectors.length > 0 && (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {collectors.map(c => (
            <div key={c.name} className="bg-slate-800/60 border border-slate-700 rounded-xl p-5">
              <div className="flex items-center justify-between mb-3">
                <div className="flex items-center gap-2">
                  <Server className="w-4 h-4 text-cyan-400" />
                  <span className="text-sm font-medium text-white">{c.name}</span>
                </div>
                <span className={cn(
                  'inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium',
                  isHealthy(c)
                    ? 'bg-emerald-900/40 text-emerald-300 border border-emerald-700/40'
                    : 'bg-red-900/40 text-red-300 border border-red-700/40'
                )}>
                  <div className={cn('w-1.5 h-1.5 rounded-full', isHealthy(c) ? 'bg-emerald-400' : 'bg-red-400')} />
                  {isHealthy(c) ? 'Healthy' : 'Unhealthy'}
                </span>
              </div>
              <div className="space-y-2 text-xs">
                <div className="flex justify-between text-slate-400">
                  <span>Signals Forwarded</span>
                  <span className="text-white font-mono">{c.signals_forwarded.toLocaleString()}</span>
                </div>
                <div className="flex justify-between text-slate-400">
                  <span>Last Seen</span>
                  <span className="text-white">{c.last_seen ? new Date(c.last_seen).toLocaleString() : 'Never'}</span>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* No collectors guidance */}
      {collectors.length === 0 && !isLoading && (
        <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-6 text-center">
          <p className="text-sm text-slate-400 mb-2">No OTel Collectors detected in the pipeline.</p>
          <p className="text-xs text-slate-500">
            Collectors are identified by resource attributes containing "otelcol" or "collector" in the service name.
            Configure and deploy a Collector using the{' '}
            <a href="/otel-config" className="text-cyan-400 hover:text-cyan-300">Config Generator</a>.
          </p>
        </div>
      )}
    </div>
  )
}
