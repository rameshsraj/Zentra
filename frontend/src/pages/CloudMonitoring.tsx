import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { Cloud, Plus, Trash2, CheckCircle, XCircle, RefreshCw, Activity } from 'lucide-react'
import PageHeader from '@/components/PageHeader'

// ============================================================================
// API helpers
// ============================================================================

const API = '/api'

interface CloudAccount {
  id: string
  provider: string
  name: string
  region: string
  status: string
  last_sync: string | null
  metrics_count: number
}

interface CloudSummary {
  total_accounts: number
  healthy: number
  error: number
  providers: Record<string, number>
  total_metrics: number
}

interface CloudMetric {
  metric_name: string
  value: number
  unit: string
  timestamp: string
}

async function fetchAccounts(): Promise<CloudAccount[]> {
  const r = await fetch(`${API}/cloud/accounts`, { credentials: 'include' })
  if (!r.ok) return []
  const j = await r.json()
  return j.accounts ?? j ?? []
}

async function fetchSummary(): Promise<CloudSummary> {
  const r = await fetch(`${API}/cloud/summary`, { credentials: 'include' })
  if (!r.ok) return { total_accounts: 0, healthy: 0, error: 0, providers: {}, total_metrics: 0 }
  return r.json()
}

async function fetchAccountMetrics(id: string): Promise<CloudMetric[]> {
  const r = await fetch(`${API}/cloud/accounts/${id}/metrics`, { credentials: 'include' })
  if (!r.ok) return []
  const j = await r.json()
  return j.metrics ?? j ?? []
}

async function validateAccount(id: string): Promise<{ valid: boolean; message?: string }> {
  const r = await fetch(`${API}/cloud/accounts/${id}/validate`, { method: 'POST', credentials: 'include' })
  return r.json()
}

async function deleteAccount(id: string): Promise<void> {
  await fetch(`${API}/cloud/accounts/${id}`, { method: 'DELETE', credentials: 'include' })
}

async function createAccount(body: Record<string, string>): Promise<void> {
  await fetch(`${API}/cloud/accounts`, {
    method: 'POST',
    credentials: 'include',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  })
}

// ============================================================================
// Provider styling
// ============================================================================

const PROVIDER_META: Record<string, { label: string; color: string; bg: string }> = {
  aws:   { label: 'AWS',   color: 'text-orange-400', bg: 'bg-orange-900/30 border-orange-700' },
  azure: { label: 'Azure', color: 'text-blue-400',   bg: 'bg-blue-900/30 border-blue-700' },
  gcp:   { label: 'GCP',   color: 'text-green-400',  bg: 'bg-green-900/30 border-green-700' },
}

function ProviderBadge({ provider }: { provider: string }) {
  const m = PROVIDER_META[provider] ?? { label: provider, color: 'text-gray-400', bg: 'bg-gray-800 border-gray-600' }
  return <span className={`text-xs px-2 py-0.5 rounded border ${m.bg} ${m.color}`}>{m.label}</span>
}

function StatusIcon({ status }: { status: string }) {
  if (status === 'healthy' || status === 'connected')
    return <CheckCircle className="w-4 h-4 text-green-400" />
  if (status === 'error')
    return <XCircle className="w-4 h-4 text-red-400" />
  return <RefreshCw className="w-4 h-4 text-yellow-400 animate-spin" />
}

// ============================================================================
// Components
// ============================================================================

function SummaryCards({ summary }: { summary: CloudSummary }) {
  return (
    <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mb-6">
      {[
        { label: 'Accounts', value: summary.total_accounts, icon: Cloud, color: 'text-blue-400' },
        { label: 'Healthy', value: summary.healthy, icon: CheckCircle, color: 'text-green-400' },
        { label: 'Errors', value: summary.error, icon: XCircle, color: 'text-red-400' },
        { label: 'Metrics', value: summary.total_metrics, icon: Activity, color: 'text-purple-400' },
      ].map(({ label, value, icon: Icon, color }) => (
        <div key={label} className="bg-gray-900 border border-gray-800 rounded-lg p-4">
          <div className="flex items-center gap-2 text-xs text-gray-500 mb-1">
            <Icon className={`w-3.5 h-3.5 ${color}`} />
            {label}
          </div>
          <div className="text-xl font-bold text-white">{value}</div>
        </div>
      ))}
    </div>
  )
}

