import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { HardDrive, Activity, Zap, Clock, AlertTriangle, TrendingUp, BarChart3 } from 'lucide-react'
import PageHeader from '@/components/PageHeader'

// ============================================================================
// API
// ============================================================================

const API = '/api'

interface ArraySummary {
  id: string
  name: string
  array_type: string
  status: string
  total_tb: number
  used_tb: number
  utilization_pct: number
  iops: number
  throughput_mb: number
  latency_ms: number
}

interface MetricPoint {
  timestamp: string
  value: number
}

interface MetricsHistory {
  iops: MetricPoint[]
  latency_ms: MetricPoint[]
  throughput_mb: MetricPoint[]
  utilization_pct: MetricPoint[]
}

async function fetchArrays(): Promise<ArraySummary[]> {
  const r = await fetch(`${API}/storage-arrays`, { credentials: 'include' })
  if (!r.ok) return []
  const j = await r.json()
  return j.arrays ?? j ?? []
}

async function fetchMetricsHistory(id: string): Promise<MetricsHistory> {
  const r = await fetch(`${API}/storage-arrays/${id}/metrics/history`, { credentials: 'include' })
  if (!r.ok) return { iops: [], latency_ms: [], throughput_mb: [], utilization_pct: [] }
  return r.json()
}

async function fetchLatestMetrics(id: string): Promise<Record<string, number>> {
  const r = await fetch(`${API}/storage-arrays/${id}/metrics/latest`, { credentials: 'include' })
  if (!r.ok) return {}
  return r.json()
}

// ============================================================================
// Sparkline component (simple SVG)
// ============================================================================

function Sparkline({ data, color = '#3b82f6', height = 40, width = 200 }: {
  data: number[]
  color?: string
  height?: number
  width?: number
}) {
  if (data.length < 2) return <div className="text-xs text-gray-600">No data</div>

  const min = Math.min(...data)
  const max = Math.max(...data)
  const range = max - min || 1

  const points = data.map((v, i) => {
    const x = (i / (data.length - 1)) * width
    const y = height - ((v - min) / range) * (height - 4) - 2
    return `${x},${y}`
  }).join(' ')

  return (
    <svg width={width} height={height} className="block">
      <polyline
        points={points}
        fill="none"
        stroke={color}
        strokeWidth="1.5"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  )
}

// ============================================================================
// Metric Card with sparkline
// ============================================================================

function MetricCard({ label, value, unit, trend, color, icon: Icon }: {
  label: string
  value: string
  unit: string
  trend: number[]
  color: string
  icon: React.ElementType
}) {
  return (
    <div className="bg-gray-900 border border-gray-800 rounded-lg p-4">
      <div className="flex items-center justify-between mb-2">
        <div className="flex items-center gap-2 text-xs text-gray-500">
          <Icon className={`w-3.5 h-3.5 ${color}`} />
          {label}
        </div>
        <span className="text-xs text-gray-600">{unit}</span>
      </div>
      <div className="text-2xl font-bold text-white mb-2">{value}</div>
      <Sparkline data={trend} color={color.includes('blue') ? '#3b82f6' : color.includes('green') ? '#22c55e' : color.includes('yellow') ? '#eab308' : color.includes('purple') ? '#a855f7' : '#3b82f6'} width={180} />
    </div>
  )
}

// ============================================================================
// Heatmap row for utilization
// ============================================================================

