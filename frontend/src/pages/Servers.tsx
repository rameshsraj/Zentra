import { useQuery, useQueryClient } from '@tanstack/react-query'
import { getTargets, getTargetStats, setupSsh, deleteTarget, type Target, type ServerStats } from '@/lib/api'
import GaugeChart from '@/components/charts/GaugeChart'
import Sparkline from '@/components/charts/Sparkline'
import PageHeader from '@/components/PageHeader'
import AddTargetModal from '@/components/AddTargetModal'
import DrilldownModal, { type DrilldownNode } from '@/components/DrilldownModal'
import { Cpu, MemoryStick, HardDrive, Loader2, X, AlertTriangle, KeyRound, CheckCircle, XCircle, Trash2, ChevronRight, Database } from 'lucide-react'
import { useState, useCallback, useMemo, useEffect } from 'react'

// Stable sparkline from a base value using deterministic sine wave (no random)
const spark = (base: number, len = 20) =>
  Array.from({ length: len }, (_, i) => Math.max(0, Math.min(100, base + Math.sin(base * 0.1 + i * 0.5) * (base * 0.05))))

interface ServerView {
  target_id: string
  name: string
  os: string
  cpu: number
  mem: number
  disk: number
  status: string
  ip: string
  cores: number
  ram_gb: number
  uptime: string
  kernel: string
  cpu_model: string
  load_avg: string
  memory_total_mb: number
  memory_used_mb: number
  swap_total_mb: number
  swap_used_mb: number
  disk_total_gb: number
  disk_used_gb: number
  process_count: number
  logged_in_users: number
  reachable: boolean
  loading: boolean
  error: string | null
  disks: { filesystem: string; mount: string; total_gb: number; used_gb: number; free_gb: number; usage_percent: number; fs_type: string }[]
  nfs_mounts: { remote: string; mount: string; total_gb: number; used_gb: number; free_gb: number; usage_percent: number }[]
}

function targetToPlaceholder(t: Target): ServerView {
  return {
    target_id: t.target_id,
    name: t.name,
    os: t.target_subtype || 'linux',
    cpu: 0, mem: 0, disk: 0,
    status: t.status,
    ip: t.host,
    cores: 0, ram_gb: 0,
    uptime: '—',
    kernel: '', cpu_model: '', load_avg: '',
    memory_total_mb: 0, memory_used_mb: 0,
    swap_total_mb: 0, swap_used_mb: 0,
    disk_total_gb: 0, disk_used_gb: 0,
    process_count: 0, logged_in_users: 0,
    reachable: false, loading: true, error: null,
    disks: [], nfs_mounts: [],
  }
}

function applyStats(sv: ServerView, stats: ServerStats): ServerView {
  return {
    ...sv,
    name: stats.hostname || sv.name,
    os: stats.os || sv.os,
    kernel: stats.kernel,
    cpu: Math.round(stats.cpu_usage_percent),
    mem: Math.round(stats.memory_usage_percent),
    disk: Math.round(stats.disk_usage_percent),
    cores: stats.cpu_cores,
    ram_gb: Math.round(stats.memory_total_mb / 1024),
    cpu_model: stats.cpu_model,
    load_avg: `${stats.load_avg_1} / ${stats.load_avg_5} / ${stats.load_avg_15}`,
    uptime: stats.uptime_display,
    memory_total_mb: stats.memory_total_mb,
    memory_used_mb: stats.memory_used_mb,
    swap_total_mb: stats.swap_total_mb,
    swap_used_mb: stats.swap_used_mb,
    disk_total_gb: stats.disk_total_gb,
    disk_used_gb: stats.disk_used_gb,
    process_count: stats.process_count,
    logged_in_users: stats.logged_in_users,
    reachable: stats.reachable,
    loading: false,
    error: stats.error,
    status: stats.reachable ? (stats.cpu_usage_percent > 90 || stats.memory_usage_percent > 90 ? 'critical' : stats.cpu_usage_percent > 75 || stats.memory_usage_percent > 80 ? 'warning' : 'healthy') : 'unreachable',
    disks: stats.disks || [],
    nfs_mounts: stats.nfs_mounts || [],
  } as ServerView
}

