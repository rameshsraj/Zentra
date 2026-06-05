import { useState, useEffect, useCallback } from 'react'
import { fetchJSON, getOracleConnections, createOracleConnection, deleteOracleConnection, testOracleConnection, getOracleAlertLog, getOracleAlertLogStats, getOracleTxEventQueues, getOracleTxEventQueueStats } from '@/lib/api'
import PageHeader from '@/components/PageHeader'
import {
  Database, Activity, HardDrive, Clock, AlertTriangle, Server,
  BarChart3, Cpu, Zap, Shield, RefreshCw,
  Search, Timer, Layers, Radio, TrendingUp, Plus, Trash2, CheckCircle, XCircle, FileText, Inbox, Terminal,
  Box, Eye, Repeat, Code, Package, Bolt, ListOrdered, Hash, Filter,
} from 'lucide-react'
import { cn } from '@/lib/utils'
// LiveModeButton removed — not currently used
import LoadingSkeleton from '@/components/LoadingSkeleton'
import SqlWorkspaceTab from '@/components/SqlWorkspaceTab'
import {
  AreaChart, Area, BarChart, Bar, XAxis, YAxis, CartesianGrid,
  Tooltip, ResponsiveContainer, PieChart, Pie, Cell, Legend,
} from 'recharts'

const TABS = [
  { id: 'overview', label: 'Overview', icon: Database },
  { id: 'connections', label: 'Instances', icon: Database },
  { id: 'tablespaces', label: 'Tablespaces', icon: HardDrive },
  { id: 'waits', label: 'Wait Classes', icon: Clock },
  { id: 'activity', label: 'Activity', icon: Activity },
  { id: 'cache', label: 'Cache & SGA', icon: Zap },
  { id: 'top-queries', label: 'Top Queries', icon: Search },
  { id: 'running', label: 'Running Queries', icon: Timer },
  { id: 'pdb', label: 'PDB / CDB', icon: Layers },
  { id: 'dataguard', label: 'Data Guard', icon: Shield },
  { id: 'rman', label: 'RMAN', icon: HardDrive },
  { id: 'listeners', label: 'Listeners', icon: Radio },
  { id: 'rac', label: 'RAC', icon: Server },
  { id: 'alertlog', label: 'Alert Log', icon: FileText },
  { id: 'txeventq', label: 'TxEventQ', icon: Inbox },
  { id: 'schema-objects', label: 'Schema Objects', icon: Box },
  { id: 'scrape-status', label: 'Scrape Status', icon: BarChart3 },
  { id: 'sql-workspace', label: 'SQL Workspace', icon: Terminal },
] as const

const COLORS = ['#3b82f6','#10b981','#f59e0b','#ef4444','#8b5cf6','#ec4899','#06b6d4','#84cc16','#f97316']

function KPI({ label, value, sub, color = 'text-white' }: { label: string; value: string | number; sub?: string; color?: string }) {
  return (
    <div className="bg-slate-800/60 rounded-lg border border-slate-700 p-3">
      <p className="text-[10px] text-slate-400 uppercase tracking-wider">{label}</p>
      <p className={cn('text-2xl font-bold mt-0.5', color)}>{typeof value === 'number' ? value.toLocaleString() : value}</p>
      {sub && <p className="text-[10px] text-slate-500 mt-0.5">{sub}</p>}
    </div>
  )
}

function SectionTitle({ icon: Icon, title }: { icon: any; title: string }) {
  return (
    <h3 className="flex items-center gap-2 text-sm font-bold text-white uppercase tracking-wider mb-3">
      <Icon className="w-4 h-4 text-cyan-400" /> {title}
    </h3>
  )
}

