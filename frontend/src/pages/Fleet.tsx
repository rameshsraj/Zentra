import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'
import {
  listAgents, deregisterAgent, listDiscoveredHosts, optInHost,
  type Agent, type DiscoveredHost,
} from '@/lib/api/fleet'
import PageHeader from '@/components/PageHeader'
import {
  Server, Trash2, Plus, Wifi, WifiOff, AlertTriangle,
  ChevronDown, ChevronRight, Eye, EyeOff, CheckCircle2, XCircle,
} from 'lucide-react'

const STATUS_STYLES: Record<string, { color: string; bg: string; label: string }> = {
  active:       { color: 'text-green-400',  bg: 'bg-green-500/20',  label: 'Active' },
  stale:        { color: 'text-yellow-400', bg: 'bg-yellow-500/20', label: 'Stale' },
  offline:      { color: 'text-red-400',    bg: 'bg-red-500/20',    label: 'Offline' },
  deregistered: { color: 'text-zinc-500',   bg: 'bg-zinc-500/20',   label: 'Deregistered' },
}

function timeAgo(iso: string | null): string {
  if (!iso) return 'Never'
  const diff = Date.now() - new Date(iso).getTime()
  const mins = Math.floor(diff / 60000)
  if (mins < 1) return 'Just now'
  if (mins < 60) return `${mins}m ago`
  const hrs = Math.floor(mins / 60)
  if (hrs < 24) return `${hrs}h ago`
  return `${Math.floor(hrs / 24)}d ago`
}

const OS_ICONS: Record<string, string> = {
  linux: '🐧', windows: '🪟', unknown: '❓',
}