function AccountRow({
  account,
  expanded,
  onToggle,
  onDelete,
  onValidate,
}: {
  account: CloudAccount
  expanded: boolean
  onToggle: () => void
  onDelete: () => void
  onValidate: () => void
}) {
  const { data: metrics = [] } = useQuery({
    queryKey: ['cloud-metrics', account.id],
    queryFn: () => fetchAccountMetrics(account.id),
    enabled: expanded,
    refetchInterval: expanded ? 30000 : false,
  })

  return (
    <div className="border border-gray-800 rounded-lg mb-3 overflow-hidden">
      <div
        className="flex items-center justify-between px-4 py-3 bg-gray-900 hover:bg-gray-800/80 cursor-pointer"
        onClick={onToggle}
      >
        <div className="flex items-center gap-3">
          <StatusIcon status={account.status} />
          <span className="text-white font-medium text-sm">{account.name}</span>
          <ProviderBadge provider={account.provider} />
          <span className="text-xs text-gray-500">{account.region}</span>
        </div>
        <div className="flex items-center gap-3">
          <span className="text-xs text-gray-500">{account.metrics_count} metrics</span>
          {account.last_sync && (
            <span className="text-xs text-gray-600">
              Synced {new Date(account.last_sync).toLocaleTimeString()}
            </span>
          )}
          <button
            onClick={(e) => { e.stopPropagation(); onValidate() }}
            className="text-xs text-blue-400 hover:text-blue-300 px-2 py-1 rounded hover:bg-blue-900/30"
            title="Validate credentials"
          >
            Test
          </button>
          <button
            onClick={(e) => { e.stopPropagation(); onDelete() }}
            className="text-xs text-red-400 hover:text-red-300 px-2 py-1 rounded hover:bg-red-900/30"
            title="Remove account"
          >
            <Trash2 className="w-3.5 h-3.5" />
          </button>
        </div>
      </div>

      {expanded && (
        <div className="p-4 bg-gray-950/50 border-t border-gray-800">
          {metrics.length === 0 ? (
            <div className="text-xs text-gray-500 text-center py-4">No metrics collected yet</div>
          ) : (
            <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
              {metrics.map((m, i) => (
                <div key={i} className="bg-gray-900 rounded p-3 border border-gray-800">
                  <div className="text-xs text-gray-500 truncate" title={m.metric_name}>
                    {m.metric_name}
                  </div>
                  <div className="text-lg font-bold text-white mt-1">
                    {typeof m.value === 'number' ? m.value.toLocaleString(undefined, { maximumFractionDigits: 2 }) : m.value}
                  </div>
                  <div className="text-xs text-gray-600">{m.unit}</div>
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </div>
  )
}

function AddAccountForm({ onClose, onCreated }: { onClose: () => void; onCreated: () => void }) {
  const [provider, setProvider] = useState('aws')
  const [name, setName] = useState('')
  const [region, setRegion] = useState('')
  const [submitting, setSubmitting] = useState(false)

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    setSubmitting(true)
    try {
      await createAccount({ provider, name, region })
      onCreated()
      onClose()
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <form onSubmit={handleSubmit} className="bg-gray-900 border border-gray-700 rounded-lg p-6 w-full max-w-md space-y-4">
        <h3 className="text-lg font-bold text-white">Add Cloud Account</h3>

        <div>
          <label className="text-xs text-gray-400 block mb-1">Provider</label>
          <select
            value={provider}
            onChange={(e) => setProvider(e.target.value)}
            className="w-full bg-gray-800 border border-gray-700 rounded px-3 py-2 text-sm text-white"
          >
            <option value="aws">AWS CloudWatch</option>
            <option value="azure">Azure Monitor</option>
            <option value="gcp">GCP Monitoring</option>
          </select>
        </div>

        <div>
          <label className="text-xs text-gray-400 block mb-1">Account Name</label>
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Production AWS"
            required
            className="w-full bg-gray-800 border border-gray-700 rounded px-3 py-2 text-sm text-white"
          />
        </div>

        <div>
          <label className="text-xs text-gray-400 block mb-1">Region</label>
          <input
            value={region}
            onChange={(e) => setRegion(e.target.value)}
            placeholder="us-east-1"
            className="w-full bg-gray-800 border border-gray-700 rounded px-3 py-2 text-sm text-white"
          />
        </div>

        <div className="flex justify-end gap-3 pt-2">
          <button type="button" onClick={onClose} className="text-sm text-gray-400 hover:text-white px-4 py-2">
            Cancel
          </button>
          <button
            type="submit"
            disabled={submitting || !name}
            className="text-sm bg-blue-600 hover:bg-blue-500 text-white px-4 py-2 rounded disabled:opacity-50"
          >
            {submitting ? 'Adding...' : 'Add Account'}
          </button>
        </div>
      </form>
    </div>
  )
}

// ============================================================================
// Page
// ============================================================================

export default function CloudMonitoring() {
  const [expandedId, setExpandedId] = useState<string | null>(null)
  const [showAdd, setShowAdd] = useState(false)

  const { data: accounts = [], refetch: refetchAccounts } = useQuery({
    queryKey: ['cloud-accounts'],
    queryFn: fetchAccounts,
    refetchInterval: 30000,
  })

  const { data: summary } = useQuery({
    queryKey: ['cloud-summary'],
    queryFn: fetchSummary,
    refetchInterval: 30000,
  })

  const handleDelete = async (id: string) => {
    if (!confirm('Remove this cloud account?')) return
    await deleteAccount(id)
    refetchAccounts()
  }

  const handleValidate = async (id: string) => {
    const result = await validateAccount(id)
    alert(result.valid ? 'Credentials valid ✓' : `Validation failed: ${result.message ?? 'Unknown error'}`)
    refetchAccounts()
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <PageHeader
          title="Cloud Monitoring"
          subtitle="AWS CloudWatch · Azure Monitor · GCP Monitoring"
        />
        <button
          onClick={() => setShowAdd(true)}
          className="flex items-center gap-2 bg-blue-600 hover:bg-blue-500 text-white text-sm px-4 py-2 rounded-lg"
        >
          <Plus className="w-4 h-4" /> Add Account
        </button>
      </div>

      {summary && <SummaryCards summary={summary} />}

      {accounts.length === 0 ? (
        <div className="text-center py-16 text-gray-500">
          <Cloud className="w-12 h-12 mx-auto mb-4 opacity-30" />
          <div className="text-sm">No cloud accounts configured</div>
          <div className="text-xs mt-1">Click "Add Account" to connect AWS, Azure, or GCP</div>
        </div>
      ) : (
        <div>
          {accounts.map((acc) => (
            <AccountRow
              key={acc.id}
              account={acc}
              expanded={expandedId === acc.id}
              onToggle={() => setExpandedId(expandedId === acc.id ? null : acc.id)}
              onDelete={() => handleDelete(acc.id)}
              onValidate={() => handleValidate(acc.id)}
            />
          ))}
        </div>
      )}

      {showAdd && (
        <AddAccountForm onClose={() => setShowAdd(false)} onCreated={() => refetchAccounts()} />
      )}
    </div>
  )
}
