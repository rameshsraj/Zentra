import { useCallback } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { getTargets, getTargetStats, getNetworkCapture, type ServerStats } from '@/lib/api'
import GaugeChart from '@/components/charts/GaugeChart'
import PageHeader from '@/components/PageHeader'
import AddTargetModal from '@/components/AddTargetModal'
import { Server, HardDrive, Network, Cpu, MemoryStick, Loader2, Wifi } from 'lucide-react'
import { useState, useEffect, useMemo } from 'react'
import { Link } from 'react-router-dom'

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

  const totalServers = serverTargets.length
  const reachableServers = Object.values(serverStats).length
  const avgCpu = Object.values(serverStats).length > 0
    ? Math.round(Object.values(serverStats).reduce((a, s) => a + s.cpu_usage_percent, 0) / Object.values(serverStats).length)
    : 0
  const avgMem = Object.values(serverStats).length > 0
    ? Math.round(Object.values(serverStats).reduce((a, s) => a + s.memory_usage_percent, 0) / Object.values(serverStats).length)
    : 0
  const avgDisk = Object.values(serverStats).length > 0
    ? Math.round(Object.values(serverStats).reduce((a, s) => a + s.disk_usage_percent, 0) / Object.values(serverStats).length)
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

      {/* KPI Strip */}
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

      {/* Gauges */}
      {Object.values(serverStats).length > 0 && (
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
        {/* Servers Section */}
        <Link to="/servers" className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-5 hover:bg-white/[0.08] transition-colors">
          <div className="flex items-center gap-2 mb-3">
            <Server className="w-5 h-5 text-blue-400" />
            <h3 className="text-sm font-bold text-white">Servers</h3>
            <span className="ml-auto text-xs text-slate-500">{totalServers} targets</span>
          </div>
          <div className="space-y-2">
            {serverTargets.slice(0, 4).map(t => {
              const stats = serverStats[t.target_id]
              return (
                <div key={t.target_id} className="flex items-center gap-2 text-xs">
                  <div className={`w-2 h-2 rounded-full ${stats ? 'bg-emerald-400' : 'bg-slate-500'}`} />
                  <span className="text-white truncate flex-1">{t.name}</span>
                  {stats ? (
                    <span className="text-slate-400 font-mono">CPU {Math.round(stats.cpu_usage_percent)}%</span>
                  ) : (
                    <Loader2 className="w-3 h-3 animate-spin text-slate-500" />
                  )}
                </div>
              )
            })}
            {totalServers > 4 && <div className="text-[10px] text-slate-500">+{totalServers - 4} more</div>}
          </div>
        </Link>

        {/* Storage Section */}
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

        {/* Network Section */}
        <Link to="/network" className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-5 hover:bg-white/[0.08] transition-colors">
          <div className="flex items-center gap-2 mb-3">
            <Network className="w-5 h-5 text-cyan-400" />
            <h3 className="text-sm font-bold text-white">Network</h3>
          </div>
          {netCapture ? (
            <div className="space-y-2 text-xs">
              <div className="flex justify-between"><span className="text-slate-400">Bytes In</span><span className="text-white font-mono">{((netCapture as any).total_bytes_in / 1e9).toFixed(1)} GB</span></div>
              <div className="flex justify-between"><span className="text-slate-400">Bytes Out</span><span className="text-white font-mono">{((netCapture as any).total_bytes_out / 1e9).toFixed(1)} GB</span></div>
              <div className="flex justify-between"><span className="text-slate-400">Active Conns</span><span className="text-white font-mono">{(netCapture as any).active_connections}</span></div>
              <div className="flex justify-between"><span className="text-slate-400">Packets In</span><span className="text-white font-mono">{((netCapture as any).total_packets_in / 1000).toFixed(0)}K</span></div>
            </div>
          ) : (
            <div className="text-xs text-slate-500 py-4 text-center">Loading network data...</div>
          )}
        </Link>
      </div>

      {/* Per-Server Detail Cards */}
      {Object.values(serverStats).length > 0 && (
        <div>
          <h3 className="text-sm font-bold text-white mb-3">Server Fleet Details</h3>
          <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-3">
            {serverTargets.map(t => {
              const s = serverStats[t.target_id]
              if (!s) return null
              return (
                <div key={t.target_id} className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-4">
                  <div className="flex items-center gap-2 mb-2">
                    <div className={`w-2 h-2 rounded-full ${s ? 'bg-emerald-400' : 'bg-red-400'}`} />
                    <span className="text-sm font-bold text-white">{s.hostname || t.name}</span>
                    <span className="ml-auto text-[10px] text-slate-500">{t.host}</span>
                  </div>
                  <div className="flex justify-around">
                    <GaugeChart value={Math.round(s.cpu_usage_percent)} label="CPU" size={70} thickness={5} />
                    <GaugeChart value={Math.round(s.memory_usage_percent)} label="Mem" size={70} thickness={5} />
                    <GaugeChart value={Math.round(s.disk_usage_percent)} label="Disk" size={70} thickness={5} />
                  </div>
                  <div className="grid grid-cols-3 gap-1 mt-2 text-[10px]">
                    <div className="text-center"><span className="text-slate-500">Cores</span><br/><span className="text-white font-mono">{s.cpu_cores}</span></div>
                    <div className="text-center"><span className="text-slate-500">RAM</span><br/><span className="text-white font-mono">{Math.round(s.memory_total_mb / 1024)}GB</span></div>
                    <div className="text-center"><span className="text-slate-500">Procs</span><br/><span className="text-white font-mono">{s.process_count}</span></div>
                  </div>
                </div>
              )
            })}
          </div>
        </div>
      )}
    </div>
  )
}
