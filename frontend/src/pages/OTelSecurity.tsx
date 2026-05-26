import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { fetchJSON } from '@/lib/api'
import PageHeader from '@/components/PageHeader'
import LoadingSkeleton from '@/components/LoadingSkeleton'
import { Shield, Plus, Key, Copy, CheckCircle, XCircle, Clock } from 'lucide-react'

interface ApiKey {
  id: string; name: string; key_prefix: string; permissions: string[]
  rate_limit_rpm: number; max_payload_bytes: number; enabled: boolean
  last_used_at: string | null; expires_at: string | null; created_at: string
}

interface AuditEntry {
  signal: string; endpoint: string; status_code: number
  span_count: number; payload_bytes: number; remote_addr: string; created_at: string
}

const getKeys = () => fetchJSON<{ keys: ApiKey[]; active_count: number }>('/otel/api-keys')
const createKey = (data: Record<string, unknown>) =>
  fetchJSON<{ raw_key: string; key: ApiKey; warning: string }>('/otel/api-keys', { method: 'POST', body: JSON.stringify(data) })
const revokeKey = (id: string) => fetchJSON<{ revoked: boolean }>(`/otel/api-keys/${id}`, { method: 'DELETE' })
const getAudit = () => fetchJSON<{ entries: AuditEntry[]; total: number }>('/otel/ingest-audit')