export default function ServersPage() {
  const qc = useQueryClient()
  const { data: targets } = useQuery({ queryKey: ['targets'], queryFn: getTargets })
  const [search, setSearch] = useState('')
  const [selected, setSelected] = useState<ServerView | null>(null)
  const [drilldown, setDrilldown] = useState<{ open: boolean; title: string; nodes: DrilldownNode[] }>({ open: false, title: '', nodes: [] })
  const [showAddModal, setShowAddModal] = useState(false)
  const [serverMap, setServerMap] = useState<Record<string, ServerView>>({})
  const [sshSetup, setSshSetup] = useState<{ open: boolean; host: string; targetId: string } | null>(null)
  const [sshForm, setSshForm] = useState({ username: 'root', password: '' })
  const [sshSetupLoading, setSshSetupLoading] = useState(false)
  const [sshSetupResult, setSshSetupResult] = useState<{ success: boolean; message: string } | null>(null)
  const [deleteConfirm, setDeleteConfirm] = useState<{ open: boolean; targetId: string; name: string } | null>(null)
  const [deleting, setDeleting] = useState(false)
  const [diskDrilldown, setDiskDrilldown] = useState(false)

  const handleDelete = async (targetId: string) => {
    setDeleting(true)
    try {
      await deleteTarget(targetId)
      setDeleteConfirm(null)
      setSelected(null)
      setServerMap(prev => {
        const next = { ...prev }
        delete next[targetId]
        return next
      })
      qc.invalidateQueries({ queryKey: ['targets'] })
    } catch (err) {
      console.error('Failed to delete target:', err)
    }
    setDeleting(false)
  }

  const serverTargets = useMemo(() =>
    (targets ?? []).filter(t => t.target_type === 'server'),
    [targets]
  )

  // Fetch real stats for each server target
  useEffect(() => {
    serverTargets.forEach(t => {
      // Set placeholder immediately
      setServerMap(prev => {
        if (prev[t.target_id] && !prev[t.target_id].loading) return prev
        return { ...prev, [t.target_id]: targetToPlaceholder(t) }
      })
      // Fetch real stats
      getTargetStats(t.target_id).then(stats => {
        setServerMap(prev => ({
          ...prev,
          [t.target_id]: applyStats(prev[t.target_id] || targetToPlaceholder(t), stats),
        }))
      }).catch(err => {
        setServerMap(prev => ({
          ...prev,
          [t.target_id]: { ...(prev[t.target_id] || targetToPlaceholder(t)), loading: false, error: String(err), reachable: false },
        }))
      })
    })
  }, [serverTargets])

  const handleRefresh = useCallback(() => {
    qc.invalidateQueries({ queryKey: ['targets'] })
    // Also re-fetch stats
    setServerMap(prev => {
      const next: Record<string, ServerView> = {}
      for (const [k, v] of Object.entries(prev)) next[k] = { ...v, loading: true }
      return next
    })
    serverTargets.forEach(t => {
      getTargetStats(t.target_id).then(stats => {
        setServerMap(prev => ({
          ...prev,
          [t.target_id]: applyStats(prev[t.target_id] || targetToPlaceholder(t), stats),
        }))
      }).catch(() => {})
    })
  }, [qc, serverTargets])

  const servers = useMemo(() =>
    serverTargets
      .map(t => serverMap[t.target_id] || targetToPlaceholder(t))
      .filter(s => s.name.toLowerCase().includes(search.toLowerCase()) || s.ip.includes(search)),
    [serverTargets, serverMap, search]
  )

  const healthy = servers.filter(s => s.status === 'healthy').length
  const warning = servers.filter(s => s.status === 'warning').length
  const critical = servers.filter(s => s.status === 'critical' || s.status === 'unreachable').length
  const avgCpu = servers.length > 0 ? Math.round(servers.reduce((a, s) => a + s.cpu, 0) / servers.length) : 0
  const avgMem = servers.length > 0 ? Math.round(servers.reduce((a, s) => a + s.mem, 0) / servers.length) : 0

  return (
    <div className="space-y-6">
      <PageHeader
        title="Server Fleet"
        subtitle={`${servers.length} servers monitored · ${healthy} healthy · ${warning} warning · ${critical} critical`}
        onRefresh={handleRefresh}
        onSearch={setSearch}
        searchPlaceholder="Search servers by name or IP..."
        showAddTarget
        onAddTarget={() => setShowAddModal(true)}
        addTargetLabel="Add Server"
      />

      {/* KPI Strip */}
      <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-3" role="region" aria-label="Server statistics">
        {[
          { label: 'Total Servers', value: String(servers.length), color: 'text-white' },
          { label: 'Healthy', value: String(healthy), color: 'text-emerald-400' },
          { label: 'Warning', value: String(warning), color: 'text-amber-400' },
          { label: 'Critical', value: String(critical), color: 'text-red-400' },
          { label: 'Avg CPU', value: `${avgCpu}%`, color: 'text-blue-400' },
          { label: 'Avg Memory', value: `${avgMem}%`, color: 'text-purple-400' },
        ].map((kpi, i) => (
          <div key={i} className="bg-white/5 backdrop-blur-sm border border-white/10 p-3 rounded-xl">
            <div className="text-[10px] text-slate-400 uppercase tracking-wider mb-1">{kpi.label}</div>
            <div className={`text-2xl font-bold font-mono ${kpi.color}`}>{kpi.value}</div>
          </div>
        ))}
      </div>

      {/* Server Cards Grid */}
      <div className="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-4" role="list" aria-label="Server list">
        {servers.map(server => (
          <div key={server.target_id} role="listitem" aria-label={`Server ${server.name}`} className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-5 hover:bg-white/[0.08] transition-colors cursor-pointer" onClick={() => setSelected(server)}>
            {/* Header */}
            <div className="flex items-center justify-between mb-4">
              <div>
                <div className="flex items-center gap-2">
                  <h3 className="text-sm font-bold text-white">{server.name}</h3>
                  {server.loading ? (
                    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-semibold bg-blue-500/20 text-blue-400 border border-blue-500/30">
                      <Loader2 className="w-3 h-3 animate-spin" /> loading
                    </span>
                  ) : !server.reachable ? (
                    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-semibold bg-red-500/20 text-red-400 border border-red-500/30">
                      <AlertTriangle className="w-3 h-3" /> unreachable
                    </span>
                  ) : (
                    <span className={`inline-flex items-center px-2 py-0.5 rounded-full text-[10px] font-semibold border ${
                      server.status === 'healthy' ? 'bg-emerald-500/20 text-emerald-400 border-emerald-500/30' :
                      server.status === 'warning' ? 'bg-amber-500/20 text-amber-400 border-amber-500/30' :
                      'bg-red-500/20 text-red-400 border-red-500/30'
                    }`}>{server.status}</span>
                  )}
                </div>
                <div className="text-xs text-slate-500 mt-0.5">
                  {server.ip} · {server.os || server.name} · {server.cores} cores · {server.ram_gb} GB RAM
                </div>
              </div>
              <div className="flex items-center gap-2">
                <span className="text-[10px] text-slate-600">Up: {server.uptime}</span>
                <button
                  onClick={e => { e.stopPropagation(); setDeleteConfirm({ open: true, targetId: server.target_id, name: server.name }) }}
                  className="p-1 rounded-md hover:bg-red-500/20 text-slate-600 hover:text-red-400 transition-colors"
                  title="Remove server"
                >
                  <Trash2 className="w-3.5 h-3.5" />
                </button>
              </div>
            </div>

            {server.loading ? (
              <div className="flex items-center justify-center py-8">
                <Loader2 className="w-8 h-8 animate-spin text-slate-500" />
                <span className="ml-2 text-sm text-slate-500">Collecting real metrics via SSH...</span>
              </div>
            ) : server.error ? (
              <div className="px-3 py-4 bg-red-500/10 border border-red-500/20 rounded-lg text-xs text-red-400">
                <AlertTriangle className="w-4 h-4 inline mr-1" />
                {server.error}
                {(server.error.includes('Permission denied') || server.error.includes('known hosts') || server.error.includes('Connection refused') || server.error.includes('SSH failed')) && (
                  <button
                    onClick={e => { e.stopPropagation(); setSshSetup({ open: true, host: server.ip, targetId: server.target_id }); setSshSetupResult(null); setSshForm({ username: 'root', password: '' }) }}
                    className="mt-2 flex items-center gap-1.5 px-3 py-1.5 bg-blue-600/20 border border-blue-500/30 text-blue-400 rounded-lg text-[10px] font-medium hover:bg-blue-600/30 transition-colors"
                  >
                    <KeyRound className="w-3 h-3" /> Setup SSH Access
                  </button>
                )}
              </div>
            ) : (
              <>
                {/* Gauges */}
                <div className="flex justify-around mb-3">
                  <GaugeChart value={server.cpu} label="CPU" size={90} thickness={7} />
                  <GaugeChart value={server.mem} label="Memory" size={90} thickness={7} />
                  <GaugeChart value={server.disk} label="Disk" size={90} thickness={7} />
                </div>

                {/* Sparklines */}
                <div className="space-y-2 mt-3">
                  <div className="flex items-center gap-2">
                    <Cpu className="w-3 h-3 text-blue-400" />
                    <span className="text-[10px] text-slate-500 w-12">CPU</span>
                    <Sparkline data={spark(server.cpu)} width={140} height={16} color="#3b82f6" fillColor="#3b82f6" />
                    <span className="text-[10px] font-mono text-slate-400 ml-auto">{server.cpu}%</span>
                  </div>
                  <div className="flex items-center gap-2">
                    <MemoryStick className="w-3 h-3 text-purple-400" />
                    <span className="text-[10px] text-slate-500 w-12">Mem</span>
                    <Sparkline data={spark(server.mem)} width={140} height={16} color="#8b5cf6" fillColor="#8b5cf6" />
                    <span className="text-[10px] font-mono text-slate-400 ml-auto">{server.mem}%</span>
                  </div>
                  <div className="flex items-center gap-2">
                    <HardDrive className="w-3 h-3 text-amber-400" />
                    <span className="text-[10px] text-slate-500 w-12">Disk</span>
                    <Sparkline data={spark(server.disk)} width={140} height={16} color="#f59e0b" fillColor="#f59e0b" />
                    <span className="text-[10px] font-mono text-slate-400 ml-auto">{server.disk}%</span>
                  </div>
                </div>
              </>
            )}
          </div>
        ))}
      </div>

      {/* Server Detail Side Panel */}
      {selected && (
        <div className="fixed inset-0 z-50 flex justify-end bg-black/40 backdrop-blur-sm" onClick={() => setSelected(null)}>
          <div className="w-full max-w-lg bg-slate-900 border-l border-white/10 shadow-2xl overflow-y-auto" onClick={e => e.stopPropagation()}>
            <div className="p-5 border-b border-white/10 flex items-center justify-between">
              <div>
                <h3 className="text-lg font-bold text-white">{selected.name}</h3>
                <div className="text-xs text-slate-500">{selected.os} · {selected.ip}</div>
              </div>
              <div className="flex items-center gap-2">
                <button
                  onClick={() => setDeleteConfirm({ open: true, targetId: selected.target_id, name: selected.name })}
                  className="p-1.5 rounded-lg hover:bg-red-500/20 text-slate-400 hover:text-red-400 transition-colors"
                  title="Remove server"
                >
                  <Trash2 className="w-5 h-5" />
                </button>
                <button onClick={() => setSelected(null)} className="p-1.5 rounded-lg hover:bg-white/10 text-slate-400">
                  <X className="w-5 h-5" />
                </button>
              </div>
            </div>
            <div className="p-5 space-y-4">
              {selected.loading ? (
                <div className="flex items-center justify-center py-12">
                  <Loader2 className="w-8 h-8 animate-spin text-slate-500" />
                </div>
              ) : selected.error ? (
                <div className="px-4 py-6 bg-red-500/10 border border-red-500/20 rounded-xl text-sm text-red-400">
                  <AlertTriangle className="w-5 h-5 inline mr-2" />
                  {selected.error}
                  {(selected.error.includes('Permission denied') || selected.error.includes('known hosts') || selected.error.includes('Connection refused') || selected.error.includes('SSH failed')) && (
                    <button
                      onClick={() => { setSshSetup({ open: true, host: selected.ip, targetId: selected.target_id }); setSshSetupResult(null); setSshForm({ username: 'root', password: '' }) }}
                      className="mt-3 flex items-center gap-2 px-4 py-2 bg-blue-600/20 border border-blue-500/30 text-blue-400 rounded-lg text-xs font-medium hover:bg-blue-600/30 transition-colors"
                    >
                      <KeyRound className="w-4 h-4" /> Setup SSH Key Access
                    </button>
                  )}
                </div>
              ) : (
                <>
                  <div className="flex justify-around">
                    <GaugeChart value={selected.cpu} label="CPU" size={100} thickness={8} />
                    <GaugeChart value={selected.mem} label="Memory" size={100} thickness={8} />
                    <GaugeChart value={selected.disk} label="Disk" size={100} thickness={8} />
                  </div>
                  <div className="grid grid-cols-2 gap-3">
                    {[
                      { k: 'Hostname', v: selected.name },
                      { k: 'IP Address', v: selected.ip },
                      { k: 'OS', v: selected.os },
                      { k: 'Kernel', v: selected.kernel },
                      { k: 'CPU Model', v: selected.cpu_model || '—' },
                      { k: 'CPU Cores', v: String(selected.cores) },
                      { k: 'RAM', v: `${selected.ram_gb} GB (${selected.memory_used_mb} / ${selected.memory_total_mb} MB)` },
                      { k: 'Swap', v: `${selected.swap_used_mb} / ${selected.swap_total_mb} MB` },
                      { k: 'Load Average', v: selected.load_avg || '—' },
                      { k: 'Uptime', v: selected.uptime },
                      { k: 'Processes', v: String(selected.process_count) },
                      { k: 'Logged-in Users', v: String(selected.logged_in_users) },
                      { k: 'Status', v: selected.status },
                    ].map(item => (
                      <div key={item.k} className="bg-white/5 rounded-lg p-3">
                        <div className="text-[10px] text-slate-500 uppercase tracking-wider">{item.k}</div>
                        <div className="text-sm font-mono text-white mt-0.5">{item.v}</div>
                      </div>
                    ))}
                  </div>
                  {/* Disk Storage Drilldown */}
                  <button
                    onClick={() => setDiskDrilldown(true)}
                    className="w-full bg-white/5 border border-white/10 rounded-xl p-4 hover:bg-white/[0.08] transition-colors text-left group"
                  >
                    <div className="flex items-center justify-between mb-3">
                      <div className="flex items-center gap-2">
                        <HardDrive className="w-4 h-4 text-amber-400" />
                        <span className="text-xs font-bold text-white uppercase tracking-wider">Disk Storage</span>
                      </div>
                      <div className="flex items-center gap-1 text-[10px] text-slate-500 group-hover:text-amber-400 transition-colors">
                        <span>{selected.disks.length} partition{selected.disks.length !== 1 ? 's' : ''}{selected.nfs_mounts.length > 0 ? ` + ${selected.nfs_mounts.length} NFS` : ''}</span>
                        <ChevronRight className="w-3.5 h-3.5" />
                      </div>
                    </div>
                    <div className="flex items-center gap-3">
                      <div className="flex-1">
                        <div className="h-2 bg-white/10 rounded-full overflow-hidden">
                          <div className="h-full rounded-full transition-all" style={{ width: `${selected.disk}%`, background: selected.disk >= 90 ? '#ef4444' : selected.disk >= 70 ? '#f59e0b' : '#22c55e' }} />
                        </div>
                      </div>
                      <span className="text-sm font-mono text-white whitespace-nowrap">{selected.disk_used_gb} / {selected.disk_total_gb} GB</span>
                      <span className="text-xs font-mono" style={{ color: selected.disk >= 90 ? '#ef4444' : selected.disk >= 70 ? '#f59e0b' : '#22c55e' }}>{selected.disk}%</span>
                    </div>
                  </button>

                  <div className="space-y-2">
                    <h4 className="text-xs font-bold text-white uppercase tracking-wider">Resource Trends</h4>
                    {[
                      { label: 'CPU', data: spark(selected.cpu), color: '#3b82f6' },
                      { label: 'Memory', data: spark(selected.mem), color: '#8b5cf6' },
                      { label: 'Disk', data: spark(selected.disk), color: '#f59e0b' },
                    ].map(t => (
                      <div key={t.label} className="flex items-center gap-2">
                        <span className="text-[10px] text-slate-500 w-12">{t.label}</span>
                        <Sparkline data={t.data} width={300} height={20} color={t.color} fillColor={t.color} />
                      </div>
                    ))}
                  </div>
                </>
              )}
            </div>
          </div>
        </div>
      )}

      <DrilldownModal open={drilldown.open} onClose={() => setDrilldown(d => ({ ...d, open: false }))} title={drilldown.title} root={drilldown.nodes} />

      {/* Disk Drilldown Modal */}
      {diskDrilldown && selected && (
        <div className="fixed inset-0 z-[60] flex items-center justify-center bg-black/60 backdrop-blur-sm" onClick={() => setDiskDrilldown(false)}>
          <div className="bg-slate-900 border border-white/10 rounded-2xl w-full max-w-2xl shadow-2xl max-h-[80vh] overflow-y-auto" onClick={e => e.stopPropagation()}>
            <div className="p-5 border-b border-white/10 flex items-center justify-between">
              <div className="flex items-center gap-3">
                <div className="p-2 bg-amber-500/20 rounded-lg"><HardDrive className="w-5 h-5 text-amber-400" /></div>
                <div>
                  <h3 className="text-white font-bold">Disk Storage — {selected.name}</h3>
                  <p className="text-xs text-slate-400">Total: {selected.disk_used_gb} / {selected.disk_total_gb} GB ({selected.disk}% used)</p>
                </div>
              </div>
              <button onClick={() => setDiskDrilldown(false)} className="p-1.5 rounded-lg hover:bg-white/10 text-slate-400"><X className="w-5 h-5" /></button>
            </div>
            <div className="p-5 space-y-4">
              {/* Local Partitions */}
              <div>
                <h4 className="text-[10px] font-bold text-slate-400 uppercase tracking-wider mb-3">Local Partitions</h4>
                {selected.disks.length === 0 ? (
                  <div className="text-xs text-slate-600 italic">No partition data available</div>
                ) : (
                  <div className="space-y-3">
                    {selected.disks.map((d, i) => (
                      <div key={i} className="bg-white/5 border border-white/10 rounded-xl p-4">
                        <div className="flex items-center justify-between mb-2">
                          <div>
                            <span className="text-sm font-mono text-white">{d.mount}</span>
                            <span className="text-[10px] text-slate-500 ml-2">{d.filesystem}</span>
                            {d.fs_type && <span className="text-[10px] text-blue-400 ml-2 px-1.5 py-0.5 bg-blue-500/10 rounded">{d.fs_type}</span>}
                          </div>
                          <span className="text-xs font-mono" style={{ color: d.usage_percent >= 90 ? '#ef4444' : d.usage_percent >= 70 ? '#f59e0b' : '#22c55e' }}>{d.usage_percent}%</span>
                        </div>
                        <div className="h-2 bg-white/10 rounded-full overflow-hidden mb-1.5">
                          <div className="h-full rounded-full transition-all" style={{ width: `${d.usage_percent}%`, background: d.usage_percent >= 90 ? '#ef4444' : d.usage_percent >= 70 ? '#f59e0b' : '#22c55e' }} />
                        </div>
                        <div className="flex justify-between text-[10px] text-slate-500">
                          <span>Used: {d.used_gb} GB</span>
                          <span>Free: {d.free_gb} GB</span>
                          <span>Total: {d.total_gb} GB</span>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </div>

              {/* NFS Mounts */}
              {selected.nfs_mounts.length > 0 && (
                <div>
                  <h4 className="text-[10px] font-bold text-slate-400 uppercase tracking-wider mb-3 flex items-center gap-2">
                    <Database className="w-3 h-3 text-blue-400" /> NFS Storage
                  </h4>
                  <div className="space-y-3">
                    {selected.nfs_mounts.map((n, i) => (
                      <div key={i} className="bg-blue-500/5 border border-blue-500/20 rounded-xl p-4">
                        <div className="flex items-center justify-between mb-2">
                          <div>
                            <span className="text-sm font-mono text-white">{n.mount}</span>
                            <span className="text-[10px] text-blue-400 ml-2 px-1.5 py-0.5 bg-blue-500/10 rounded">NFS</span>
                          </div>
                          <span className="text-xs font-mono" style={{ color: n.usage_percent >= 90 ? '#ef4444' : n.usage_percent >= 70 ? '#f59e0b' : '#3b82f6' }}>{n.usage_percent}%</span>
                        </div>
                        <div className="text-[10px] text-slate-500 mb-2">Remote: {n.remote}</div>
                        <div className="h-2 bg-white/10 rounded-full overflow-hidden mb-1.5">
                          <div className="h-full rounded-full transition-all" style={{ width: `${n.usage_percent}%`, background: n.usage_percent >= 90 ? '#ef4444' : n.usage_percent >= 70 ? '#f59e0b' : '#3b82f6' }} />
                        </div>
                        <div className="flex justify-between text-[10px] text-slate-500">
                          <span>Used: {n.used_gb} GB</span>
                          <span>Free: {n.free_gb} GB</span>
                          <span>Total: {n.total_gb} GB</span>
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>
          </div>
        </div>
      )}
      <AddTargetModal open={showAddModal} onClose={() => setShowAddModal(false)} defaultType="server" title="Add Server" />

      {/* Delete Confirmation Dialog */}
      {deleteConfirm?.open && (
        <div className="fixed inset-0 z-[60] flex items-center justify-center bg-black/60 backdrop-blur-sm" onClick={() => setDeleteConfirm(null)}>
          <div className="bg-slate-900 border border-white/10 rounded-2xl p-6 w-full max-w-sm shadow-2xl" onClick={e => e.stopPropagation()}>
            <div className="flex items-center gap-3 mb-4">
              <div className="p-2 bg-red-500/20 rounded-lg">
                <Trash2 className="w-5 h-5 text-red-400" />
              </div>
              <div>
                <h3 className="text-white font-bold">Remove Server</h3>
                <p className="text-xs text-slate-400">This action cannot be undone</p>
              </div>
            </div>
            <p className="text-sm text-slate-300 mb-5">
              Are you sure you want to remove <span className="font-semibold text-white">{deleteConfirm.name}</span> from monitoring?
            </p>
            <div className="flex gap-3">
              <button
                onClick={() => setDeleteConfirm(null)}
                className="flex-1 px-4 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-slate-400 hover:text-white transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={() => handleDelete(deleteConfirm.targetId)}
                disabled={deleting}
                className="flex-1 flex items-center justify-center gap-2 px-4 py-2.5 bg-red-600 hover:bg-red-700 rounded-xl text-sm font-medium text-white transition-colors disabled:opacity-50"
              >
                {deleting ? <Loader2 className="w-4 h-4 animate-spin" /> : <Trash2 className="w-4 h-4" />}
                {deleting ? 'Removing...' : 'Remove'}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* SSH Key Setup Modal */}
      {sshSetup?.open && (
        <div className="fixed inset-0 z-[60] flex items-center justify-center bg-black/60 backdrop-blur-sm" onClick={() => setSshSetup(null)}>
          <div className="bg-slate-900 border border-white/10 rounded-2xl p-6 w-full max-w-md shadow-2xl" onClick={e => e.stopPropagation()}>
            <div className="flex items-center gap-3 mb-4">
              <div className="p-2 bg-blue-500/20 rounded-lg">
                <KeyRound className="w-5 h-5 text-blue-400" />
              </div>
              <div>
                <h3 className="text-white font-bold">Setup SSH Key Access</h3>
                <p className="text-xs text-slate-400">Automatically configure passwordless SSH to <span className="font-mono text-blue-400">{sshSetup.host}</span></p>
              </div>
            </div>

            <div className="space-y-3">
              <div className="px-3 py-2 bg-blue-500/5 border border-blue-500/10 rounded-xl text-[11px] text-blue-400/80">
                This will copy the Zentra server's SSH public key to the target machine, enabling passwordless authentication for future monitoring.
              </div>

              <div>
                <label className="block text-[10px] font-medium text-slate-400 uppercase tracking-wider mb-1">Username</label>
                <input
                  value={sshForm.username}
                  onChange={e => setSshForm(f => ({ ...f, username: e.target.value }))}
                  className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-blue-500/50"
                  placeholder="root"
                />
              </div>
              <div>
                <label className="block text-[10px] font-medium text-slate-400 uppercase tracking-wider mb-1">Password (one-time, for key setup only)</label>
                <input
                  type="password"
                  value={sshForm.password}
                  onChange={e => setSshForm(f => ({ ...f, password: e.target.value }))}
                  className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-blue-500/50"
                  placeholder="Enter password for SSH key setup"
                />
              </div>

              {sshSetupResult && (
                <div className={`px-4 py-2.5 rounded-xl text-sm flex items-center gap-2 border ${
                  sshSetupResult.success
                    ? 'bg-emerald-500/10 border-emerald-500/20 text-emerald-400'
                    : 'bg-red-500/10 border-red-500/20 text-red-400'
                }`}>
                  {sshSetupResult.success ? <CheckCircle className="w-4 h-4" /> : <XCircle className="w-4 h-4" />}
                  {sshSetupResult.message}
                </div>
              )}

              <div className="flex gap-3 pt-2">
                <button
                  onClick={() => setSshSetup(null)}
                  className="flex-1 px-4 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-slate-400 hover:text-white transition-colors"
                >
                  Cancel
                </button>
                <button
                  onClick={async () => {
                    if (!sshForm.password) return
                    setSshSetupLoading(true)
                    setSshSetupResult(null)
                    try {
                      const res = await setupSsh(sshSetup.host, sshForm.username, sshForm.password)
                      setSshSetupResult(res)
                      if (res.success) {
                        // Re-fetch stats after successful setup
                        setTimeout(() => {
                          handleRefresh()
                          setSshSetup(null)
                        }, 2000)
                      }
                    } catch (err) {
                      setSshSetupResult({ success: false, message: String(err) })
                    }
                    setSshSetupLoading(false)
                  }}
                  disabled={sshSetupLoading || !sshForm.password}
                  className="flex-1 flex items-center justify-center gap-2 px-4 py-2.5 bg-blue-600 hover:bg-blue-700 rounded-xl text-sm font-medium text-white transition-colors disabled:opacity-50"
                >
                  {sshSetupLoading ? <Loader2 className="w-4 h-4 animate-spin" /> : <KeyRound className="w-4 h-4" />}
                  {sshSetupLoading ? 'Setting up...' : 'Setup SSH Key'}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
