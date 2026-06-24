import { useState, useCallback, useEffect } from 'react'
import TrafficChart from '@/components/charts/TrafficChart'
import DonutChart from '@/components/charts/DonutChart'
import Sparkline from '@/components/charts/Sparkline'
import PageHeader from '@/components/PageHeader'
import DrilldownModal, { type DrilldownNode } from '@/components/DrilldownModal'
import {
  Network as NetworkIcon, Wifi, ArrowUpRight, ArrowDownRight, Zap, Radio,
  Star, StarOff, Ban, Eye, Server, Monitor, ArrowRightLeft, Cpu, Filter, Play, Square,
  ChevronDown, Download, Upload, FileDown, Trash2, Bell,
} from 'lucide-react'
import {
  getNetworkAdapters, getNetworkPrograms,
  getNetworkCapture, startCapture, stopCapture,
  getNetworkStats, getTrafficChart, getProtocolBreakdown,
  getNetworkServiceBreakdown, getCountryBreakdown, getPaginatedConnections,
  startPcapExport, stopPcapExport, listBlacklist, addBlacklistIP, removeBlacklistIP,
  getNotificationLog,
  getNetworkInterfaces, getProtocolStats, getSocketStats, getNetworkContainers, getNetworkHosts, getNetworkDevices, getNetworkDeviceDetail,
  type NetworkAdapter, type NetworkProgram, type NetworkStats, type NetworkDeviceDetail,
  type TrafficChartPoint, type ProtocolBreakdown as ProtocolBreakdownType,
  type ServiceBreakdownEntry, type CountryBreakdownEntry, type PaginatedConnections,
  type BlacklistEntryType, type NotificationLogEntry,
  type NetworkInterface, type ProtocolStat, type SocketStats, type ContainerNetInfo, type NetworkDeviceStatus,
} from '@/lib/api'
import AddNetworkDeviceModal from '@/components/AddNetworkDeviceModal'

// M10: Flat sparkline from live value — no synthetic Math.sin generation
const spark = (base: number, len = 20) =>
  Array.from({ length: len }, () => Math.max(0, base))

const fmt = (bytes: number) => {
  if (bytes > 1e9) return `${(bytes / 1e9).toFixed(1)} GB`
  if (bytes > 1e6) return `${(bytes / 1e6).toFixed(1)} MB`
  if (bytes > 1e3) return `${(bytes / 1e3).toFixed(1)} KB`
  return `${bytes} B`
}

type Tab = 'overview' | 'connections' | 'hosts' | 'devices' | 'blacklist' | 'programs' | 'notifications' | 'interfaces' | 'protocol-stats' | 'sockets' | 'containers'

