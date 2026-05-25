import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { fetchJSON } from '@/lib/api/_client'
import PageHeader from '@/components/PageHeader'
import { cn } from '@/lib/utils'
import {
  Zap, ToggleLeft, ToggleRight, Settings, BarChart3,
  Clock, TrendingUp, ArrowRight,
} from 'lucide-react'

interface Connector {
  type: string
  enabled: boolean
  description: string
  dimensions: string[]
  interval_seconds: number
  metrics_generated: string[]
}

interface ConnectorsResponse {
  connectors: Connector[]
}

interface DerivedMetric {
  service_name: string
  metric_name: string
  bucket_start: string
  value: number
  dimensions: Record<string, string>
}

interface DerivedMetricsResponse {
  metrics: DerivedMetric[]
  count: number
}

const getConnectors = () => fetchJSON<ConnectorsResponse>('/otel/connectors')
const getDerivedMetrics = (service?: string) =>
  fetchJSON<DerivedMetricsResponse>(`/otel/derived-metrics${service ? `?service=${service}` : ''}`)

export default function OTelConnectors() {
  const [selectedService, setSelectedService] = useState('')
  const queryClient = useQueryClient()

  const { data: connectorsData } = useQuery({
    queryKey: ['otel-connectors'],
    queryFn: getConnectors,
  })

  const { data: metricsData, isLoading: metricsLoading } = useQuery({
    queryKey: ['otel-derived-metrics', selectedService],
    queryFn: () => getDerivedMetrics(selectedService || undefined),
    refetchInterval: 30000,
  })

  const toggleMutation = useMutation({
    mutationFn: (connector: Connector) =>
      fetchJSON('/otel/connectors', {
        method: 'POST',
        body: JSON.stringify({
          connector_type: connector.type,
          enabled: !connector.enabled,
          dimensions: connector.dimensions,
          interval_seconds: connector.interval_seconds,
        }),
      }),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['otel-connectors'] }),
  })

  const connectors = connectorsData?.connectors ?? []
  const metrics = metricsData?.metrics ?? []

  return (
    <div className="space-y-6">
      <PageHeader
        title="Signal Connectors"
        subtitle="Transform signals automatically — derive RED metrics from traces without separate instrumentation"
      />

      {/* Connector cards */}
      <div className="space-y-4">
        {connectors.map(c => (
          <div key={c.type} className="bg-slate-800/60 border border-slate-700 rounded-xl p-5">
            <div className="flex items-center justify-between mb-3">
              <div className="flex items-center gap-3">
                <Zap className={cn('w-5 h-5', c.enabled ? 'text-emerald-400' : 'text-slate-500')} />
                <div>
                  <h3 className="text-sm font-semibold text-white capitalize">{c.type.replace(/-/g, ' ')}</h3>
                  <p className="text-xs text-slate-400">{c.description}</p>
                </div>
              </div>
              <button
                onClick={() => toggleMutation.mutate(c)}
                className="flex items-center gap-1.5 text-sm transition"
              >
                {c.enabled ? (
                  <ToggleRight className="w-8 h-8 text-emerald-400" />
                ) : (
                  <ToggleLeft className="w-8 h-8 text-slate-500" />
                )}
              </button>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-3 gap-4 mt-4">
              <div className="bg-slate-900/60 rounded-lg p-3">
                <div className="flex items-center gap-1.5 text-xs text-slate-400 mb-2">
                  <Settings className="w-3 h-3" />
                  Dimensions
                </div>
                <div className="flex flex-wrap gap-1">
                  {c.dimensions.map(d => (
                    <span key={d} className="px-2 py-0.5 text-[10px] bg-slate-700 text-slate-300 rounded">{d}</span>
                  ))}
                </div>
              </div>

              <div className="bg-slate-900/60 rounded-lg p-3">
                <div className="flex items-center gap-1.5 text-xs text-slate-400 mb-2">
                  <BarChart3 className="w-3 h-3" />
                  Metrics Generated
                </div>
                <div className="flex flex-wrap gap-1">
                  {c.metrics_generated.map(m => (
                    <span key={m} className="px-2 py-0.5 text-[10px] bg-cyan-900/40 text-cyan-300 rounded border border-cyan-800/40">{m}</span>
                  ))}
                </div>
              </div>

              <div className="bg-slate-900/60 rounded-lg p-3">
                <div className="flex items-center gap-1.5 text-xs text-slate-400 mb-2">
                  <Clock className="w-3 h-3" />
                  Aggregation Interval
                </div>
                <span className="text-sm text-white font-mono">{c.interval_seconds}s</span>
              </div>
            </div>
          </div>
        ))}

        {connectors.length === 0 && (
          <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-8 text-center">
            <Zap className="w-10 h-10 text-slate-600 mx-auto mb-3" />
            <h3 className="text-sm font-medium text-slate-400">No connectors configured</h3>
          </div>
        )}
      </div>

      {/* Derived metrics preview */}
      <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-5">
        <div className="flex items-center justify-between mb-4">
          <h3 className="text-sm font-semibold text-white flex items-center gap-2">
            <TrendingUp className="w-4 h-4 text-cyan-400" />
            Derived Metrics Preview
          </h3>
          <input
            value={selectedService} onChange={e => setSelectedService(e.target.value)}
            placeholder="Filter by service..."
            className="px-3 py-1.5 bg-slate-900 border border-slate-600 rounded-lg text-xs text-white placeholder-slate-500 focus:border-cyan-500 focus:outline-none w-48"
          />
        </div>

        {metricsLoading ? (
          <p className="text-xs text-slate-500 text-center py-6">Loading derived metrics...</p>
        ) : metrics.length === 0 ? (
          <div className="text-center py-6">
            <p className="text-xs text-slate-500">No derived metrics yet.</p>
            <p className="text-[10px] text-slate-600 mt-1">Metrics will appear after spans are ingested with the connector enabled.</p>
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-xs">
              <thead>
                <tr className="border-b border-slate-700 text-slate-400">
                  <th className="text-left px-3 py-2">Service</th>
                  <th className="text-left px-3 py-2">Metric</th>
                  <th className="text-right px-3 py-2">Value</th>
                  <th className="text-left px-3 py-2">Bucket</th>
                </tr>
              </thead>
              <tbody>
                {metrics.slice(0, 20).map((m, i) => (
                  <tr key={i} className="border-b border-slate-700/50 hover:bg-slate-700/20">
                    <td className="px-3 py-2 text-white">{m.service_name}</td>
                    <td className="px-3 py-2">
                      <span className="px-1.5 py-0.5 bg-cyan-900/30 text-cyan-300 rounded text-[10px]">{m.metric_name}</span>
                    </td>
                    <td className="px-3 py-2 text-right font-mono text-slate-300">{m.value.toFixed(2)}</td>
                    <td className="px-3 py-2 text-slate-500">{new Date(m.bucket_start).toLocaleString()}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* How it works diagram */}
      <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-5">
        <h3 className="text-sm font-semibold text-white mb-4">How Span-to-Metrics Works</h3>
        <div className="flex items-center justify-center gap-3 text-xs">
          <div className="bg-blue-900/30 border border-blue-700/40 rounded-lg px-4 py-3 text-center">
            <div className="text-blue-300 font-medium">Ingested Spans</div>
            <div className="text-[10px] text-slate-500 mt-1">OTLP /v1/traces</div>
          </div>
          <ArrowRight className="w-5 h-5 text-slate-600" />
          <div className="bg-emerald-900/30 border border-emerald-700/40 rounded-lg px-4 py-3 text-center">
            <div className="text-emerald-300 font-medium">Connector</div>
            <div className="text-[10px] text-slate-500 mt-1">Aggregate per minute</div>
          </div>
          <ArrowRight className="w-5 h-5 text-slate-600" />
          <div className="bg-amber-900/30 border border-amber-700/40 rounded-lg px-4 py-3 text-center">
            <div className="text-amber-300 font-medium">RED Metrics</div>
            <div className="text-[10px] text-slate-500 mt-1">Rate, Error, Duration</div>
          </div>
        </div>
      </div>
    </div>
  )
}
