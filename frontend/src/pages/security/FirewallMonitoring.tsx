import { useState, useCallback } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { getFirewallLogs, getFirewallStats, getNetworkDevices, getFirewallRules } from '../../lib/api'
import type { NetworkDeviceStatus } from '../../lib/api'
import AddNetworkDeviceModal from '../../components/AddNetworkDeviceModal'
import { Flame, RefreshCw, Loader2, Shield, Plus, Activity, Wifi, WifiOff, List, Settings } from 'lucide-react'

const actionColor = (a: string) =>
  a === 'accept' ? 'bg-green-600/20 text-green-400' : a === 'drop' ? 'bg-red-600/20 text-red-400' : 'bg-orange-600/20 text-orange-400'

type ViewTab = 'logs' | 'rules' | 'devices'

export default function FirewallMonitoring() {
  const qc = useQueryClient()
  const [offset, setOffset] = useState(0)
  const [actionFilter, setActionFilter] = useState('')
  const [selectedFw, setSelectedFw] = useState<string>('')
  const [showAddModal, setShowAddModal] = useState(false)
  const [viewTab, setViewTab] = useState<ViewTab>('logs')

  const params: Record<string, string> = { limit: '50', offset: String(offset) }
  if (actionFilter) params.action = actionFilter

  const { data, isLoading } = useQuery({ queryKey: ['fw-logs', offset, actionFilter], queryFn: () => getFirewallLogs(params) })
  const { data: stats } = useQuery({ queryKey: ['fw-stats'], queryFn: getFirewallStats })
  const { data: allDevices } = useQuery({ queryKey: ['network-devices'], queryFn: getNetworkDevices })
  const { data: rulesData } = useQuery({ queryKey: ['fw-rules'], queryFn: getFirewallRules, enabled: viewTab === 'rules' })

  const firewalls: NetworkDeviceStatus[] = (allDevices || []).filter((d: NetworkDeviceStatus) =>
    d.subtype === 'firewall' || d.device_type === 'firewall'
  )

  const refresh = useCallback(() => {
    qc.invalidateQueries({ queryKey: ['fw-logs'] })
    qc.invalidateQueries({ queryKey: ['fw-stats'] })
    qc.invalidateQueries({ queryKey: ['network-devices'] })
    qc.invalidateQueries({ queryKey: ['fw-rules'] })
  }, [qc])

  const entries = data?.entries || []
  const total = data?.total || 0
  const rules = rulesData?.rules || []

  return (
    <div className="space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="p-2 rounded-lg bg-red-600/20"><Flame className="w-5 h-5 text-red-400" /></div>
          <div>
            <h1 className="text-2xl font-bold text-white">Firewall Monitoring</h1>
            <p className="text-sm text-gray-400">{firewalls.length} device{firewalls.length !== 1 ? 's' : ''} · {total} events</p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <button onClick={() => setShowAddModal(true)}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs font-medium text-white bg-gradient-to-r from-red-500 to-red-600 hover:from-red-600 hover:to-red-700 transition-all">
            <Plus className="w-3.5 h-3.5" /> Add Firewall
          </button>
          <button onClick={refresh} className="p-2 rounded-lg hover:bg-gray-700 text-gray-400"><RefreshCw className="w-4 h-4" /></button>
        </div>
      </div>

      {/* Firewall Devices */}
      {firewalls.length > 0 && (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-3">
          {firewalls.map(fw => {
            const isUp = fw.status === 'up' || fw.snmp_reachable
            return (
              <button key={fw.target_id}
                onClick={() => setSelectedFw(selectedFw === fw.target_id ? '' : fw.target_id)}
                className={`text-left bg-gray-800/50 border rounded-xl p-4 transition-all hover:bg-gray-700/30 ${
                  selectedFw === fw.target_id ? 'border-red-500/60 ring-1 ring-red-500/30' : 'border-gray-700/50'
                }`}>
                <div className="flex items-center justify-between mb-2">
                  <div className="flex items-center gap-2">
                    <Shield className={`w-4 h-4 ${isUp ? 'text-red-400' : 'text-gray-600'}`} />
                    <span className="text-sm font-medium text-white">{fw.name}</span>
                  </div>
                  {isUp
                    ? <span className="flex items-center gap-1 text-[10px] text-green-400"><Wifi className="w-3 h-3" /> Up</span>
                    : <span className="flex items-center gap-1 text-[10px] text-red-400"><WifiOff className="w-3 h-3" /> Down</span>
                  }
                </div>
                <div className="space-y-1 text-[11px]">
                  <div className="flex justify-between">
                    <span className="text-gray-500">Host</span>
                    <span className="text-gray-300 font-mono">{fw.host}:{fw.port}</span>
                  </div>
                  <div className="flex justify-between">
                    <span className="text-gray-500">Type</span>
                    <span className="text-gray-300 capitalize">{fw.device_type || fw.subtype}</span>
                  </div>
                  {fw.uptime && (
                    <div className="flex justify-between">
                      <span className="text-gray-500">Uptime</span>
                      <span className="text-gray-300">{fw.uptime}</span>
                    </div>
                  )}
                  {fw.vendor && (
                    <div className="flex justify-between">
                      <span className="text-gray-500">Vendor</span>
                      <span className="text-gray-300">{fw.vendor}</span>
                    </div>
                  )}
                </div>
              </button>
            )
          })}

          {/* Add new card */}
          <button onClick={() => setShowAddModal(true)}
            className="flex flex-col items-center justify-center bg-gray-800/30 border border-dashed border-gray-700/50 rounded-xl p-4 hover:bg-gray-700/20 hover:border-gray-600 transition-all min-h-[120px]">
            <Plus className="w-5 h-5 text-gray-500 mb-1" />
            <span className="text-xs text-gray-500">Add Firewall</span>
          </button>
        </div>
      )}

      {firewalls.length === 0 && (
        <div className="bg-gray-800/50 border border-gray-700/50 rounded-xl p-8 text-center">
          <Shield className="w-10 h-10 text-gray-600 mx-auto mb-3" />
          <h3 className="text-sm font-medium text-white mb-1">No Firewall Devices Configured</h3>
          <p className="text-xs text-gray-500 mb-4">Add your first firewall device to start monitoring traffic and events.</p>
          <button onClick={() => setShowAddModal(true)}
            className="inline-flex items-center gap-1.5 px-4 py-2 rounded-lg text-xs font-medium text-white bg-gradient-to-r from-red-500 to-red-600 hover:from-red-600 hover:to-red-700 transition-all">
            <Plus className="w-3.5 h-3.5" /> Add Firewall Device
          </button>
        </div>
      )}

      {/* View Tabs */}
      <div className="flex items-center gap-1 border-b border-gray-700/50 pb-px">
        {([
          { key: 'logs' as ViewTab, label: 'Event Logs', icon: List },
          { key: 'rules' as ViewTab, label: 'Firewall Rules', icon: Settings },
          { key: 'devices' as ViewTab, label: 'Device Details', icon: Activity },
        ]).map(t => (
          <button key={t.key} onClick={() => setViewTab(t.key)}
            className={`flex items-center gap-1.5 px-3 py-2 text-xs font-medium rounded-t-lg transition-colors ${
              viewTab === t.key ? 'bg-gray-800 text-white border-b-2 border-red-500' : 'text-gray-500 hover:text-gray-300'
            }`}>
            <t.icon className="w-3.5 h-3.5" /> {t.label}
          </button>
        ))}
      </div>

      {/* ── Logs Tab ── */}
      {viewTab === 'logs' && (
        <>
          {/* Stats */}
          {stats && (
            <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
              <div className="bg-gray-800/50 border border-gray-700/50 rounded-xl p-4">
                <p className="text-xs text-gray-500 uppercase">Total Events</p>
                <p className="text-2xl font-bold text-white">{stats.total_events?.toLocaleString()}</p>
              </div>
              <div className="bg-gray-800/50 border border-gray-700/50 rounded-xl p-4">
                <p className="text-xs text-gray-500 uppercase">Action Breakdown</p>
                <div className="mt-2 space-y-1">
                  {(stats.action_distribution || []).map((a: any) => (
                    <div key={a.action} className="flex justify-between text-xs"><span className={`px-1.5 py-0.5 rounded ${actionColor(a.action)}`}>{a.action}</span><span className="text-gray-300 font-mono">{a.count}</span></div>
                  ))}
                </div>
              </div>
              <div className="bg-gray-800/50 border border-gray-700/50 rounded-xl p-4">
                <p className="text-xs text-gray-500 uppercase">Top Blocked IPs</p>
                <div className="mt-2 space-y-1">
                  {(stats.top_blocked_ips || []).slice(0, 5).map((b: any) => (
                    <div key={b.ip} className="flex justify-between text-xs"><span className="font-mono text-gray-300">{b.ip}</span><span className="text-red-400 font-mono">{b.count}</span></div>
                  ))}
                </div>
              </div>
            </div>
          )}

          {/* Filter */}
          <div className="flex gap-2">
            {['', 'accept', 'drop', 'reject'].map(a => (
              <button key={a} onClick={() => { setActionFilter(a); setOffset(0) }}
                className={`px-3 py-1 rounded text-xs capitalize ${actionFilter === a ? 'bg-red-600 text-white' : 'bg-gray-700 text-gray-400 hover:text-white'}`}>
                {a || 'All'}
              </button>
            ))}
          </div>

          {/* Table */}
          <div className="bg-gray-800/50 border border-gray-700/50 rounded-xl overflow-hidden">
            {isLoading ? <div className="p-8 text-center"><Loader2 className="w-5 h-5 animate-spin inline text-gray-500" /></div> : (
              <table className="w-full text-xs">
                <thead><tr className="text-gray-400 border-b border-gray-700/50 text-left">
                  <th className="p-3">Time</th><th className="p-3">Source</th><th className="p-3">Destination</th>
                  <th className="p-3">Proto</th><th className="p-3">Action</th><th className="p-3">Rule</th><th className="p-3">Interface</th>
                </tr></thead>
                <tbody>{entries.map((e: any) => (
                  <tr key={e.id} className="border-b border-gray-700/30 hover:bg-gray-700/20">
                    <td className="p-3 text-gray-400 whitespace-nowrap">{new Date(e.timestamp).toLocaleTimeString()}</td>
                    <td className="p-3 font-mono text-gray-300">{e.src_ip || '—'}:{e.src_port || 0}</td>
                    <td className="p-3 font-mono text-gray-300">{e.dest_ip || '—'}:{e.dest_port || 0}</td>
                    <td className="p-3 text-gray-400">{e.proto || '—'}</td>
                    <td className="p-3"><span className={`px-1.5 py-0.5 rounded text-[10px] ${actionColor(e.action)}`}>{e.action}</span></td>
                    <td className="p-3 text-gray-400">{e.rule_name || '—'}</td>
                    <td className="p-3 text-gray-400">{e.interface || '—'}</td>
                  </tr>
                ))}</tbody>
              </table>
            )}
          </div>

          {total > 50 && (
            <div className="flex items-center justify-center gap-3">
              <button disabled={offset === 0} onClick={() => setOffset(Math.max(0, offset - 50))} className="px-3 py-1 rounded text-sm bg-gray-700 text-gray-300 disabled:opacity-50">Previous</button>
              <span className="text-sm text-gray-400">{offset + 1}–{Math.min(offset + 50, total)} of {total}</span>
              <button disabled={offset + 50 >= total} onClick={() => setOffset(offset + 50)} className="px-3 py-1 rounded text-sm bg-gray-700 text-gray-300 disabled:opacity-50">Next</button>
            </div>
          )}
        </>
      )}

      {/* ── Rules Tab ── */}
      {viewTab === 'rules' && (
        <div className="bg-gray-800/50 border border-gray-700/50 rounded-xl overflow-hidden">
          <table className="w-full text-xs">
            <thead><tr className="text-gray-400 border-b border-gray-700/50 text-left">
              <th className="p-3">Rule Name</th><th className="p-3">Action</th><th className="p-3">Proto</th>
              <th className="p-3">Source</th><th className="p-3">Destination</th><th className="p-3">Port</th>
              <th className="p-3">Interface</th><th className="p-3">Enabled</th><th className="p-3">Hits</th>
            </tr></thead>
            <tbody>
              {rules.map((r: any) => (
                <tr key={r.id} className="border-b border-gray-700/30 hover:bg-gray-700/20">
                  <td className="p-3 text-white font-medium">{r.rule_name}</td>
                  <td className="p-3"><span className={`px-1.5 py-0.5 rounded text-[10px] ${actionColor(r.action)}`}>{r.action}</span></td>
                  <td className="p-3 text-gray-400">{r.proto || 'any'}</td>
                  <td className="p-3 font-mono text-gray-300">{r.src_cidr || 'any'}</td>
                  <td className="p-3 font-mono text-gray-300">{r.dest_cidr || 'any'}</td>
                  <td className="p-3 text-gray-400">{r.dest_port || 'any'}</td>
                  <td className="p-3 text-gray-400">{r.interface || 'all'}</td>
                  <td className="p-3">{r.enabled ? <span className="text-green-400">Yes</span> : <span className="text-gray-600">No</span>}</td>
                  <td className="p-3 font-mono text-gray-300">{(r.hit_count || 0).toLocaleString()}</td>
                </tr>
              ))}
              {rules.length === 0 && (
                <tr><td colSpan={9} className="p-8 text-center text-gray-500">No firewall rules configured</td></tr>
              )}
            </tbody>
          </table>
        </div>
      )}

      {/* ── Devices Tab ── */}
      {viewTab === 'devices' && (
        <div className="space-y-4">
          {firewalls.length === 0 ? (
            <div className="bg-gray-800/50 border border-gray-700/50 rounded-xl p-8 text-center">
              <p className="text-sm text-gray-500">No firewall devices registered. Click "Add Firewall" to get started.</p>
            </div>
          ) : (
            <div className="bg-gray-800/50 border border-gray-700/50 rounded-xl overflow-hidden">
              <table className="w-full text-xs">
                <thead><tr className="text-gray-400 border-b border-gray-700/50 text-left">
                  <th className="p-3">Name</th><th className="p-3">Type</th><th className="p-3">Host</th>
                  <th className="p-3">Status</th><th className="p-3">Vendor</th><th className="p-3">Uptime</th>
                  <th className="p-3">Response</th><th className="p-3">Last Check</th>
                </tr></thead>
                <tbody>
                  {firewalls.map(fw => {
                    const isUp = fw.status === 'up' || fw.snmp_reachable
                    return (
                      <tr key={fw.target_id} className="border-b border-gray-700/30 hover:bg-gray-700/20">
                        <td className="p-3">
                          <div className="flex items-center gap-2">
                            <Shield className="w-3.5 h-3.5 text-red-400" />
                            <span className="text-white font-medium">{fw.name}</span>
                          </div>
                        </td>
                        <td className="p-3 text-gray-400 capitalize">{fw.device_type || fw.subtype}</td>
                        <td className="p-3 font-mono text-gray-300">{fw.host}:{fw.port}</td>
                        <td className="p-3">
                          {isUp
                            ? <span className="flex items-center gap-1 text-green-400"><span className="w-1.5 h-1.5 rounded-full bg-green-400" /> Up</span>
                            : <span className="flex items-center gap-1 text-red-400"><span className="w-1.5 h-1.5 rounded-full bg-red-400" /> Down</span>
                          }
                        </td>
                        <td className="p-3 text-gray-300">{fw.vendor || '—'}</td>
                        <td className="p-3 text-gray-300">{fw.uptime || '—'}</td>
                        <td className="p-3 text-gray-300">{fw.response_time_ms != null ? `${fw.response_time_ms}ms` : '—'}</td>
                        <td className="p-3 text-gray-400">{fw.last_check_at ? new Date(fw.last_check_at).toLocaleString() : '—'}</td>
                      </tr>
                    )
                  })}
                </tbody>
              </table>
            </div>
          )}
        </div>
      )}

      {/* Add Firewall Modal */}
      <AddNetworkDeviceModal open={showAddModal} onClose={() => { setShowAddModal(false); refresh() }} />
    </div>
  )
}