export default function NetworkPage() {
  const [tab, setTab] = useState<Tab>('overview')
  const [search, setSearch] = useState('')
  const [protoFilter, setProtoFilter] = useState('')
  const [showFavOnly, setShowFavOnly] = useState(false)
  const [selectedConn, setSelectedConn] = useState<Record<string, any> | null>(null)
  const [drilldown, setDrilldown] = useState<{ open: boolean; title: string; nodes: DrilldownNode[] }>({ open: false, title: '', nodes: [] })
  const [showAddDevice, setShowAddDevice] = useState(false)
  const [timeRange, setTimeRange] = useState('1h')

  // S3: Adapter selection + BPF filter state
  const [adapters, setAdapters] = useState<NetworkAdapter[]>([])
  const [selectedAdapter, setSelectedAdapter] = useState('')
  const [bpfFilter, setBpfFilter] = useState('')
  const [showBpfInput, setShowBpfInput] = useState(false)
  const [isCapturing, setIsCapturing] = useState(false)
  const [captureError, setCaptureError] = useState('')

  // S2: Programs data
  const [programs, setPrograms] = useState<NetworkProgram[]>([])

  // S4: Real stats data from API
  const [stats, setStats] = useState<NetworkStats | null>(null)
  const [apiTrafficData, setApiTrafficData] = useState<TrafficChartPoint[]>([])
  const [protocolData, setProtocolData] = useState<ProtocolBreakdownType[]>([])
  const [serviceData, setServiceData] = useState<ServiceBreakdownEntry[]>([])
  const [countryData, setCountryData] = useState<CountryBreakdownEntry[]>([])
  const [paginatedConns, setPaginatedConns] = useState<PaginatedConnections | null>(null)
  const [connPage, setConnPage] = useState(1)
  const [dataRepr, setDataRepr] = useState<'bytes' | 'bits' | 'packets'>('bytes')

  // S5: PCAP export state
  const [isExporting, setIsExporting] = useState(false)
  const [exportFileName, setExportFileName] = useState('')

  // S8: Blacklist from API
  const [blacklistData, setBlacklistData] = useState<BlacklistEntryType[]>([])
  const [newBlacklistIP, setNewBlacklistIP] = useState('')
  const [newBlacklistReason, setNewBlacklistReason] = useState('')

  // S9: Notifications
  const [notifLog, setNotifLog] = useState<NotificationLogEntry[]>([])

  // P3: Interfaces, Protocol Stats, Sockets
  const [interfaces, setInterfaces] = useState<NetworkInterface[]>([])
  const [protoStats, setProtoStats] = useState<ProtocolStat[]>([])
  const [socketData, setSocketData] = useState<SocketStats | null>(null)

  // P17: Containers
  const [containers, setContainers] = useState<ContainerNetInfo[]>([])
  const [networkHosts, setNetworkHosts] = useState<any[]>([])

  // Network Devices (registered SNMP targets)
  const [networkDevices, setNetworkDevices] = useState<NetworkDeviceStatus[]>([])
  const [devicesLoading, setDevicesLoading] = useState(false)
  const [devicesLoaded, setDevicesLoaded] = useState(false)

  // Device detail drill-down
  const [selectedDevice, setSelectedDevice] = useState<NetworkDeviceDetail | null>(null)
  const [deviceDetailLoading, setDeviceDetailLoading] = useState(false)

  // Load all data
  const loadAllData = useCallback(() => {
    getNetworkAdapters().then(setAdapters).catch(() => {})
    getNetworkPrograms().then(setPrograms).catch(() => {})
    getNetworkStats().then(setStats).catch(() => {})
    getTrafficChart(timeRange, dataRepr).then(setApiTrafficData).catch(() => {})
    getProtocolBreakdown().then(setProtocolData).catch(() => {})
    getNetworkServiceBreakdown().then(setServiceData).catch(() => {})
    getCountryBreakdown().then(setCountryData).catch(() => {})
    getPaginatedConnections({ page: String(connPage), limit: '30', proto: protoFilter, search }).then(setPaginatedConns).catch(() => {})
    listBlacklist().then(setBlacklistData).catch(() => {})
    getNotificationLog().then(setNotifLog).catch(() => {})
    getNetworkInterfaces().then(setInterfaces).catch(() => {})
    getProtocolStats().then(setProtoStats).catch(() => {})
    getSocketStats().then(setSocketData).catch(() => {})
    getNetworkContainers().then(setContainers).catch(() => {})
    getNetworkHosts().then(setNetworkHosts).catch(() => {})
  }, [timeRange, dataRepr, connPage, protoFilter, search])

  // Devices loaded separately (SNMP polling is slow, don't refresh every 5s)
  const loadDevices = useCallback(() => {
    setDevicesLoading(true)
    getNetworkDevices().then(d => { setNetworkDevices(d); setDevicesLoading(false); setDevicesLoaded(true) }).catch(() => setDevicesLoading(false))
  }, [])

  // Load devices only when Devices tab is clicked for the first time (lazy + persistent)
  useEffect(() => {
    if (tab === 'devices' && !devicesLoaded && !devicesLoading) {
      loadDevices()
    }
  }, [tab, devicesLoaded, devicesLoading, loadDevices])

  // Load device detail when a device row is clicked
  const openDeviceDetail = useCallback((targetId: string) => {
    setDeviceDetailLoading(true)
    setSelectedDevice(null)
    getNetworkDeviceDetail(targetId).then(d => { setSelectedDevice(d); setDeviceDetailLoading(false) }).catch(() => setDeviceDetailLoading(false))
  }, [])

  // Load on mount + auto-refresh every 5s
  useEffect(() => {
    getNetworkCapture().then(cap => {
      setIsCapturing(cap.is_running)
      setSelectedAdapter(cap.interface_name)
    }).catch(() => {})
    loadAllData()
    const interval = setInterval(loadAllData, 5000)
    return () => clearInterval(interval)
  }, [loadAllData])

  const handleStartCapture = async () => {
    try {
      setCaptureError('')
      await startCapture(selectedAdapter || 'eth0')
      setIsCapturing(true)
    } catch (e: any) {
      setCaptureError(e.message || 'Failed to start capture')
    }
  }

  const handleStopCapture = async () => {
    try {
      await stopCapture()
      setIsCapturing(false)
    } catch (e: any) {
      setCaptureError(e.message || 'Failed to stop capture')
    }
  }

  const handleRefresh = useCallback(() => {
    loadAllData()
  }, [loadAllData])

  // Use real paginated connections from API; fall back to empty array
  const liveConnections = (paginatedConns?.connections ?? []).map(c => ({
    ...c,
    id: `${c.src_ip}:${c.src_port}-${c.dst_ip}:${c.dst_port}`,
    service: c.service ?? '',
    domain: c.country_code ?? '',
    asn: '' as string,
    country: c.country_code ?? '',
    is_favorite: c.is_favorite ?? false,
    is_blacklisted: c.is_blacklisted ?? false,
  }))
  const liveHosts = networkHosts ?? []
  const liveTrafficFallback: { time: string; inbound: number; outbound: number }[] = []

  const filteredConns = liveConnections.filter(c => {
    if (search && !`${c.src_ip} ${c.dst_ip} ${c.domain} ${c.service}`.toLowerCase().includes(search.toLowerCase())) return false
    if (protoFilter && c.protocol !== protoFilter) return false
    if (showFavOnly && !c.is_favorite) return false
    return true
  })

  const blacklisted = liveConnections.filter(c => c.is_blacklisted)

  const openKPIDrilldown = (label: string, nodes: DrilldownNode[]) => {
    setDrilldown({ open: true, title: label, nodes })
  }

  const bl = blacklisted.length

  const inboundVal = stats
    ? (stats.bps_in > 0 ? `${fmt(Math.round(stats.bps_in))}/s` : fmt(stats.total_bytes_in))
    : '—'
  const outboundVal = stats
    ? (stats.bps_out > 0 ? `${fmt(Math.round(stats.bps_out))}/s` : fmt(stats.total_bytes_out))
    : '—'
  const ppsVal = stats
    ? (stats.capture_duration_secs > 0
        ? Math.round((stats.total_packets_in + stats.total_packets_out) / stats.capture_duration_secs)
        : 0)
    : 0
  const ppsDisplay = ppsVal > 1000 ? `${(ppsVal / 1000).toFixed(1)}K` : String(ppsVal)

  const kpis = [
    { label: 'Inbound', value: inboundVal, icon: ArrowDownRight, color: 'text-blue-400', sparkData: spark(stats?.bps_in ?? 0), drill: [] },
    { label: 'Outbound', value: outboundVal, icon: ArrowUpRight, color: 'text-amber-400', sparkData: spark(stats?.bps_out ?? 0), drill: [] },
    { label: 'Packets/s', value: ppsDisplay, icon: Zap, color: 'text-purple-400', sparkData: spark(ppsVal), drill: [] },
    { label: 'Connections', value: String(stats?.active_connections ?? paginatedConns?.total ?? liveConnections.length), icon: Wifi, color: 'text-cyan-400', sparkData: spark(stats?.active_connections ?? 0), drill: [] },
    { label: 'Hosts', value: String(stats?.unique_src_hosts ?? liveHosts.length), icon: Monitor, color: 'text-emerald-400', sparkData: spark(stats?.unique_src_hosts ?? 0), drill: [] },
    { label: 'Blacklisted', value: String(bl), icon: Ban, color: bl > 0 ? 'text-red-400' : 'text-emerald-400', sparkData: spark(bl), drill: [] },
  ]

  return (
    <div className="space-y-6">
      <PageHeader
        title="Network Traffic Monitor"
        subtitle={`Real-time packet capture · Zentra analysis · ${stats?.active_connections ?? liveConnections.length} active connections`}
        onRefresh={handleRefresh}
        onSearch={setSearch}
        searchPlaceholder="Search IP, domain, service..."
        showAddTarget
        onAddTarget={() => setShowAddDevice(true)}
        addTargetLabel="Add Network Device"
      >
            <div className="flex items-center gap-2">
          {/* Capture status badge */}
              <div className={`flex items-center gap-2 px-3 py-1.5 rounded-lg border text-xs ${isCapturing ? 'bg-emerald-500/10 border-emerald-500/30' : 'bg-white/5 border-white/10'}`}>
                <Radio className={`w-3 h-3 ${isCapturing ? 'text-emerald-400 animate-pulse' : 'text-slate-500'}`} />
                <span className={`font-mono ${isCapturing ? 'text-emerald-400' : 'text-slate-500'}`}>{isCapturing ? 'CAPTURING' : 'STOPPED'}</span>
              </div>
          {/* Start/Stop */}
          {isCapturing ? (
                <button onClick={handleStopCapture} className="flex items-center gap-1.5 px-3 py-1.5 bg-red-500/20 text-red-400 border border-red-500/30 rounded-lg text-xs hover:bg-red-500/30 transition-colors">
                  <Square className="w-3 h-3" />Stop
                </button>
          ) : (
                <button onClick={handleStartCapture} className="flex items-center gap-1.5 px-3 py-1.5 bg-emerald-500/20 text-emerald-400 border border-emerald-500/30 rounded-lg text-xs hover:bg-emerald-500/30 transition-colors">
                  <Play className="w-3 h-3" />Start
                </button>
          )}
          {/* PCAP Export/Import */}
          {isExporting ? (
                <button onClick={async () => { await stopPcapExport(); setIsExporting(false); }} className="flex items-center gap-1.5 px-3 py-1.5 bg-amber-500/20 text-amber-400 border border-amber-500/30 rounded-lg text-xs hover:bg-amber-500/30 transition-colors">
                  <FileDown className="w-3 h-3 animate-pulse" />Stop Export
                </button>
          ) : (
                <button onClick={async () => {
              const res = await startPcapExport();
              setIsExporting(true);
              setExportFileName(res.file_name);
            }} className="flex items-center gap-1.5 px-3 py-1.5 bg-white/5 text-slate-400 border border-white/10 rounded-lg text-xs hover:text-white transition-colors">
                  <Download className="w-3 h-3" />Export PCAP
                </button>
          )}
              <button className="flex items-center gap-1.5 px-3 py-1.5 bg-white/5 text-slate-400 border border-white/10 rounded-lg text-xs hover:text-white transition-colors">
                <Upload className="w-3 h-3" />Import PCAP
              </button>
          {exportFileName && isExporting && (
                <span className="text-[10px] text-amber-400 font-mono">{exportFileName}</span>
          )}
            </div>
      </PageHeader>

      {/* Adapter Selector + BPF Filter Bar */}
      <div className="flex items-center gap-3 flex-wrap">
        {/* Adapter dropdown */}
            <div className="relative">
              <select
            value={selectedAdapter}
            onChange={e => setSelectedAdapter(e.target.value)}
            className="appearance-none bg-white/5 border border-white/10 text-white text-xs rounded-lg px-3 py-2 pr-8 focus:outline-none focus:border-brand-500/50"
          >
            {selectedAdapter ? (
                  <option value={selectedAdapter} className="bg-slate-900">
                {selectedAdapter}
                  </option>
            ) : (
                  <option value="" className="bg-slate-900">Auto-detect adapter</option>
            )}
            {adapters.map(a => (
                  <option key={a.name} value={a.name} className="bg-slate-900">
                {a.name} — {a.ip} ({a.mac})
                  </option>
            ))}
              </select>
              <ChevronDown className="absolute right-2 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-slate-400 pointer-events-none" />
            </div>

        {/* BPF Filter toggle + input */}
            <button
          onClick={() => setShowBpfInput(!showBpfInput)}
          className={`flex items-center gap-1.5 px-3 py-2 text-xs rounded-lg border transition-colors ${showBpfInput ? 'bg-brand-600/20 text-brand-400 border-brand-500/30' : 'bg-white/5 text-slate-400 border-white/10 hover:text-white'}`}
        >
              <Filter className="w-3 h-3" />BPF Filter
            </button>

        {showBpfInput && (
              <input
            type="text"
            value={bpfFilter}
            onChange={e => setBpfFilter(e.target.value)}
            placeholder='e.g. "tcp port 443" or "host 10.0.0.1"'
            className="flex-1 min-w-[200px] bg-white/5 border border-white/10 text-white text-xs rounded-lg px-3 py-2 font-mono placeholder-slate-600 focus:outline-none focus:border-brand-500/50"
          />
        )}

        {captureError && (
              <span className="text-xs text-red-400">{captureError}</span>
        )}
      </div>

      {/* Vertical Side Menu */}
      <div className="flex gap-6">
        <div className="w-48 flex-shrink-0 space-y-1">
          {([
            { key: 'overview', label: 'Overview', icon: NetworkIcon },
            { key: 'connections', label: 'Connections', icon: ArrowRightLeft },
            { key: 'hosts', label: 'Hosts', icon: Server },
            { key: 'devices', label: `Devices (${networkDevices.length})`, icon: NetworkIcon },
            { key: 'programs', label: `Programs (${programs.length})`, icon: Cpu },
            { key: 'blacklist', label: `Blacklist (${blacklistData.length || blacklisted.length})`, icon: Ban },
            { key: 'notifications', label: `Alerts (${notifLog.length})`, icon: Bell },
            { key: 'interfaces', label: `Interfaces (${interfaces.length})`, icon: Monitor },
            { key: 'protocol-stats', label: 'Protocol Stats', icon: Zap },
            { key: 'sockets', label: 'Sockets', icon: Radio },
            { key: 'containers', label: `Containers (${containers.length})`, icon: Server },
          ] as const).map(t => (
            <button
              key={t.key}
              onClick={() => setTab(t.key)}
              className={`flex items-center gap-2 px-3 py-2 text-xs font-medium rounded-lg transition-colors w-full text-left ${
                tab === t.key ? 'bg-brand-600/20 text-brand-400 border border-brand-500/30' : 'bg-white/5 text-slate-400 hover:text-white border border-transparent hover:border-white/10'
              }`}
            >
              <t.icon className="w-4 h-4" />
              {t.label}
            </button>
          ))}
        </div>

        {/* Right side content */}
        <div className="flex-1 space-y-6">
          {/* KPI Strip — clickable drilldown */}
          <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-3">
        {kpis.map((kpi, i) => (
            <div
            key={i}
            onClick={() => kpi.drill.length > 0 && openKPIDrilldown(kpi.label, kpi.drill)}
            className={`bg-white/5 backdrop-blur-sm border border-white/10 p-3 rounded-xl ${kpi.drill.length > 0 ? 'cursor-pointer hover:bg-white/[0.08]' : ''} transition-colors`}
          >
              <div className="flex items-center gap-1.5 text-[10px] text-slate-400 uppercase tracking-wider mb-1">
                <kpi.icon className="w-3 h-3" />
              {kpi.label}
              </div>
              <div className={`text-xl font-bold font-mono ${kpi.color}`}>{kpi.value}</div>
              <Sparkline data={kpi.sparkData} width={80} height={14} color={kpi.color.includes('blue') ? '#3b82f6' : kpi.color.includes('amber') ? '#f59e0b' : '#8b5cf6'} className="mt-1" />
            </div>
        ))}
      </div>

      {/* TAB: Overview */}
      {tab === 'overview' && (
          <>
            <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
              <div className="lg:col-span-9">
                <div className="bg-slate-900/80 border border-white/10 rounded-xl p-5 shadow-2xl">
                  <div className="flex items-center justify-between mb-4">
                    <h3 className="text-sm font-bold text-white uppercase tracking-wider flex items-center gap-2">
                      <Zap className="w-4 h-4 text-emerald-400" />
                    Live Traffic (Bytes/s)
                    </h3>
                      <div className="flex gap-1 items-center">
                    {/* Data representation toggle */}
                    {(['bytes', 'bits', 'packets'] as const).map(r => (
                          <button key={r} onClick={() => setDataRepr(r)} className={`px-2 py-1 text-[10px] rounded-lg border transition-colors capitalize ${dataRepr === r ? 'bg-brand-600/20 text-brand-400 border-brand-500/30' : 'bg-white/5 text-slate-600 border-white/10 hover:text-white'}`}>
                        {r}
                          </button>
                    ))}
                        <div className="w-px h-4 bg-white/10 mx-1" />
                    {['1h', '6h', '24h', '7d'].map(t => (
                          <button key={t} onClick={() => setTimeRange(t)} className={`px-2.5 py-1 text-[10px] rounded-lg border transition-colors ${timeRange === t ? 'bg-brand-600/20 text-brand-400 border-brand-500/30' : 'bg-white/5 text-slate-500 border-white/10 hover:text-white'}`}>
                        {t}
                          </button>
                    ))}
                      </div>
                    </div>
                    <TrafficChart data={apiTrafficData.length > 0
                  ? apiTrafficData.map(p => ({ time: new Date(p.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }), inbound: p.bytes_in, outbound: p.bytes_out }))
                  : liveTrafficFallback
                } height={300} />
                  </div>
                </div>

                <div className="lg:col-span-3 space-y-4">
                  <div className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-4">
                    <h3 className="text-xs font-bold text-white uppercase tracking-wider mb-3">Protocol Breakdown</h3>
                    <div className="flex justify-center">
                      <DonutChart size={140} thickness={16} centerValue="100%" centerLabel="Traffic" segments={
                    protocolData.length > 0
                      ? protocolData.map((p, i) => ({ label: p.name, value: p.percentage, color: ['#3b82f6', '#f59e0b', '#8b5cf6', '#64748b'][i % 4] }))
                      : [{ label: 'TCP', value: 72, color: '#3b82f6' }, { label: 'UDP', value: 18, color: '#f59e0b' }, { label: 'ICMP', value: 6, color: '#8b5cf6' }, { label: 'ARP', value: 4, color: '#64748b' }]
                  } />
                    </div>
                    <div className="space-y-2 mt-3">
                  {(protocolData.length > 0 ? protocolData : [
                    { name: 'TCP', percentage: 72, packets: 1730000, bytes: 1764000000 },
                    { name: 'UDP', percentage: 18, packets: 432000, bytes: 490000000 },
                    { name: 'ICMP', percentage: 6, packets: 144000, bytes: 122000000 },
                    { name: 'ARP', percentage: 4, packets: 96000, bytes: 74000000 },
                  ]).map((p, i) => (
                        <div key={p.name} className="flex items-center gap-2">
                          <div className={`w-2 h-2 rounded-full`} style={{ backgroundColor: ['#3b82f6', '#f59e0b', '#8b5cf6', '#64748b'][i % 4] }} />
                          <span className="text-[10px] text-slate-400 flex-1">{p.name}</span>
                          <span className="text-[10px] font-mono text-slate-500 mr-1">{fmt(p.bytes)}</span>
                          <span className="text-[10px] font-mono text-slate-300">{p.percentage.toFixed(1)}%</span>
                        </div>
                  ))}
                    </div>
                  </div>

                  <div className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-4">
                    <h3 className="text-xs font-bold text-white uppercase tracking-wider mb-3">Top Services</h3>
                    <div className="space-y-2">
                  {(serviceData.length > 0 ? serviceData.slice(0, 6) : [
                    { service: 'HTTPS', percentage: 45, bytes: 0, packets: 0 },
                    { service: 'Oracle DB', percentage: 20, bytes: 0, packets: 0 },
                    { service: 'DNS', percentage: 12, bytes: 0, packets: 0 },
                    { service: 'Redis', percentage: 8, bytes: 0, packets: 0 },
                    { service: 'HTTP', percentage: 15, bytes: 0, packets: 0 },
                  ]).map((s, i) => (
                        <div key={s.service}>
                          <div className="flex justify-between text-[10px] mb-0.5">
                            <span className="text-slate-400">{s.service}</span>
                            <span className="font-mono text-slate-300">{s.percentage.toFixed(1)}%</span>
                          </div>
                          <div className="h-1.5 bg-slate-800 rounded-full overflow-hidden">
                            <div className="h-full rounded-full transition-all duration-500" style={{ width: `${Math.min(s.percentage, 100)}%`, backgroundColor: ['#3b82f6', '#22c55e', '#f59e0b', '#ef4444', '#8b5cf6', '#64748b'][i % 6] }} />
                          </div>
                        </div>
                  ))}
                    </div>
                  </div>

                  <div className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-4">
                    <div className="flex items-center justify-between mb-3">
                      <h3 className="text-xs font-bold text-white uppercase tracking-wider">Top Countries</h3>
                    </div>
                    <div className="space-y-1.5">
                  {(countryData.length > 0 ? countryData.slice(0, 6).map(c => ({
                    code: c.code, name: c.name, pct: Math.round((c.bytes / (countryData.reduce((s, x) => s + x.bytes, 0) || 1)) * 100)
                      })) : [
                    { code: 'US', name: 'United States', pct: 38 },
                    { code: 'DE', name: 'Germany', pct: 18 },
                    { code: 'JP', name: 'Japan', pct: 12 },
                    { code: 'GB', name: 'United Kingdom', pct: 10 },
                    { code: 'FR', name: 'France', pct: 8 },
                  ]).map(c => (
                        <div key={c.code} className="flex items-center gap-2 text-[10px]">
                          <span className="text-slate-400">{c.code}</span>
                          <span className="text-slate-400 flex-1">{c.name}</span>
                          <span className="font-mono text-slate-300">{c.pct.toFixed(1)}%</span>
                        </div>
                  ))}
                    </div>
                  </div>
                </div>
              </div>

          {/* Quick connections table on overview */}
              <div className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl overflow-hidden">
                <div className="px-5 py-4 border-b border-white/10 flex items-center justify-between">
                  <h3 className="text-sm font-bold text-white uppercase tracking-wider flex items-center gap-2">
                    <Wifi className="w-4 h-4 text-cyan-400" />
                Recent Connections
                  </h3>
                  <button onClick={() => setTab('connections')} className="text-[10px] text-brand-400 hover:underline">View All →</button>
                </div>
                <div className="overflow-x-auto">
                  <table className="w-full text-sm">
                    <thead>
                      <tr className="bg-white/5">
                    {['', 'Source', 'Destination', 'Protocol', 'Service', 'Domain', 'Country', 'Bytes ↕', 'Status'].map(h => (
                          <th key={h} className="px-4 py-3 text-left text-[10px] font-semibold text-slate-400 uppercase tracking-wider">{h}</th>
                    ))}
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-white/5">
                  {liveConnections.slice(0, 8).map(c => (
                        <tr key={c.id} className="hover:bg-white/5 transition-colors cursor-pointer" onClick={() => setSelectedConn(c)}>
                          <td className="px-4 py-2">
                        {c.is_favorite ? <Star className="w-3 h-3 text-amber-400 fill-amber-400" /> : <StarOff className="w-3 h-3 text-slate-600" />}
                          </td>
                          <td className="px-4 py-2 font-mono text-xs text-blue-400">{c.src_ip}:{c.src_port}</td>
                          <td className="px-4 py-2 font-mono text-xs text-slate-300">{c.dst_ip}:{c.dst_port}</td>
                          <td className="px-4 py-2"><span className={`px-2 py-0.5 rounded text-[10px] font-semibold ${c.protocol === 'TCP' ? 'bg-blue-500/20 text-blue-400' : 'bg-amber-500/20 text-amber-400'}`}>{c.protocol}</span></td>
                          <td className="px-4 py-2 text-xs text-slate-300">{c.service}</td>
                          <td className="px-4 py-2 text-xs text-slate-400">{c.domain}</td>
                          <td className="px-4 py-2 text-xs text-slate-400">{c.country}</td>
                          <td className="px-4 py-2 font-mono text-xs text-slate-300">{fmt(c.bytes_sent + c.bytes_recv)}</td>
                          <td className="px-4 py-2">
                            <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-semibold bg-emerald-500/20 text-emerald-400">
                              <span className="w-1.5 h-1.5 rounded-full bg-emerald-500 animate-pulse" />active
                            </span>
                          </td>
                        </tr>
                  ))}
                    </tbody>
                  </table>
                </div>
              </div>
            </>
      )}

      {/* TAB: Connections */}
      {tab === 'connections' && (
          <div className="space-y-4">
            <div className="flex items-center gap-2 flex-wrap">
            {['', 'TCP', 'UDP', 'ICMP'].map(p => (
                <button key={p} onClick={() => setProtoFilter(p)} className={`px-3 py-1.5 text-[10px] rounded-lg border transition-colors uppercase ${protoFilter === p ? 'bg-brand-600/20 text-brand-400 border-brand-500/30' : 'bg-white/5 text-slate-500 border-white/10 hover:text-white'}`}>
                {p || 'All'}
                </button>
            ))}
              <button onClick={() => setShowFavOnly(!showFavOnly)} className={`flex items-center gap-1 px-3 py-1.5 text-[10px] rounded-lg border transition-colors ${showFavOnly ? 'bg-amber-500/20 text-amber-400 border-amber-500/30' : 'bg-white/5 text-slate-500 border-white/10 hover:text-white'}`}>
                <Star className="w-3 h-3" />Favorites
              </button>
              <span className="text-[10px] text-slate-500 ml-auto">{paginatedConns?.total ?? filteredConns.length} connections</span>
            </div>
            <div className="bg-white/5 border border-white/10 rounded-xl overflow-hidden">
              <div className="overflow-x-auto">
                <table className="w-full text-sm">
                  <thead>
                    <tr className="bg-white/5">
                    {['', 'Source', 'Destination', 'Protocol', 'Service', 'Domain', 'ASN', 'Country', 'Sent', 'Received', 'Packets'].map(h => (
                        <th key={h} className="px-3 py-3 text-left text-[10px] font-semibold text-slate-400 uppercase tracking-wider whitespace-nowrap cursor-pointer hover:text-white transition-colors">{h}</th>
                    ))}
                    </tr>
                    <tr className="bg-white/3 border-b border-white/5">
                      <th />
                    {['src_ip', 'dst_ip', 'proto', 'service', 'domain', 'asn', 'country', '', '', ''].map((field, i) => (
                        <th key={i} className="px-2 py-1">
                        {field && (
                            <input
                            type="text"
                            placeholder="filter..."
                            className="w-full bg-white/5 border border-white/10 text-white text-[10px] rounded px-1.5 py-1 font-mono placeholder-slate-700 focus:outline-none focus:border-brand-500/50"
                          />
                        )}
                        </th>
                    ))}
                      </tr>
                  </thead>
                  <tbody className="divide-y divide-white/5">
                  {filteredConns.map(c => (
                      <tr key={c.id} className={`hover:bg-white/5 transition-colors cursor-pointer ${c.is_blacklisted ? 'bg-red-500/5' : ''}`} onClick={() => setSelectedConn(c)}>
                        <td className="px-3 py-2"><Star className={`w-3 h-3 ${c.is_favorite ? 'text-amber-400 fill-amber-400' : 'text-slate-700'}`} /></td>
                        <td className="px-3 py-2 font-mono text-xs text-blue-400 whitespace-nowrap">{c.src_ip}:{c.src_port}</td>
                        <td className="px-3 py-2 font-mono text-xs text-slate-300 whitespace-nowrap">{c.dst_ip}:{c.dst_port}</td>
                        <td className="px-3 py-2"><span className={`px-2 py-0.5 rounded text-[10px] font-semibold ${c.protocol === 'TCP' ? 'bg-blue-500/20 text-blue-400' : 'bg-amber-500/20 text-amber-400'}`}>{c.protocol}</span></td>
                        <td className="px-3 py-2 text-xs text-slate-300">{c.service}</td>
                        <td className="px-3 py-2 text-xs text-slate-400">{c.domain}</td>
                        <td className="px-3 py-2 text-xs text-slate-500">{c.asn}</td>
                        <td className="px-3 py-2 text-xs text-slate-400">{c.country}</td>
                        <td className="px-3 py-2 font-mono text-xs text-emerald-400">{fmt(c.bytes_sent)}</td>
                        <td className="px-3 py-2 font-mono text-xs text-blue-400">{fmt(c.bytes_recv)}</td>
                        <td className="px-3 py-2 font-mono text-xs text-slate-300">{(c.packets_sent + c.packets_recv).toLocaleString()}</td>
                      </tr>
                  ))}
                  </tbody>
                  </table>
                </div>
              </div>

          {/* Pagination controls */}
          {paginatedConns && paginatedConns.total_pages > 1 && (
              <div className="flex items-center justify-between mt-3">
                <span className="text-[10px] text-slate-500">
                Page {paginatedConns.page} of {paginatedConns.total_pages} · {paginatedConns.total} total
                </span>
                <div className="flex gap-1">
                  <button
                  disabled={connPage <= 1}
                  onClick={() => setConnPage(p => Math.max(1, p - 1))}
                  className="px-3 py-1.5 text-[10px] rounded-lg border bg-white/5 border-white/10 text-slate-400 hover:text-white disabled:opacity-30 disabled:cursor-not-allowed"
                >
                  ← Prev
                  </button>
                  <button
                  disabled={connPage >= paginatedConns.total_pages}
                  onClick={() => setConnPage(p => p + 1)}
                  className="px-3 py-1.5 text-[10px] rounded-lg border bg-white/5 border-white/10 text-slate-400 hover:text-white disabled:opacity-30 disabled:cursor-not-allowed"
                >
                  Next →
                  </button>
                </div>
              </div>
          )}
          </div>
      )}

      {/* TAB: Hosts */}
      {tab === 'hosts' && (
          <div className="bg-white/5 border border-white/10 rounded-xl overflow-hidden">
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="bg-white/5">
                  {['IP Address', 'Domain', 'ASN', 'Country', 'Bytes In', 'Bytes Out', 'Connections'].map(h => (
                      <th key={h} className="px-4 py-3 text-left text-[10px] font-semibold text-slate-400 uppercase tracking-wider">{h}</th>
                  ))}
                  </tr>
                </thead>
                <tbody className="divide-y divide-white/5">
                {liveHosts.map((h, i) => (
                    <tr key={i} className="hover:bg-white/5 transition-colors">
                      <td className="px-4 py-2.5 font-mono text-xs text-blue-400">{h.ip}</td>
                      <td className="px-4 py-2.5 text-xs text-slate-300">{h.domain}</td>
                      <td className="px-4 py-2.5 text-xs text-slate-500">{h.asn}</td>
                      <td className="px-4 py-2.5 text-xs text-slate-400">{h.country}</td>
                      <td className="px-4 py-2.5 font-mono text-xs text-emerald-400">{fmt(h.bytes_in)}</td>
                      <td className="px-4 py-2.5 font-mono text-xs text-amber-400">{fmt(h.bytes_out)}</td>
                      <td className="px-4 py-2.5 font-mono text-xs text-slate-300">{h.connections}</td>
                    </tr>
                ))}
                </tbody>
                </table>
              </div>
            </div>
        )}

      {/* TAB: Blacklist */}
      {tab === 'blacklist' && (
        <div className="space-y-4">
          <div className="flex items-center justify-between">
            <p className="text-xs text-slate-400">IP blacklist management — manually add, import, or remove blacklisted IPs.</p>
            <div className="flex gap-2">
              <button className="flex items-center gap-1.5 px-3 py-1.5 bg-white/5 text-slate-400 border border-white/10 rounded-lg text-xs hover:text-white transition-colors">
                <Upload className="w-3 h-3" />Import File
              </button>
            </div>
          </div>

          {/* Add new blacklist entry */}
          <div className="flex items-center gap-2">
            <input
              type="text" value={newBlacklistIP} onChange={e => setNewBlacklistIP(e.target.value)}
              placeholder="IP address (e.g. 203.0.113.50)"
              className="flex-1 max-w-[200px] bg-white/5 border border-white/10 text-white text-xs rounded-lg px-3 py-2 font-mono placeholder-slate-600 focus:outline-none focus:border-brand-500/50"
            />
            <input
              type="text" value={newBlacklistReason} onChange={e => setNewBlacklistReason(e.target.value)}
              placeholder="Reason (optional)"
              className="flex-1 max-w-[300px] bg-white/5 border border-white/10 text-white text-xs rounded-lg px-3 py-2 placeholder-slate-600 focus:outline-none focus:border-brand-500/50"
            />
            <button
              onClick={async () => {
                if (!newBlacklistIP.trim()) return
                await addBlacklistIP(newBlacklistIP.trim(), newBlacklistReason.trim() || undefined)
                setNewBlacklistIP('')
                setNewBlacklistReason('')
              }}
              className="px-3 py-1.5 bg-brand-600/20 text-brand-400 border border-brand-500/30 rounded-lg text-xs hover:bg-brand-600/30 transition-colors"
            >
              Add
            </button>
          </div>

          {blacklistData.length === 0 && blacklisted.length === 0 ? (
            <div className="bg-white/5 border border-white/10 rounded-xl text-center py-12">
              <Ban className="w-10 h-10 text-slate-600 mx-auto mb-2" />
              <p className="text-slate-400 text-sm">No blacklisted IPs</p>
            </div>
          ) : (
            <div className="bg-white/5 border border-white/10 rounded-xl overflow-hidden">
              <div className="overflow-x-auto">
                <table className="w-full text-sm">
                  <thead>
                    <tr className="bg-red-500/10">
                      {['IP Address', 'Reason', 'Source', 'Added', 'Actions'].map(h => (
                        <th key={h} className="px-4 py-3 text-left text-[10px] font-semibold text-red-400 uppercase tracking-wider">{h}</th>
                      ))}
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-white/5">
                    {blacklistData.map(entry => (
                      <tr key={entry.id} className="hover:bg-red-500/5 transition-colors">
                        <td className="px-4 py-2.5 font-mono text-xs text-red-400">{entry.ip}</td>
                        <td className="px-4 py-2.5 text-xs text-slate-300">{entry.reason}</td>
                        <td className="px-4 py-2.5">
                          <span className={`px-2 py-0.5 rounded text-[10px] font-semibold ${entry.source === 'manual' ? 'bg-blue-500/20 text-blue-400' : entry.source === 'threat_feed' ? 'bg-red-500/20 text-red-400' : 'bg-amber-500/20 text-amber-400'}`}>
                            {entry.source}
                          </span>
                        </td>
                        <td className="px-4 py-2.5 text-xs text-slate-500">{new Date(entry.created_at).toLocaleString()}</td>
                        <td className="px-4 py-2.5">
                          <button
                            onClick={async () => {
                              await removeBlacklistIP(entry.id)
                              listBlacklist().then(setBlacklistData).catch(() => {})
                            }}
                            className="flex items-center gap-1 px-2 py-1 bg-white/5 text-slate-400 border border-white/10 rounded text-[10px] hover:text-red-400 hover:border-red-500/30 transition-colors"
                          >
                            <Trash2 className="w-3 h-3" />Remove
                          </button>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}
        </div>
      )}

      {/* TAB: Devices */}
          {tab === 'devices' && (
            <div className="space-y-4">
              <div className="flex items-center justify-between">
                <p className="text-xs text-slate-400">Registered network devices (SNMP-monitored) — click any device for detailed metrics.</p>
                <button
                  onClick={loadDevices}
                  disabled={devicesLoading}
                  className="flex items-center gap-1.5 px-3 py-1.5 bg-white/5 text-slate-400 border border-white/10 rounded-lg text-xs hover:text-white transition-colors disabled:opacity-50"
                >
                  <NetworkIcon className={`w-3 h-3 ${devicesLoading ? 'animate-spin' : ''}`} />
                  {devicesLoading ? 'Polling...' : 'Refresh Devices'}
                </button>
              </div>
              {devicesLoading && networkDevices.length === 0 ? (
                <div className="bg-white/5 border border-white/10 rounded-xl text-center py-12">
                  <div className="animate-spin w-8 h-8 border-2 border-brand-400 border-t-transparent rounded-full mx-auto mb-3" />
                  <p className="text-slate-400 text-sm">Polling SNMP devices...</p>
                </div>
              ) : networkDevices.length === 0 ? (
                <div className="bg-white/5 border border-white/10 rounded-xl text-center py-12">
                  <NetworkIcon className="w-10 h-10 text-slate-600 mx-auto mb-2" />
                  <p className="text-slate-400 text-sm">No network devices registered</p>
                  <p className="text-slate-500 text-xs mt-1">Use "Add Network Device" to register SNMP targets</p>
                </div>
              ) : (
                <div className="bg-white/5 border border-white/10 rounded-xl overflow-hidden">
                  <div className="overflow-x-auto">
                    <table className="w-full text-sm">
                      <thead>
                        <tr className="bg-white/5">
                          {['Status', 'Name', 'Type', 'Vendor', 'Host', 'Port', 'sysName', 'sysDescr', 'Uptime', 'Latency'].map(h => (
                            <th key={h} className="px-4 py-3 text-left text-[10px] font-semibold text-slate-400 uppercase tracking-wider">{h}</th>
                          ))}
                        </tr>
                      </thead>
                      <tbody className="divide-y divide-white/5">
                        {networkDevices.map(d => (
                          <tr
                            key={d.target_id}
                            className="hover:bg-brand-600/10 transition-colors cursor-pointer group"
                            onClick={() => openDeviceDetail(d.target_id)}
                          >
                            <td className="px-4 py-2.5">
                              <span className={`inline-flex items-center gap-1.5 px-2 py-0.5 rounded-full text-[10px] font-semibold ${
                                d.snmp_reachable
                                  ? 'bg-emerald-500/20 text-emerald-400 border border-emerald-500/30'
                                  : 'bg-red-500/20 text-red-400 border border-red-500/30'
                              }`}>
                                <span className={`w-1.5 h-1.5 rounded-full ${d.snmp_reachable ? 'bg-emerald-400' : 'bg-red-400'}`} />
                                {d.snmp_reachable ? 'Online' : 'Unreachable'}
                              </span>
                            </td>
                            <td className="px-4 py-2.5">
                              <div className="text-xs font-medium text-white group-hover:text-brand-400 transition-colors">{d.name}</div>
                              {d.description && <div className="text-[10px] text-slate-500">{d.description}</div>}
                            </td>
                            <td className="px-4 py-2.5">
                              <span className={`px-2 py-0.5 rounded text-[10px] font-semibold ${
                                (d.device_type || d.subtype) === 'router' ? 'bg-blue-500/20 text-blue-400' :
                                (d.device_type || d.subtype) === 'switch' ? 'bg-cyan-500/20 text-cyan-400' :
                                (d.device_type || d.subtype) === 'firewall' ? 'bg-red-500/20 text-red-400' :
                                (d.device_type || d.subtype) === 'access_point' ? 'bg-purple-500/20 text-purple-400' :
                                (d.device_type || d.subtype) === 'storage' ? 'bg-amber-500/20 text-amber-400' :
                                'bg-white/10 text-slate-400'
                              }`}>{d.device_type || d.subtype}</span>
                            </td>
                            <td className="px-4 py-2.5 text-xs text-slate-300">{d.vendor || '—'}</td>
                            <td className="px-4 py-2.5 font-mono text-xs text-slate-300">{d.host}</td>
                            <td className="px-4 py-2.5 font-mono text-xs text-slate-400">{d.port}</td>
                            <td className="px-4 py-2.5 text-xs text-cyan-400">{d.sys_name || '—'}</td>
                            <td className="px-4 py-2.5 text-[10px] text-slate-500 max-w-[250px] truncate" title={d.sys_descr || ''}>{d.sys_descr || '—'}</td>
                            <td className="px-4 py-2.5 text-xs text-slate-400">{d.uptime || '—'}</td>
                            <td className="px-4 py-2.5 font-mono text-xs text-emerald-400">{d.response_time_ms ? `${d.response_time_ms.toFixed(1)}ms` : '—'}</td>
                          </tr>
                        ))}
                      </tbody>
                    </table>
                  </div>
                  <div className="px-4 py-3 border-t border-white/10 flex items-center justify-between">
                    <span className="text-[10px] text-slate-500">
                      {networkDevices.filter(d => d.snmp_reachable).length} of {networkDevices.length} devices online
                    </span>
                    <span className="text-[10px] text-slate-500">
                      Last polled: {networkDevices[0]?.last_check_at ? new Date(networkDevices[0].last_check_at).toLocaleString() : 'never'}
                    </span>
                  </div>
                </div>
              )}
            </div>
          )}

      {/* Device Detail Slide-Out Panel */}
      {(selectedDevice || deviceDetailLoading) && (
        <div className="fixed inset-0 z-50 flex justify-end bg-black/40 backdrop-blur-sm">
          <div className="w-full max-w-2xl bg-slate-900 border-l border-white/10 shadow-2xl overflow-y-auto">
            {deviceDetailLoading && !selectedDevice ? (
              <div className="flex items-center justify-center h-full">
                <div className="text-center">
                  <div className="animate-spin w-10 h-10 border-2 border-brand-400 border-t-transparent rounded-full mx-auto mb-3" />
                  <p className="text-slate-400 text-sm">Loading device details via SNMP...</p>
                </div>
              </div>
            ) : selectedDevice && (
              <>
                <div className="p-5 border-b border-white/10 flex items-center justify-between">
                  <div>
                    <h3 className="text-lg font-bold text-white flex items-center gap-2">
                      <NetworkIcon className="w-5 h-5 text-brand-400" />
                      {selectedDevice.name}
                    </h3>
                    <p className="text-xs text-slate-400 mt-0.5">{selectedDevice.host}:{selectedDevice.port} · {selectedDevice.vendor || selectedDevice.subtype}</p>
                  </div>
                  <button onClick={() => setSelectedDevice(null)} className="p-1 rounded hover:bg-white/10 text-slate-400">✕</button>
                </div>
                <div className="p-5 space-y-5">
                  {/* Status + Response Time */}
                  <div className="flex items-center gap-3">
                    <span className={`inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-semibold ${
                      selectedDevice.snmp_reachable
                        ? 'bg-emerald-500/20 text-emerald-400 border border-emerald-500/30'
                        : 'bg-red-500/20 text-red-400 border border-red-500/30'
                    }`}>
                      <span className={`w-2 h-2 rounded-full ${selectedDevice.snmp_reachable ? 'bg-emerald-400 animate-pulse' : 'bg-red-400'}`} />
                      {selectedDevice.snmp_reachable ? 'Online' : 'Unreachable'}
                    </span>
                    {selectedDevice.response_time_ms && (
                      <span className="text-xs font-mono text-slate-400">{selectedDevice.response_time_ms.toFixed(1)}ms response</span>
                    )}
                    {selectedDevice.device_type && (
                      <span className={`px-2 py-0.5 rounded text-[10px] font-semibold ${
                        selectedDevice.device_type === 'router' ? 'bg-blue-500/20 text-blue-400' :
                        selectedDevice.device_type === 'switch' ? 'bg-cyan-500/20 text-cyan-400' :
                        selectedDevice.device_type === 'firewall' ? 'bg-red-500/20 text-red-400' :
                        selectedDevice.device_type === 'access_point' ? 'bg-purple-500/20 text-purple-400' :
                        'bg-white/10 text-slate-400'
                      }`}>{selectedDevice.device_type}</span>
                    )}
                  </div>

                  {/* System Info Grid */}
                  <div className="grid grid-cols-2 gap-3">
                    {[
                      { k: 'sysName', v: selectedDevice.sys_name },
                      { k: 'sysLocation', v: selectedDevice.sys_location },
                      { k: 'sysContact', v: selectedDevice.sys_contact },
                      { k: 'Uptime', v: selectedDevice.uptime },
                    ].map(item => (
                      <div key={item.k} className="bg-white/5 rounded-lg p-3">
                        <div className="text-[10px] text-slate-500 uppercase tracking-wider">{item.k}</div>
                        <div className="text-sm font-mono text-white mt-0.5">{item.v || '—'}</div>
                      </div>
                    ))}
                  </div>

                  {/* sysDescr */}
                  {selectedDevice.sys_descr && (
                    <div className="bg-white/5 rounded-lg p-3">
                      <div className="text-[10px] text-slate-500 uppercase tracking-wider mb-1">System Description</div>
                      <div className="text-xs font-mono text-slate-300 leading-relaxed">{selectedDevice.sys_descr}</div>
                    </div>
                  )}

                  {/* CPU + Memory Gauges */}
                  {(selectedDevice.cpu_usage_pct !== null || selectedDevice.mem_total_kb !== null) && (
                    <div className="grid grid-cols-2 gap-3">
                      {selectedDevice.cpu_usage_pct !== null && (
                        <div className="bg-white/5 rounded-xl p-4">
                          <div className="text-[10px] text-slate-500 uppercase tracking-wider mb-2">CPU Usage</div>
                          <div className="text-2xl font-bold font-mono text-white">{selectedDevice.cpu_usage_pct.toFixed(1)}%</div>
                          <div className="h-2 bg-slate-800 rounded-full mt-2 overflow-hidden">
                            <div
                              className={`h-full rounded-full transition-all ${
                                selectedDevice.cpu_usage_pct > 80 ? 'bg-red-500' :
                                selectedDevice.cpu_usage_pct > 60 ? 'bg-amber-500' : 'bg-emerald-500'
                              }`}
                              style={{ width: `${Math.min(selectedDevice.cpu_usage_pct, 100)}%` }}
                            />
                          </div>
                        </div>
                      )}
                      {selectedDevice.mem_total_kb !== null && (
                        <div className="bg-white/5 rounded-xl p-4">
                          <div className="text-[10px] text-slate-500 uppercase tracking-wider mb-2">Memory</div>
                          <div className="text-2xl font-bold font-mono text-white">
                            {selectedDevice.mem_used_kb ? `${(selectedDevice.mem_used_kb / 1e6).toFixed(1)}` : '—'}
                            <span className="text-sm text-slate-500"> / {(selectedDevice.mem_total_kb / 1e6).toFixed(1)} GB</span>
                          </div>
                          {selectedDevice.mem_used_kb && (
                            <div className="h-2 bg-slate-800 rounded-full mt-2 overflow-hidden">
                              <div
                                className="h-full rounded-full bg-blue-500 transition-all"
                                style={{ width: `${Math.min((selectedDevice.mem_used_kb / selectedDevice.mem_total_kb) * 100, 100)}%` }}
                              />
                            </div>
                          )}
                        </div>
                      )}
                    </div>
                  )}

                  {/* Interfaces Table */}
                  {selectedDevice.interfaces.length > 0 && (
                    <div>
                      <h4 className="text-xs font-bold text-white uppercase tracking-wider mb-3 flex items-center gap-2">
                        <Monitor className="w-3.5 h-3.5 text-cyan-400" />
                        Interfaces ({selectedDevice.interfaces.length})
                      </h4>
                      <div className="bg-white/5 border border-white/10 rounded-xl overflow-hidden">
                        <div className="overflow-x-auto">
                          <table className="w-full text-sm">
                            <thead>
                              <tr className="bg-white/5">
                                {['Name', 'Type', 'Status', 'Speed', 'MTU', 'In', 'Out', 'Errors', 'MAC'].map(h => (
                                  <th key={h} className="px-3 py-2 text-left text-[10px] font-semibold text-slate-400 uppercase">{h}</th>
                                ))}
                              </tr>
                            </thead>
                            <tbody className="divide-y divide-white/5">
                              {selectedDevice.interfaces.map(iface => (
                                <tr key={iface.index} className="hover:bg-white/5">
                                  <td className="px-3 py-2 font-mono text-xs text-cyan-400 font-semibold">{iface.name || `if${iface.index}`}</td>
                                  <td className="px-3 py-2 text-[10px] text-slate-400">{iface.if_type}</td>
                                  <td className="px-3 py-2">
                                    <span className={`px-1.5 py-0.5 rounded text-[10px] font-semibold ${
                                      iface.oper_status === 'up' ? 'bg-emerald-500/20 text-emerald-400' : 'bg-red-500/20 text-red-400'
                                    }`}>{iface.oper_status}</span>
                                  </td>
                                  <td className="px-3 py-2 text-xs text-slate-300">{iface.speed_mbps > 0 ? `${iface.speed_mbps} Mbps` : '—'}</td>
                                  <td className="px-3 py-2 text-xs text-slate-400">{iface.mtu}</td>
                                  <td className="px-3 py-2 font-mono text-xs text-blue-400">{fmt(iface.in_octets)}</td>
                                  <td className="px-3 py-2 font-mono text-xs text-purple-400">{fmt(iface.out_octets)}</td>
                                  <td className="px-3 py-2 font-mono text-xs text-red-400">{iface.in_errors + iface.out_errors > 0 ? iface.in_errors + iface.out_errors : '—'}</td>
                                  <td className="px-3 py-2 font-mono text-[10px] text-slate-500">{iface.mac_address || '—'}</td>
                                </tr>
                              ))}
                            </tbody>
                          </table>
                        </div>
                      </div>
                    </div>
                  )}

                  {/* Refresh Button */}
                  <button
                    onClick={() => openDeviceDetail(selectedDevice.target_id)}
                    className="w-full py-2.5 bg-brand-600/20 text-brand-400 border border-brand-500/30 rounded-xl text-xs font-medium hover:bg-brand-600/30 transition-colors"
                  >
                    Refresh SNMP Data
                  </button>
                </div>
              </>
            )}
          </div>
        </div>
      )}

      {/* TAB: Programs */}
          {tab === 'programs' && (
            <div className="space-y-4">
              <p className="text-xs text-slate-400">Programs generating network traffic on the monitored host, sorted by total bytes.</p>
              <div className="bg-white/5 border border-white/10 rounded-xl overflow-hidden">
                <div className="overflow-x-auto">
                  <table className="w-full text-sm">
                    <thead>
                      <tr className="bg-white/5">
                    {['Program', 'Path', 'Bytes Sent', 'Bytes Received', 'Total Packets', 'Connections'].map(h => (
                          <th key={h} className="px-4 py-3 text-left text-[10px] font-semibold text-slate-400 uppercase tracking-wider">{h}</th>
                    ))}
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-white/5">
                  {programs.map((p, i) => (
                        <tr key={i} className="hover:bg-white/5 transition-colors">
                          <td className="px-4 py-2.5">
                            <div className="flex items-center gap-2">
                              <Cpu className="w-3.5 h-3.5 text-brand-400" />
                              <span className="text-xs font-medium text-white">{p.name}</span>
                            </div>
                          </td>
                          <td className="px-4 py-2.5 font-mono text-xs text-slate-500">—</td>
                          <td className="px-4 py-2.5 font-mono text-xs text-emerald-400">—</td>
                          <td className="px-4 py-2.5 font-mono text-xs text-blue-400">—</td>
                          <td className="px-4 py-2.5 font-mono text-xs text-slate-300">{p.connections}</td>
                          <td className="px-4 py-2.5 font-mono text-xs text-slate-300">{p.connections}</td>
                        </tr>
                  ))}
                    </tbody>
                  </table>
                </div>
              </div>
            </div>
          )}

      {/* TAB: Notifications */}
      {tab === 'notifications' && (
        <div className="space-y-4">
          <p className="text-xs text-slate-400">Network notification log — data thresholds, favorite activity, and blacklist hit alerts.</p>
          {notifLog.length === 0 ? (
            <div className="bg-white/5 border border-white/10 rounded-xl text-center py-12">
              <Bell className="w-10 h-10 text-slate-600 mx-auto mb-2" />
              <p className="text-slate-400 text-sm">No notification events yet</p>
            </div>
          ) : (
            <div className="space-y-2">
              {notifLog.map(entry => {
                const typeColors: Record<string, string> = {
                  data_threshold: 'bg-amber-500/20 text-amber-400 border-amber-500/30',
                  blacklist_hit: 'bg-red-500/20 text-red-400 border-red-500/30',
                  favorite_activity: 'bg-blue-500/20 text-blue-400 border-blue-500/30',
                }
                const typeIcons: Record<string, string> = {
                  data_threshold: '📊',
                  blacklist_hit: '🛑',
                  favorite_activity: '⭐',
                }
                return (
                      <div key={entry.id} className="bg-white/5 border border-white/10 rounded-xl p-4 flex items-start gap-3">
                        <span className="text-lg">{typeIcons[entry.event_type] || '🔔'}</span>
                        <div className="flex-1 min-w-0">
                          <div className="flex items-center gap-2 mb-1">
                            <span className={`px-2 py-0.5 rounded text-[10px] font-semibold border ${typeColors[entry.event_type] || 'bg-white/10 text-slate-400 border-white/10'}`}>
                          {entry.event_type.replace(/_/g, ' ')}
                            </span>
                            <span className="text-[10px] text-slate-500">{new Date(entry.timestamp).toLocaleString()}</span>
                          </div>
                          <p className="text-xs text-slate-300">{String(entry.details.message || JSON.stringify(entry.details))}</p>
                        </div>
                      </div>
                )
                  })}
                </div>
          )}
              </div>
          )}

      {/* Connection Detail Side Panel */}
      {selectedConn && (
            <div className="fixed inset-0 z-50 flex justify-end bg-black/40 backdrop-blur-sm">
              <div className="w-full max-w-lg bg-slate-900 border-l border-white/10 shadow-2xl overflow-y-auto">
                <div className="p-5 border-b border-white/10 flex items-center justify-between">
                  <h3 className="text-lg font-bold text-white flex items-center gap-2">
                    <Eye className="w-5 h-5 text-brand-400" />
                Connection Details
                  </h3>
                  <button onClick={() => setSelectedConn(null)} className="p-1 rounded hover:bg-white/10 text-slate-400">✕</button>
                </div>
                <div className="p-5 space-y-4">
                  <div className="grid grid-cols-2 gap-3">
                {[
                  { k: 'Source IP', v: `${selectedConn.src_ip}:${selectedConn.src_port}` },
                  { k: 'Dest IP', v: `${selectedConn.dst_ip}:${selectedConn.dst_port}` },
                  { k: 'Protocol', v: selectedConn.protocol },
                  { k: 'Service', v: selectedConn.service },
                  { k: 'Domain', v: selectedConn.domain },
                  { k: 'ASN', v: selectedConn.asn },
                  { k: 'Country', v: selectedConn.country },
                  { k: 'Bytes Sent', v: fmt(selectedConn.bytes_sent) },
                  { k: 'Bytes Received', v: fmt(selectedConn.bytes_received) },
                  { k: 'Packets Sent', v: selectedConn.packets_sent.toLocaleString() },
                  { k: 'Packets Received', v: selectedConn.packets_received.toLocaleString() },
                  { k: 'First Seen', v: new Date(selectedConn.first_seen).toLocaleString() },
                  { k: 'Last Seen', v: new Date(selectedConn.last_seen).toLocaleString() },
                ].map(item => (
                      <div key={item.k} className="bg-white/5 rounded-lg p-3">
                        <div className="text-[10px] text-slate-500 uppercase tracking-wider">{item.k}</div>
                        <div className="text-sm font-mono text-white mt-0.5">{item.v}</div>
                      </div>
                ))}
                  </div>
                  <div className="flex gap-2">
                    <button className="flex-1 flex items-center justify-center gap-2 px-4 py-2.5 bg-amber-500/20 text-amber-400 border border-amber-500/30 rounded-xl text-xs font-medium">
                      <Star className="w-3.5 h-3.5" />{selectedConn.is_favorite ? 'Remove Favorite' : 'Add to Favorites'}
                    </button>
                    <button className="flex-1 flex items-center justify-center gap-2 px-4 py-2.5 bg-red-500/20 text-red-400 border border-red-500/30 rounded-xl text-xs font-medium">
                      <Ban className="w-3.5 h-3.5" />{selectedConn.is_blacklisted ? 'Remove from Blacklist' : 'Add to Blacklist'}
                    </button>
                  </div>
                  <div className="bg-white/5 border border-white/10 rounded-xl p-4">
                    <h4 className="text-xs font-bold text-white uppercase tracking-wider mb-2">Traffic Over Time</h4>
                    <Sparkline data={spark(selectedConn.bytes_sent / 1e6, 30)} width={400} height={60} color="#3b82f6" fillColor="#3b82f6" />
                  </div>
                </div>
              </div>
              </div>
          )}

      {/* P3: Interfaces Tab */}
          {tab === 'interfaces' && (
            <div className="bg-white/5 border border-white/10 rounded-xl overflow-hidden">
              <table className="w-full text-sm">
                <thead><tr className="bg-white/5">
              {['Name', 'IP Address', 'MAC', 'Speed', 'MTU', 'State', 'RX Bytes', 'TX Bytes', 'RX Pkts', 'TX Pkts', 'Errors', 'Dropped'].map(h => (
                    <th key={h} className="px-3 py-2 text-left text-[10px] font-semibold text-slate-400 uppercase">{h}</th>
              ))}
                </tr></thead>
                <tbody className="divide-y divide-white/5">
              {interfaces.map(iface => (
                    <tr key={iface.name} className="hover:bg-white/5">
                      <td className="px-3 py-2 font-mono text-xs text-cyan-400 font-semibold">{iface.name}</td>
                      <td className="px-3 py-2 font-mono text-xs text-white">{iface.ip_address}</td>
                      <td className="px-3 py-2 font-mono text-[10px] text-slate-500">{iface.mac_address}</td>
                      <td className="px-3 py-2 text-xs text-slate-300">{iface.speed_mbps > 0 ? `${iface.speed_mbps} Mbps` : '-'}</td>
                      <td className="px-3 py-2 text-xs text-slate-400">{iface.mtu}</td>
                      <td className="px-3 py-2"><span className={`px-1.5 py-0.5 rounded text-[10px] font-semibold ${iface.state === 'up' ? 'bg-emerald-500/20 text-emerald-400' : 'bg-red-500/20 text-red-400'}`}>{iface.state}</span></td>
                      <td className="px-3 py-2 font-mono text-xs text-blue-400">{fmt(iface.rx_bytes)}</td>
                      <td className="px-3 py-2 font-mono text-xs text-purple-400">{fmt(iface.tx_bytes)}</td>
                      <td className="px-3 py-2 font-mono text-xs text-slate-300">{iface.rx_packets.toLocaleString()}</td>
                      <td className="px-3 py-2 font-mono text-xs text-slate-300">{iface.tx_packets.toLocaleString()}</td>
                      <td className="px-3 py-2 font-mono text-xs text-red-400">{iface.rx_errors + iface.tx_errors}</td>
                      <td className="px-3 py-2 font-mono text-xs text-amber-400">{iface.rx_dropped + iface.tx_dropped}</td>
                    </tr>
              ))}
                </tbody>
              </table>
              </div>
          )}

      {/* P3: Protocol Stats Tab */}
          {tab === 'protocol-stats' && (
            <div className="bg-white/5 border border-white/10 rounded-xl overflow-hidden">
              <table className="w-full text-sm">
                <thead><tr className="bg-white/5">
              {['Protocol', 'Connections', 'Bytes In', 'Bytes Out', 'Packets In', 'Packets Out', 'Errors', 'Retransmits', 'RTT (avg)'].map(h => (
                    <th key={h} className="px-3 py-2 text-left text-[10px] font-semibold text-slate-400 uppercase">{h}</th>
              ))}
                </tr></thead>
                <tbody className="divide-y divide-white/5">
              {protoStats.map(ps => (
                    <tr key={ps.protocol} className="hover:bg-white/5">
                      <td className="px-3 py-2 font-semibold text-xs text-cyan-400">{ps.protocol}</td>
                      <td className="px-3 py-2 font-mono text-xs text-white">{ps.connections.toLocaleString()}</td>
                      <td className="px-3 py-2 font-mono text-xs text-blue-400">{fmt(ps.bytes_in)}</td>
                      <td className="px-3 py-2 font-mono text-xs text-purple-400">{fmt(ps.bytes_out)}</td>
                      <td className="px-3 py-2 font-mono text-xs text-slate-300">{ps.packets_in.toLocaleString()}</td>
                      <td className="px-3 py-2 font-mono text-xs text-slate-300">{ps.packets_out.toLocaleString()}</td>
                      <td className="px-3 py-2 font-mono text-xs text-red-400">{ps.errors}</td>
                      <td className="px-3 py-2 font-mono text-xs text-amber-400">{ps.retransmits}</td>
                      <td className="px-3 py-2 font-mono text-xs text-slate-300">{ps.rtt_avg_ms > 0 ? `${ps.rtt_avg_ms}ms` : '-'}</td>
                    </tr>
              ))}
                </tbody>
              </table>
              </div>
          )}

      {/* P3: Socket Stats Tab */}
          {tab === 'sockets' && socketData && (
            <div className="space-y-4">
              <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-3">
            {Object.entries(socketData.summary).map(([k, v]) => (
                  <div key={k} className="bg-white/5 border border-white/10 rounded-xl p-4">
                    <div className="text-[10px] text-slate-500 uppercase tracking-wider">{k.replace(/_/g, ' ')}</div>
                    <div className="text-2xl font-bold font-mono text-white mt-1">{typeof v === 'number' ? v.toLocaleString() : v}</div>
                  </div>
            ))}
              </div>
              <div className="bg-white/5 border border-white/10 rounded-xl overflow-hidden">
                <table className="w-full text-sm">
                  <thead><tr className="bg-white/5">
                {['TCP State', 'Count', 'Recv-Q', 'Send-Q'].map(h => (
                      <th key={h} className="px-4 py-2 text-left text-[10px] font-semibold text-slate-400 uppercase">{h}</th>
                ))}
                  </tr></thead>
                  <tbody className="divide-y divide-white/5">
                {socketData.tcp_states.map(s => (
                      <tr key={s.state} className="hover:bg-white/5">
                        <td className="px-4 py-2 font-semibold text-xs text-cyan-400">{s.state}</td>
                        <td className="px-4 py-2 font-mono text-xs text-white">{s.count.toLocaleString()}</td>
                        <td className="px-4 py-2 font-mono text-xs text-slate-400">{s.recv_q}</td>
                        <td className="px-4 py-2 font-mono text-xs text-slate-400">{s.send_q}</td>
                      </tr>
                ))}
                  </tbody>
                </table>
              </div>
              </div>
          )}

      {/* P17: Containers Tab */}
          {tab === 'containers' && (
            <div className="bg-white/5 border border-white/10 rounded-xl overflow-hidden">
              <table className="w-full text-sm">
                <thead><tr className="bg-white/5">
              {['Container', 'Image', 'Namespace', 'IP', 'Connections', 'RX', 'TX', 'State'].map(h => (
                    <th key={h} className="px-3 py-2 text-left text-[10px] font-semibold text-slate-400 uppercase">{h}</th>
              ))}
                </tr></thead>
                <tbody className="divide-y divide-white/5">
              {containers.map(c => (
                    <tr key={c.container_id} className="hover:bg-white/5">
                      <td className="px-3 py-2"><div className="text-xs text-white font-medium">{c.container_name}</div><div className="text-[10px] text-slate-500 font-mono">{c.container_id.slice(0, 12)}</div></td>
                      <td className="px-3 py-2 text-[11px] font-mono text-slate-300">{c.image}</td>
                      <td className="px-3 py-2"><span className="px-1.5 py-0.5 bg-blue-500/20 text-blue-400 rounded text-[10px]">{c.namespace}</span></td>
                      <td className="px-3 py-2 font-mono text-xs text-slate-300">{c.ip_address}</td>
                      <td className="px-3 py-2 font-mono text-xs text-white">{c.connections}</td>
                      <td className="px-3 py-2 font-mono text-xs text-blue-400">{fmt(c.rx_bytes)}</td>
                      <td className="px-3 py-2 font-mono text-xs text-purple-400">{fmt(c.tx_bytes)}</td>
                      <td className="px-3 py-2"><span className={`px-1.5 py-0.5 rounded text-[10px] font-semibold ${c.state === 'running' ? 'bg-emerald-500/20 text-emerald-400' : 'bg-red-500/20 text-red-400'}`}>{c.state}</span></td>
                    </tr>
              ))}
                </tbody>
              </table>
              </div>
          )}

      {/* Drilldown Modal */}
      <DrilldownModal open={drilldown.open} onClose={() => setDrilldown(d => ({ ...d, open: false }))} title={drilldown.title} root={drilldown.nodes} />

      {/* Add Network Device Modal */}
      <AddNetworkDeviceModal open={showAddDevice} onClose={() => setShowAddDevice(false)} />
        </div>
      </div>
    </div>
  )
}
