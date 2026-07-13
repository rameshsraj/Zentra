import { useCallback } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { getTargets, getTargetStats, getNetworkCapture, type ServerStats } from '@/lib/api'
import GaugeChart from '@/components/charts/GaugeChart'
import PageHeader from '@/components/PageHeader'
import AddTargetModal from '@/components/AddTargetModal'
import { Server, HardDrive, Network, Cpu, MemoryStick, Loader2, Wifi } from 'lucide-react'
import { useState, useEffect, useMemo } from 'react'
import { Link } from 'react-router-dom'

function formatGb(value: unknown) {
  const n = typeof value === 'number' ? value : 0
  return `${(Number.isFinite(n) ? n / 1e9 : 0).toFixed(1)} GB`
}

function formatPackets(value: unknown) {
  const n = typeof value === 'number' ? value : 0
  return `${(Number.isFinite(n) ? n / 1000 : 0).toFixed(0)}K`
}

export default function InfrastructureDashboardPage() {
  const qc = useQueryClient()
  const { data: targets } = useQuery({ queryKey: ['targets'], queryFn: getTargets })
  const { data: netCapture } = useQuery({ queryKey: ['net-capture'], queryFn: getNetworkCapture })

  const serverTargets = useMemo(() => (targets ?? []).filter(t => t.target_type === 'server'), [targets])
  const storageTargets = useMemo(() => (targets ?? []).filter(t => t.target_type === 'storage'), [targets])

  const [serverStats, setServerStats] = useState<Record<string, ServerStats>>({})
  const [showAddModal, setShowAddModal] = useState(false)

  useEffect(() => {
    serverTargets.forEach(t => {
      getTargetStats(t.target_id).then(stats => {
        setServerStats(prev => ({ ...prev, [t.target_id]: stats }))
      }).catch(() => {})
    })
  }, [serverTargets])

  const handleRefresh = useCallback(() => {
    qc.invalidateQueries({ queryKey: ['targets'] })
    qc.invalidateQueries({ queryKey: ['net-capture'] })
    serverTargets.forEach(t => {
      getTargetStats(t.target_id).then(stats => {
        setServerStats(prev => ({ ...prev, [t.target_id]: stats }))
      }).catch(() => {})
    })
  }, [qc, serverTargets])

  const statsValues = Object.values(serverStats)
  const totalServers = serverTargets.length
  const reachableServers = statsValues.length
  const avgCpu = statsValues.length > 0
    ? Math.round(statsValues.reduce((a, s) => a + s.cpu_usage_percent, 0) / statsValues.length)
    : 0
  const avgMem = statsValues.length > 0
    ? Math.round(statsValues.reduce((a, s) => a + s.memory_usage_percent, 0) / statsValues.length)
    : 0
  const avgDisk = statsValues.length > 0
    ? Math.round(statsValues.reduce((a, s) => a + s.disk_usage_percent, 0) / statsValues.length)
    : 0

  return (
    <div className="space-y-6">
      <PageHeader
        title="Infrastructure Dashboard"
        subtitle={`${totalServers} servers \u00b7 ${storageTargets.length} storage targets \u00b7 Network monitoring active`}
        onRefresh={handleRefresh}
        showAddTarget
        onAddTarget={() => setShowAddModal(true)}
        addTargetLabel="Add Server"
      />
      <AddTargetModal open={showAddModal} onClose={() => setShowAddModal(false)} defaultType="server" title="Add Server" />

      <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-3">
        {[
          { label: 'Total Servers', value: String(totalServers), icon: Server, color: 'text-blue-400' },
          { label: 'Reachable', value: String(reachableServers), icon: Wifi, color: 'text-emerald-400' },
          { label: 'Storage Targets', value: String(storageTargets.length), icon: HardDrive, color: 'text-purple-400' },
          { label: 'Avg CPU', value: `${avgCpu}%`, icon: Cpu, color: 'text-cyan-400' },
          { label: 'Avg Memory', value: `${avgMem}%`, icon: MemoryStick, color: 'text-amber-400' },
          { label: 'Avg Disk', value: `${avgDisk}%`, icon: HardDrive, color: 'text-rose-400' },
        ].map((kpi, i) => (
          <div key={i} className="bg-white/5 backdrop-blur-sm border border-white/10 p-3 rounded-xl">
            <div className="flex items-center gap-1.5 mb-1">
              <kpi.icon className={`w-3 h-3 ${kpi.color}`} />
              <span className="text-[10px] text-slate-400 uppercase tracking-wider">{kpi.label}</span>
            </div>
            <div className={`text-2xl font-bold font-mono ${kpi.color}`}>{kpi.value}</div>
          </div>
        ))}
      </div>

      {statsValues.length > 0 && (
        <div className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-5">
          <h3 className="text-sm font-bold text-white mb-4">Fleet Resource Utilization</h3>
          <div className="flex justify-around">
            <GaugeChart value={avgCpu} label="Avg CPU" size={110} thickness={8} />
            <GaugeChart value={avgMem} label="Avg Memory" size={110} thickness={8} />
            <GaugeChart value={avgDisk} label="Avg Disk" size={110} thickness={8} />
          </div>
        </div>
      )}

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        <Link to="/servers" className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-5 hover:bg-white/[0.08] transition-colors">
          <div className="flex items-center gap-2 mb-3">
            <Server className="w-5 h-5 text-blue-400" />
            <h3 className="text-sm font-bold text-white">Servers</h3>
            <span className="ml-auto text-xs text-slate-500">{totalServers} targets</span>
          </div>
          <div className="space-y-2">
            <div className="flex justify-between text-xs">
              <span className="text-slate-400">Reachable</span>
              <span className="text-white font-mono">{reachableServers}/{totalServers}</span>
            </div>
            <div className="flex justify-between text-xs">
              <span className="text-slate-400">Average CPU</span>
              <span className="text-white font-mono">{avgCpu}%</span>
            </div>
            <div className="flex justify-between text-xs">
              <span className="text-slate-400">Average Memory</span>
              <span className="text-white font-mono">{avgMem}%</span>
            </div>
            <div className="flex justify-between text-xs">
              <span className="text-slate-400">Average Disk</span>
              <span className="text-white font-mono">{avgDisk}%</span>
            </div>
            {serverTargets.length > 0 && reachableServers === 0 && (
              <div className="flex items-center gap-2 text-xs text-slate-500 pt-1"><Loader2 className="w-3 h-3 animate-spin" /> Loading server health</div>
            )}
          </div>
        </Link>

        <Link to="/storage" className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-5 hover:bg-white/[0.08] transition-colors">
          <div className="flex items-center gap-2 mb-3">
            <HardDrive className="w-5 h-5 text-purple-400" />
            <h3 className="text-sm font-bold text-white">Storage</h3>
            <span className="ml-auto text-xs text-slate-500">{storageTargets.length} targets</span>
          </div>
          {storageTargets.length > 0 ? (
            <div className="space-y-2">
              {storageTargets.slice(0, 4).map(t => (
                <div key={t.target_id} className="flex items-center gap-2 text-xs">
                  <div className={`w-2 h-2 rounded-full ${t.status === 'healthy' ? 'bg-emerald-400' : 'bg-amber-400'}`} />
                  <span className="text-white truncate flex-1">{t.name}</span>
                  <span className="text-slate-400">{t.status}</span>
                </div>
              ))}
            </div>
          ) : (
            <div className="text-xs text-slate-500 py-4 text-center">No storage targets configured</div>
          )}
        </Link>

        <Link to="/network" className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-5 hover:bg-white/[0.08] transition-colors">
          <div className="flex items-center gap-2 mb-3">
            <Network className="w-5 h-5 text-cyan-400" />
            <h3 className="text-sm font-bold text-white">Network</h3>
          </div>
          {netCapture ? (
            <div className="space-y-2 text-xs">
              <div className="flex justify-between"><span className="text-slate-400">Bytes In</span><span className="text-white font-mono">{formatGb((netCapture as any).total_bytes_in)}</span></div>
              <div className="flex justify-between"><span className="text-slate-400">Bytes Out</span><span className="text-white font-mono">{formatGb((netCapture as any).total_bytes_out)}</span></div>
              <div className="flex justify-between"><span className="text-slate-400">Active Conns</span><span className="text-white font-mono">{(netCapture as any).active_connections ?? 0}</span></div>
              <div className="flex justify-between"><span className="text-slate-400">Packets In</span><span className="text-white font-mono">{formatPackets((netCapture as any).total_packets_in)}</span></div>
            </div>
          ) : (
            <div className="text-xs text-slate-500 py-4 text-center">Loading network data...</div>
          )}
        </Link>
      </div>
    </div>
  )
}
