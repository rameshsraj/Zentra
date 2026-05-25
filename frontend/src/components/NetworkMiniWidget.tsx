import { useState, useEffect } from 'react'
import DonutChart from '@/components/charts/DonutChart'
import Sparkline from '@/components/charts/Sparkline'
import { getNetworkStats, getNetworkServiceBreakdown, getNetworkHosts, type NetworkStats, type ServiceBreakdownEntry, type NetworkHost } from '@/lib/api'

const fmt = (bytes: number) => {
  if (bytes > 1e9) return `${(bytes / 1e9).toFixed(1)} GB`
  if (bytes > 1e6) return `${(bytes / 1e6).toFixed(1)} MB`
  if (bytes > 1e3) return `${(bytes / 1e3).toFixed(1)} KB`
  return `${bytes} B`
}

export default function NetworkMiniWidget() {
  const [stats, setStats] = useState<NetworkStats | null>(null)
  const [services, setServices] = useState<ServiceBreakdownEntry[]>([])
  const [hosts, setHosts] = useState<NetworkHost[]>([])
  const [sparkData, setSparkData] = useState<number[]>(Array.from({ length: 20 }, () => 50))

  useEffect(() => {
    const load = () => {
      getNetworkStats().then(setStats).catch(() => {})
      getNetworkServiceBreakdown().then(s => setServices(s.slice(0, 4))).catch(() => {})
      getNetworkHosts().then(h => setHosts(h.slice(0, 4))).catch(() => {})
      // Update sparkline from real bandwidth data if available
      setSparkData(prev => {
        const latest = stats ? (stats.bytes_in_per_sec ?? 0) / 10000 : prev[prev.length - 1] ?? 50
        return [...prev.slice(1), Math.min(100, Math.max(0, latest))]
      })
    }
    load()
    const interval = setInterval(load, 2000)
    return () => clearInterval(interval)
  }, [])

  const totalBytes = stats ? stats.total_bytes_in + stats.total_bytes_out : 0

  return (
    <div className="w-[400px] bg-slate-900/95 border border-white/10 rounded-xl p-4 shadow-2xl space-y-3">
      {/* Header */}
      <div className="flex items-center justify-between">
        <h3 className="text-xs font-bold text-white uppercase tracking-wider">Network Monitor</h3>
        <span className="text-[10px] text-emerald-400 flex items-center gap-1">
          <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-pulse" />
          Live
        </span>
      </div>

      <div className="grid grid-cols-2 gap-3">
        {/* Donut */}
        <div className="flex flex-col items-center">
          <DonutChart
            size={100}
            thickness={12}
            centerValue={fmt(totalBytes)}
            centerLabel="Total"
            segments={[
              { label: 'In', value: stats?.total_bytes_in ?? 60, color: '#3b82f6' },
              { label: 'Out', value: stats?.total_bytes_out ?? 40, color: '#f59e0b' },
            ]}
          />
          <div className="flex gap-3 mt-1 text-[9px]">
            <span className="text-blue-400">▼ {stats ? fmt(stats.total_bytes_in) : '—'}</span>
            <span className="text-amber-400">▲ {stats ? fmt(stats.total_bytes_out) : '—'}</span>
          </div>
        </div>

        {/* Sparkline + KPIs */}
        <div className="space-y-2">
          <Sparkline data={sparkData} color="#10b981" height={40} />
          <div className="grid grid-cols-2 gap-1 text-[9px]">
            <div className="bg-white/5 rounded px-2 py-1">
              <span className="text-slate-500">Conns</span>
              <span className="block font-mono text-white">{stats?.active_connections ?? '—'}</span>
            </div>
            <div className="bg-white/5 rounded px-2 py-1">
              <span className="text-slate-500">Hosts</span>
              <span className="block font-mono text-white">{stats?.unique_hosts ?? '—'}</span>
            </div>
            <div className="bg-white/5 rounded px-2 py-1">
              <span className="text-slate-500">Pkts</span>
              <span className="block font-mono text-white">{stats ? ((stats.total_packets_in + stats.total_packets_out) / 1000).toFixed(0) + 'K' : '—'}</span>
            </div>
            <div className="bg-white/5 rounded px-2 py-1">
              <span className="text-slate-500">Dropped</span>
              <span className="block font-mono text-white">{stats?.dropped_packets ?? '—'}</span>
            </div>
          </div>
        </div>
      </div>

      {/* Top Hosts */}
      <div>
        <h4 className="text-[9px] font-semibold text-slate-500 uppercase tracking-wider mb-1">Top Hosts</h4>
        <div className="space-y-0.5">
          {hosts.map(h => (
            <div key={h.ip} className="flex items-center gap-2 text-[10px]">
              <span className="text-slate-500 w-6">{h.country}</span>
              <span className="text-slate-300 flex-1 truncate">{h.domain || h.ip}</span>
              <span className="font-mono text-blue-400">{fmt(h.bytes_in + h.bytes_out)}</span>
            </div>
          ))}
        </div>
      </div>

      {/* Top Services */}
      <div>
        <h4 className="text-[9px] font-semibold text-slate-500 uppercase tracking-wider mb-1">Top Services</h4>
        <div className="space-y-0.5">
          {services.map(s => (
            <div key={s.service} className="flex items-center gap-2 text-[10px]">
              <span className="text-slate-300 flex-1">{s.service}</span>
              <span className="font-mono text-emerald-400">{fmt(s.bytes)}</span>
              <span className="font-mono text-slate-500">{s.percentage.toFixed(0)}%</span>
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}