export default function OTelSecurityPage() {
  const qc = useQueryClient()
  const [showCreate, setShowCreate] = useState(false)
  const [createdKey, setCreatedKey] = useState<string | null>(null)
  const [copied, setCopied] = useState(false)
  const [form, setForm] = useState({ name: '', rate_limit_rpm: '10000', expires_in_days: '' })

  const { data: keysData, isLoading } = useQuery({ queryKey: ['otel-api-keys'], queryFn: getKeys })
  const { data: auditData } = useQuery({ queryKey: ['otel-audit'], queryFn: getAudit, refetchInterval: 15_000 })

  const keys = keysData?.keys ?? []
  const audit = auditData?.entries ?? []

  const createMut = useMutation({
    mutationFn: createKey,
    onSuccess: (data) => {
      setCreatedKey(data.raw_key)
      setShowCreate(false)
      setForm({ name: '', rate_limit_rpm: '10000', expires_in_days: '' })
      qc.invalidateQueries({ queryKey: ['otel-api-keys'] })
    },
  })

  const revokeMut = useMutation({
    mutationFn: revokeKey,
    onSuccess: () => qc.invalidateQueries({ queryKey: ['otel-api-keys'] }),
  })

  const handleCopy = () => {
    if (createdKey) { navigator.clipboard.writeText(createdKey); setCopied(true); setTimeout(() => setCopied(false), 2000) }
  }

  return (
    <div className="space-y-6">
      <PageHeader title="OTLP Security" subtitle="Manage API keys, rate limits, and view ingest audit logs" />

      {/* Summary Cards */}
      <div className="grid grid-cols-3 gap-4">
        <div className="bg-white/5 border border-white/10 rounded-xl p-4">
          <div className="text-[10px] text-slate-500 uppercase tracking-wider font-medium mb-1">Active Keys</div>
          <div className="text-2xl font-bold text-white">{keysData?.active_count ?? 0}</div>
        </div>
        <div className="bg-white/5 border border-white/10 rounded-xl p-4">
          <div className="text-[10px] text-slate-500 uppercase tracking-wider font-medium mb-1">Audit Entries (1h)</div>
          <div className="text-2xl font-bold text-white">{auditData?.total ?? 0}</div>
        </div>
        <div className="bg-white/5 border border-white/10 rounded-xl p-4">
          <div className="text-[10px] text-slate-500 uppercase tracking-wider font-medium mb-1">Auth Methods</div>
          <div className="text-sm text-slate-300 mt-1">JWT · API Key (isok_) · Header</div>
        </div>
      </div>

      {/* Created Key Banner */}
      {createdKey && (
        <div className="bg-amber-500/10 border border-amber-500/20 rounded-xl p-4">
          <div className="flex items-center gap-2 mb-2">
            <Key className="w-4 h-4 text-amber-400" />
            <span className="text-sm font-bold text-amber-400">API Key Created — Copy it now!</span>
          </div>
          <div className="flex items-center gap-2">
            <code className="flex-1 px-3 py-2 bg-black/30 rounded-lg text-sm font-mono text-white break-all">{createdKey}</code>
            <button onClick={handleCopy} className="flex items-center gap-1 px-3 py-2 bg-amber-500/20 rounded-lg text-xs text-amber-400 hover:text-white transition-colors">
              {copied ? <CheckCircle className="w-3.5 h-3.5" /> : <Copy className="w-3.5 h-3.5" />}
              {copied ? 'Copied!' : 'Copy'}
            </button>
          </div>
          <p className="text-xs text-amber-400/60 mt-2">This key will not be shown again. Store it securely.</p>
        </div>
      )}

      {/* Create + Keys */}
      <div className="flex items-center gap-3 mb-2">
        <h2 className="text-sm font-bold text-white">API Keys</h2>
        <button onClick={() => setShowCreate(!showCreate)}
          className="flex items-center gap-1 px-3 py-1.5 bg-brand-600 hover:bg-brand-700 rounded-lg text-xs font-medium transition-colors">
          <Plus className="w-3 h-3" /> Create Key
        </button>
      </div>

      {showCreate && (
        <div className="bg-white/5 border border-white/10 rounded-xl p-4 space-y-3">
          <div className="grid grid-cols-3 gap-3">
            <input value={form.name} onChange={e => setForm(f => ({ ...f, name: e.target.value }))} placeholder="Key name"
              className="px-3 py-2 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none" autoFocus />
            <input value={form.rate_limit_rpm} onChange={e => setForm(f => ({ ...f, rate_limit_rpm: e.target.value }))} placeholder="Rate limit (RPM)" type="number"
              className="px-3 py-2 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none" />
            <input value={form.expires_in_days} onChange={e => setForm(f => ({ ...f, expires_in_days: e.target.value }))} placeholder="Expires in (days, optional)" type="number"
              className="px-3 py-2 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none" />
          </div>
          <div className="flex gap-2">
            <button onClick={() => createMut.mutate({ name: form.name, rate_limit_rpm: parseInt(form.rate_limit_rpm) || 10000, expires_in_days: form.expires_in_days ? parseInt(form.expires_in_days) : undefined })}
              disabled={!form.name || createMut.isPending}
              className="px-4 py-2 bg-brand-600 hover:bg-brand-700 rounded-xl text-xs font-medium transition-colors disabled:opacity-50">
              {createMut.isPending ? 'Creating...' : 'Create Key'}
            </button>
            <button onClick={() => setShowCreate(false)} className="px-4 py-2 bg-white/5 border border-white/10 rounded-xl text-xs text-slate-400 hover:text-white">Cancel</button>
          </div>
        </div>
      )}

      {isLoading && <LoadingSkeleton variant="table" count={3} />}

      {!isLoading && keys.length > 0 && (
        <div className="bg-white/5 border border-white/10 rounded-xl overflow-hidden">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-white/10 text-left">
                <th className="px-4 py-3 text-xs font-medium text-slate-500 uppercase">Prefix</th>
                <th className="px-4 py-3 text-xs font-medium text-slate-500 uppercase">Name</th>
                <th className="px-4 py-3 text-xs font-medium text-slate-500 uppercase">Permissions</th>
                <th className="px-4 py-3 text-xs font-medium text-slate-500 uppercase">Rate Limit</th>
                <th className="px-4 py-3 text-xs font-medium text-slate-500 uppercase">Status</th>
                <th className="px-4 py-3"></th>
              </tr>
            </thead>
            <tbody className="divide-y divide-white/5">
              {keys.map(k => (
                <tr key={k.id} className="hover:bg-white/5">
                  <td className="px-4 py-3 font-mono text-xs text-brand-400">{k.key_prefix}…</td>
                  <td className="px-4 py-3 text-white">{k.name}</td>
                  <td className="px-4 py-3">
                    <div className="flex gap-1 flex-wrap">
                      {k.permissions.map(p => (
                        <span key={p} className="px-1.5 py-0.5 bg-white/5 rounded text-[10px] text-slate-400">{p}</span>
                      ))}
                    </div>
                  </td>
                  <td className="px-4 py-3 text-xs text-slate-400">{k.rate_limit_rpm.toLocaleString()} RPM</td>
                  <td className="px-4 py-3">
                    {k.enabled
                      ? <span className="inline-flex items-center gap-1 text-xs text-emerald-400"><CheckCircle className="w-3 h-3" /> Active</span>
                      : <span className="inline-flex items-center gap-1 text-xs text-red-400"><XCircle className="w-3 h-3" /> Revoked</span>}
                  </td>
                  <td className="px-4 py-3">
                    {k.enabled && (
                      <button onClick={() => revokeMut.mutate(k.id)}
                        className="text-xs text-red-400 hover:text-red-300 transition-colors">Revoke</button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {/* Audit Log */}
      <div className="flex items-center gap-2 mt-6 mb-2">
        <Clock className="w-4 h-4 text-slate-500" />
        <h2 className="text-sm font-bold text-white">Ingest Audit Log (Last 1h)</h2>
      </div>

      {audit.length > 0 ? (
        <div className="bg-white/5 border border-white/10 rounded-xl overflow-hidden max-h-[40vh] overflow-y-auto">
          <table className="w-full text-xs">
            <thead className="sticky top-0 bg-slate-900">
              <tr className="border-b border-white/10 text-left">
                <th className="px-3 py-2 text-slate-500 uppercase">Signal</th>
                <th className="px-3 py-2 text-slate-500 uppercase">Endpoint</th>
                <th className="px-3 py-2 text-slate-500 uppercase">Status</th>
                <th className="px-3 py-2 text-slate-500 uppercase">Spans</th>
                <th className="px-3 py-2 text-slate-500 uppercase">Size</th>
                <th className="px-3 py-2 text-slate-500 uppercase">IP</th>
                <th className="px-3 py-2 text-slate-500 uppercase">Time</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-white/5">
              {audit.map((a, i) => (
                <tr key={i} className="hover:bg-white/5">
                  <td className="px-3 py-2 text-slate-300">{a.signal}</td>
                  <td className="px-3 py-2 font-mono text-slate-400">{a.endpoint}</td>
                  <td className="px-3 py-2">
                    <span className={a.status_code < 300 ? 'text-emerald-400' : 'text-red-400'}>{a.status_code}</span>
                  </td>
                  <td className="px-3 py-2 text-slate-400">{a.span_count}</td>
                  <td className="px-3 py-2 text-slate-400">{a.payload_bytes ? `${(a.payload_bytes / 1024).toFixed(1)}KB` : '-'}</td>
                  <td className="px-3 py-2 font-mono text-slate-500">{a.remote_addr || '-'}</td>
                  <td className="px-3 py-2 text-slate-500">{new Date(a.created_at).toLocaleTimeString()}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="bg-white/5 border border-white/10 rounded-xl p-8 text-center">
          <Shield className="w-10 h-10 text-slate-600 mx-auto mb-2" />
          <p className="text-sm text-slate-400">No audit entries in the last hour.</p>
        </div>
      )}
    </div>
  )
}