// ─────────────────────────────────────────────────────────────────────────────
// Discovered hosts panel shown when an agent row is expanded
// ─────────────────────────────────────────────────────────────────────────────
function DiscoveredHostsPanel({ agentId }: { agentId: string }) {
  const queryClient = useQueryClient()
  const [showIgnored, setShowIgnored] = useState(false)

  const { data: hosts = [], isLoading } = useQuery({
    queryKey: ['discovered', agentId],
    queryFn: () => listDiscoveredHosts(agentId),
    refetchInterval: 60_000,
  })

  const patchMutation = useMutation({
    mutationFn: ({ hostId, body }: { hostId: string; body: { opted_in?: boolean; ignored?: boolean } }) =>
      optInHost(agentId, hostId, body),
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['discovered', agentId] }),
  })

  const visible = hosts.filter(h => showIgnored || !h.ignored)
  const newCount = hosts.filter(h => !h.opted_in && !h.ignored).length
  const monitoredCount = hosts.filter(h => h.opted_in).length

  if (isLoading) {
    return <p className="px-6 py-4 text-sm text-muted-foreground animate-pulse">Loading discovered hosts…</p>
  }

  if (hosts.length === 0) {
    return (
      <div className="px-6 py-5 text-sm text-muted-foreground">
        <p className="font-medium text-foreground mb-1">No hosts discovered yet</p>
        <p>The agent scans its local subnet every 10 minutes. Check back shortly after the agent is running.</p>
      </div>
    )
  }

  return (
    <div className="px-4 py-4 space-y-3">
      {/* Summary bar */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-4 text-xs text-muted-foreground">
          <span className="flex items-center gap-1">
            <span className="h-2 w-2 rounded-full bg-blue-400 inline-block" />
            {newCount} new
          </span>
          <span className="flex items-center gap-1">
            <span className="h-2 w-2 rounded-full bg-green-400 inline-block" />
            {monitoredCount} monitored
          </span>
          <span className="flex items-center gap-1">
            <span className="h-2 w-2 rounded-full bg-zinc-500 inline-block" />
            {hosts.filter(h => h.ignored).length} ignored
          </span>
        </div>
        <button
          onClick={() => setShowIgnored(v => !v)}
          className="flex items-center gap-1 text-xs text-muted-foreground hover:text-foreground transition-colors"
        >
          {showIgnored ? <EyeOff className="h-3.5 w-3.5" /> : <Eye className="h-3.5 w-3.5" />}
          {showIgnored ? 'Hide ignored' : 'Show ignored'}
        </button>
      </div>

      {/* Host table */}
      <div className="rounded-lg border border-border overflow-hidden">
        <table className="w-full text-xs">
          <thead>
            <tr className="border-b border-border bg-muted/20 text-left text-muted-foreground">
              <th className="px-3 py-2 font-medium">IP Address</th>
              <th className="px-3 py-2 font-medium">Hostname</th>
              <th className="px-3 py-2 font-medium">OS</th>
              <th className="px-3 py-2 font-medium">Open Ports</th>
              <th className="px-3 py-2 font-medium">Last Seen</th>
              <th className="px-3 py-2 font-medium">Status</th>
              <th className="px-3 py-2 font-medium text-right">Action</th>
            </tr>
          </thead>
          <tbody>
            {visible.map((h: DiscoveredHost) => (
              <tr
                key={h.id}
                className={`border-b border-border last:border-0 transition-colors ${
                  h.ignored ? 'opacity-40' : h.opted_in ? 'bg-green-500/5' : 'hover:bg-muted/20'
                }`}
              >
                <td className="px-3 py-2 font-mono font-medium">{h.ip_address}</td>
                <td className="px-3 py-2 text-muted-foreground">{h.hostname ?? '—'}</td>
                <td className="px-3 py-2">
                  {OS_ICONS[h.os_hint ?? 'unknown'] ?? '❓'}{' '}
                  <span className="text-muted-foreground capitalize">{h.os_hint ?? '—'}</span>
                </td>
                <td className="px-3 py-2 font-mono text-muted-foreground">
                  {h.open_ports?.length ? h.open_ports.join(', ') : '—'}
                </td>
                <td className="px-3 py-2 text-muted-foreground">{timeAgo(h.last_seen)}</td>
                <td className="px-3 py-2">
                  {h.opted_in ? (
                    <span className="inline-flex items-center gap-1 text-green-400">
                      <CheckCircle2 className="h-3.5 w-3.5" /> Monitored
                    </span>
                  ) : h.ignored ? (
                    <span className="inline-flex items-center gap-1 text-zinc-500">
                      <XCircle className="h-3.5 w-3.5" /> Ignored
                    </span>
                  ) : (
                    <span className="text-blue-400">New</span>
                  )}
                </td>
                <td className="px-3 py-2 text-right">
                  <div className="flex items-center justify-end gap-2">
                    {!h.opted_in && !h.ignored && (
                      <>
                        <button
                          onClick={() => patchMutation.mutate({ hostId: h.id, body: { opted_in: true } })}
                          disabled={patchMutation.isPending}
                          className="rounded px-2 py-1 bg-green-600 text-white hover:bg-green-700 transition-colors disabled:opacity-50"
                        >
                          Monitor
                        </button>
                        <button
                          onClick={() => patchMutation.mutate({ hostId: h.id, body: { ignored: true } })}
                          disabled={patchMutation.isPending}
                          className="rounded px-2 py-1 bg-zinc-700 text-muted-foreground hover:bg-zinc-600 transition-colors disabled:opacity-50"
                        >
                          Ignore
                        </button>
                      </>
                    )}
                    {h.opted_in && (
                      <button
                        onClick={() => patchMutation.mutate({ hostId: h.id, body: { opted_in: false } })}
                        disabled={patchMutation.isPending}
                        className="rounded px-2 py-1 bg-zinc-700 text-muted-foreground hover:bg-zinc-600 transition-colors text-xs disabled:opacity-50"
                      >
                        Remove
                      </button>
                    )}
                    {h.ignored && (
                      <button
                        onClick={() => patchMutation.mutate({ hostId: h.id, body: { ignored: false } })}
                        disabled={patchMutation.isPending}
                        className="rounded px-2 py-1 bg-zinc-700 text-muted-foreground hover:bg-zinc-600 transition-colors text-xs disabled:opacity-50"
                      >
                        Unignore
                      </button>
                    )}
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}

// ─────────────────────────────────────────────────────────────────────────────
// Main Fleet page
// ─────────────────────────────────────────────────────────────────────────────
export default function FleetPage() {
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const [confirmId, setConfirmId] = useState<string | null>(null)
  const [expandedId, setExpandedId] = useState<string | null>(null)

  const { data: agents = [], isLoading } = useQuery({
    queryKey: ['agents'],
    queryFn: listAgents,
    refetchInterval: 15_000,
  })

  const removeMutation = useMutation({
    mutationFn: deregisterAgent,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['agents'] })
      setConfirmId(null)
    },
  })

  return (
    <div className="space-y-6">
      <PageHeader
        title="Agent Fleet"
        subtitle="Manage your monitoring agents across all datacenters."
      />

      <div className="flex justify-end">
        <button
          onClick={() => navigate('/fleet/install')}
          className="flex items-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 transition-colors"
        >
          <Plus className="h-4 w-4" />
          Register new datacenter
        </button>
      </div>

      {isLoading ? (
        <div className="flex items-center justify-center py-20 text-sm text-muted-foreground">
          Loading agents…
        </div>
      ) : agents.length === 0 ? (
        <div className="flex flex-col items-center justify-center py-20 space-y-4">
          <WifiOff className="h-12 w-12 text-muted-foreground" />
          <p className="text-muted-foreground">No agents registered yet.</p>
          <button
            onClick={() => navigate('/fleet/install')}
            className="rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700"
          >
            Install your first agent
          </button>
        </div>
      ) : (
        <div className="rounded-xl border border-border bg-card overflow-hidden">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-border text-left text-muted-foreground">
                <th className="px-4 py-3 font-medium w-8" />
                <th className="px-4 py-3 font-medium">Datacenter</th>
                <th className="px-4 py-3 font-medium">Status</th>
                <th className="px-4 py-3 font-medium">Last Seen</th>
                <th className="px-4 py-3 font-medium">Version</th>
                <th className="px-4 py-3 font-medium">Targets</th>
                <th className="px-4 py-3 font-medium">IP</th>
                <th className="px-4 py-3 font-medium" />
              </tr>
            </thead>
            <tbody>
              {agents.map((a: Agent) => {
                const s = STATUS_STYLES[a.status] ?? STATUS_STYLES.offline
                const isExpanded = expandedId === a.agent_id
                return (
                  <>
                    <tr
                      key={a.agent_id}
                      className="border-b border-border hover:bg-muted/30 transition-colors cursor-pointer"
                      onClick={() => setExpandedId(isExpanded ? null : a.agent_id)}
                    >
                      <td className="px-4 py-3 text-muted-foreground">
                        {isExpanded
                          ? <ChevronDown className="h-4 w-4" />
                          : <ChevronRight className="h-4 w-4" />}
                      </td>
                      <td className="px-4 py-3 font-medium">
                        <div>{a.datacenter_name}</div>
                        {a.region_label && (
                          <div className="text-xs text-muted-foreground">{a.region_label}</div>
                        )}
                      </td>
                      <td className="px-4 py-3">
                        <span className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium ${s.bg} ${s.color}`}>
                          {a.status === 'active'
                            ? <Wifi className="h-3 w-3" />
                            : a.status === 'offline'
                            ? <WifiOff className="h-3 w-3" />
                            : <AlertTriangle className="h-3 w-3" />}
                          {s.label}
                        </span>
                      </td>
                      <td className="px-4 py-3 text-muted-foreground">{timeAgo(a.last_seen)}</td>
                      <td className="px-4 py-3 font-mono text-xs">{a.agent_version ?? '—'}</td>
                      <td className="px-4 py-3">{a.active_target_count}</td>
                      <td className="px-4 py-3 font-mono text-xs text-muted-foreground">{a.ip_address ?? '—'}</td>
                      <td className="px-4 py-3 text-right" onClick={e => e.stopPropagation()}>
                        {a.status !== 'deregistered' && (
                          confirmId === a.agent_id ? (
                            <div className="flex items-center gap-2 justify-end">
                              <span className="text-xs text-red-400">Confirm?</span>
                              <button
                                onClick={() => removeMutation.mutate(a.agent_id)}
                                className="rounded bg-red-600 px-2 py-1 text-xs text-white hover:bg-red-700"
                                disabled={removeMutation.isPending}
                              >
                                Yes
                              </button>
                              <button
                                onClick={() => setConfirmId(null)}
                                className="rounded bg-zinc-600 px-2 py-1 text-xs text-white hover:bg-zinc-700"
                              >
                                No
                              </button>
                            </div>
                          ) : (
                            <button
                              onClick={() => setConfirmId(a.agent_id)}
                              className="rounded p-1.5 text-muted-foreground hover:text-red-400 hover:bg-red-500/10 transition-colors"
                              title="Deregister agent"
                            >
                              <Trash2 className="h-4 w-4" />
                            </button>
                          )
                        )}
                      </td>
                    </tr>
                    {isExpanded && (
                      <tr key={`${a.agent_id}-discovered`} className="bg-muted/10 border-b border-border">
                        <td colSpan={8} className="p-0">
                          <div className="border-t border-dashed border-border/50">
                            <div className="flex items-center gap-2 px-6 pt-3 pb-1 text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                              <Server className="h-3.5 w-3.5" />
                              Discovered Hosts
                            </div>
                            <DiscoveredHostsPanel agentId={a.agent_id} />
                          </div>
                        </td>
                      </tr>
                    )}
                  </>
                )
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}