function UtilizationHeatmap({ arrays }: { arrays: ArraySummary[] }) {
  return (
    <div className="bg-gray-900 border border-gray-800 rounded-lg p-4">
      <div className="flex items-center gap-2 text-sm text-gray-300 mb-3">
        <BarChart3 className="w-4 h-4 text-blue-400" />
        Capacity Utilization Heatmap
      </div>
      <div className="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-2">
        {arrays.map((a) => {
          const pct = a.utilization_pct
          const bg = pct >= 90 ? 'bg-red-900/60 border-red-700'
            : pct >= 80 ? 'bg-yellow-900/40 border-yellow-700'
            : pct >= 60 ? 'bg-blue-900/40 border-blue-700'
            : 'bg-green-900/30 border-green-700'
          const textColor = pct >= 90 ? 'text-red-300' : pct >= 80 ? 'text-yellow-300' : 'text-gray-300'
          return (
            <div key={a.id} className={`border rounded p-3 ${bg}`}>
              <div className="text-xs text-gray-400 truncate" title={a.name}>{a.name}</div>
              <div className={`text-lg font-bold ${textColor}`}>{pct.toFixed(0)}%</div>
              <div className="text-xs text-gray-500">{a.used_tb.toFixed(1)} / {a.total_tb.toFixed(0)} TB</div>
            </div>
          )
        })}
      </div>
      <div className="flex items-center gap-4 mt-3 text-xs text-gray-500">
        <span className="flex items-center gap-1"><span className="w-3 h-3 rounded bg-green-900/60 border border-green-700" /> &lt;60%</span>
        <span className="flex items-center gap-1"><span className="w-3 h-3 rounded bg-blue-900/60 border border-blue-700" /> 60-80%</span>
        <span className="flex items-center gap-1"><span className="w-3 h-3 rounded bg-yellow-900/60 border border-yellow-700" /> 80-90%</span>
        <span className="flex items-center gap-1"><span className="w-3 h-3 rounded bg-red-900/60 border border-red-700" /> &gt;90%</span>
      </div>
    </div>
  )
}

// ============================================================================
// Per-array deep-dive panel
// ============================================================================