export default function OracleMonitoringPage() {
  const [tab, setTab] = useState('overview')
  const [overview, setOverview] = useState<any>(null)
  const [tablespaces, setTablespaces] = useState<any>(null)
  const [waits, setWaits] = useState<any>(null)
  const [topQueries, setTopQueries] = useState<any>(null)
  const [runningQueries, setRunningQueries] = useState<any>(null)
  const [cache, setCache] = useState<any>(null)
  const [dataguard, setDataguard] = useState<any>(null)
  const [rman, setRman] = useState<any>(null)
  const [pdb, setPdb] = useState<any>(null)
  const [listeners, setListeners] = useState<any>(null)
  const [rac, setRac] = useState<any>(null)
  const [perfHistory, setPerfHistory] = useState<any[]>([])
  const [sysmetrics, setSysmetrics] = useState<any>(null)
  const [loading, setLoading] = useState(true)
  const [autoRefresh, setAutoRefresh] = useState(false)
  const [connections, setConnections] = useState<any[]>([])
  const [selectedConnId, setSelectedConnId] = useState('')
  const [scrapeStatus, setScrapeStatus] = useState<any>(null)

  const loadData = useCallback(async () => {
    setLoading(true)
    const qs = selectedConnId ? `?connection_id=${selectedConnId}` : ''
    try {
      const [ov, ts, wc, tq, rq, ch, dg, rm, pd, li, rc, ph, sm] = await Promise.all([
        fetchJSON(`/oracle/overview${qs}`).catch(() => null),
        fetchJSON(`/oracle/tablespaces${qs}`).catch(() => null),
        fetchJSON(`/oracle/wait-classes${qs}`).catch(() => null),
        fetchJSON(`/oracle/top-queries${qs}`).catch(() => null),
        fetchJSON(`/oracle/running-queries${qs}`).catch(() => null),
        fetchJSON(`/oracle/cache-hit-ratio${qs}`).catch(() => null),
        fetchJSON(`/oracle/dataguard${qs}`).catch(() => null),
        fetchJSON(`/oracle/rman${qs}`).catch(() => null),
        fetchJSON(`/oracle/pdb${qs}`).catch(() => null),
        fetchJSON(`/oracle/listeners${qs}`).catch(() => null),
        fetchJSON(`/oracle/rac${qs}`).catch(() => null),
        fetchJSON(`/oracle/performance-history${qs}`).catch(() => null),
        fetchJSON(`/oracle/sysmetrics${qs}`).catch(() => null),
      ])
      setOverview(ov)
      setTablespaces(ts)
      setWaits(wc)
      setTopQueries(tq)
      setRunningQueries(rq)
      setCache(ch)
      setDataguard(dg)
      setRman(rm)
      setPdb(pd)
      setListeners(li)
      setRac(rc)
      setPerfHistory((ph as any)?.data || [])
      setSysmetrics(sm)
    } finally { setLoading(false) }
  }, [selectedConnId])

  useEffect(() => { loadData() }, [loadData])

  // P7: Load connections and scrape status
  const loadConnections = useCallback(() => {
    getOracleConnections().then(c => { setConnections(c || []); if (c?.length && !selectedConnId) setSelectedConnId(c[0].id) }).catch(() => {})
  }, []) // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    loadConnections()
    fetchJSON<any>('/oracle/scrape/status').then(setScrapeStatus).catch(() => {})
  }, [loadConnections])

  useEffect(() => {
    if (!autoRefresh) return
    const id = setInterval(loadData, 10000)
    return () => clearInterval(id)
  }, [autoRefresh, loadData])

  const inst = overview?.instance
  const health = overview?.health

  return (
    <div className="space-y-4">
      <PageHeader
        title="Oracle Database Monitoring"
        subtitle={inst ? `${inst.instance_name} @ ${inst.host_name} — ${inst.version} ${inst.edition}` : 'Loading...'}
        onRefresh={loadData}
      />

      {/* P7: Database selector */}
      {connections.length > 0 && (
        <div className="flex items-center gap-2 bg-slate-800/60 rounded-lg border border-slate-700 px-3 py-1.5 w-fit">
          <Database className="w-4 h-4 text-orange-400" />
          <select value={selectedConnId} onChange={e => setSelectedConnId(e.target.value)}
            className="bg-transparent text-sm text-white border-none focus:outline-none cursor-pointer">
            {connections.map((c: any) => <option key={c.id} value={c.id} className="bg-slate-800">{c.name || c.id} ({c.host})</option>)}
          </select>
        </div>
      )}

      {/* Controls */}
      <div className="flex items-center gap-2">
        <button onClick={loadData} className="flex items-center gap-1 px-3 py-1.5 bg-white/5 hover:bg-white/10 border border-white/10 rounded text-xs text-slate-300">
          <RefreshCw className={cn('w-3.5 h-3.5', loading && 'animate-spin')} /> Refresh
        </button>
        <button onClick={() => setAutoRefresh(!autoRefresh)} className={cn('px-3 py-1.5 border rounded text-xs', autoRefresh ? 'bg-green-600/20 border-green-500/30 text-green-400' : 'bg-white/5 border-white/10 text-slate-400')}>
          {autoRefresh ? 'Auto-Refresh ON (10s)' : 'Auto-Refresh OFF'}
        </button>
      </div>

      {/* Vertical Sidebar Tabs + Content Layout */}
      <div className="flex gap-4">
        {/* Left Sidebar Tabs */}
        <nav className="flex-shrink-0 w-44 space-y-1" aria-label="Oracle monitoring sections">
          {TABS.map(t => {
            const Icon = t.icon
            const isSeparator = t.id === 'sql-workspace'
            return (
              <div key={t.id}>
                {isSeparator && <div className="border-t border-slate-700 my-2" />}
                <button
                  onClick={() => setTab(t.id)}
                  role="tab"
                  aria-selected={tab === t.id}
                  aria-label={t.label}
                  className={cn(
                    'flex items-center gap-2 w-full px-3 py-2 rounded-lg text-xs font-medium transition-colors text-left',
                    tab === t.id
                      ? 'bg-cyan-600/20 text-cyan-400 border border-cyan-500/30'
                      : 'text-slate-400 hover:bg-white/5 hover:text-white border border-transparent'
                  )}
                >
                  <Icon className="w-3.5 h-3.5 flex-shrink-0" /> {t.label}
                </button>
              </div>
            )
          })}
        </nav>

        {/* Right Content Area */}
        <div className="flex-1 min-w-0">

      {loading && !inst && <LoadingSkeleton variant="kpi" count={6} />}

      {/* ======== OVERVIEW TAB ======== */}
      {!loading && tab === 'overview' && (
        <div className="space-y-4">
          {/* Instance KPIs */}
          <div className="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-6 gap-3">
            <KPI label="Status" value={inst?.status || '—'} color={inst?.status === 'OPEN' ? 'text-green-400' : 'text-red-400'} />
            <KPI label="Uptime" value={`${inst?.uptime_days || 0}d`} />
            <KPI label="Active Sessions" value={health?.active_sessions || 0} color="text-cyan-400" />
            <KPI label="Total Sessions" value={health?.session_count || 0} />
            <KPI label="Buffer Cache Hit" value={`${health?.buffer_cache_hit_ratio || 0}%`} color="text-green-400" />
            <KPI label="Library Cache Hit" value={`${health?.library_cache_hit_ratio || 0}%`} color="text-green-400" />
            <KPI label="SGA Size" value={`${health?.sga_size_gb || 0} GB`} />
            <KPI label="PGA Size" value={`${health?.pga_size_gb || 0} GB`} />
            <KPI label="DB Block Gets/s" value={health?.db_block_gets_per_sec || 0} />
            <KPI label="Physical Reads/s" value={health?.physical_reads_per_sec || 0} />
            <KPI label="Commits/s" value={health?.user_commits_per_sec || 0} color="text-blue-400" />
            <KPI label="Redo Writes/s" value={health?.redo_writes_per_sec || 0} />
          </div>

          {/* Performance Chart */}
          {perfHistory.length > 0 && (
            <div className="bg-white/5 border border-white/10 rounded-xl p-4">
              <SectionTitle icon={TrendingUp} title="Performance History (Last 60 min)" />
              <div className="h-[300px]">
                <ResponsiveContainer width="100%" height="100%">
                  <AreaChart data={perfHistory.map(p => ({
                    time: new Date(p.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }),
                    ...p,
                  }))}>
                    <CartesianGrid strokeDasharray="3 3" stroke="#334155" />
                    <XAxis dataKey="time" tick={{ fontSize: 10, fill: '#94a3b8' }} />
                    <YAxis tick={{ fontSize: 10, fill: '#94a3b8' }} />
                    <Tooltip contentStyle={{ backgroundColor: '#1e293b', border: '1px solid #475569', borderRadius: 8, fontSize: 11 }} />
                    <Area type="monotone" dataKey="active_sessions" name="Active Sessions" stroke="#3b82f6" fill="#3b82f6" fillOpacity={0.15} />
                    <Area type="monotone" dataKey="cpu_pct" name="CPU %" stroke="#10b981" fill="#10b981" fillOpacity={0.1} />
                    <Area type="monotone" dataKey="io_wait_pct" name="I/O Wait %" stroke="#f59e0b" fill="#f59e0b" fillOpacity={0.1} />
                    <Legend wrapperStyle={{ fontSize: 11 }} />
                  </AreaChart>
                </ResponsiveContainer>
              </div>
            </div>
          )}

          {/* Alerts */}
          {overview?.alerts?.length > 0 && (
            <div className="bg-white/5 border border-white/10 rounded-xl p-4">
              <SectionTitle icon={AlertTriangle} title="Active Alerts" />
              <div className="space-y-2">
                {overview.alerts.map((a: any, i: number) => (
                  <div key={i} className={cn('flex items-center gap-3 px-3 py-2 rounded border', a.severity === 'warning' ? 'bg-amber-500/10 border-amber-500/30 text-amber-300' : 'bg-blue-500/10 border-blue-500/30 text-blue-300')}>
                    <AlertTriangle className="w-4 h-4 flex-shrink-0" />
                    <span className="text-xs flex-1">{a.message}</span>
                    <span className="text-[10px] text-slate-500">{new Date(a.time).toLocaleTimeString()}</span>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      )}

      {/* ======== TABLESPACES TAB ======== */}
      {tab === 'tablespaces' && tablespaces && (
        <div className="space-y-4">
          <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
            <KPI label="Total Tablespaces" value={tablespaces.summary?.total_tablespaces || 0} />
            <KPI label="Total Size" value={`${tablespaces.summary?.total_size_gb || 0} GB`} />
            <KPI label="Used" value={`${tablespaces.summary?.total_used_gb || 0} GB`} color="text-amber-400" />
            <KPI label="Free" value={`${tablespaces.summary?.total_free_gb || 0} GB`} color="text-green-400" />
          </div>

          {/* Bar Chart */}
          <div className="bg-white/5 border border-white/10 rounded-xl p-4">
            <SectionTitle icon={BarChart3} title="Tablespace Usage %" />
            <div className="h-[300px]">
              <ResponsiveContainer width="100%" height="100%">
                <BarChart data={tablespaces.tablespaces} layout="vertical">
                  <CartesianGrid strokeDasharray="3 3" stroke="#334155" />
                  <XAxis type="number" domain={[0, 100]} tick={{ fontSize: 10, fill: '#94a3b8' }} />
                  <YAxis dataKey="name" type="category" width={120} tick={{ fontSize: 10, fill: '#94a3b8' }} />
                  <Tooltip contentStyle={{ backgroundColor: '#1e293b', border: '1px solid #475569', borderRadius: 8, fontSize: 11 }} />
                  <Bar dataKey="used_pct" name="Used %">
                    {(tablespaces.tablespaces || []).map((ts: any, i: number) => (
                      <Cell key={i} fill={ts.used_pct > 80 ? '#ef4444' : ts.used_pct > 60 ? '#f59e0b' : '#10b981'} />
                    ))}
                  </Bar>
                </BarChart>
              </ResponsiveContainer>
            </div>
          </div>

          {/* Table */}
          <div className="bg-white/5 border border-white/10 rounded-xl overflow-hidden">
            <table className="w-full text-xs">
              <thead><tr className="bg-slate-800/80 border-b border-white/10">
                <th className="text-left px-4 py-2 text-slate-400 uppercase">Name</th>
                <th className="text-center px-3 py-2 text-slate-400 uppercase">Status</th>
                <th className="text-center px-3 py-2 text-slate-400 uppercase">Contents</th>
                <th className="text-right px-3 py-2 text-slate-400 uppercase">Size MB</th>
                <th className="text-right px-3 py-2 text-slate-400 uppercase">Used MB</th>
                <th className="text-right px-3 py-2 text-slate-400 uppercase">Free MB</th>
                <th className="text-right px-3 py-2 text-slate-400 uppercase">Used %</th>
                <th className="text-center px-3 py-2 text-slate-400 uppercase">Auto</th>
              </tr></thead>
              <tbody className="divide-y divide-white/5">
                {(tablespaces.tablespaces || []).map((ts: any) => (
                  <tr key={ts.name} className="hover:bg-white/5">
                    <td className="px-4 py-2 font-mono text-white font-medium">{ts.name}</td>
                    <td className="px-3 py-2 text-center"><span className={cn('px-2 py-0.5 rounded text-[10px] font-bold', ts.status === 'ONLINE' ? 'bg-green-500/20 text-green-400' : 'bg-red-500/20 text-red-400')}>{ts.status}</span></td>
                    <td className="px-3 py-2 text-center text-slate-400">{ts.contents}</td>
                    <td className="px-3 py-2 text-right text-slate-300 font-mono">{ts.size_mb.toLocaleString()}</td>
                    <td className="px-3 py-2 text-right text-slate-300 font-mono">{ts.used_mb.toLocaleString()}</td>
                    <td className="px-3 py-2 text-right text-slate-300 font-mono">{ts.free_mb.toLocaleString()}</td>
                    <td className="px-3 py-2 text-right"><span className={cn('font-bold font-mono', ts.used_pct > 80 ? 'text-red-400' : ts.used_pct > 60 ? 'text-amber-400' : 'text-green-400')}>{ts.used_pct}%</span></td>
                    <td className="px-3 py-2 text-center text-slate-400">{ts.autoextend ? '✓' : '✗'}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* ======== WAIT CLASSES TAB ======== */}
      {tab === 'waits' && waits && (
        <div className="space-y-4">
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
            {/* Pie Chart */}
            <div className="bg-white/5 border border-white/10 rounded-xl p-4">
              <SectionTitle icon={Clock} title="Wait Class Distribution" />
              <div className="h-[300px]">
                <ResponsiveContainer width="100%" height="100%">
                  <PieChart>
                    <Pie data={(waits.wait_classes || []).map((w: any) => ({ name: w.wait_class, value: w.pct_of_total }))} dataKey="value" nameKey="name" cx="50%" cy="50%" outerRadius={100} label={({ name, value }: any) => `${name}: ${value}%`} labelLine={false}>
                      {(waits.wait_classes || []).map((_: any, i: number) => <Cell key={i} fill={COLORS[i % COLORS.length]} />)}
                    </Pie>
                    <Tooltip contentStyle={{ backgroundColor: '#1e293b', border: '1px solid #475569', borderRadius: 8, fontSize: 11 }} />
                  </PieChart>
                </ResponsiveContainer>
              </div>
            </div>

            {/* Bar Chart */}
            <div className="bg-white/5 border border-white/10 rounded-xl p-4">
              <SectionTitle icon={BarChart3} title="Wait Time (ms/sec)" />
              <div className="h-[300px]">
                <ResponsiveContainer width="100%" height="100%">
                  <BarChart data={waits.wait_classes} layout="vertical">
                    <CartesianGrid strokeDasharray="3 3" stroke="#334155" />
                    <XAxis type="number" tick={{ fontSize: 10, fill: '#94a3b8' }} />
                    <YAxis dataKey="wait_class" type="category" width={100} tick={{ fontSize: 10, fill: '#94a3b8' }} />
                    <Tooltip contentStyle={{ backgroundColor: '#1e293b', border: '1px solid #475569', borderRadius: 8, fontSize: 11 }} />
                    <Bar dataKey="wait_time_ms" name="Wait Time (ms/sec)">
                      {(waits.wait_classes || []).map((_: any, i: number) => <Cell key={i} fill={COLORS[i % COLORS.length]} />)}
                    </Bar>
                  </BarChart>
                </ResponsiveContainer>
              </div>
            </div>
          </div>

          {/* Table */}
          <div className="bg-white/5 border border-white/10 rounded-xl overflow-hidden">
            <table className="w-full text-xs">
              <thead><tr className="bg-slate-800/80 border-b border-white/10">
                <th className="text-left px-4 py-2 text-slate-400 uppercase">Wait Class</th>
                <th className="text-right px-3 py-2 text-slate-400 uppercase">Wait Time (ms/s)</th>
                <th className="text-right px-3 py-2 text-slate-400 uppercase">Total Waits</th>
                <th className="text-right px-3 py-2 text-slate-400 uppercase">Avg Wait (ms)</th>
                <th className="text-right px-3 py-2 text-slate-400 uppercase">% of Total</th>
              </tr></thead>
              <tbody className="divide-y divide-white/5">
                {(waits.wait_classes || []).map((w: any, i: number) => (
                  <tr key={w.wait_class} className="hover:bg-white/5">
                    <td className="px-4 py-2 font-medium text-white flex items-center gap-2">
                      <span className="w-3 h-3 rounded-full" style={{ backgroundColor: COLORS[i % COLORS.length] }} />
                      {w.wait_class}
                    </td>
                    <td className="px-3 py-2 text-right font-mono text-slate-300">{w.wait_time_ms}</td>
                    <td className="px-3 py-2 text-right font-mono text-slate-300">{w.total_waits.toLocaleString()}</td>
                    <td className="px-3 py-2 text-right font-mono text-slate-300">{w.avg_wait_ms}</td>
                    <td className="px-3 py-2 text-right"><div className="flex items-center justify-end gap-2"><div className="w-16 bg-slate-700 rounded-full h-1.5"><div className="h-1.5 rounded-full" style={{ width: `${w.pct_of_total}%`, backgroundColor: COLORS[i % COLORS.length] }} /></div><span className="font-mono text-slate-300">{w.pct_of_total}%</span></div></td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* ======== ACTIVITY TAB ======== */}
      {tab === 'activity' && (
        <div className="space-y-4">
          {perfHistory.length > 0 && (
            <>
              <div className="bg-white/5 border border-white/10 rounded-xl p-4">
                <SectionTitle icon={Activity} title="Active Sessions & CPU" />
                <div className="h-[250px]">
                  <ResponsiveContainer width="100%" height="100%">
                    <AreaChart data={perfHistory.map(p => ({ time: new Date(p.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }), ...p }))}>
                      <CartesianGrid strokeDasharray="3 3" stroke="#334155" />
                      <XAxis dataKey="time" tick={{ fontSize: 10, fill: '#94a3b8' }} />
                      <YAxis tick={{ fontSize: 10, fill: '#94a3b8' }} />
                      <Tooltip contentStyle={{ backgroundColor: '#1e293b', border: '1px solid #475569', borderRadius: 8, fontSize: 11 }} />
                      <Area type="monotone" dataKey="active_sessions" name="Active Sessions" stroke="#3b82f6" fill="#3b82f6" fillOpacity={0.2} />
                      <Area type="monotone" dataKey="cpu_pct" name="CPU %" stroke="#10b981" fill="#10b981" fillOpacity={0.1} />
                      <Legend wrapperStyle={{ fontSize: 11 }} />
                    </AreaChart>
                  </ResponsiveContainer>
                </div>
              </div>

              <div className="bg-white/5 border border-white/10 rounded-xl p-4">
                <SectionTitle icon={TrendingUp} title="I/O Activity" />
                <div className="h-[250px]">
                  <ResponsiveContainer width="100%" height="100%">
                    <AreaChart data={perfHistory.map(p => ({ time: new Date(p.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }), ...p }))}>
                      <CartesianGrid strokeDasharray="3 3" stroke="#334155" />
                      <XAxis dataKey="time" tick={{ fontSize: 10, fill: '#94a3b8' }} />
                      <YAxis tick={{ fontSize: 10, fill: '#94a3b8' }} />
                      <Tooltip contentStyle={{ backgroundColor: '#1e293b', border: '1px solid #475569', borderRadius: 8, fontSize: 11 }} />
                      <Area type="monotone" dataKey="physical_reads_per_sec" name="Physical Reads/s" stroke="#f59e0b" fill="#f59e0b" fillOpacity={0.15} />
                      <Area type="monotone" dataKey="physical_writes_per_sec" name="Physical Writes/s" stroke="#8b5cf6" fill="#8b5cf6" fillOpacity={0.1} />
                      <Area type="monotone" dataKey="redo_writes_per_sec" name="Redo Writes/s" stroke="#ec4899" fill="#ec4899" fillOpacity={0.1} />
                      <Legend wrapperStyle={{ fontSize: 11 }} />
                    </AreaChart>
                  </ResponsiveContainer>
                </div>
              </div>
            </>
          )}

          {/* Sysmetrics */}
          {sysmetrics?.metrics && (
            <div className="bg-white/5 border border-white/10 rounded-xl p-4">
              <SectionTitle icon={Cpu} title="v$sysmetric — Key Metrics" />
              <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-2">
                {Object.entries(sysmetrics.metrics as Record<string, number>).map(([k, v]) => (
                  <div key={k} className="bg-slate-800 rounded p-2 border border-slate-700">
                    <p className="text-[10px] text-slate-400 truncate">{k}</p>
                    <p className="text-sm font-mono text-white mt-0.5">{typeof v === 'number' ? v.toLocaleString(undefined, { maximumFractionDigits: 2 }) : v}</p>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      )}

      {/* ======== CACHE & SGA TAB ======== */}
      {tab === 'cache' && cache && (
        <div className="space-y-4">
          <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
            <KPI label="Buffer Cache Hit" value={`${cache.ratios?.buffer_cache_hit_ratio}%`} color="text-green-400" />
            <KPI label="Library Cache Hit" value={`${cache.ratios?.library_cache_hit_ratio}%`} color="text-green-400" />
            <KPI label="Row Cache Hit" value={`${cache.ratios?.row_cache_hit_ratio}%`} color="text-green-400" />
            <KPI label="Cursor Cache Hit" value={`${cache.ratios?.cursor_cache_hit_ratio}%`} color="text-green-400" />
            <KPI label="Shared Pool Free" value={`${cache.ratios?.shared_pool_free_pct}%`} />
            <KPI label="PGA Cache Hit" value={`${cache.ratios?.pga_cache_hit_pct}%`} color="text-green-400" />
            <KPI label="Total SGA" value={`${cache.sga_breakdown?.total_sga_mb} MB`} color="text-cyan-400" />
            <KPI label="PGA In Use" value={`${cache.pga?.pga_in_use_mb} MB`} />
          </div>

          {/* SGA Breakdown */}
          {cache.sga_breakdown && (
            <div className="bg-white/5 border border-white/10 rounded-xl p-4">
              <SectionTitle icon={Layers} title="SGA Memory Breakdown" />
              <div className="h-[300px]">
                <ResponsiveContainer width="100%" height="100%">
                  <PieChart>
                    <Pie
                      data={Object.entries(cache.sga_breakdown as Record<string, number>).filter(([k]) => k !== 'total_sga_mb').map(([k, v]) => ({ name: k.replace(/_mb$/, '').replace(/_/g, ' '), value: v }))}
                      dataKey="value" nameKey="name" cx="50%" cy="50%" outerRadius={110}
                      label={({ name, value }: any) => `${name}: ${value} MB`}
                    >
                      {Object.keys(cache.sga_breakdown).filter(k => k !== 'total_sga_mb').map((_, i) => <Cell key={i} fill={COLORS[i % COLORS.length]} />)}
                    </Pie>
                    <Tooltip contentStyle={{ backgroundColor: '#1e293b', border: '1px solid #475569', borderRadius: 8, fontSize: 11 }} />
                  </PieChart>
                </ResponsiveContainer>
              </div>
            </div>
          )}
        </div>
      )}

      {/* ======== TOP QUERIES TAB ======== */}
      {tab === 'top-queries' && topQueries && (
        <div className="space-y-4">
          <div className="grid grid-cols-2 md:grid-cols-3 gap-3">
            <KPI label="Queries Tracked" value={topQueries.total || 0} />
            <KPI label="Sort By" value={topQueries.sort_by || 'elapsed_time'} />
          </div>
          <div className="bg-white/5 border border-white/10 rounded-xl overflow-hidden overflow-x-auto">
            <table className="w-full text-xs min-w-[1200px]">
              <thead><tr className="bg-slate-800/80 border-b border-white/10">
                <th className="text-left px-3 py-2 text-slate-400 uppercase">SQL ID</th>
                <th className="text-left px-3 py-2 text-slate-400 uppercase">Schema</th>
                <th className="text-left px-3 py-2 text-slate-400 uppercase max-w-[300px]">SQL Text</th>
                <th className="text-right px-3 py-2 text-slate-400 uppercase">Execs</th>
                <th className="text-right px-3 py-2 text-slate-400 uppercase">Total (ms)</th>
                <th className="text-right px-3 py-2 text-slate-400 uppercase">Avg (ms)</th>
                <th className="text-right px-3 py-2 text-slate-400 uppercase">CPU (ms)</th>
                <th className="text-right px-3 py-2 text-slate-400 uppercase">Buffer Gets</th>
                <th className="text-right px-3 py-2 text-slate-400 uppercase">Disk Reads</th>
                <th className="text-right px-3 py-2 text-slate-400 uppercase">Rows</th>
                <th className="text-left px-3 py-2 text-slate-400 uppercase">Module</th>
              </tr></thead>
              <tbody className="divide-y divide-white/5">
                {(topQueries.queries || []).map((q: any) => (
                  <tr key={q.sql_id} className="hover:bg-white/5">
                    <td className="px-3 py-2 font-mono text-cyan-400">{q.sql_id}</td>
                    <td className="px-3 py-2 text-slate-300">{q.schema}</td>
                    <td className="px-3 py-2 text-slate-400 max-w-[300px] truncate font-mono text-[10px]" title={q.sql_text}>{q.sql_text}</td>
                    <td className="px-3 py-2 text-right font-mono text-white">{q.executions?.toLocaleString()}</td>
                    <td className="px-3 py-2 text-right font-mono text-amber-400">{(q.elapsed_time_ms / 1000).toFixed(1)}s</td>
                    <td className="px-3 py-2 text-right font-mono text-slate-300">{q.avg_elapsed_ms?.toFixed(1)}</td>
                    <td className="px-3 py-2 text-right font-mono text-slate-300">{(q.cpu_time_ms / 1000).toFixed(1)}s</td>
                    <td className="px-3 py-2 text-right font-mono text-slate-300">{q.buffer_gets?.toLocaleString()}</td>
                    <td className="px-3 py-2 text-right font-mono text-slate-300">{q.disk_reads?.toLocaleString()}</td>
                    <td className="px-3 py-2 text-right font-mono text-slate-300">{q.rows_processed?.toLocaleString()}</td>
                    <td className="px-3 py-2 text-slate-400">{q.module}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* ======== RUNNING QUERIES TAB ======== */}
      {tab === 'running' && runningQueries && (
        <div className="space-y-4">
          <KPI label="Active Non-Background Sessions" value={runningQueries.active_count || 0} color="text-cyan-400" />
          <div className="bg-white/5 border border-white/10 rounded-xl overflow-hidden overflow-x-auto">
            <table className="w-full text-xs min-w-[1100px]">
              <thead><tr className="bg-slate-800/80 border-b border-white/10">
                <th className="text-left px-3 py-2 text-slate-400 uppercase">SID</th>
                <th className="text-left px-3 py-2 text-slate-400 uppercase">Serial#</th>
                <th className="text-left px-3 py-2 text-slate-400 uppercase">User</th>
                <th className="text-left px-3 py-2 text-slate-400 uppercase">SQL ID</th>
                <th className="text-left px-3 py-2 text-slate-400 uppercase">Wait Event</th>
                <th className="text-left px-3 py-2 text-slate-400 uppercase">Wait Class</th>
                <th className="text-right px-3 py-2 text-slate-400 uppercase">Wait (s)</th>
                <th className="text-right px-3 py-2 text-slate-400 uppercase">Elapsed (ms)</th>
                <th className="text-left px-3 py-2 text-slate-400 uppercase max-w-[250px]">SQL Text</th>
                <th className="text-left px-3 py-2 text-slate-400 uppercase">Machine</th>
              </tr></thead>
              <tbody className="divide-y divide-white/5">
                {(runningQueries.sessions || []).map((s: any) => (
                  <tr key={s.sid} className="hover:bg-white/5">
                    <td className="px-3 py-2 font-mono text-white">{s.sid}</td>
                    <td className="px-3 py-2 font-mono text-slate-300">{s.serial}</td>
                    <td className="px-3 py-2 text-cyan-400">{s.username}</td>
                    <td className="px-3 py-2 font-mono text-slate-300">{s.sql_id}</td>
                    <td className="px-3 py-2 text-slate-300">{s.event}</td>
                    <td className="px-3 py-2"><span className={cn('px-2 py-0.5 rounded text-[10px] font-bold', s.wait_class === 'Application' ? 'bg-red-500/20 text-red-400' : 'bg-blue-500/20 text-blue-400')}>{s.wait_class}</span></td>
                    <td className="px-3 py-2 text-right font-mono">{s.seconds_in_wait > 3 ? <span className="text-red-400">{s.seconds_in_wait}</span> : <span className="text-slate-300">{s.seconds_in_wait}</span>}</td>
                    <td className="px-3 py-2 text-right font-mono text-slate-300">{s.elapsed_ms?.toLocaleString()}</td>
                    <td className="px-3 py-2 text-slate-400 max-w-[250px] truncate font-mono text-[10px]" title={s.sql_text}>{s.sql_text}</td>
                    <td className="px-3 py-2 text-slate-400 text-[10px]">{s.machine}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* ======== PDB/CDB TAB ======== */}
      {tab === 'pdb' && pdb && (
        <div className="space-y-4">
          <div className="grid grid-cols-2 md:grid-cols-3 gap-3">
            <KPI label="CDB Name" value={pdb.cdb_name || '—'} />
            <KPI label="Open Mode" value={pdb.cdb_open_mode || '—'} color="text-green-400" />
            <KPI label="PDBs" value={pdb.pdbs?.length || 0} color="text-cyan-400" />
          </div>
          <div className="bg-white/5 border border-white/10 rounded-xl overflow-hidden">
            <table className="w-full text-xs">
              <thead><tr className="bg-slate-800/80 border-b border-white/10">
                <th className="text-left px-4 py-2 text-slate-400 uppercase">PDB Name</th>
                <th className="text-center px-3 py-2 text-slate-400 uppercase">Status</th>
                <th className="text-center px-3 py-2 text-slate-400 uppercase">Open Mode</th>
                <th className="text-right px-3 py-2 text-slate-400 uppercase">Size (GB)</th>
                <th className="text-right px-3 py-2 text-slate-400 uppercase">Sessions</th>
                <th className="text-right px-3 py-2 text-slate-400 uppercase">CPU %</th>
                <th className="text-right px-3 py-2 text-slate-400 uppercase">Memory %</th>
                <th className="text-right px-3 py-2 text-slate-400 uppercase">I/O %</th>
              </tr></thead>
              <tbody className="divide-y divide-white/5">
                {(pdb.pdbs || []).map((p: any) => (
                  <tr key={p.pdb_name} className="hover:bg-white/5">
                    <td className="px-4 py-2 font-medium text-white">{p.pdb_name}</td>
                    <td className="px-3 py-2 text-center"><span className="px-2 py-0.5 rounded text-[10px] font-bold bg-green-500/20 text-green-400">{p.status}</span></td>
                    <td className="px-3 py-2 text-center text-slate-300">{p.open_mode}</td>
                    <td className="px-3 py-2 text-right font-mono text-slate-300">{p.size_gb}</td>
                    <td className="px-3 py-2 text-right font-mono text-slate-300">{p.sessions}</td>
                    <td className="px-3 py-2 text-right font-mono text-slate-300">{p.cpu_pct}%</td>
                    <td className="px-3 py-2 text-right font-mono text-slate-300">{p.memory_pct}%</td>
                    <td className="px-3 py-2 text-right font-mono text-slate-300">{p.io_pct}%</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* ======== DATA GUARD TAB ======== */}
      {tab === 'dataguard' && dataguard && (
        <div className="space-y-4">
          <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
            <KPI label="Protection Mode" value={dataguard.protection_mode || '—'} />
            <KPI label="Database Role" value={dataguard.database_role || '—'} color="text-cyan-400" />
            <KPI label="Switchover Status" value={dataguard.switchover_status || '—'} />
            <KPI label="Log Mode" value={dataguard.archive_log?.log_mode || '—'} color="text-green-400" />
          </div>

          {(dataguard.standby_databases || []).map((stby: any) => (
            <div key={stby.db_unique_name} className="bg-white/5 border border-white/10 rounded-xl p-4">
              <SectionTitle icon={Shield} title={`Standby: ${stby.db_unique_name}`} />
              <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
                <KPI label="Role" value={stby.role} />
                <KPI label="Open Mode" value={stby.open_mode} color="text-green-400" />
                <KPI label="Transport Lag" value={`${stby.transport_lag_seconds}s`} color={stby.transport_lag_seconds < 5 ? 'text-green-400' : 'text-amber-400'} />
                <KPI label="Apply Lag" value={`${stby.apply_lag_seconds}s`} color={stby.apply_lag_seconds < 10 ? 'text-green-400' : 'text-amber-400'} />
                <KPI label="Apply Rate" value={`${stby.apply_rate_kbps} KB/s`} />
                <KPI label="Redo Transport" value={stby.redo_transport} />
                <KPI label="Status" value={stby.status} color="text-green-400" />
              </div>
            </div>
          ))}

          {/* Redo Log Groups */}
          <div className="bg-white/5 border border-white/10 rounded-xl overflow-hidden">
            <div className="p-3 border-b border-white/10"><SectionTitle icon={Layers} title="Redo Log Groups" /></div>
            <table className="w-full text-xs">
              <thead><tr className="bg-slate-800/80 border-b border-white/10">
                <th className="text-left px-4 py-2 text-slate-400">Group</th>
                <th className="text-left px-3 py-2 text-slate-400">Thread</th>
                <th className="text-right px-3 py-2 text-slate-400">Size (MB)</th>
                <th className="text-center px-3 py-2 text-slate-400">Status</th>
                <th className="text-center px-3 py-2 text-slate-400">Archived</th>
              </tr></thead>
              <tbody className="divide-y divide-white/5">
                {(dataguard.redo_log_groups || []).map((r: any) => (
                  <tr key={r.group_number} className="hover:bg-white/5">
                    <td className="px-4 py-2 font-mono text-white">{r.group_number}</td>
                    <td className="px-3 py-2 text-slate-300">{r.thread}</td>
                    <td className="px-3 py-2 text-right font-mono text-slate-300">{r.bytes_mb}</td>
                    <td className="px-3 py-2 text-center"><span className={cn('px-2 py-0.5 rounded text-[10px] font-bold', r.status === 'CURRENT' ? 'bg-green-500/20 text-green-400' : 'bg-slate-500/20 text-slate-400')}>{r.status}</span></td>
                    <td className="px-3 py-2 text-center text-slate-400">{r.archived}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* ======== RMAN TAB ======== */}
      {tab === 'rman' && rman && (
        <div className="space-y-4">
          <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
            <KPI label="Hours Since Last Full" value={rman.hours_since_last_full || 0} color={rman.hours_since_last_full < 24 ? 'text-green-400' : 'text-amber-400'} />
            <KPI label="Hours Since Incremental" value={rman.hours_since_last_incremental || 0} color="text-green-400" />
            <KPI label="Failed (24h)" value={rman.failed_backups_24h || 0} color={rman.failed_backups_24h === 0 ? 'text-green-400' : 'text-red-400'} />
            <KPI label="Backup Storage" value={`${rman.backup_storage?.total_used_gb || 0} GB`} />
          </div>

          <div className="bg-white/5 border border-white/10 rounded-xl overflow-hidden">
            <div className="p-3 border-b border-white/10"><SectionTitle icon={HardDrive} title="Recent Backups" /></div>
            <table className="w-full text-xs">
              <thead><tr className="bg-slate-800/80 border-b border-white/10">
                <th className="text-left px-4 py-2 text-slate-400">Type</th>
                <th className="text-center px-3 py-2 text-slate-400">Status</th>
                <th className="text-left px-3 py-2 text-slate-400">Start</th>
                <th className="text-right px-3 py-2 text-slate-400">Duration</th>
                <th className="text-right px-3 py-2 text-slate-400">Input (GB)</th>
                <th className="text-right px-3 py-2 text-slate-400">Output (GB)</th>
                <th className="text-right px-3 py-2 text-slate-400">Ratio</th>
                <th className="text-left px-3 py-2 text-slate-400">Device</th>
              </tr></thead>
              <tbody className="divide-y divide-white/5">
                {(rman.last_backups || []).map((b: any, i: number) => (
                  <tr key={i} className="hover:bg-white/5">
                    <td className="px-4 py-2 font-medium text-white">{b.backup_type}</td>
                    <td className="px-3 py-2 text-center"><span className={cn('px-2 py-0.5 rounded text-[10px] font-bold', b.status === 'COMPLETED' ? 'bg-green-500/20 text-green-400' : 'bg-red-500/20 text-red-400')}>{b.status}</span></td>
                    <td className="px-3 py-2 text-slate-300">{new Date(b.start_time).toLocaleString()}</td>
                    <td className="px-3 py-2 text-right font-mono text-slate-300">{b.duration_minutes}m</td>
                    <td className="px-3 py-2 text-right font-mono text-slate-300">{b.input_size_gb}</td>
                    <td className="px-3 py-2 text-right font-mono text-slate-300">{b.output_size_gb}</td>
                    <td className="px-3 py-2 text-right font-mono text-cyan-400">{b.compression_ratio}x</td>
                    <td className="px-3 py-2 text-slate-400">{b.device_type}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {rman.backup_schedule && (
            <div className="bg-white/5 border border-white/10 rounded-xl p-4">
              <SectionTitle icon={Clock} title="Backup Schedule" />
              <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
                <KPI label="Full Backup" value={rman.backup_schedule.full_backup} />
                <KPI label="Incremental L1" value={rman.backup_schedule.incremental_l1} />
                <KPI label="Archive Log" value={rman.backup_schedule.archivelog} />
              </div>
            </div>
          )}
        </div>
      )}

      {/* ======== LISTENERS TAB ======== */}
      {tab === 'listeners' && listeners && (
        <div className="space-y-4">
          {(listeners.listeners || []).map((l: any) => (
            <div key={l.name} className="bg-white/5 border border-white/10 rounded-xl p-4">
              <div className="flex items-center gap-3 mb-3">
                <Radio className="w-5 h-5 text-cyan-400" />
                <span className="font-bold text-white">{l.name}</span>
                <span className={cn('px-2 py-0.5 rounded text-[10px] font-bold', l.status === 'READY' ? 'bg-green-500/20 text-green-400' : 'bg-red-500/20 text-red-400')}>{l.status}</span>
                <span className="text-xs text-slate-400">{l.host}:{l.port}</span>
              </div>
              <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
                <KPI label="Current Connections" value={l.connections_current} color="text-cyan-400" />
                <KPI label="Total Connections" value={l.connections_total} />
                <KPI label="Refused" value={l.connections_refused} color={l.connections_refused > 10 ? 'text-amber-400' : 'text-green-400'} />
                <KPI label="Avg Connect Time" value={`${l.avg_connection_time_ms} ms`} />
              </div>
              <div className="mt-3">
                <p className="text-[10px] text-slate-400 uppercase mb-1">Registered Services</p>
                <div className="flex flex-wrap gap-1">
                  {(l.services_registered || []).map((s: string) => (
                    <span key={s} className="px-2 py-0.5 bg-cyan-500/10 text-cyan-400 border border-cyan-500/20 rounded text-[10px] font-mono">{s}</span>
                  ))}
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* ======== RAC TAB ======== */}
      {tab === 'rac' && rac && (
        <div className="space-y-4">
          <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
            <KPI label="Cluster" value={rac.cluster_name || '—'} color="text-cyan-400" />
            <KPI label="Nodes" value={rac.node_count || 0} />
            <KPI label="Interconnect Traffic" value={`${rac.interconnect?.total_traffic_mbps || 0} Mbps`} />
            <KPI label="IC Latency" value={`${rac.interconnect?.avg_latency_us || 0} µs`} color={(rac.interconnect?.avg_latency_us || 0) < 1000 ? 'text-green-400' : 'text-amber-400'} />
          </div>

          {/* Per-instance */}
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
            {(rac.instances || []).map((inst: any) => (
              <div key={inst.inst_id} className="bg-white/5 border border-white/10 rounded-xl p-4">
                <div className="flex items-center gap-3 mb-3">
                  <Server className="w-5 h-5 text-cyan-400" />
                  <span className="font-bold text-white">{inst.instance_name}</span>
                  <span className={cn('px-2 py-0.5 rounded text-[10px] font-bold', inst.status === 'OPEN' ? 'bg-green-500/20 text-green-400' : 'bg-red-500/20 text-red-400')}>{inst.status}</span>
                  <span className="text-xs text-slate-400">{inst.host_name}</span>
                </div>
                <div className="grid grid-cols-2 gap-2">
                  <KPI label="CPU Usage" value={`${inst.cpu_usage_pct}%`} color={inst.cpu_usage_pct > 80 ? 'text-red-400' : 'text-green-400'} />
                  <KPI label="Memory Usage" value={`${inst.memory_usage_pct}%`} color={inst.memory_usage_pct > 80 ? 'text-red-400' : 'text-green-400'} />
                  <KPI label="Active Sessions" value={inst.sessions_active} color="text-cyan-400" />
                  <KPI label="Total Sessions" value={inst.sessions_total} />
                </div>
              </div>
            ))}
          </div>

          {/* Interconnect Details */}
          {rac.interconnect && (
            <div className="bg-white/5 border border-white/10 rounded-xl p-4">
              <SectionTitle icon={Zap} title="Cache Fusion / Interconnect" />
              <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
                <KPI label="GC Blocks Received/s" value={rac.interconnect.gc_blocks_received_per_sec} />
                <KPI label="GC Blocks Served/s" value={rac.interconnect.gc_blocks_served_per_sec} />
                <KPI label="CR Blocks Received" value={rac.interconnect.gc_cr_blocks_received} />
                <KPI label="Current Blocks Received" value={rac.interconnect.gc_current_blocks_received} />
                <KPI label="Blocks Corrupted" value={rac.interconnect.global_cache_blocks_corrupted} color={rac.interconnect.global_cache_blocks_corrupted === 0 ? 'text-green-400' : 'text-red-400'} />
                <KPI label="Blocks Lost" value={rac.interconnect.global_cache_blocks_lost} color={rac.interconnect.global_cache_blocks_lost === 0 ? 'text-green-400' : 'text-red-400'} />
              </div>
            </div>
          )}

          {/* Cache Fusion Latency */}
          {rac.cache_fusion && (
            <div className="bg-white/5 border border-white/10 rounded-xl p-4">
              <SectionTitle icon={Timer} title="Cache Fusion Latency" />
              <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
                <KPI label="CR Block Receive" value={`${rac.cache_fusion.gc_cr_block_receive_time_ms} ms`} />
                <KPI label="Current Block Receive" value={`${rac.cache_fusion.gc_current_block_receive_time_ms} ms`} />
                <KPI label="CR Block Send" value={`${rac.cache_fusion.gc_cr_block_send_time_ms} ms`} />
                <KPI label="Current Block Send" value={`${rac.cache_fusion.gc_current_block_send_time_ms} ms`} />
              </div>
            </div>
          )}
        </div>
      )}

      {/* ======== CONNECTIONS TAB (V5 Prompt 14) ======== */}
      {tab === 'connections' && <ConnectionsTab onConnectionsChanged={loadConnections} />}

      {/* ======== ALERT LOG TAB (V5 Prompt 16) ======== */}
      {tab === 'alertlog' && <AlertLogTab connectionId={selectedConnId} />}

      {/* ======== TxEventQ TAB (V5 Prompt 18) ======== */}
      {tab === 'txeventq' && <TxEventQTab />}

      {/* ======== SCRAPE STATUS TAB (P7) ======== */}
      {tab === 'scrape-status' && (
        <div className="space-y-4">
          <SectionTitle icon={BarChart3} title="Scrape Status" />
          {scrapeStatus ? (
            <div className="space-y-4">
              <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
                <KPI label="Status" value={scrapeStatus.running ? 'Running' : 'Stopped'} color={scrapeStatus.running ? 'text-green-400' : 'text-red-400'} />
                <KPI label="Total Scrapes" value={scrapeStatus.total_scrapes || 0} />
                <KPI label="Total Errors" value={scrapeStatus.total_errors || 0} color={scrapeStatus.total_errors > 0 ? 'text-red-400' : 'text-green-400'} />
                <KPI label="Cache Hits" value={scrapeStatus.cache_hits || 0} />
              </div>
              <div className="bg-slate-800/60 border border-slate-700 rounded-xl overflow-hidden">
                <table className="w-full text-xs">
                  <thead><tr className="border-b border-slate-700">
                    <th className="text-left px-3 py-2 text-slate-400 font-medium">Connection</th>
                    <th className="text-left px-3 py-2 text-slate-400 font-medium">Status</th>
                    <th className="text-left px-3 py-2 text-slate-400 font-medium">Last Scrape</th>
                    <th className="text-left px-3 py-2 text-slate-400 font-medium">Metrics</th>
                    <th className="text-left px-3 py-2 text-slate-400 font-medium">Errors</th>
                    <th className="text-left px-3 py-2 text-slate-400 font-medium">Duration</th>
                  </tr></thead>
                  <tbody>
                    {(scrapeStatus.connections || []).map((c: any, i: number) => (
                      <tr key={i} className="border-b border-slate-800/50 hover:bg-slate-700/30">
                        <td className="px-3 py-2 text-white font-medium">{c.connection_id}</td>
                        <td className="px-3 py-2">
                          <span className={cn('px-1.5 py-0.5 rounded text-[9px] font-medium',
                            c.status === 'healthy' ? 'bg-green-500/20 text-green-400' : c.status === 'degraded' ? 'bg-yellow-500/20 text-yellow-400' : 'bg-red-500/20 text-red-400')}>
                            {c.status}
                          </span>
                        </td>
                        <td className="px-3 py-2 text-slate-400">{c.last_scrape || 'Never'}</td>
                        <td className="px-3 py-2 text-slate-300">{c.metrics_collected || 0}</td>
                        <td className="px-3 py-2 text-slate-300">{c.errors || 0}</td>
                        <td className="px-3 py-2 text-slate-400">{c.duration_ms || 0}ms</td>
                      </tr>
                    ))}
                    {(!scrapeStatus.connections || scrapeStatus.connections.length === 0) && (
                      <tr><td colSpan={6} className="px-3 py-6 text-center text-slate-500">No connection scrape data yet. Start the scheduler or connect an Oracle database.</td></tr>
                    )}
                  </tbody>
                </table>
              </div>
            </div>
          ) : (
            <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-8 text-center text-slate-500">
              <BarChart3 className="w-10 h-10 mx-auto mb-3 opacity-30" />
              <p>Scrape status not available. Ensure the scheduler is running.</p>
            </div>
          )}
        </div>
      )}

      {/* ======== SCHEMA OBJECTS TAB ======== */}
      {tab === 'schema-objects' && <SchemaObjectsTab connectionId={selectedConnId} />}

      {/* ======== SQL WORKSPACE TAB ======== */}
      {tab === 'sql-workspace' && (
        <SqlWorkspaceTab dbType="oracle" />
      )}

        </div>{/* end Right Content Area */}
      </div>{/* end flex sidebar+content */}
    </div>
  )
}

// ============================================================================
// Schema Objects Tab — browse Views, MVs, Functions, Procedures, etc.
// ============================================================================

const SCHEMA_SUBTABS = [
  { id: 'summary', label: 'Summary', icon: BarChart3 },
  { id: 'views', label: 'Views', icon: Eye },
  { id: 'mviews', label: 'Materialized Views', icon: Repeat },
  { id: 'functions', label: 'Functions', icon: Code },
  { id: 'procedures', label: 'Procedures', icon: Bolt },
  { id: 'packages', label: 'Packages', icon: Package },
  { id: 'triggers', label: 'Triggers', icon: Zap },
  { id: 'indexes', label: 'Indexes', icon: ListOrdered },
  { id: 'sequences', label: 'Sequences', icon: Hash },
] as const

type SchemaSubTab = typeof SCHEMA_SUBTABS[number]['id']

function SchemaObjectsTab({ connectionId }: { connectionId: string }) {
  const [subTab, setSubTab] = useState<SchemaSubTab>('summary')
  const [owner, setOwner] = useState('')
  const [owners, setOwners] = useState<string[]>([])
  const [data, setData] = useState<any>(null)
  const [loading, setLoading] = useState(false)
  const [search, setSearch] = useState('')

  // Load owners on mount
  useEffect(() => {
    const qs = connectionId ? `?connection_id=${connectionId}` : ''
    fetchJSON<any>(`/oracle/schema/owners${qs}`).then(r => {
      setOwners(r?.owners || [])
    }).catch(() => {})
  }, [connectionId])

  // Load data when subtab or owner changes
  const loadData = useCallback(async () => {
    setLoading(true)
    const params = new URLSearchParams()
    if (connectionId) params.set('connection_id', connectionId)
    if (owner) params.set('owner', owner)
    const qs = params.toString() ? `?${params}` : ''
    try {
      const endpoint = subTab === 'summary' ? 'summary' : subTab
      const result = await fetchJSON<any>(`/oracle/schema/${endpoint}${qs}`)
      setData(result)
    } catch {
      setData(null)
    } finally {
      setLoading(false)
    }
  }, [subTab, owner, connectionId])

  useEffect(() => { loadData() }, [loadData])

  const filteredRows = (rows: any[], nameKey: string) => {
    if (!search) return rows
    const q = search.toLowerCase()
    return rows.filter((r: any) =>
      (r[nameKey] || '').toLowerCase().includes(q) ||
      (r.owner || '').toLowerCase().includes(q)
    )
  }

  return (
    <div className="space-y-4">
      <SectionTitle icon={Box} title="Schema Object Browser" />

      {/* Sub-tab navigation + filters */}
      <div className="flex flex-wrap items-center gap-2">
        {SCHEMA_SUBTABS.map(st => {
          const Icon = st.icon
          return (
            <button key={st.id} onClick={() => { setSubTab(st.id); setSearch('') }}
              className={cn('flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors',
                subTab === st.id
                  ? 'bg-orange-500/20 text-orange-400 border border-orange-500/30'
                  : 'text-slate-400 hover:bg-white/5 hover:text-white border border-transparent'
              )}>
              <Icon className="w-3 h-3" /> {st.label}
            </button>
          )
        })}
      </div>

      {/* Owner filter + search */}
      <div className="flex items-center gap-3">
        <div className="flex items-center gap-2">
          <Filter className="w-3.5 h-3.5 text-slate-500" />
          <select value={owner} onChange={e => setOwner(e.target.value)}
            className="bg-slate-800/80 border border-slate-600/50 rounded px-3 py-1.5 text-xs text-white focus:outline-none focus:ring-1 focus:ring-orange-500/40">
            <option value="">All Schemas</option>
            {owners.map(o => <option key={o} value={o}>{o}</option>)}
          </select>
        </div>
        {subTab !== 'summary' && (
          <div className="flex items-center gap-2 flex-1 max-w-xs">
            <Search className="w-3.5 h-3.5 text-slate-500" />
            <input value={search} onChange={e => setSearch(e.target.value)}
              placeholder="Filter by name..."
              className="bg-slate-800/80 border border-slate-600/50 rounded px-3 py-1.5 text-xs text-white w-full focus:outline-none focus:ring-1 focus:ring-orange-500/40" />
          </div>
        )}
        <button onClick={loadData} className="flex items-center gap-1 px-3 py-1.5 bg-white/5 hover:bg-white/10 border border-white/10 rounded text-xs text-slate-300">
          <RefreshCw className={cn('w-3 h-3', loading && 'animate-spin')} /> Refresh
        </button>
      </div>

      {loading && <LoadingSkeleton variant="table" count={5} />}

      {/* ---- SUMMARY ---- */}
      {!loading && subTab === 'summary' && data?.summary && (
        <div className="space-y-4">
          <div className="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-5 gap-3">
            {Object.entries(data.summary as Record<string, number>).sort(([,a], [,b]) => (b as number) - (a as number)).map(([key, count]) => (
              <div key={key} className="bg-slate-800/60 rounded-lg border border-slate-700 p-3 cursor-pointer hover:border-orange-500/40 transition-colors"
                onClick={() => {
                  const mapping: Record<string, SchemaSubTab> = {
                    view: 'views', materialized_view: 'mviews', function: 'functions',
                    procedure: 'procedures', package: 'packages', package_body: 'packages',
                    trigger: 'triggers', index: 'indexes', sequence: 'sequences',
                  }
                  const target = mapping[key]
                  if (target) setSubTab(target)
                }}>
                <p className="text-[10px] text-slate-400 uppercase tracking-wider truncate">{key.replace(/_/g, ' ')}</p>
                <p className="text-2xl font-bold text-white mt-0.5">{(count as number).toLocaleString()}</p>
              </div>
            ))}
          </div>
          {Object.keys(data.summary).length === 0 && (
            <div className="text-center py-12 text-slate-500">
              <Box className="w-10 h-10 mx-auto mb-3 opacity-30" />
              <p>No schema objects found. Connect an Oracle instance to browse.</p>
            </div>
          )}
        </div>
      )}

      {/* ---- VIEWS ---- */}
      {!loading && subTab === 'views' && (
        <SchemaTable
          rows={filteredRows(data?.views || [], 'view_name')}
          columns={[
            { key: 'owner', label: 'Owner' },
            { key: 'view_name', label: 'View Name', mono: true },
            { key: 'text_length', label: 'Text Length', align: 'right' },
            { key: 'editioning_view', label: 'Editioning' },
          ]}
          emptyMsg="No views found"
        />
      )}

      {/* ---- MATERIALIZED VIEWS ---- */}
      {!loading && subTab === 'mviews' && (
        <SchemaTable
          rows={filteredRows(data?.mviews || [], 'mview_name')}
          columns={[
            { key: 'owner', label: 'Owner' },
            { key: 'mview_name', label: 'MView Name', mono: true },
            { key: 'refresh_mode', label: 'Refresh Mode' },
            { key: 'refresh_method', label: 'Method' },
            { key: 'last_refresh_date', label: 'Last Refresh' },
            { key: 'staleness', label: 'Staleness', badge: true },
            { key: 'compile_state', label: 'State', badge: true },
          ]}
          emptyMsg="No materialized views found"
        />
      )}

      {/* ---- FUNCTIONS ---- */}
      {!loading && subTab === 'functions' && (
        <SchemaTable
          rows={filteredRows(data?.functions || [], 'object_name')}
          columns={[
            { key: 'owner', label: 'Owner' },
            { key: 'object_name', label: 'Function Name', mono: true },
            { key: 'status', label: 'Status', badge: true },
            { key: 'created', label: 'Created' },
            { key: 'last_ddl_time', label: 'Last DDL' },
          ]}
          emptyMsg="No functions found"
        />
      )}

      {/* ---- PROCEDURES ---- */}
      {!loading && subTab === 'procedures' && (
        <SchemaTable
          rows={filteredRows(data?.procedures || [], 'object_name')}
          columns={[
            { key: 'owner', label: 'Owner' },
            { key: 'object_name', label: 'Procedure Name', mono: true },
            { key: 'status', label: 'Status', badge: true },
            { key: 'created', label: 'Created' },
            { key: 'last_ddl_time', label: 'Last DDL' },
          ]}
          emptyMsg="No procedures found"
        />
      )}

      {/* ---- PACKAGES ---- */}
      {!loading && subTab === 'packages' && (
        <SchemaTable
          rows={filteredRows(data?.packages || [], 'object_name')}
          columns={[
            { key: 'owner', label: 'Owner' },
            { key: 'object_name', label: 'Package Name', mono: true },
            { key: 'object_type', label: 'Type' },
            { key: 'status', label: 'Status', badge: true },
            { key: 'created', label: 'Created' },
            { key: 'last_ddl_time', label: 'Last DDL' },
          ]}
          emptyMsg="No packages found"
        />
      )}

      {/* ---- TRIGGERS ---- */}
      {!loading && subTab === 'triggers' && (
        <SchemaTable
          rows={filteredRows(data?.triggers || [], 'trigger_name')}
          columns={[
            { key: 'owner', label: 'Owner' },
            { key: 'trigger_name', label: 'Trigger Name', mono: true },
            { key: 'trigger_type', label: 'Type' },
            { key: 'triggering_event', label: 'Event' },
            { key: 'table_name', label: 'Table' },
            { key: 'status', label: 'Status', badge: true },
          ]}
          emptyMsg="No triggers found"
        />
      )}

      {/* ---- INDEXES ---- */}
      {!loading && subTab === 'indexes' && (
        <SchemaTable
          rows={filteredRows(data?.indexes || [], 'index_name')}
          columns={[
            { key: 'owner', label: 'Owner' },
            { key: 'index_name', label: 'Index Name', mono: true },
            { key: 'index_type', label: 'Type' },
            { key: 'table_name', label: 'Table' },
            { key: 'uniqueness', label: 'Unique' },
            { key: 'status', label: 'Status', badge: true },
            { key: 'num_rows', label: 'Rows', align: 'right' },
            { key: 'last_analyzed', label: 'Last Analyzed' },
          ]}
          emptyMsg="No indexes found"
        />
      )}

      {/* ---- SEQUENCES ---- */}
      {!loading && subTab === 'sequences' && (
        <SchemaTable
          rows={filteredRows(data?.sequences || [], 'sequence_name')}
          columns={[
            { key: 'owner', label: 'Owner' },
            { key: 'sequence_name', label: 'Sequence Name', mono: true },
            { key: 'min_value', label: 'Min', align: 'right' },
            { key: 'max_value', label: 'Max', align: 'right' },
            { key: 'increment_by', label: 'Increment', align: 'right' },
            { key: 'last_number', label: 'Last Number', align: 'right' },
            { key: 'cache_size', label: 'Cache', align: 'right' },
            { key: 'cycle_flag', label: 'Cycle' },
            { key: 'order_flag', label: 'Order' },
          ]}
          emptyMsg="No sequences found"
        />
      )}
    </div>
  )
}

interface SchemaColDef {
  key: string
  label: string
  mono?: boolean
  align?: 'left' | 'right' | 'center'
  badge?: boolean
}

function SchemaTable({ rows, columns, emptyMsg }: { rows: any[]; columns: SchemaColDef[]; emptyMsg: string }) {
  if (rows.length === 0) {
    return (
      <div className="text-center py-12 text-slate-500">
        <Box className="w-10 h-10 mx-auto mb-3 opacity-30" />
        <p className="text-sm">{emptyMsg}</p>
      </div>
    )
  }

  return (
    <div className="bg-white/5 border border-white/10 rounded-xl overflow-hidden overflow-x-auto">
      <table className="w-full text-xs">
        <thead>
          <tr className="bg-slate-800/80 border-b border-white/10">
            {columns.map(col => (
              <th key={col.key} className={cn('px-3 py-2 text-slate-400 uppercase font-medium',
                col.align === 'right' ? 'text-right' : col.align === 'center' ? 'text-center' : 'text-left'
              )}>{col.label}</th>
            ))}
          </tr>
        </thead>
        <tbody className="divide-y divide-white/5">
          {rows.map((row: any, i: number) => (
            <tr key={i} className="hover:bg-white/5">
              {columns.map(col => {
                const val = row[col.key] ?? ''
                const isValid = val === 'VALID' || val === 'ENABLED' || val === 'FRESH'
                const isInvalid = val === 'INVALID' || val === 'DISABLED' || val === 'STALE' || val === 'NEEDS_COMPILE'
                return (
                  <td key={col.key} className={cn('px-3 py-2',
                    col.align === 'right' ? 'text-right' : col.align === 'center' ? 'text-center' : 'text-left',
                    col.mono ? 'font-mono text-white font-medium' : 'text-slate-300'
                  )}>
                    {col.badge ? (
                      <span className={cn('px-1.5 py-0.5 rounded text-[10px] font-bold',
                        isValid ? 'bg-green-500/20 text-green-400' :
                        isInvalid ? 'bg-red-500/20 text-red-400' :
                        'bg-slate-500/20 text-slate-400'
                      )}>{val}</span>
                    ) : val}
                  </td>
                )
              })}
            </tr>
          ))}
        </tbody>
      </table>
      <div className="px-3 py-2 text-[10px] text-slate-500 border-t border-white/5">
        {rows.length} object{rows.length !== 1 ? 's' : ''} found
      </div>
    </div>
  )
}

function AddInstanceModal({ open, onClose, onCreated }: { open: boolean; onClose: () => void; onCreated: () => void }) {
  const [submitting, setSubmitting] = useState(false)
  if (!open) return null

  const handleSubmit = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault()
    setSubmitting(true)
    try {
      const fd = new FormData(e.currentTarget)
      await createOracleConnection({
        name: fd.get('name'), host: fd.get('host'),
        port: Number(fd.get('port')) || 1521,
        service_name: fd.get('service_name') || 'ORCL',
        username: fd.get('username') || 'system',
        password: fd.get('password') || '',
        auth_mode: fd.get('auth_mode') || 'password',
        role: fd.get('role') || 'DEFAULT',
        pool_min: Number(fd.get('pool_min')) || 2,
        pool_max: Number(fd.get('pool_max')) || 10,
      })
      onCreated()
      onClose()
    } finally { setSubmitting(false) }
  }

  const inputCls = 'w-full bg-slate-800/80 border border-slate-600/50 rounded-lg px-3.5 py-2.5 text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-orange-500/40 focus:border-orange-500/50 transition-all'
  const labelCls = 'block text-xs font-medium text-slate-400 mb-1.5'
  const selectCls = 'w-full bg-slate-800/80 border border-slate-600/50 rounded-lg px-3.5 py-2.5 text-sm text-white focus:outline-none focus:ring-2 focus:ring-orange-500/40 focus:border-orange-500/50 transition-all appearance-none'

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center p-4">
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" />
      <div className="relative w-full max-w-lg bg-gradient-to-b from-slate-800 to-slate-900 border border-slate-700/60 rounded-2xl shadow-2xl shadow-black/50 overflow-hidden">
        {/* Header */}
        <div className="px-6 pt-6 pb-4 border-b border-slate-700/40">
          <div className="flex items-center gap-3 mb-1">
            <div className="p-2 bg-orange-500/10 rounded-lg">
              <Database className="w-5 h-5 text-orange-400" />
            </div>
            <div>
              <h2 className="text-lg font-semibold text-white">Add Oracle Instance</h2>
              <p className="text-xs text-slate-400">Configure a new Oracle database connection for monitoring</p>
            </div>
          </div>
        </div>

        {/* Form */}
        <form onSubmit={handleSubmit} className="px-6 py-5 space-y-5 max-h-[70vh] overflow-y-auto">
          {/* Instance Details */}
          <div>
            <h3 className="text-xs font-semibold text-slate-300 uppercase tracking-wider mb-3 flex items-center gap-2">
              <Server className="w-3.5 h-3.5 text-orange-400" /> Instance Details
            </h3>
            <div className="grid grid-cols-2 gap-3">
              <div className="col-span-2">
                <label className={labelCls}>Instance Name *</label>
                <input name="name" placeholder="e.g. Production XEPDB1" required className={inputCls} />
              </div>
              <div>
                <label className={labelCls}>Host *</label>
                <input name="host" placeholder="e.g. 10.88.218.59" required className={inputCls} />
              </div>
              <div>
                <label className={labelCls}>Port</label>
                <input name="port" placeholder="1521" type="number" defaultValue={1521} className={inputCls} />
              </div>
              <div className="col-span-2">
                <label className={labelCls}>Service Name / SID</label>
                <input name="service_name" placeholder="e.g. XEPDB1" defaultValue="ORCL" className={inputCls} />
              </div>
            </div>
          </div>

          {/* Authentication */}
          <div>
            <h3 className="text-xs font-semibold text-slate-300 uppercase tracking-wider mb-3 flex items-center gap-2">
              <Shield className="w-3.5 h-3.5 text-cyan-400" /> Authentication
            </h3>
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className={labelCls}>Username</label>
                <input name="username" placeholder="system" defaultValue="system" className={inputCls} />
              </div>
              <div>
                <label className={labelCls}>Password</label>
                <input name="password" placeholder="••••••••" type="password" className={inputCls} />
              </div>
              <div>
                <label className={labelCls}>Role</label>
                <select name="role" className={selectCls}>
                  <option value="DEFAULT">DEFAULT</option>
                  <option value="SYSDBA">SYSDBA</option>
                  <option value="SYSOPER">SYSOPER</option>
                </select>
              </div>
              <div>
                <label className={labelCls}>Auth Mode</label>
                <select name="auth_mode" className={selectCls}>
                  <option value="password">Password</option>
                  <option value="wallet">Oracle Wallet</option>
                  <option value="external">External Auth</option>
                  <option value="vault">HashiCorp Vault</option>
                </select>
              </div>
            </div>
          </div>

          {/* Connection Pool */}
          <div>
            <h3 className="text-xs font-semibold text-slate-300 uppercase tracking-wider mb-3 flex items-center gap-2">
              <Activity className="w-3.5 h-3.5 text-green-400" /> Connection Pool
            </h3>
            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className={labelCls}>Min Connections</label>
                <input name="pool_min" type="number" defaultValue={2} min={1} className={inputCls} />
              </div>
              <div>
                <label className={labelCls}>Max Connections</label>
                <input name="pool_max" type="number" defaultValue={10} min={1} className={inputCls} />
              </div>
            </div>
          </div>

          {/* Actions */}
          <div className="flex items-center justify-end gap-3 pt-2 border-t border-slate-700/40">
            <button type="button" onClick={onClose} className="px-4 py-2 text-sm text-slate-400 hover:text-white border border-slate-600/50 rounded-lg hover:bg-slate-700/50 transition-all">
              Cancel
            </button>
            <button type="submit" disabled={submitting}
              className="px-5 py-2 text-sm font-medium text-white bg-gradient-to-r from-orange-500 to-orange-600 rounded-lg hover:from-orange-600 hover:to-orange-700 shadow-lg shadow-orange-500/20 disabled:opacity-50 transition-all flex items-center gap-2">
              {submitting ? <><RefreshCw className="w-3.5 h-3.5 animate-spin" /> Adding...</> : <><Plus className="w-3.5 h-3.5" /> Add Instance</>}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}

function ConnectionsTab({ onConnectionsChanged }: { onConnectionsChanged?: () => void }) {
  const [conns, setConns] = useState<any[]>([])
  const [showAdd, setShowAdd] = useState(false)
  const [testing, setTesting] = useState<string | null>(null)
  const [testResults, setTestResults] = useState<Record<string, any>>({})

  const load = useCallback(async () => {
    const data = await getOracleConnections().catch(() => [])
    setConns(data)
  }, [])
  useEffect(() => { load() }, [load])

  const handleTest = async (id: string) => {
    setTesting(id)
    const res = await testOracleConnection(id).catch(() => ({ success: false, error: 'Request failed' }))
    setTestResults(prev => ({ ...prev, [id]: res }))
    setTesting(null); load()
  }

  const handleDelete = async (id: string) => { await deleteOracleConnection(id); load(); onConnectionsChanged?.() }

  const statusIcon = (s: string) => s === 'connected' ? <CheckCircle className="w-4 h-4 text-green-400" /> : s === 'error' ? <XCircle className="w-4 h-4 text-red-400" /> : <RefreshCw className="w-4 h-4 text-yellow-400" />
  const statusColor: Record<string, string> = { connected: 'border-green-500/30 bg-green-500/5', pending: 'border-yellow-500/30 bg-yellow-500/5', error: 'border-red-500/30 bg-red-500/5' }

  return (
    <div className="space-y-4">
      <AddInstanceModal open={showAdd} onClose={() => setShowAdd(false)} onCreated={() => { load(); onConnectionsChanged?.() }} />

      <div className="flex justify-between items-center">
        <SectionTitle icon={Database} title="Oracle Instances" />
        <button onClick={() => setShowAdd(true)} className="flex items-center gap-2 px-4 py-2 bg-gradient-to-r from-orange-500 to-orange-600 hover:from-orange-600 hover:to-orange-700 text-white rounded-lg text-sm font-medium shadow-lg shadow-orange-500/20 transition-all">
          <Plus className="w-4 h-4" /> Add Instance
        </button>
      </div>

      <div className="grid gap-3">
        {conns.map((c: any) => (
          <div key={c.id} className={cn('border rounded-xl p-4 transition-all hover:shadow-lg hover:shadow-black/20', statusColor[c.status] || 'border-slate-700/50 bg-slate-800/30')}>
            <div className="flex items-center justify-between mb-3">
              <div className="flex items-center gap-3">
                {statusIcon(c.status)}
                <div>
                  <h4 className="text-sm font-semibold text-white">{c.name}</h4>
                  <p className="text-xs text-slate-400 mt-0.5">{c.host}:{c.port}/{c.service_name}</p>
                </div>
              </div>
              <div className="flex items-center gap-2">
                <span className={cn('px-2.5 py-1 rounded-full text-[10px] font-medium uppercase tracking-wide',
                  c.status === 'connected' ? 'bg-green-500/15 text-green-400 ring-1 ring-green-500/20' :
                  c.status === 'error' ? 'bg-red-500/15 text-red-400 ring-1 ring-red-500/20' :
                  'bg-yellow-500/15 text-yellow-400 ring-1 ring-yellow-500/20'
                )}>{c.status}</span>
              </div>
            </div>

            <div className="grid grid-cols-3 md:grid-cols-6 gap-x-4 gap-y-1 mb-3 text-[11px]">
              <div><span className="text-slate-500">User:</span> <span className="text-slate-300">{c.username}</span></div>
              <div><span className="text-slate-500">Role:</span> <span className="text-slate-300">{c.role}</span></div>
              <div><span className="text-slate-500">Auth:</span> <span className="text-slate-300">{c.auth_mode}</span></div>
              <div><span className="text-slate-500">Pool:</span> <span className="text-slate-300">{c.pool_min}–{c.pool_max}</span></div>
              <div><span className="text-slate-500">Timeout:</span> <span className="text-slate-300">{c.query_timeout_secs}s</span></div>
              <div><span className="text-slate-500">Last Ping:</span> <span className="text-slate-300">{c.last_ping ? new Date(c.last_ping).toLocaleTimeString() : '—'}</span></div>
            </div>

            {c.last_error && <div className="text-xs text-red-400 bg-red-500/10 border border-red-500/20 rounded-lg px-3 py-2 mb-3">{c.last_error}</div>}

            {testResults[c.id] && (
              <div className={cn('flex items-center gap-2 text-xs p-2.5 rounded-lg mb-3',
                testResults[c.id].success ? 'bg-green-500/10 text-green-400 border border-green-500/20' : 'bg-red-500/10 text-red-400 border border-red-500/20')}>
                {testResults[c.id].success ? <CheckCircle className="w-3.5 h-3.5" /> : <XCircle className="w-3.5 h-3.5" />}
                {testResults[c.id].success ? `Connected (${testResults[c.id].latency_ms}ms) — ${testResults[c.id].version}` : testResults[c.id].error}
              </div>
            )}

            <div className="flex gap-2 pt-1">
              <button onClick={() => handleTest(c.id)} disabled={testing === c.id}
                className="px-3 py-1.5 text-xs font-medium bg-cyan-500/10 text-cyan-400 border border-cyan-500/20 rounded-lg hover:bg-cyan-500/20 disabled:opacity-50 transition-all flex items-center gap-1.5">
                {testing === c.id ? <><RefreshCw className="w-3 h-3 animate-spin" /> Testing...</> : <><Zap className="w-3 h-3" /> Test Connection</>}
              </button>
              <button onClick={() => handleDelete(c.id)}
                className="px-3 py-1.5 text-xs font-medium bg-red-500/10 text-red-400 border border-red-500/20 rounded-lg hover:bg-red-500/20 transition-all flex items-center gap-1.5">
                <Trash2 className="w-3 h-3" /> Remove
              </button>
            </div>
          </div>
        ))}
        {conns.length === 0 && (
          <div className="text-center py-16 border border-dashed border-slate-700/50 rounded-xl">
            <Database className="w-10 h-10 text-slate-600 mx-auto mb-3" />
            <p className="text-sm text-slate-400 mb-1">No Oracle instances configured</p>
            <p className="text-xs text-slate-500 mb-4">Add your first Oracle database to start monitoring</p>
            <button onClick={() => setShowAdd(true)} className="px-4 py-2 text-xs font-medium text-white bg-gradient-to-r from-orange-500 to-orange-600 rounded-lg hover:from-orange-600 hover:to-orange-700 transition-all">
              <Plus className="w-3.5 h-3.5 inline mr-1" /> Add Oracle Instance
            </button>
          </div>
        )}
      </div>
    </div>
  )
}

function AlertLogTab({ connectionId }: { connectionId: string }) {
  const [entries, setEntries] = useState<any[]>([])
  const [stats, setStats] = useState<any>(null)
  const [levelFilter, setLevelFilter] = useState<string>('')

  const load = useCallback(async () => {
    const params: Record<string, string> = {}
    if (levelFilter) params.level = levelFilter
    if (connectionId) params.connection_id = connectionId
    const [logData, statsData] = await Promise.all([
      getOracleAlertLog(Object.keys(params).length ? params : undefined).catch(() => ({ entries: [] })),
      getOracleAlertLogStats(connectionId || undefined).catch(() => null),
    ])
    setEntries(logData.entries || [])
    setStats(statsData)
  }, [levelFilter, connectionId])
  useEffect(() => { load() }, [load])

  const levelColor: Record<string, string> = { critical: 'bg-red-500/20 text-red-400', warning: 'bg-yellow-500/20 text-yellow-400', info: 'bg-blue-500/20 text-blue-400', trace: 'bg-gray-500/20 text-gray-400' }

  return (
    <div className="space-y-4">
      <SectionTitle icon={FileText} title="Oracle Alert Log" />
      {stats && (
        <div className="grid grid-cols-2 md:grid-cols-5 gap-3">
          <KPI label="Total Entries" value={stats.total || 0} />
          <KPI label="Critical" value={stats.critical_count || 0} color="text-red-400" />
          <KPI label="Warnings" value={stats.warning_count || 0} color="text-yellow-400" />
          <KPI label="Last 24h" value={stats.last_24h || 0} />
          <KPI label="Databases" value={stats.by_database ? Object.keys(stats.by_database).length : 0} />
        </div>
      )}
      <div className="flex gap-1">
        {['', 'critical', 'warning', 'info'].map(lv => (
          <button key={lv} onClick={() => setLevelFilter(lv)} className={cn('px-3 py-1 rounded text-xs capitalize', levelFilter === lv ? 'bg-cyan-600 text-white' : 'bg-white/5 text-slate-400 hover:text-white')}>
            {lv || 'All'}
          </button>
        ))}
      </div>
      <div className="overflow-x-auto">
        <table className="w-full text-xs">
          <thead><tr className="text-slate-500 border-b border-white/10">
            <th className="text-left p-2">Time</th><th className="text-left p-2">Level</th><th className="text-left p-2">Component</th><th className="text-left p-2">ORA Code</th><th className="text-left p-2">Message</th>
          </tr></thead>
          <tbody>
            {entries.map((e: any) => (
              <tr key={e.id} className="border-b border-white/5 hover:bg-white/5">
                <td className="p-2 text-slate-400 whitespace-nowrap">{new Date(e.timestamp).toLocaleTimeString()}</td>
                <td className="p-2"><span className={cn('px-1.5 py-0.5 rounded text-[10px]', levelColor[e.level] || '')}>{e.level}</span></td>
                <td className="p-2 text-slate-300">{e.component}</td>
                <td className="p-2 text-orange-400 font-mono">{e.ora_code || '—'}</td>
                <td className="p-2 text-white max-w-md truncate">{e.message}</td>
              </tr>
            ))}
          </tbody>
        </table>
        {entries.length === 0 && <div className="text-center text-slate-500 py-6 text-sm">No alert log entries</div>}
      </div>
    </div>
  )
}

function TxEventQTab() {
  const [queues, setQueues] = useState<any[]>([])
  const [stats, setStats] = useState<any>(null)

  const load = useCallback(async () => {
    const [qData, sData] = await Promise.all([
      getOracleTxEventQueues().catch(() => ({ queues: [] })),
      getOracleTxEventQueueStats().catch(() => null),
    ])
    setQueues(qData.queues || [])
    setStats(sData)
  }, [])
  useEffect(() => { load() }, [load])

  const statusColor: Record<string, string> = { active: 'bg-green-500/20 text-green-400', backlog: 'bg-yellow-500/20 text-yellow-400', disabled: 'bg-gray-500/20 text-gray-400' }

  return (
    <div className="space-y-4">
      <SectionTitle icon={Inbox} title="Transactional Event Queues (TxEventQ)" />
      {stats && (
        <div className="grid grid-cols-2 md:grid-cols-6 gap-3">
          <KPI label="Total Queues" value={stats.total_queues || 0} />
          <KPI label="Active" value={stats.active_queues || 0} color="text-green-400" />
          <KPI label="Messages Ready" value={stats.total_ready || 0} />
          <KPI label="Processed" value={stats.total_processed || 0} />
          <KPI label="Avg Latency" value={`${stats.avg_latency_ms || 0}ms`} />
          <KPI label="Backlog Alerts" value={stats.backlog_alert_count || 0} color={stats.backlog_alert_count > 0 ? 'text-yellow-400' : 'text-green-400'} />
        </div>
      )}
      <div className="space-y-3">
        {queues.map((q: any) => (
          <div key={q.queue_name} className="bg-white/[0.02] border border-white/10 rounded-xl p-4">
            <div className="flex items-center justify-between mb-2">
              <div className="flex items-center gap-2">
                <Inbox className="w-4 h-4 text-cyan-400" />
                <span className="text-sm font-medium text-white">{q.queue_name}</span>
                <span className="text-[9px] text-slate-500">{q.owner} · {q.queue_type}</span>
              </div>
              <span className={cn('px-2 py-0.5 rounded text-[10px]', statusColor[q.status] || 'bg-gray-500/20 text-gray-400')}>{q.status}</span>
            </div>
            <div className="grid grid-cols-2 md:grid-cols-5 gap-2 text-[10px]">
              <div><span className="text-slate-500">Ready:</span> <span className="text-white">{(q.messages_ready || 0).toLocaleString()}</span></div>
              <div><span className="text-slate-500">Processed:</span> <span className="text-white">{(q.messages_processed || 0).toLocaleString()}</span></div>
              <div><span className="text-slate-500">Expired:</span> <span className="text-white">{q.messages_expired || 0}</span></div>
              <div><span className="text-slate-500">Subscribers:</span> <span className="text-white">{q.subscribers || 0}</span></div>
              <div><span className="text-slate-500">Throughput:</span> <span className="text-white">{q.throughput_per_sec || 0}/s</span></div>
            </div>
            <div className="grid grid-cols-2 md:grid-cols-4 gap-2 mt-1 text-[10px]">
              <div><span className="text-slate-500">Avg Latency:</span> <span className="text-white">{q.avg_latency_ms || 0}ms</span></div>
              <div><span className="text-slate-500">Oldest Msg:</span> <span className="text-white">{q.oldest_message_age_secs || 0}s</span></div>
              <div><span className="text-slate-500">Enqueue:</span> <span className={q.enqueue_enabled ? 'text-green-400' : 'text-red-400'}>{q.enqueue_enabled ? 'ON' : 'OFF'}</span></div>
              <div><span className="text-slate-500">Dequeue:</span> <span className={q.dequeue_enabled ? 'text-green-400' : 'text-red-400'}>{q.dequeue_enabled ? 'ON' : 'OFF'}</span></div>
            </div>
          </div>
        ))}
        {queues.length === 0 && <div className="text-sm text-slate-500 text-center py-8">No TxEventQ queues found</div>}
      </div>
    </div>
  )
}
