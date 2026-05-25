import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { fetchJSON } from '@/lib/api/_client'
import PageHeader from '@/components/PageHeader'
import { cn } from '@/lib/utils'
import {
  Activity, ScrollText, Cpu, BarChart3, Server,
  AlertTriangle, Clock,
} from 'lucide-react'

interface OtelStats {
  spans_per_minute: number
  metrics_per_minute: number
  logs_per_minute: number
  profiles_per_minute: number
  unique_services: number
  error_rate_percent: number
  avg_span_duration_ms: number
}

interface TimeseriesPoint {
  timestamp: string
  count: number
}

interface TimeseriesResponse {
  signal: string
  series: TimeseriesPoint[]
}

const timeRanges = [
  { label: '1h', value: '1h' },
  { label: '6h', value: '6h' },
  { label: '24h', value: '24h' },
  { label: '7d', value: '7d' },
]

export default function OTelDashboard() {
  const [window, setWindow] = useState('24h')

  const { data: stats } = useQuery({
    queryKey: ['otel-stats'],
    queryFn: () => fetchJSON<OtelStats>('/otel/stats'),
    refetchInterval: 15000,
  })

  const { data: traceTs } = useQuery({
    queryKey: ['otel-ts-traces', window],
    queryFn: () => fetchJSON<TimeseriesResponse>(`/otel/stats/timeseries?signal=traces&window=${window}&step=5m`),
    refetchInterval: 30000,
  })

  const { data: metricTs } = useQuery({
    queryKey: ['otel-ts-metrics', window],
    queryFn: () => fetchJSON<TimeseriesResponse>(`/otel/stats/timeseries?signal=metrics&window=${window}&step=5m`),
    refetchInterval: 30000,
  })

  const { data: logTs } = useQuery({
    queryKey: ['otel-ts-logs', window],
    queryFn: () => fetchJSON<TimeseriesResponse>(`/otel/stats/timeseries?signal=logs&window=${window}&step=5m`),
    refetchInterval: 30000,
  })

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <PageHeader
          title="Telemetry Health"
          subtitle="Real-time visibility into the OpenTelemetry data pipeline"
        />
        <div className="flex gap-1 bg-slate-800 border border-slate-700 rounded-lg p-0.5">
          {timeRanges.map(tr => (
            <button
              key={tr.value}
              onClick={() => setWindow(tr.value)}
              className={cn(
                'px-3 py-1.5 text-xs rounded-md font-medium transition',
                window === tr.value ? 'bg-cyan-600 text-white' : 'text-slate-400 hover:text-white'
              )}
            >
              {tr.label}
            </button>
          ))}
        </div>
      </div>

      {/* Top stat cards */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
        <StatCard icon={Activity} label="Spans / min" value={stats?.spans_per_minute ?? 0} color="blue" />
        <StatCard icon={Cpu} label="Metrics / min" value={stats?.metrics_per_minute ?? 0} color="emerald" />
        <StatCard icon={ScrollText} label="Logs / min" value={stats?.logs_per_minute ?? 0} color="amber" />
        <StatCard icon={BarChart3} label="Profiles / min" value={stats?.profiles_per_minute ?? 0} color="purple" />
      </div>

      {/* Second row: services, error rate, avg latency */}
      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-5">
          <div className="flex items-center gap-2 mb-2">
            <Server className="w-4 h-4 text-cyan-400" />
            <span className="text-xs text-slate-400">Unique Services</span>
          </div>
          <div className="text-3xl font-bold text-white">{stats?.unique_services ?? 0}</div>
          <p className="text-[10px] text-slate-500 mt-1">Last 24 hours</p>
        </div>

        <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-5">
          <div className="flex items-center gap-2 mb-2">
            <AlertTriangle className={cn('w-4 h-4', (stats?.error_rate_percent ?? 0) > 5 ? 'text-red-400' : 'text-emerald-400')} />
            <span className="text-xs text-slate-400">Error Rate</span>
          </div>
          <div className="flex items-end gap-2">
            <span className={cn('text-3xl font-bold', (stats?.error_rate_percent ?? 0) > 5 ? 'text-red-400' : 'text-emerald-400')}>
              {(stats?.error_rate_percent ?? 0).toFixed(2)}%
            </span>
          </div>
          <ErrorGauge percent={stats?.error_rate_percent ?? 0} />
        </div>

        <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-5">
          <div className="flex items-center gap-2 mb-2">
            <Clock className="w-4 h-4 text-amber-400" />
            <span className="text-xs text-slate-400">Avg Span Duration</span>
          </div>
          <div className="text-3xl font-bold text-white">
            {(stats?.avg_span_duration_ms ?? 0).toFixed(1)}<span className="text-sm text-slate-400 ml-1">ms</span>
          </div>
          <p className="text-[10px] text-slate-500 mt-1">Last 5 minutes</p>
        </div>
      </div>

      {/* Signal volume charts */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        <VolumeChart title="Trace Volume" color="blue" icon={Activity} series={traceTs?.series ?? []} />
        <VolumeChart title="Metric Volume" color="emerald" icon={Cpu} series={metricTs?.series ?? []} />
        <VolumeChart title="Log Volume" color="amber" icon={ScrollText} series={logTs?.series ?? []} />
      </div>
    </div>
  )
}

function StatCard({ icon: Icon, label, value, color }: {
  icon: React.ElementType; label: string; value: number; color: string
}) {
  return (
    <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-4">
      <div className="flex items-center gap-2 mb-2">
        <Icon className={cn('w-4 h-4', `text-${color}-400`)} />
        <span className="text-xs text-slate-400">{label}</span>
      </div>
      <div className="text-2xl font-bold text-white">{value.toLocaleString()}</div>
    </div>
  )
}

function ErrorGauge({ percent }: { percent: number }) {
  const clamped = Math.min(100, Math.max(0, percent))
  const color = clamped > 10 ? 'bg-red-500' : clamped > 5 ? 'bg-amber-500' : 'bg-emerald-500'
  return (
    <div className="mt-2 h-1.5 bg-slate-700 rounded-full overflow-hidden">
      <div className={cn('h-full rounded-full transition-all', color)} style={{ width: `${clamped}%` }} />
    </div>
  )
}

function VolumeChart({ title, color, icon: Icon, series }: {
  title: string; color: string; icon: React.ElementType; series: TimeseriesPoint[]
}) {
  const maxVal = Math.max(1, ...series.map(s => s.count))

  return (
    <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-5">
      <div className="flex items-center gap-2 mb-4">
        <Icon className={cn('w-4 h-4', `text-${color}-400`)} />
        <span className="text-sm font-medium text-white">{title}</span>
        <span className="ml-auto text-xs text-slate-500">{series.length} buckets</span>
      </div>
      {series.length === 0 ? (
        <p className="text-xs text-slate-500 text-center py-8">No data in this window</p>
      ) : (
        <div className="flex items-end gap-px h-24">
          {series.map((p, i) => (
            <div
              key={i}
              className={cn('flex-1 rounded-t transition-all', `bg-${color}-500/60 hover:bg-${color}-400/80`)}
              style={{ height: `${(p.count / maxVal) * 100}%`, minHeight: '2px' }}
              title={`${new Date(p.timestamp).toLocaleTimeString()}: ${p.count.toLocaleString()}`}
            />
          ))}
        </div>
      )}
    </div>
  )
}