function ArrayDeepDive({ array }: { array: ArraySummary }) {
  const { data: history } = useQuery({
    queryKey: ['storage-history', array.id],
    queryFn: () => fetchMetricsHistory(array.id),
    refetchInterval: 60000,
  })

  const { data: latest = {} } = useQuery({
    queryKey: ['storage-latest', array.id],
    queryFn: () => fetchLatestMetrics(array.id),
    refetchInterval: 30000,
  })

  const iopsData = history?.iops?.map(p => p.value) ?? []
  const latencyData = history?.latency_ms?.map(p => p.value) ?? []
  const throughputData = history?.throughput_mb?.map(p => p.value) ?? []
  const utilData = history?.utilization_pct?.map(p => p.value) ?? []

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-3">
        <span className={`w-2.5 h-2.5 rounded-full ${array.status === 'healthy' ? 'bg-green-400' : array.status === 'warning' ? 'bg-yellow-400' : 'bg-red-400'}`} />
        <h3 className="text-lg font-semibold text-white">{array.name}</h3>
        <span className="text-xs text-gray-500 bg-gray-800 px-2 py-0.5 rounded">{array.array_type}</span>
      </div>

      <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
        <MetricCard
          label="IOPS" value={array.iops.toLocaleString()} unit="ops/sec"
          trend={iopsData} color="text-blue-400" icon={Zap}
        />
        <MetricCard
          label="Latency" value={`${array.latency_ms.toFixed(1)}`} unit="ms"
          trend={latencyData} color="text-yellow-400" icon={Clock}
        />
        <MetricCard
          label="Throughput" value={`${array.throughput_mb.toFixed(0)}`} unit="MB/s"
          trend={throughputData} color="text-green-400" icon={TrendingUp}
        />
        <MetricCard
          label="Utilization" value={`${array.utilization_pct.toFixed(0)}%`} unit="capacity"
          trend={utilData} color="text-purple-400" icon={HardDrive}
        />
      </div>

      {/* Latest raw metrics table */}
      {Object.keys(latest).length > 0 && (
        <div className="bg-gray-900 border border-gray-800 rounded-lg p-4">
          <div className="text-xs text-gray-400 mb-2">Latest Metrics</div>
          <div className="grid grid-cols-2 md:grid-cols-4 gap-2 text-xs">
            {Object.entries(latest).map(([key, val]) => (
              <div key={key} className="bg-gray-800 rounded px-3 py-2">
                <div className="text-gray-500 truncate" title={key}>{key}</div>
                <div className="text-white font-mono">{typeof val === 'number' ? val.toLocaleString(undefined, { maximumFractionDigits: 2 }) : String(val)}</div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  )
}

// ============================================================================
// Page
// ============================================================================

export default function StorageDeepDive() {
  const [selectedId, setSelectedId] = useState<string | null>(null)

  const { data: arrays = [] } = useQuery({
    queryKey: ['storage-arrays'],
    queryFn: fetchArrays,
    refetchInterval: 30000,
  })

  const selected = arrays.find(a => a.id === selectedId)

  // Aggregate stats
  const totalTb = arrays.reduce((s, a) => s + a.total_tb, 0)
  const usedTb = arrays.reduce((s, a) => s + a.used_tb, 0)
  const totalIops = arrays.reduce((s, a) => s + a.iops, 0)
  const avgLatency = arrays.length > 0 ? arrays.reduce((s, a) => s + a.latency_ms, 0) / arrays.length : 0
  const critical = arrays.filter(a => a.utilization_pct >= 90).length
  const warnings = arrays.filter(a => a.utilization_pct >= 80 && a.utilization_pct < 90).length

  return (
    <div className="p-6 space-y-6">
      <PageHeader title="Storage Deep-Dive" subtitle="Performance analytics & capacity planning across all storage arrays" />

      {/* Aggregate KPIs */}
      <div className="grid grid-cols-2 md:grid-cols-6 gap-4">
        {[
          { label: 'Arrays', value: arrays.length, color: 'text-blue-400', icon: HardDrive },
          { label: 'Total Capacity', value: `${(totalTb / 1024).toFixed(1)} PB`, color: 'text-gray-300', icon: HardDrive },
          { label: 'Used', value: `${(usedTb / 1024).toFixed(1)} PB`, color: 'text-blue-400', icon: Activity },
          { label: 'Aggregate IOPS', value: totalIops.toLocaleString(), color: 'text-green-400', icon: Zap },
          { label: 'Avg Latency', value: `${avgLatency.toFixed(1)} ms`, color: 'text-yellow-400', icon: Clock },
          { label: 'Alerts', value: `${critical} crit / ${warnings} warn`, color: critical > 0 ? 'text-red-400' : 'text-gray-400', icon: AlertTriangle },
        ].map(({ label, value, color, icon: Icon }) => (
          <div key={label} className="bg-gray-900 border border-gray-800 rounded-lg p-3">
            <div className="flex items-center gap-1.5 text-xs text-gray-500 mb-1">
              <Icon className={`w-3 h-3 ${color}`} /> {label}
            </div>
            <div className={`text-base font-bold ${color}`}>{value}</div>
          </div>
        ))}
      </div>

      {/* Utilization heatmap */}
      {arrays.length > 0 && <UtilizationHeatmap arrays={arrays} />}

      {/* Array selector + deep-dive */}
      <div className="flex gap-6">
        <div className="w-64 shrink-0 space-y-1">
          <div className="text-xs text-gray-500 mb-2 font-medium">Select Array</div>
          {arrays.map(a => (
            <button
              key={a.id}
              onClick={() => setSelectedId(a.id)}
              className={`w-full text-left px-3 py-2 rounded text-sm transition-colors ${
                selectedId === a.id
                  ? 'bg-blue-600/20 border border-blue-600 text-white'
                  : 'bg-gray-900 border border-gray-800 text-gray-400 hover:text-white hover:bg-gray-800'
              }`}
            >
              <div className="flex items-center gap-2">
                <span className={`w-2 h-2 rounded-full ${a.status === 'healthy' ? 'bg-green-400' : a.status === 'warning' ? 'bg-yellow-400' : 'bg-red-400'}`} />
                <span className="truncate">{a.name}</span>
              </div>
              <div className="text-xs text-gray-600 mt-0.5 pl-4">{a.utilization_pct.toFixed(0)}% · {a.iops.toLocaleString()} IOPS</div>
            </button>
          ))}
        </div>

        <div className="flex-1 min-w-0">
          {selected ? (
            <ArrayDeepDive array={selected} />
          ) : (
            <div className="flex items-center justify-center h-64 text-gray-500 text-sm">
              <div className="text-center">
                <HardDrive className="w-10 h-10 mx-auto mb-3 opacity-20" />
                Select an array to view detailed performance metrics
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
