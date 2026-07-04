import { useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'
import {
  listAgents, deregisterAgent, getComponentsByDc,
  type Agent, type DiscoveredComponent,
} from '@/lib/api/fleet'
import PageHeader from '@/components/PageHeader'
import {
  Server, Trash2, Plus, Wifi, WifiOff, AlertTriangle,
  ChevronDown, ChevronRight, CheckCircle2, XCircle,
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
  const { data, isLoading } = useQuery({
    queryKey: ['dc-components', dcName],
    queryFn: () => getComponentsByDc(dcName),
    refetchInterval: 60_000,
  })

  const components: DiscoveredComponent[] = data?.components ?? []

  if (isLoading) {
    return <p className="px-6 py-4 text-sm text-muted-foreground animate-pulse">Loading components…</p>
  }

  if (components.length === 0) {
    return (
      <div className="px-6 py-5 text-sm text-muted-foreground">
        <p className="font-medium text-foreground mb-1">No components discovered yet</p>
        <p>The agent reports discovered services every 5 minutes. Check back shortly after the agent is running.</p>
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
                    <span className="inline-flex items-center gap-1 text-yellow-400">
                      <XCircle className="h-3.5 w-3.5" /> Pending
                    </span>
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
                        <td colSpan={8} className="p-0">
                          <div className="border-t border-dashed border-border/50">
                            <div className="flex items-center gap-2 px-6 pt-3 pb-1 text-xs font-semibold text-muted-foreground uppercase tracking-wider">
                              <Server className="h-3.5 w-3.5" />
                              Discovered Components
                            </div>
                            <DiscoveredComponentsPanel dcName={a.datacenter_name} />
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
