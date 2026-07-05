import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'
import {
  listAgents, deregisterAgent, getComponentsByDc, confirmComponents, getAgentLogsByDc,
  listOrgEnrollmentKeys, createOrgEnrollmentKey, updateOrgEnrollmentKey, revokeOrgEnrollmentKey,
  listOrgEnrollmentRegistrations,
  type Agent, type DiscoveredComponent, type AgentLogLine, type OrgEnrollmentKey,
} from '@/lib/api/fleet'
import PageHeader from '@/components/PageHeader'
import {
  Server, Trash2, Plus, Wifi, WifiOff, AlertTriangle,
  ChevronDown, ChevronRight, CheckCircle2, ScrollText, RefreshCw, KeyRound, Copy,
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

// ─────────────────────────────────────────────────────────────────────────────
// Discovered components panel — shows services / log dirs found on the agent host
// ─────────────────────────────────────────────────────────────────────────────
const TYPE_LABELS: Record<string, string> = {
  linux: 'OS Host', windows: 'OS Host', oracle: 'Database',
  redis: 'Cache', nginx: 'Web Server', apache: 'Web Server',
  haproxy: 'Load Balancer', tomcat: 'App Server', phpfpm: 'PHP-FPM',
  elasticsearch: 'Search', kafka: 'Messaging', rabbitmq: 'Messaging',
  memcached: 'Cache', log_directory: 'Log Dir', network_interface: 'Network',
}

function DiscoveredComponentsPanel({ dcName }: { dcName: string }) {
  const qc = useQueryClient()
  const { data, isLoading } = useQuery({
    queryKey: ['dc-components', dcName],
    queryFn: () => getComponentsByDc(dcName),
    refetchInterval: 30_000,
  })

  const [confirming, setConfirming] = useState<string | null>(null)

  const confirmMutation = useMutation({
    mutationFn: (componentId: string) =>
      confirmComponents({ component_ids: [componentId], datacenter_name: dcName }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['dc-components', dcName] })
      qc.invalidateQueries({ queryKey: ['targets'] })
      setConfirming(null)
    },
    onSettled: () => setConfirming(null),
  })

  const components: DiscoveredComponent[] = data?.components ?? []

  if (isLoading) {
    return <p className="px-6 py-4 text-sm text-muted-foreground animate-pulse">Loading components…</p>
  }

  if (components.length === 0) {
    return (
      <div className="px-6 py-5 text-sm text-muted-foreground">
        <p className="font-medium text-foreground mb-1">No components discovered yet</p>
        <p>The agent scans for running services on startup and reports them within 30 seconds. If nothing appears, check the Agent Logs panel below for errors.</p>
      </div>
    )
  }

  return (
    <div className="px-4 py-4">
      <div className="rounded-lg border border-border overflow-hidden">
        <table className="w-full text-xs">
          <thead>
            <tr className="border-b border-border bg-muted/20 text-left text-muted-foreground">
              <th className="px-3 py-2 font-medium">Component</th>
              <th className="px-3 py-2 font-medium">Type</th>
              <th className="px-3 py-2 font-medium">Address</th>
              <th className="px-3 py-2 font-medium">Version</th>
              <th className="px-3 py-2 font-medium">Status</th>
            </tr>
          </thead>
          <tbody>
            {components.map((c: DiscoveredComponent) => (
              <tr key={c.id} className="border-b border-border last:border-0 hover:bg-muted/20">
                <td className="px-3 py-2 font-medium">{c.component_name}</td>
                <td className="px-3 py-2 text-muted-foreground">{TYPE_LABELS[c.component_type] ?? c.component_type}</td>
                <td className="px-3 py-2 font-mono text-muted-foreground">{c.address}{c.port ? `:${c.port}` : ''}</td>
                <td className="px-3 py-2 text-muted-foreground">{c.version ?? '—'}</td>
                <td className="px-3 py-2">
                  {c.confirmed ? (
                    <span className="inline-flex items-center gap-1 text-green-400">
                      <CheckCircle2 className="h-3.5 w-3.5" /> Monitoring
                    </span>
                  ) : (
                    <button
                      onClick={() => { setConfirming(c.id); confirmMutation.mutate(c.id) }}
                      disabled={confirming === c.id}
                      className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-xs font-medium bg-blue-500/20 text-blue-400 hover:bg-blue-500/30 disabled:opacity-50 transition-colors"
                    >
                      {confirming === c.id ? 'Adding…' : '+ Monitor'}
                    </button>
                  )}
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
// Agent logs panel — shows tailed log lines shipped from the agent host
// ─────────────────────────────────────────────────────────────────────────────
const SEV_STYLES: Record<string, string> = {
  ERROR: 'bg-red-500/20 text-red-400 border border-red-500/30',
  WARN:  'bg-yellow-500/20 text-yellow-400 border border-yellow-500/30',
  WARNING: 'bg-yellow-500/20 text-yellow-400 border border-yellow-500/30',
  INFO:  'bg-blue-500/20 text-blue-400 border border-blue-500/30',
  DEBUG: 'bg-zinc-500/20 text-zinc-400 border border-zinc-500/30',
}

function AgentLogsPanel({ dcName }: { dcName: string }) {
  const [filter, setFilter] = useState<string>('ALL')
  const { data, isLoading } = useQuery({
    queryKey: ['agent-logs', dcName],
    queryFn: () => getAgentLogsByDc(dcName),
    refetchInterval: 15_000,
  })

  const lines: AgentLogLine[] = data?.lines ?? []
  const filtered = filter === 'ALL' ? lines : lines.filter(l => l.level.toUpperCase() === filter)

  return (
    <div className="px-4 py-4 space-y-3">
      {/* Filter bar */}
      <div className="flex items-center gap-2">
        {['ALL', 'ERROR', 'WARN', 'INFO', 'DEBUG'].map(f => (
          <button
            key={f}
            onClick={() => setFilter(f)}
            className={`px-2.5 py-0.5 rounded text-xs font-medium transition-colors ${
              filter === f
                ? 'bg-blue-600 text-white'
                : 'bg-muted/40 text-muted-foreground hover:bg-muted/70'
            }`}
          >
            {f}
          </button>
        ))}
        <span className="ml-auto text-xs text-muted-foreground">{filtered.length} lines</span>
      </div>

      {isLoading ? (
        <p className="text-xs text-muted-foreground animate-pulse">Loading logs…</p>
      ) : filtered.length === 0 ? (
        <p className="text-xs text-muted-foreground">No log lines received yet. Agent ships logs from confirmed log directories.</p>
      ) : (
        <div className="rounded-lg border border-border bg-zinc-950 overflow-hidden">
          <div className="max-h-72 overflow-y-auto font-mono text-xs divide-y divide-border/30">
            {filtered.slice(-200).reverse().map((line, i) => {
              const sev = (line.level ?? 'INFO').toUpperCase()
              const sevStyle = SEV_STYLES[sev] ?? SEV_STYLES.INFO
              const ts = line.timestamp ? new Date(line.timestamp).toLocaleTimeString() : ''
              return (
                <div key={i} className="flex items-start gap-2 px-3 py-1.5 hover:bg-white/5">
                  <span className="shrink-0 text-zinc-600 w-20">{ts}</span>
                  <span className={`shrink-0 inline-block px-1.5 py-0 rounded text-[10px] font-bold leading-5 ${sevStyle}`}>{sev}</span>
                  {line.target && (
                    <span className="shrink-0 text-purple-400 max-w-[120px] truncate">{line.target}</span>
                  )}
                  <span className="text-zinc-300 break-all">{line.message}</span>
                </div>
              )
            })}
          </div>
        </div>
      )}
    </div>
  )
}

function EnrollmentKeyPanel() {
  const qc = useQueryClient()
  const [newKey, setNewKey] = useState<string>('')

  const { data: keys = [] } = useQuery({
    queryKey: ['org-enrollment-keys'],
    queryFn: listOrgEnrollmentKeys,
    refetchInterval: 30_000,
  })

  const activeKey = keys[0] as OrgEnrollmentKey | undefined

  const { data: registrations = [] } = useQuery({
    queryKey: ['org-enrollment-registrations', activeKey?.id],
    queryFn: () => listOrgEnrollmentRegistrations(activeKey!.id),
    enabled: !!activeKey?.id,
    refetchInterval: 30_000,
  })

  const createMutation = useMutation({
    mutationFn: () => createOrgEnrollmentKey({}),
    onSuccess: (data) => {
      setNewKey(data.org_key)
      qc.invalidateQueries({ queryKey: ['org-enrollment-keys'] })
    },
  })

  const revokeMutation = useMutation({
    mutationFn: (id: string) => revokeOrgEnrollmentKey(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['org-enrollment-keys'] }),
  })

  const toggleApproval = useMutation({
    mutationFn: ({ id, value }: { id: string; value: boolean }) =>
      updateOrgEnrollmentKey(id, { require_approval: value }),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['org-enrollment-keys'] }),
  })

  return (
    <div className="rounded-xl border border-border bg-card p-5 space-y-4">
      <div className="flex items-start justify-between gap-3">
        <div>
          <h3 className="text-base font-semibold flex items-center gap-2">
            <KeyRound className="h-4 w-4 text-blue-400" />
            Organization Enrollment Key
          </h3>
          <p className="text-xs text-muted-foreground mt-1">Install agents using one shared isok_ key. Each server receives its own isak_ key during registration.</p>
        </div>
        <button
          onClick={() => createMutation.mutate()}
          className="rounded-lg bg-blue-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-blue-700"
        >
          Generate New Key
        </button>
      </div>

      {(newKey || activeKey) && (
        <div className="rounded-lg border border-border p-3 bg-muted/20 space-y-3">
          <div className="flex items-center gap-2">
            <span className="font-mono text-sm">{newKey || `${activeKey?.display_prefix}••••••••••`}</span>
            {newKey && (
              <button
                onClick={() => navigator.clipboard.writeText(newKey)}
                className="rounded px-2 py-1 text-xs bg-zinc-700 text-zinc-100 hover:bg-zinc-600 inline-flex items-center gap-1"
              >
                <Copy className="h-3 w-3" /> Copy
              </button>
            )}
          </div>

          {activeKey && (
            <div className="flex flex-wrap items-center gap-3 text-xs">
              <span className="text-muted-foreground">Max registrations/hour: <span className="text-foreground">{activeKey.max_registrations_per_hour}</span></span>
              <span className="text-muted-foreground">Allowed CIDR: <span className="text-foreground">{activeKey.allowed_cidr ?? 'Any'}</span></span>
              <label className="inline-flex items-center gap-1 text-muted-foreground">
                <input
                  type="checkbox"
                  checked={activeKey.require_approval}
                  onChange={(e) => toggleApproval.mutate({ id: activeKey.id, value: e.target.checked })}
                />
                Require manual approval
              </label>
              <button
                onClick={() => revokeMutation.mutate(activeKey.id)}
                className="rounded bg-red-600 px-2 py-1 text-white hover:bg-red-700"
              >
                Revoke Key
              </button>
            </div>
          )}
        </div>
      )}

      {registrations.length > 0 && (
        <div className="space-y-2">
          <p className="text-xs font-semibold text-muted-foreground uppercase tracking-wide">Recent Registrations</p>
          <div className="rounded-lg border border-border overflow-hidden">
            {registrations.slice(0, 5).map((r) => (
              <div key={r.id} className="px-3 py-2 text-xs border-b last:border-b-0 border-border flex items-center justify-between">
                <span className="font-mono text-muted-foreground">{r.request_ip ?? 'n/a'}</span>
                <span>{r.hostname ?? 'unknown-host'}</span>
                <span className="text-muted-foreground">{timeAgo(r.created_at)}</span>
                <span className={r.status === 'registered' ? 'text-green-400' : 'text-yellow-400'}>{r.status}</span>
              </div>
            ))}
          </div>
        </div>
      )}
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

  const { data: agents = [], isLoading, isFetching, refetch } = useQuery({
    queryKey: ['agents'],
    queryFn: listAgents,
    refetchInterval: 15_000,
  })

  const [removePending, setRemovePending] = useState<string | null>(null)
  const handleDeregister = async (agentId: string) => {
    setRemovePending(agentId)
    try {
      await deregisterAgent(agentId)
      queryClient.invalidateQueries({ queryKey: ['agents'] })
      setConfirmId(null)
    } finally {
      setRemovePending(null)
    }
  }

  return (
    <div className="space-y-6">
      <PageHeader
        title="Agent Fleet"
        subtitle="Manage your monitoring agents across all datacenters."
      />

      <EnrollmentKeyPanel />

      <div className="flex items-center justify-end gap-2">
        <button
          onClick={() => refetch()}
          disabled={isFetching}
          className="flex items-center justify-center rounded-lg border border-border p-2 text-muted-foreground hover:text-foreground hover:bg-muted/40 disabled:opacity-40 transition-colors"
          title="Refresh"
        >
          <RefreshCw className={`h-4 w-4 ${isFetching ? 'animate-spin' : ''}`} />
        </button>
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
                <th className="px-4 py-3 font-medium">Key Status</th>
                <th className="px-4 py-3 font-medium">Key Expiry</th>
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
                      <td className="px-4 py-3 font-mono text-xs text-muted-foreground">{a.registered_ip ?? a.ip_address ?? '—'}</td>
                      <td className="px-4 py-3 text-xs">
                        <span className={`inline-flex items-center rounded-full px-2 py-0.5 ${
                          a.key_status === 'active' ? 'bg-green-500/20 text-green-400' :
                          a.key_status === 'expiring_soon' ? 'bg-yellow-500/20 text-yellow-300' :
                          a.key_status === 'expired' ? 'bg-orange-500/20 text-orange-300' :
                          'bg-red-500/20 text-red-300'
                        }`}>
                          {a.key_status ?? 'unknown'}
                        </span>
                      </td>
                      <td className="px-4 py-3 text-xs text-muted-foreground">{a.key_expires_at ? timeAgo(a.key_expires_at) : '—'}</td>
                      <td className="px-4 py-3 text-right" onClick={e => e.stopPropagation()}>
                        {a.status !== 'deregistered' && (
                          confirmId === a.agent_id ? (
                            <div className="flex items-center gap-2 justify-end">
                              <span className="text-xs text-red-400">Confirm?</span>
                              <button
                                onClick={() => handleDeregister(a.agent_id)}
                                className="rounded bg-red-600 px-2 py-1 text-xs text-white hover:bg-red-700"
                                disabled={removePending === a.agent_id}
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
                        <td colSpan={10} className="p-0">
                          <div className="border-t border-dashed border-border/50">
                            <div className="flex items-center gap-2 px-6 pt-3 pb-1 text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                              <Server className="h-3.5 w-3.5" />
                              Discovered Components
                            </div>
                            <DiscoveredComponentsPanel dcName={a.datacenter_name} />
                          </div>
                          <div className="border-t border-dashed border-border/50">
                            <div className="flex items-center gap-2 px-6 pt-3 pb-1 text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                              <ScrollText className="h-3.5 w-3.5" />
                              Agent Logs
                            </div>
                            <AgentLogsPanel dcName={a.datacenter_name} />
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
