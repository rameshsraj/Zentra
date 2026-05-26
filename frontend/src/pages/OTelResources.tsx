import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { fetchJSON } from '@/lib/api/_client'
import PageHeader from '@/components/PageHeader'
import { cn } from '@/lib/utils'
import {
  Server, Search, Activity, ScrollText, Cpu,
  Clock, ExternalLink,
} from 'lucide-react'

interface ResourceEntry {
  service_name: string
  last_seen: string | null
  span_count: number
  version?: string
  sdk_language?: string
  sdk_version?: string
  environment?: string
  signals?: { traces: boolean; metrics: boolean; logs: boolean; profiles: boolean }
}

interface ResourcesResponse {
  resources: ResourceEntry[]
  count: number
}

const getResources = () => fetchJSON<ResourcesResponse>('/otel/resources')

export default function OTelResources() {
  const [filter, setFilter] = useState('')
  const { data, isLoading } = useQuery({ queryKey: ['otel-resources'], queryFn: getResources, refetchInterval: 30000 })

  const resources = data?.resources ?? []
  const filtered = resources.filter(r =>
    r.service_name.toLowerCase().includes(filter.toLowerCase())
  )

  const totalServices = resources.length
  const withTraces = resources.filter(r => r.signals?.traces).length
  const withMetrics = resources.filter(r => r.signals?.metrics).length
  const withLogs = resources.filter(r => r.signals?.logs).length

  return (
    <div className="space-y-6">
      <PageHeader
        title="OTel Resources"
        subtitle="Discovered services sending OpenTelemetry data — verify instrumentation is working"
      />

      {/* Summary cards */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
        <SummaryCard icon={Server} label="Total Services" value={totalServices} color="cyan" />
        <SummaryCard icon={Activity} label="With Traces" value={withTraces} color="blue" />
        <SummaryCard icon={Cpu} label="With Metrics" value={withMetrics} color="emerald" />
        <SummaryCard icon={ScrollText} label="With Logs" value={withLogs} color="amber" />
      </div>

      {/* Search */}
      <div className="relative">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-500" />
        <input
          value={filter} onChange={e => setFilter(e.target.value)}
          placeholder="Filter services..."
          className="w-full pl-10 pr-4 py-2.5 bg-slate-800/60 border border-slate-700 rounded-xl text-sm text-white placeholder-slate-500 focus:border-cyan-500 focus:outline-none"
        />
      </div>

      {/* Table */}
      <div className="bg-slate-800/60 border border-slate-700 rounded-xl overflow-hidden">
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-slate-700 text-xs text-slate-400 uppercase tracking-wider">
                <th className="text-left px-4 py-3">Service Name</th>
                <th className="text-left px-4 py-3">Signals</th>
                <th className="text-right px-4 py-3">Span Count</th>
                <th className="text-left px-4 py-3">Last Seen</th>
                <th className="text-center px-4 py-3">Actions</th>
              </tr>
            </thead>
            <tbody>
              {isLoading ? (
                <tr><td colSpan={5} className="px-4 py-8 text-center text-slate-500">Loading resources...</td></tr>
              ) : filtered.length === 0 ? (
                <tr><td colSpan={5} className="px-4 py-8 text-center text-slate-500">
                  {filter ? 'No services match your filter' : 'No services discovered yet — send OTLP data to get started'}
                </td></tr>
              ) : (
                filtered.map(r => (
                  <tr key={r.service_name} className="border-b border-slate-700/50 hover:bg-slate-700/20 transition">
                    <td className="px-4 py-3">
                      <div className="flex items-center gap-2">
                        <Server className="w-4 h-4 text-cyan-400 flex-shrink-0" />
                        <span className="text-white font-medium">{r.service_name}</span>
                      </div>
                      {r.environment && (
                        <span className="text-[10px] text-slate-500 ml-6">{r.environment}</span>
                      )}
                    </td>
                    <td className="px-4 py-3">
                      <div className="flex gap-1.5">
                        <SignalBadge label="T" active={r.span_count > 0} color="blue" title="Traces" />
                        <SignalBadge label="M" active={!!r.signals?.metrics} color="emerald" title="Metrics" />
                        <SignalBadge label="L" active={!!r.signals?.logs} color="amber" title="Logs" />
                        <SignalBadge label="P" active={!!r.signals?.profiles} color="purple" title="Profiles" />
                      </div>
                    </td>
                    <td className="px-4 py-3 text-right font-mono text-slate-300">
                      {r.span_count.toLocaleString()}
                    </td>
                    <td className="px-4 py-3 text-xs text-slate-400">
                      <div className="flex items-center gap-1">
                        <Clock className="w-3 h-3" />
                        {r.last_seen ? new Date(r.last_seen).toLocaleString() : 'N/A'}
                      </div>
                    </td>
                    <td className="px-4 py-3 text-center">
                      <a
                        href={`/log-explorer?service=${encodeURIComponent(r.service_name)}`}
                        className="text-cyan-400 hover:text-cyan-300 transition"
                        title="View traces"
                      >
                        <ExternalLink className="w-4 h-4 inline" />
                      </a>
                    </td>
                  </tr>
                ))
              )}
            </tbody>
          </table>
        </div>
      </div>
    </div>
  )
}

function SummaryCard({ icon: Icon, label, value, color }: {
  icon: React.ElementType; label: string; value: number; color: string
}) {
  return (
    <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-4">
      <div className="flex items-center gap-2 mb-2">
        <Icon className={cn('w-4 h-4', `text-${color}-400`)} />
        <span className="text-xs text-slate-400">{label}</span>
      </div>
      <div className="text-2xl font-bold text-white">{value}</div>
    </div>
  )
}

function SignalBadge({ label, active, color, title }: {
  label: string; active: boolean; color: string; title: string
}) {
  return (
    <span
      title={title}
      className={cn(
        'inline-flex items-center justify-center w-6 h-6 rounded text-[10px] font-bold',
        active ? `bg-${color}-600/30 text-${color}-300 border border-${color}-600/40` : 'bg-slate-700/50 text-slate-600 border border-slate-700'
      )}
    >
      {label}
    </span>
  )
}
