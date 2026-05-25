import { useState, useCallback } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { scanSubnet, testNetworkDevice, createTarget } from '@/lib/api'
import type { SubnetScanResult } from '@/lib/api'
import { useAppMode } from '@/contexts/AppModeContext'
import {
  X, Search, Loader2, Plus, Wifi, WifiOff, Router,
  Shield, Server, Radio, Printer, HelpCircle,
  CheckCircle, XCircle, ChevronDown, ChevronUp, Zap,
} from 'lucide-react'
import { cn } from '@/lib/utils'

interface Props {
  open: boolean
  onClose: () => void
}

type Tab = 'manual' | 'scan'
type Protocol = 'snmp' | 'ssh' | 'ping'

const deviceTypeIcon: Record<string, typeof Router> = {
  router: Router,
  switch: Wifi,
  firewall: Shield,
  server: Server,
  access_point: Radio,
  printer: Printer,
  host: Server,
  unknown: HelpCircle,
}

const deviceTypeColor: Record<string, string> = {
  router: 'text-blue-400 bg-blue-500/10',
  switch: 'text-cyan-400 bg-cyan-500/10',
  firewall: 'text-red-400 bg-red-500/10',
  server: 'text-emerald-400 bg-emerald-500/10',
  access_point: 'text-purple-400 bg-purple-500/10',
  printer: 'text-amber-400 bg-amber-500/10',
  host: 'text-slate-400 bg-slate-500/10',
  unknown: 'text-slate-400 bg-slate-500/10',
}

