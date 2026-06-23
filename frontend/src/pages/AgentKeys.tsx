import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { listAgentKeys, createAgentKey, revokeAgentKey, type AgentKey } from '@/lib/api/fleet'
import { useAuth } from '@/contexts/AuthContext'
import { Key, Plus, Trash2, Copy, Check, AlertTriangle, Wifi, WifiOff, Clock, Shield } from 'lucide-react'

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

const AGENT_STATUS_STYLE: Record<string, { color: string; bg: string }> = {
  active:       { color: 'text-green-400',  bg: 'bg-green-500/20' },
  stale:        { color: 'text-yellow-400', bg: 'bg-yellow-500/20' },
  offline:      { color: 'text-red-400',    bg: 'bg-red-500/20' },
  deregistered: { color: 'text-zinc-500',   bg: 'bg-zinc-500/20' },
}

export default function AgentKeysPage() {
  const { isAdmin, isSuperAdmin } = useAuth()
  const qc = useQueryClient()

  const [showCreate, setShowCreate] = useState(false)
  const [newKeyName, setNewKeyName] = useState('')
  const [createdKey, setCreatedKey] = useState<string | null>(null)
  const [copied, setCopied] = useState(false)
  const [confirmRevoke, setConfirmRevoke] = useState<string | null>(null)

  const { data: keys = [], isLoading } = useQuery({
    queryKey: ['agent-keys'],
    queryFn: listAgentKeys,
    refetchInterval: 30_000,
  })

  const createMutation = useMutation({
    mutationFn: createAgentKey,
    onSuccess: (data) => {
      qc.invalidateQueries({ queryKey: ['agent-keys'] })
      setCreatedKey(data.api_key)
      setNewKeyName('')
      setShowCreate(false)
    },
  })

  const revokeMutation = useMutation({
    mutationFn: revokeAgentKey,
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['agent-keys'] })
      setConfirmRevoke(null)
    },
  })

  const handleCopy = () => {
    if (createdKey) {
      navigator.clipboard.writeText(createdKey)
      setCopied(true)
      setTimeout(() => setCopied(false), 2000)
    }
  }

  if (!isAdmin && !isSuperAdmin) {
    return (
      <div className="flex flex-col items-center justify-center h-64 gap-4">
        <Shield className="w-12 h-12 text-red-400 opacity-60" />
        <p className="text-slate-400 text-sm">Only Org Admins can manage agent keys.</p>
      </div>
    )
  }

  return (
    <div className="space-y-6">
      <div className="mb-6">
        <h1 className="text-2xl font-bold text-white flex items-center gap-2">
          <Key className="w-6 h-6 text-blue-400" /> Agent Keys
        </h1>
        <p className="text-sm text-slate-400 mt-1">Manage <code className="font-mono text-blue-400">isok_</code> API keys used by agents to authenticate. Only Org Admins can create or revoke keys.</p>
      </div>

      {/* New key reveal banner */}
      {createdKey && (
        <div className="bg-emerald-500/10 border border-emerald-500/30 rounded-xl p-5 space-y-3">
          <div className="flex items-center gap-2 text-emerald-400 font-semibold text-sm">
            <Check className="w-4 h-4" /> Key created — copy it now. It will not be shown again.
          </div>
          <div className="flex items-center gap-3">
            <code className="flex-1 bg-black/30 rounded-lg px-4 py-2.5 text-sm font-mono text-emerald-300 break-all">
              {createdKey}
            </code>
            <button
              onClick={handleCopy}
              className="flex items-center gap-1.5 px-3 py-2.5 bg-emerald-600 hover:bg-emerald-700 rounded-lg text-xs font-medium text-white transition-colors flex-shrink-0"
            >
              {copied ? <Check className="w-3.5 h-3.5" /> : <Copy className="w-3.5 h-3.5" />}
              {copied ? 'Copied!' : 'Copy'}
            </button>
          </div>
          <button
            onClick={() => setCreatedKey(null)}
            className="text-xs text-slate-500 hover:text-slate-300 transition-colors"
          >
            Dismiss
          </button>
        </div>
      )}

      {/* Toolbar */}
      <div className="flex justify-end">
        <button
          onClick={() => setShowCreate(true)}
          className="flex items-center gap-2 px-4 py-2 bg-blue-600 hover:bg-blue-700 rounded-lg text-sm font-medium text-white transition-colors"
        >
          <Plus className="w-4 h-4" /> Create Agent Key
        </button>
      </div>

      {/* Create modal */}
      {showCreate && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
          <div className="bg-slate-900 border border-white/10 rounded-2xl p-6 w-full max-w-md shadow-2xl space-y-4">
            <div className="flex items-center gap-3">
              <div className="p-2 bg-blue-500/20 rounded-lg">
                <Key className="w-5 h-5 text-blue-400" />
              </div>
              <div>
                <h3 className="text-white font-bold">Create Agent Key</h3>
                <p className="text-xs text-slate-400">Generates a new <code className="font-mono">isok_</code> API key</p>
              </div>
            </div>
            <div>
              <label className="block text-xs font-medium text-slate-400 mb-1 uppercase tracking-wider">Key Name</label>
              <input
                autoFocus
                value={newKeyName}
                onChange={e => setNewKeyName(e.target.value)}
                placeholder="e.g. lab-datacenter-01"
                className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-blue-500/50"
                onKeyDown={e => e.key === 'Enter' && newKeyName.trim() && createMutation.mutate({ name: newKeyName.trim() })}
              />
            </div>
            {createMutation.isError && (
              <div className="flex items-center gap-2 px-3 py-2 bg-red-500/10 border border-red-500/20 rounded-lg text-xs text-red-400">
                <AlertTriangle className="w-3.5 h-3.5" />
                {String(createMutation.error)}
              </div>
            )}
            <div className="flex gap-3 pt-1">
              <button
                onClick={() => { setShowCreate(false); setNewKeyName('') }}
                className="flex-1 px-4 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-slate-400 hover:text-white transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={() => createMutation.mutate({ name: newKeyName.trim() })}
                disabled={!newKeyName.trim() || createMutation.isPending}
                className="flex-1 flex items-center justify-center gap-2 px-4 py-2.5 bg-blue-600 hover:bg-blue-700 rounded-xl text-sm font-medium text-white transition-colors disabled:opacity-50"
              >
                {createMutation.isPending ? 'Creating…' : 'Create Key'}
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Keys table */}
      {isLoading ? (
        <div className="text-slate-500 text-sm text-center py-12">Loading keys…</div>
      ) : keys.length === 0 ? (
        <div className="text-center py-16 space-y-3">
          <Key className="w-10 h-10 text-slate-600 mx-auto" />
          <p className="text-slate-400 text-sm">No agent keys yet. Create one to connect your first agent.</p>
        </div>
      ) : (
        <div className="space-y-3">
          {keys.map((k: AgentKey) => {
            const st = k.agent_status ? AGENT_STATUS_STYLE[k.agent_status] ?? AGENT_STATUS_STYLE.offline : null
            return (
              <div
                key={k.id}
                className={`bg-slate-900/60 border rounded-xl p-4 flex items-center gap-4 ${
                  k.enabled ? 'border-white/10' : 'border-white/5 opacity-60'
                }`}
              >
                {/* Key icon */}
                <div className={`p-2.5 rounded-lg flex-shrink-0 ${k.enabled ? 'bg-blue-500/15' : 'bg-slate-700/40'}`}>
                  <Key className={`w-4 h-4 ${k.enabled ? 'text-blue-400' : 'text-slate-500'}`} />
                </div>

                {/* Main info */}
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 flex-wrap">
                    <span className="text-sm font-semibold text-white">{k.name}</span>
                    {!k.enabled && (
                      <span className="px-1.5 py-0.5 bg-red-500/15 border border-red-500/20 rounded text-[10px] text-red-400 font-medium">REVOKED</span>
                    )}
                  </div>
                  <div className="flex items-center gap-3 mt-1 flex-wrap">
                    <code className="text-xs font-mono text-slate-400">{k.key_prefix}…</code>
                    <span className="text-[11px] text-slate-500 flex items-center gap-1">
                      <Clock className="w-3 h-3" /> Created {timeAgo(k.created_at)}
                    </span>
                  </div>
                </div>

                {/* Agent status */}
                <div className="flex-shrink-0 text-right min-w-[140px]">
                  {k.datacenter_name ? (
                    <div className="space-y-1">
                      <div className="text-xs text-slate-300 font-medium truncate">{k.datacenter_name}</div>
                      {st && (
                        <div className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium ${st.color} ${st.bg}`}>
                          {k.agent_status === 'active' ? <Wifi className="w-2.5 h-2.5" /> : <WifiOff className="w-2.5 h-2.5" />}
                          {k.agent_status}
                        </div>
                      )}
                      <div className="text-[10px] text-slate-500">{timeAgo(k.last_seen)}</div>
                    </div>
                  ) : (
                    <span className="text-xs text-slate-600 italic">No agent connected</span>
                  )}
                </div>

                {/* Actions */}
                {k.enabled && (
                  <button
                    onClick={() => setConfirmRevoke(k.id)}
                    className="p-2 text-slate-500 hover:text-red-400 hover:bg-red-500/10 rounded-lg transition-colors flex-shrink-0"
                    title="Revoke key"
                  >
                    <Trash2 className="w-4 h-4" />
                  </button>
                )}
              </div>
            )
          })}
        </div>
      )}

      {/* Revoke confirmation */}
      {confirmRevoke && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
          <div className="bg-slate-900 border border-white/10 rounded-2xl p-6 w-full max-w-sm shadow-2xl space-y-4">
            <div className="flex items-center gap-3">
              <div className="p-2 bg-red-500/20 rounded-lg">
                <AlertTriangle className="w-5 h-5 text-red-400" />
              </div>
              <div>
                <h3 className="text-white font-bold">Revoke Key?</h3>
                <p className="text-xs text-slate-400">Any agent using this key will immediately stop authenticating.</p>
              </div>
            </div>
            <div className="flex gap-3">
              <button
                onClick={() => setConfirmRevoke(null)}
                className="flex-1 px-4 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-slate-400 hover:text-white transition-colors"
              >
                Cancel
              </button>
              <button
                onClick={() => revokeMutation.mutate(confirmRevoke)}
                disabled={revokeMutation.isPending}
                className="flex-1 flex items-center justify-center gap-2 px-4 py-2.5 bg-red-600 hover:bg-red-700 rounded-xl text-sm font-medium text-white transition-colors disabled:opacity-50"
              >
                {revokeMutation.isPending ? 'Revoking…' : 'Revoke'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
