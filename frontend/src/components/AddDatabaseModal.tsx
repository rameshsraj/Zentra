import { useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { createTarget, testDbConnection } from '@/lib/api'
import { X, Plus, Loader2, Database, CheckCircle } from 'lucide-react'
import { useAppMode } from '@/contexts/AppModeContext'

interface AddDatabaseModalProps {
  open: boolean
  onClose: () => void
  dbType: 'oracle'
  title?: string
}

const DB_CONFIGS: Record<string, {
  label: string
  defaultPort: number
  subtypes: string[]
  connectionFields: { key: string; label: string; placeholder: string; type?: string }[]
}> = {
  oracle: {
    label: 'Oracle Database',
    defaultPort: 1521,
    subtypes: ['oracle'],
    connectionFields: [
      { key: 'sid', label: 'SID / Service Name', placeholder: 'e.g. ORCL or orcl.example.com' },
      { key: 'username', label: 'Username', placeholder: 'e.g. system or monitoring_user' },
      { key: 'password', label: 'Password', placeholder: '••••••••', type: 'password' },
      { key: 'role', label: 'Connection Role', placeholder: 'e.g. SYSDBA, NORMAL' },
    ],
  },
}

export default function AddDatabaseModal({ open, onClose, dbType, title }: AddDatabaseModalProps) {
  const qc = useQueryClient()
  const { isDemo } = useAppMode()
  const config = DB_CONFIGS[dbType]

  const [form, setForm] = useState({
    name: '',
    host: '',
    port: config.defaultPort,
    target_subtype: config.subtypes[0],
    environment: 'production',
    description: '',
    ...Object.fromEntries(config.connectionFields.map(f => [f.key, ''])),
  })
  const [error, setError] = useState('')
  const [testResult, setTestResult] = useState<null | { ok: boolean; msg: string }>(null)
  const [testing, setTesting] = useState(false)

  const mutation = useMutation({
    mutationFn: () => createTarget({
      name: form.name,
      target_type: 'database',
      target_subtype: form.target_subtype,
      host: form.host,
      port: form.port,
      description: form.description || null,
      environment: form.environment,
      ssl_enabled: false,
      is_active: true,
    }),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['targets'] })
      onClose()
      resetForm()
    },
    onError: (e: Error) => setError(e.message),
  })

  const resetForm = () => {
    setForm({
      name: '', host: '', port: config.defaultPort,
      target_subtype: config.subtypes[0], environment: 'production', description: '',
      ...Object.fromEntries(config.connectionFields.map(f => [f.key, ''])),
    })
    setError('')
    setTestResult(null)
  }

  const update = (field: string, value: string | number) => setForm(f => ({ ...f, [field]: value }))

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (isDemo) { setError('Adding databases is disabled in Demo mode.'); return }
    if (!form.name || !form.host) { setError('Name and Host are required'); return }
    if (!testResult?.ok) { setError('Please run "Test Connection" successfully before adding.'); return }
    setError('')
    mutation.mutate()
  }

  const handleTestConnection = async () => {
    setTestResult(null)
    setTesting(true)
    try {
      if (!form.host) { setTestResult({ ok: false, msg: 'Host is required' }); return }
      const f = form as any
      const result = await testDbConnection({
        host: form.host,
        port: form.port,
        target_subtype: form.target_subtype,
        username: f.username || undefined,
        password: f.password || undefined,
        database_name: f.database || f.sid || undefined,
      })
      if (result.reachable) {
        const extra = result.detail ? ` — ${result.detail}` : ''
        setTestResult({ ok: true, msg: `Reachable in ${result.latency_ms}ms${extra}` })
      } else {
        setTestResult({ ok: false, msg: result.error || 'Connection failed' })
      }
    } catch (e: any) {
      setTestResult({ ok: false, msg: e?.message || 'Test failed' })
    } finally {
      setTesting(false)
    }
  }

  if (!open) return null

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm" onClick={onClose}>
      <div className="w-full max-w-lg bg-slate-900 border border-white/10 rounded-2xl shadow-2xl max-h-[90vh] flex flex-col" onClick={e => e.stopPropagation()}>
        {/* Header */}
        <div className="flex items-center justify-between px-6 py-4 border-b border-white/10 flex-shrink-0">
          <h2 className="text-lg font-bold text-white flex items-center gap-2">
            <Database className="w-5 h-5 text-brand-400" />
            {title || `Add ${config.label} Instance`}
          </h2>
          <button onClick={onClose} className="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 transition-colors">
            <X className="w-5 h-5" />
          </button>
        </div>

        {/* Form */}
        <form onSubmit={handleSubmit} className="p-6 space-y-4 overflow-y-auto">
          {error && (
            <div className="px-4 py-2.5 bg-red-500/10 border border-red-500/20 rounded-xl text-sm text-red-400">{error}</div>
          )}

          {/* Instance Name + Environment */}
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-xs font-medium text-slate-400 mb-1.5">Instance Name *</label>
              <input
                value={form.name}
                onChange={e => update('name', e.target.value)}
                placeholder={`e.g. ${dbType}-prod-01`}
                className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-brand-500/50"
                autoFocus
              />
            </div>
            <div>
              <label className="block text-xs font-medium text-slate-400 mb-1.5">Environment</label>
              <select
                value={form.environment}
                onChange={e => update('environment', e.target.value)}
                className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white focus:outline-none focus:ring-2 focus:ring-brand-500/50 [&>option]:bg-slate-900 [&>option]:text-white"
              >
                <option value="production">Production</option>
                <option value="staging">Staging</option>
                <option value="development">Development</option>
                <option value="testing">Testing</option>
              </select>
            </div>
          </div>

          {/* Subtype selector (if multiple subtypes) */}
          {config.subtypes.length > 1 && (
            <div>
              <label className="block text-xs font-medium text-slate-400 mb-1.5">Database Type</label>
              <select
                value={form.target_subtype}
                onChange={e => update('target_subtype', e.target.value)}
                className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white focus:outline-none focus:ring-2 focus:ring-brand-500/50 [&>option]:bg-slate-900 [&>option]:text-white"
              >
                {config.subtypes.map(s => <option key={s} value={s}>{s.charAt(0).toUpperCase() + s.slice(1)}</option>)}
              </select>
            </div>
          )}

          {/* Host + Port */}
          <div className="grid grid-cols-3 gap-4">
            <div className="col-span-2">
              <label className="block text-xs font-medium text-slate-400 mb-1.5">Host / IP *</label>
              <input
                value={form.host}
                onChange={e => update('host', e.target.value)}
                placeholder="e.g. 10.0.1.10 or db-hostname"
                className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-brand-500/50"
              />
            </div>
            <div>
              <label className="block text-xs font-medium text-slate-400 mb-1.5">Port</label>
              <input
                type="number"
                value={form.port || ''}
                onChange={e => update('port', parseInt(e.target.value) || 0)}
                placeholder={String(config.defaultPort)}
                className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-brand-500/50"
              />
            </div>
          </div>

          {/* Connection-specific fields */}
          <div className="border-t border-white/5 pt-4">
            <p className="text-[10px] text-slate-500 uppercase tracking-wider font-medium mb-3">Connection Details</p>
            <div className="grid grid-cols-2 gap-4">
              {config.connectionFields.map(field => (
                <div key={field.key} className={field.key === 'password' ? '' : ''}>
                  <label className="block text-xs font-medium text-slate-400 mb-1.5">{field.label}</label>
                  <input
                    type={field.type || 'text'}
                    value={(form as any)[field.key] || ''}
                    onChange={e => update(field.key, e.target.value)}
                    placeholder={field.placeholder}
                    className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-brand-500/50"
                  />
                </div>
              ))}
            </div>
          </div>

          {/* Description */}
          <div>
            <label className="block text-xs font-medium text-slate-400 mb-1.5">Description</label>
            <textarea
              value={form.description}
              onChange={e => update('description', e.target.value)}
              rows={2}
              placeholder="Optional notes about this instance..."
              className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-brand-500/50 resize-none"
            />
          </div>

          {/* Test Connection Result */}
          {testResult && (
            <div className={`px-4 py-2.5 rounded-xl text-sm flex items-center gap-2 border ${
              testResult.ok ? 'bg-emerald-500/10 border-emerald-500/20 text-emerald-400' : 'bg-red-500/10 border-red-500/20 text-red-400'
            }`}>
              <CheckCircle className="w-4 h-4 flex-shrink-0" />
              {testResult.msg}
            </div>
          )}

          {/* Actions */}
          <div className="flex justify-between items-center pt-2">
            <button
              type="button"
              onClick={handleTestConnection}
              disabled={testing}
              className="flex items-center gap-1.5 px-3 py-2 bg-white/5 border border-white/10 rounded-xl text-xs text-slate-400 hover:text-white transition-colors disabled:opacity-50"
            >
              {testing ? <Loader2 className="w-3 h-3 animate-spin" /> : null}
              {testing ? 'Testing...' : 'Test Connection'}
            </button>
            <div className="flex gap-3">
              <button
                type="button"
                onClick={() => { onClose(); resetForm() }}
                className="px-4 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-slate-400 hover:text-white transition-colors"
              >
                Cancel
              </button>
              <button
                type="submit"
                disabled={mutation.isPending || !testResult?.ok}
                title={!testResult?.ok ? 'Run Test Connection successfully first' : ''}
                className="flex items-center gap-2 px-5 py-2.5 bg-brand-600 hover:bg-brand-700 rounded-xl text-sm font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed"
              >
                {mutation.isPending ? <Loader2 className="w-4 h-4 animate-spin" /> : <Plus className="w-4 h-4" />}
                {mutation.isPending ? 'Creating...' : 'Add Database'}
              </button>
            </div>
          </div>
        </form>
      </div>
    </div>
  )
}