export default function AddNetworkDeviceModal({ open, onClose }: Props) {
  const qc = useQueryClient()
  const { isDemo } = useAppMode()
  const [tab, setTab] = useState<Tab>('scan')
  const [error, setError] = useState('')
  const [success, setSuccess] = useState('')

  // Manual add form
  const [manualForm, setManualForm] = useState({
    name: '',
    host: '',
    port: 161,
    protocol: 'snmp' as Protocol,
    snmp_community: 'public',
    snmp_version: '2c',
    username: '',
    password: '',
    description: '',
    subtype: 'router',
  })

  // Scan form
  const [scanForm, setScanForm] = useState({
    cidr: '10.88.11.0/24',
    snmp_community: 'public',
    snmp_version: '2c',
    timeout_ms: 3000,
  })
  const [scanResult, setScanResult] = useState<SubnetScanResult | null>(null)
  const [selectedDevices, setSelectedDevices] = useState<Set<string>>(new Set())
  const [expandedDevice, setExpandedDevice] = useState<string | null>(null)

  // Test result
  const [testResult, setTestResult] = useState<{ success: boolean; message: string } | null>(null)

  // Mutations
  const scanMut = useMutation({
    mutationFn: () => scanSubnet(scanForm.cidr, scanForm.snmp_community, scanForm.snmp_version, scanForm.timeout_ms),
    onSuccess: (data) => {
      setScanResult(data)
      setSelectedDevices(new Set(data.devices.map(d => d.ip)))
      setError('')
    },
    onError: (e: Error) => setError(e.message),
  })

  const testMut = useMutation({
    mutationFn: () => testNetworkDevice(manualForm.host, manualForm.protocol, {
      port: manualForm.port,
      snmp_community: manualForm.snmp_community,
      snmp_version: manualForm.snmp_version,
      username: manualForm.username,
      password: manualForm.password,
    }),
    onSuccess: (data) => {
      setTestResult({ success: data.success, message: data.message })
      if (data.device_type && data.success) {
        setManualForm(f => ({ ...f, subtype: data.device_type! }))
      }
    },
    onError: (e: Error) => setTestResult({ success: false, message: e.message }),
  })

  const addDeviceMut = useMutation({
    mutationFn: (device: { name: string; host: string; port: number; subtype: string; description: string }) =>
      createTarget({
        name: device.name,
        target_type: 'network',
        target_subtype: device.subtype,
        host: device.host,
        port: device.port,
        description: device.description || null,
        environment: 'production',
        ssl_enabled: false,
        is_active: true,
      }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['targets'] })
      setSuccess('Device added successfully!')
      setTimeout(() => setSuccess(''), 3000)
    },
    onError: (e: Error) => setError(e.message),
  })

  const handleManualAdd = useCallback((e: React.FormEvent) => {
    e.preventDefault()
    if (isDemo) { setError('Adding targets is disabled in Demo mode.'); return }
    if (!manualForm.name || !manualForm.host) { setError('Name and host are required'); return }
    setError('')
    addDeviceMut.mutate({
      name: manualForm.name,
      host: manualForm.host,
      port: manualForm.port,
      subtype: manualForm.subtype,
      description: manualForm.description,
    })
  }, [manualForm, isDemo, addDeviceMut])

  const handleBulkAdd = useCallback(() => {
    if (isDemo) { setError('Adding targets is disabled in Demo mode.'); return }
    if (!scanResult) return
    const devices = scanResult.devices.filter(d => selectedDevices.has(d.ip))
    if (devices.length === 0) { setError('No devices selected'); return }
    setError('')
    devices.forEach(d => {
      addDeviceMut.mutate({
        name: d.hostname || d.sys_name || `device-${d.ip}`,
        host: d.ip,
        port: d.snmp_reachable ? 161 : d.ssh_reachable ? 22 : 0,
        subtype: d.device_type,
        description: d.sys_descr || `Discovered via subnet scan (${d.vendor || 'unknown vendor'})`,
      })
    })
  }, [scanResult, selectedDevices, isDemo, addDeviceMut])

  const toggleDevice = (ip: string) => {
    setSelectedDevices(prev => {
      const next = new Set(prev)
      if (next.has(ip)) next.delete(ip); else next.add(ip)
      return next
    })
  }

  const toggleAll = () => {
    if (!scanResult) return
    if (selectedDevices.size === scanResult.devices.length) {
      setSelectedDevices(new Set())
    } else {
      setSelectedDevices(new Set(scanResult.devices.map(d => d.ip)))
    }
  }

  if (!open) return null

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm" onClick={onClose}>
      <div className="w-full max-w-3xl max-h-[85vh] bg-slate-900 border border-white/10 rounded-2xl shadow-2xl flex flex-col" onClick={e => e.stopPropagation()}>
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-white/10 flex-shrink-0">
          <div>
            <h2 className="text-lg font-bold text-white flex items-center gap-2">
              <Router className="w-5 h-5 text-brand-400" />
              Add Network Device
            </h2>
            <p className="text-xs text-slate-500 mt-0.5">Discover and add network infrastructure devices</p>
          </div>
          <button onClick={onClose} className="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 transition-colors">
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Tabs */}
        <div className="flex border-b border-white/10 px-6 flex-shrink-0">
          {(['scan', 'manual'] as Tab[]).map(t => (
            <button
              key={t}
              onClick={() => { setTab(t); setError(''); setTestResult(null) }}
              className={cn(
                'px-4 py-2.5 text-sm font-medium border-b-2 transition-colors capitalize',
                tab === t ? 'text-brand-400 border-brand-400' : 'text-slate-400 border-transparent hover:text-white'
              )}
            >
              {t === 'scan' ? '🔍 Subnet Scanner' : '✏️ Manual Add'}
            </button>
          ))}
        </div>

        {/* Body — scrollable */}
        <div className="flex-1 overflow-y-auto p-6 space-y-4">
          {error && (
            <div className="px-4 py-2.5 bg-red-500/10 border border-red-500/20 rounded-xl text-sm text-red-400">{error}</div>
          )}
          {success && (
            <div className="px-4 py-2.5 bg-emerald-500/10 border border-emerald-500/20 rounded-xl text-sm text-emerald-400 flex items-center gap-2">
              <CheckCircle className="w-4 h-4" /> {success}
            </div>
          )}

          {/* ===== SCAN TAB ===== */}
          {tab === 'scan' && (
            <>
              {/* Scan Config */}
              <div className="bg-white/[0.02] border border-white/10 rounded-xl p-4">
                <h3 className="text-sm font-bold text-white mb-3">Scan Configuration</h3>
                <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
                  <div className="col-span-2">
                    <label className="block text-[10px] font-medium text-slate-400 uppercase tracking-wider mb-1">CIDR Range *</label>
                    <input
                      value={scanForm.cidr}
                      onChange={e => setScanForm(f => ({ ...f, cidr: e.target.value }))}
                      placeholder="e.g. 192.168.1.0/24"
                      className="w-full px-3 py-2 bg-white/5 border border-white/10 rounded-lg text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-brand-500/50 font-mono"
                    />
                  </div>
                  <div>
                    <label className="block text-[10px] font-medium text-slate-400 uppercase tracking-wider mb-1">SNMP Community</label>
                    <input
                      value={scanForm.snmp_community}
                      onChange={e => setScanForm(f => ({ ...f, snmp_community: e.target.value }))}
                      placeholder="public"
                      className="w-full px-3 py-2 bg-white/5 border border-white/10 rounded-lg text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-brand-500/50"
                    />
                  </div>
                  <div>
                    <label className="block text-[10px] font-medium text-slate-400 uppercase tracking-wider mb-1">Timeout (ms)</label>
                    <input
                      type="number"
                      value={scanForm.timeout_ms}
                      onChange={e => setScanForm(f => ({ ...f, timeout_ms: parseInt(e.target.value) || 3000 }))}
                      className="w-full px-3 py-2 bg-white/5 border border-white/10 rounded-lg text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-brand-500/50"
                    />
                  </div>
                </div>
                <button
                  onClick={() => scanMut.mutate()}
                  disabled={scanMut.isPending || !scanForm.cidr}
                  className="mt-3 flex items-center gap-2 px-4 py-2 bg-brand-600 hover:bg-brand-700 rounded-lg text-sm font-medium transition-colors disabled:opacity-50"
                >
                  {scanMut.isPending ? <Loader2 className="w-4 h-4 animate-spin" /> : <Search className="w-4 h-4" />}
                  {scanMut.isPending ? 'Scanning...' : 'Start Scan'}
                </button>
              </div>

              {/* Scan Results */}
              {scanResult && (
                <div className="space-y-3">
                  {/* Summary */}
                  <div className="grid grid-cols-4 gap-3">
                    {[
                      { label: 'Scanned', value: String(scanResult.total_scanned), color: 'text-slate-300' },
                      { label: 'Found', value: String(scanResult.devices_found), color: 'text-emerald-400' },
                      { label: 'Selected', value: String(selectedDevices.size), color: 'text-brand-400' },
                      { label: 'Duration', value: `${(scanResult.scan_duration_ms / 1000).toFixed(1)}s`, color: 'text-amber-400' },
                    ].map((kpi, i) => (
                      <div key={i} className="bg-white/5 border border-white/10 rounded-lg p-2.5 text-center">
                        <div className="text-[10px] text-slate-500 uppercase">{kpi.label}</div>
                        <div className={`text-lg font-bold font-mono ${kpi.color}`}>{kpi.value}</div>
                      </div>
                    ))}
                  </div>

                  {/* Device List */}
                  <div className="bg-white/[0.02] border border-white/10 rounded-xl overflow-hidden">
                    <div className="flex items-center justify-between px-4 py-2.5 border-b border-white/10">
                      <div className="flex items-center gap-2">
                        <input
                          type="checkbox"
                          checked={selectedDevices.size === scanResult.devices.length}
                          onChange={toggleAll}
                          className="rounded border-white/20 bg-white/5"
                        />
                        <span className="text-xs font-medium text-slate-400">Select All ({scanResult.devices.length})</span>
                      </div>
                      <button
                        onClick={handleBulkAdd}
                        disabled={selectedDevices.size === 0 || addDeviceMut.isPending}
                        className="flex items-center gap-1.5 px-3 py-1.5 bg-emerald-600 hover:bg-emerald-700 rounded-lg text-xs font-medium transition-colors disabled:opacity-50"
                      >
                        {addDeviceMut.isPending ? <Loader2 className="w-3 h-3 animate-spin" /> : <Plus className="w-3 h-3" />}
                        Add {selectedDevices.size} Device{selectedDevices.size !== 1 ? 's' : ''}
                      </button>
                    </div>

                    <div className="max-h-[320px] overflow-y-auto divide-y divide-white/5">
                      {scanResult.devices.map(device => {
                        const Icon = deviceTypeIcon[device.device_type] || HelpCircle
                        const colorCls = deviceTypeColor[device.device_type] || deviceTypeColor.unknown
                        const isExpanded = expandedDevice === device.ip
                        return (
                          <div key={device.ip} className="hover:bg-white/[0.03] transition-colors">
                            <div className="flex items-center gap-3 px-4 py-2.5">
                              <input
                                type="checkbox"
                                checked={selectedDevices.has(device.ip)}
                                onChange={() => toggleDevice(device.ip)}
                                className="rounded border-white/20 bg-white/5 flex-shrink-0"
                              />
                              <div className={cn('p-1.5 rounded-lg flex-shrink-0', colorCls)}>
                                <Icon className="w-4 h-4" />
                              </div>
                              <div className="flex-1 min-w-0">
                                <div className="flex items-center gap-2">
                                  <span className="text-sm font-mono text-white">{device.ip}</span>
                                  {device.hostname && <span className="text-xs text-slate-400">({device.hostname})</span>}
                                  <span className={cn('px-1.5 py-0.5 rounded text-[9px] font-bold uppercase', colorCls)}>{device.device_type}</span>
                                  {device.vendor && <span className="text-[10px] text-slate-500">{device.vendor}</span>}
                                </div>
                                <div className="flex items-center gap-3 mt-0.5">
                                  {device.snmp_reachable && <span className="text-[10px] text-emerald-400 flex items-center gap-0.5"><Wifi className="w-2.5 h-2.5" /> SNMP</span>}
                                  {device.ssh_reachable && <span className="text-[10px] text-blue-400 flex items-center gap-0.5"><Zap className="w-2.5 h-2.5" /> SSH</span>}
                                  {device.ping_reachable && <span className="text-[10px] text-cyan-400 flex items-center gap-0.5"><Radio className="w-2.5 h-2.5" /> Ping</span>}
                                  <span className="text-[10px] text-slate-500">{device.response_time_ms.toFixed(1)}ms</span>
                                </div>
                              </div>
                              <button
                                onClick={() => setExpandedDevice(isExpanded ? null : device.ip)}
                                className="p-1 text-slate-500 hover:text-white"
                              >
                                {isExpanded ? <ChevronUp className="w-4 h-4" /> : <ChevronDown className="w-4 h-4" />}
                              </button>
                            </div>
                            {isExpanded && (
                              <div className="px-12 pb-3 space-y-1">
                                {device.sys_descr && <div className="text-[11px] text-slate-400"><span className="text-slate-500">sysDescr:</span> {device.sys_descr}</div>}
                                {device.sys_name && <div className="text-[11px] text-slate-400"><span className="text-slate-500">sysName:</span> {device.sys_name}</div>}
                                {device.mac && <div className="text-[11px] text-slate-400"><span className="text-slate-500">MAC:</span> <span className="font-mono">{device.mac}</span></div>}
                                {device.uptime && <div className="text-[11px] text-slate-400"><span className="text-slate-500">Uptime:</span> {device.uptime}</div>}
                                {device.open_ports.length > 0 && (
                                  <div className="text-[11px] text-slate-400">
                                    <span className="text-slate-500">Open Ports:</span>{' '}
                                    {device.open_ports.map(p => (
                                      <span key={p} className="inline-block bg-white/5 border border-white/10 rounded px-1.5 py-0.5 mr-1 font-mono text-[10px]">{p}</span>
                                    ))}
                                  </div>
                                )}
                              </div>
                            )}
                          </div>
                        )
                      })}
                    </div>
                  </div>
                </div>
              )}
            </>
          )}

          {/* ===== MANUAL TAB ===== */}
          {tab === 'manual' && (
            <form onSubmit={handleManualAdd} className="space-y-4">
              {/* Name + Subtype */}
              <div className="grid grid-cols-2 gap-4">
                <div>
                  <label className="block text-[10px] font-medium text-slate-400 uppercase tracking-wider mb-1">Device Name *</label>
                  <input
                    value={manualForm.name}
                    onChange={e => setManualForm(f => ({ ...f, name: e.target.value }))}
                    placeholder="e.g. core-router-01"
                    className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-brand-500/50"
                  />
                </div>
                <div>
                  <label className="block text-[10px] font-medium text-slate-400 uppercase tracking-wider mb-1">Device Type</label>
                  <select
                    value={manualForm.subtype}
                    onChange={e => setManualForm(f => ({ ...f, subtype: e.target.value }))}
                    className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white focus:outline-none focus:ring-2 focus:ring-brand-500/50 [&>option]:bg-slate-900 [&>option]:text-white"
                  >
                    {['router', 'switch', 'firewall', 'access_point', 'server', 'printer'].map(t => (
                      <option key={t} value={t}>{t.replace('_', ' ')}</option>
                    ))}
                  </select>
                </div>
              </div>

              {/* Host + Port + Protocol */}
              <div className="grid grid-cols-4 gap-4">
                <div className="col-span-2">
                  <label className="block text-[10px] font-medium text-slate-400 uppercase tracking-wider mb-1">Host / IP *</label>
                  <input
                    value={manualForm.host}
                    onChange={e => setManualForm(f => ({ ...f, host: e.target.value }))}
                    placeholder="e.g. 192.168.1.1"
                    className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-brand-500/50 font-mono"
                  />
                </div>
                <div>
                  <label className="block text-[10px] font-medium text-slate-400 uppercase tracking-wider mb-1">Port</label>
                  <input
                    type="number"
                    value={manualForm.port || ''}
                    onChange={e => setManualForm(f => ({ ...f, port: parseInt(e.target.value) || 0 }))}
                    placeholder="161"
                    className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-brand-500/50"
                  />
                </div>
                <div>
                  <label className="block text-[10px] font-medium text-slate-400 uppercase tracking-wider mb-1">Protocol</label>
                  <select
                    value={manualForm.protocol}
                    onChange={e => setManualForm(f => ({
                      ...f,
                      protocol: e.target.value as Protocol,
                      port: e.target.value === 'snmp' ? 161 : e.target.value === 'ssh' ? 22 : 0,
                    }))}
                    className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white focus:outline-none focus:ring-2 focus:ring-brand-500/50 [&>option]:bg-slate-900 [&>option]:text-white"
                  >
                    <option value="snmp">SNMP</option>
                    <option value="ssh">SSH</option>
                    <option value="ping">Ping</option>
                  </select>
                </div>
              </div>

              {/* Protocol-specific options */}
              {manualForm.protocol === 'snmp' && (
                <div className="grid grid-cols-2 gap-4">
                  <div>
                    <label className="block text-[10px] font-medium text-slate-400 uppercase tracking-wider mb-1">Community String</label>
                    <input
                      value={manualForm.snmp_community}
                      onChange={e => setManualForm(f => ({ ...f, snmp_community: e.target.value }))}
                      placeholder="public"
                      className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-brand-500/50"
                    />
                  </div>
                  <div>
                    <label className="block text-[10px] font-medium text-slate-400 uppercase tracking-wider mb-1">SNMP Version</label>
                    <select
                      value={manualForm.snmp_version}
                      onChange={e => setManualForm(f => ({ ...f, snmp_version: e.target.value }))}
                      className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white focus:outline-none focus:ring-2 focus:ring-brand-500/50 [&>option]:bg-slate-900 [&>option]:text-white"
                    >
                      <option value="2c">v2c</option>
                      <option value="3">v3</option>
                      <option value="1">v1</option>
                    </select>
                  </div>
                </div>
              )}

              {manualForm.protocol === 'ssh' && (
                <div className="grid grid-cols-2 gap-4">
                  <div>
                    <label className="block text-[10px] font-medium text-slate-400 uppercase tracking-wider mb-1">Username</label>
                    <input
                      value={manualForm.username}
                      onChange={e => setManualForm(f => ({ ...f, username: e.target.value }))}
                      placeholder="admin"
                      className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-brand-500/50"
                    />
                  </div>
                  <div>
                    <label className="block text-[10px] font-medium text-slate-400 uppercase tracking-wider mb-1">Password</label>
                    <input
                      type="password"
                      value={manualForm.password}
                      onChange={e => setManualForm(f => ({ ...f, password: e.target.value }))}
                      placeholder="••••••••"
                      className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-brand-500/50"
                    />
                  </div>
                </div>
              )}

              {/* Description */}
              <div>
                <label className="block text-[10px] font-medium text-slate-400 uppercase tracking-wider mb-1">Description</label>
                <textarea
                  value={manualForm.description}
                  onChange={e => setManualForm(f => ({ ...f, description: e.target.value }))}
                  rows={2}
                  placeholder="Optional description..."
                  className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-brand-500/50 resize-none"
                />
              </div>

              {/* Test Result */}
              {testResult && (
                <div className={cn(
                  'px-4 py-2.5 rounded-xl text-sm flex items-center gap-2 border',
                  testResult.success
                    ? 'bg-emerald-500/10 border-emerald-500/20 text-emerald-400'
                    : 'bg-red-500/10 border-red-500/20 text-red-400'
                )}>
                  {testResult.success ? <CheckCircle className="w-4 h-4" /> : <XCircle className="w-4 h-4" />}
                  {testResult.message}
                </div>
              )}

              {/* Info note */}
              <div className="px-3 py-2 bg-blue-500/5 border border-blue-500/10 rounded-xl text-[11px] text-blue-400/80">
                ℹ️ Connection tests run from the <span className="font-semibold">Zentra server</span>, not your browser. Devices on your local LAN (e.g. 192.168.x.x) may not be reachable from the server. You can still add the device — it will be monitored once network connectivity is established.
              </div>

              {/* Actions */}
              <div className="flex items-center justify-between pt-2">
                <button
                  type="button"
                  onClick={() => { setTestResult(null); testMut.mutate() }}
                  disabled={!manualForm.host || testMut.isPending}
                  className="flex items-center gap-2 px-4 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-slate-300 hover:text-white hover:bg-white/10 transition-colors disabled:opacity-50"
                >
                  {testMut.isPending ? <Loader2 className="w-4 h-4 animate-spin" /> : <WifiOff className="w-4 h-4" />}
                  Test Connection
                </button>
                <div className="flex gap-3">
                  <button
                    type="button"
                    onClick={onClose}
                    className="px-4 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-slate-400 hover:text-white transition-colors"
                  >
                    Cancel
                  </button>
                  <button
                    type="submit"
                    disabled={addDeviceMut.isPending}
                    className="flex items-center gap-2 px-5 py-2.5 bg-brand-600 hover:bg-brand-700 rounded-xl text-sm font-medium transition-colors disabled:opacity-50"
                  >
                    {addDeviceMut.isPending ? <Loader2 className="w-4 h-4 animate-spin" /> : <Plus className="w-4 h-4" />}
                    Add Device
                  </button>
                </div>
              </div>
            </form>
          )}
        </div>
      </div>
    </div>
  )
}
