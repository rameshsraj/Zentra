import { useCallback } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { getIdsStats, getConnectionStats, getDnsStats } from '../../lib/api'
import { Shield, RefreshCw } from 'lucide-react'
import Sparkline from '@/components/charts/Sparkline'

// M10: Flat sparkline from live value — no synthetic Math.sin generation
const spark = (base: number, _v?: number, len = 20) =>
  Array.from({ length: len }, () => Math.max(0, base))

export default function SecurityDashboard() {
  const qc = useQueryClient()
  const { data: idsStats } = useQuery({ queryKey: ['sec-dash-ids'], queryFn: getIdsStats })
  const { data: connStats } = useQuery({ queryKey: ['sec-dash-conn'], queryFn: getConnectionStats })
  const { data: dnsStats } = useQuery({ queryKey: ['sec-dash-dns'], queryFn: getDnsStats })
  const refresh = useCallback(() => {
    qc.invalidateQueries({ queryKey: ['sec-dash-ids'] })
    qc.invalidateQueries({ queryKey: ['sec-dash-conn'] })
    qc.invalidateQueries({ queryKey: ['sec-dash-dns'] })
  }, [qc])

  const kpis = [
    { label: 'IDS Rules', value: idsStats?.total_rules || 0, color: 'text-blue-400', sub: `${idsStats?.enabled_rules || 0} enabled` },
    { label: 'Alerts (24h)', value: idsStats?.alerts_24h || 0, color: 'text-red-400', sub: 'IDS alerts' },
    { label: 'Connections', value: connStats?.total_connections || 0, color: 'text-cyan-400', sub: 'Total logged' },
    { label: 'DNS Queries', value: dnsStats?.total_queries || 0, color: 'text-green-400', sub: `${dnsStats?.nxdomain_count || 0} NXDOMAIN` },
  ]

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="p-2 rounded-lg bg-red-600/20"><Shield className="w-5 h-5 text-red-400" /></div>
          <div><h1 className="text-2xl font-bold text-white">Security Dashboard</h1><p className="text-sm text-gray-400">Unified security overview</p></div>
        </div>
        <button onClick={refresh} className="p-2 rounded-lg hover:bg-gray-700 text-gray-400"><RefreshCw className="w-4 h-4" /></button>
      </div>

      <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
        {kpis.map((kpi, i) => (
          <div key={i} className="bg-gray-800/50 border border-gray-700/50 rounded-xl p-4">
            <p className="text-xs text-gray-500 uppercase">{kpi.label}</p>
            <div className="flex items-center justify-between">
              <p className={`text-2xl font-bold font-mono ${kpi.color}`}>{kpi.value.toLocaleString()}</p>
              <Sparkline data={spark(kpi.value, kpi.value * 0.1)} width={48} height={16} color={kpi.color.replace('text-', '')} />
            </div>
            <p className="text-xs text-gray-500 mt-1">{kpi.sub}</p>
          </div>
        ))}
      </div>

      {/* Severity breakdown */}
      {idsStats?.severity_distribution && (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div className="bg-gray-800/50 border border-gray-700/50 rounded-xl p-4">
            <h3 className="text-sm font-semibold text-white mb-3">Alert Severity (24h)</h3>
            <div className="space-y-2">
              {idsStats.severity_distribution.map((s: any) => {
                const pct = idsStats.alerts_24h ? (s.count / idsStats.alerts_24h) * 100 : 0
                const color = s.severity === 'critical' ? 'bg-red-500' : s.severity === 'high' ? 'bg-orange-500' : s.severity === 'medium' ? 'bg-yellow-500' : 'bg-blue-500'
                return (
                  <div key={s.severity} className="flex items-center gap-2 text-xs">
                    <span className="w-16 text-gray-400 capitalize">{s.severity}</span>
                    <div className="flex-1 bg-gray-700/50 rounded-full h-2"><div className={`${color} h-2 rounded-full`} style={{ width: `${pct}%` }} /></div>
                    <span className="w-10 text-right text-gray-300 font-mono">{s.count}</span>
                  </div>
                )
              })}
            </div>
          </div>
          <div className="bg-gray-800/50 border border-gray-700/50 rounded-xl p-4">
            <h3 className="text-sm font-semibold text-white mb-3">Top Sources</h3>
            <div className="space-y-1.5">
              {(idsStats.top_sources || []).slice(0, 6).map((s: any) => (
                <div key={s.ip} className="flex justify-between text-xs bg-gray-700/20 rounded px-3 py-1.5">
                  <span className="font-mono text-gray-300">{s.ip}</span><span className="text-red-400 font-mono">{s.count}</span>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {/* Top talkers + DNS */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
        <div className="bg-gray-800/50 border border-gray-700/50 rounded-xl p-4">
          <h3 className="text-sm font-semibold text-white mb-3">Top Talkers (by bytes)</h3>
          <div className="space-y-1.5">
            {(connStats?.top_talkers || []).slice(0, 6).map((t: any) => (
              <div key={t.ip} className="flex justify-between text-xs bg-gray-700/20 rounded px-3 py-1.5">
                <span className="font-mono text-gray-300">{t.ip}</span><span className="text-cyan-400 font-mono">{(t.bytes || 0).toLocaleString()} B</span>
              </div>
            ))}
          </div>
        </div>
        <div className="bg-gray-800/50 border border-gray-700/50 rounded-xl p-4">
          <h3 className="text-sm font-semibold text-white mb-3">Top DNS Domains</h3>
          <div className="space-y-1.5">
            {(dnsStats?.top_domains || []).slice(0, 6).map((d: any) => (
              <div key={d.domain} className="flex justify-between text-xs bg-gray-700/20 rounded px-3 py-1.5">
                <span className="font-mono text-gray-300">{d.domain}</span><span className="text-green-400 font-mono">{d.count}</span>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  )
}
