import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'
import { listAgents, deregisterAgent, type Agent } from '@/lib/api/fleet'
import PageHeader from '@/components/PageHeader'
import { Server, Trash2, Plus, Wifi, WifiOff, AlertTriangle } from 'lucide-react'

const STATUS_STYLES: Record<string, { color: string; bg: string; label: string }> = {
  active: { color: 'text-green-400', bg: 'bg-green-500/20', label: 'Active' },
  stale: { color: 'text-yellow-400', bg: 'bg-yellow-500/20', label: 'Stale' },
  offline: { color: 'text-red-400', bg: 'bg-red-500/20', label: 'Offline' },
  deregistered: { color: 'text-zinc-500', bg: 'bg-zinc-500/20', label: 'Deregistered' },
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

export default function FleetPage() {
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const [confirmId, setConfirmId] = useState<string | null>(null)

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
        description="Manage your monitoring agents across all datacenters."
        icon={Server}
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
        <div className="overflow-x-auto rounded-xl border border-border bg-card">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-border text-left text-muted-foreground">
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
                return (
                  <tr key={a.agent_id} className="border-b border-border last:border-0 hover:bg-muted/30 transition-colors">
                    <td className="px-4 py-3 font-medium">{a.datacenter_name}</td>
                    <td className="px-4 py-3">
                      <span className={`inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium ${s.bg} ${s.color}`}>
                        {a.status === 'active' ? <Wifi className="h-3 w-3" /> : a.status === 'offline' ? <WifiOff className="h-3 w-3" /> : <AlertTriangle className="h-3 w-3" />}
                        {s.label}
                      </span>
                    </td>
                    <td className="px-4 py-3 text-muted-foreground">{timeAgo(a.last_seen)}</td>
                    <td className="px-4 py-3 font-mono text-xs">{a.agent_version ?? '—'}</td>
                    <td className="px-4 py-3">{a.active_target_count}</td>
                    <td className="px-4 py-3 font-mono text-xs text-muted-foreground">{a.ip_address ?? '—'}</td>
                    <td className="px-4 py-3 text-right">
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
                )
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}
