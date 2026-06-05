import { useState, useEffect, useCallback } from 'react'
import { getTenants, createTenant, deleteTenant, suspendTenant, activateTenant, getTenantUsage } from '../../lib/api'
import type { Organization, TenantUsage } from '../../lib/api/types'
import { useAuth } from '../../contexts/AuthContext'
import { Building2, Plus, Search, X, Loader2, AlertCircle, Pause, Play, Trash2, Eye, Users, Server, BarChart3, Crown } from 'lucide-react'

const PLAN_COLORS: Record<string, string> = {
  enterprise: 'bg-purple-600/20 text-purple-400 border-purple-500/30',
  professional: 'bg-blue-600/20 text-blue-400 border-blue-500/30',
  starter: 'bg-green-600/20 text-green-400 border-green-500/30',
  free: 'bg-gray-600/20 text-gray-400 border-gray-500/30',
}

const STATUS_COLORS: Record<string, string> = {
  active: 'bg-green-600/20 text-green-400',
  trial: 'bg-amber-600/20 text-amber-400',
  suspended: 'bg-red-600/20 text-red-400',
  cancelled: 'bg-gray-600/20 text-gray-500',
}

export default function OrganizationManagement() {
  const { isSuperAdmin } = useAuth()
  const [orgs, setOrgs] = useState<Organization[]>([])
  const [loading, setLoading] = useState(true)
  const [search, setSearch] = useState('')
  const [statusFilter, setStatusFilter] = useState('')
  const [planFilter, setPlanFilter] = useState('')
  const [showCreate, setShowCreate] = useState(false)
  const [showDetail, setShowDetail] = useState<string | null>(null)
  const [error, setError] = useState('')

  const fetchOrgs = useCallback(async () => {
    setLoading(true)
    try {
      const params: Record<string, string> = {}
      if (statusFilter) params.status = statusFilter
      if (planFilter) params.plan = planFilter
      const res = await getTenants(params)
      setOrgs(res)
    } catch (e: any) { setError(e.message) }
    setLoading(false)
  }, [statusFilter, planFilter])

  useEffect(() => { fetchOrgs() }, [fetchOrgs])

  const filtered = orgs.filter(o =>
    !search || o.name.toLowerCase().includes(search.toLowerCase()) || o.slug.toLowerCase().includes(search.toLowerCase()) || o.owner_email.toLowerCase().includes(search.toLowerCase())
  )

  const activeCount = orgs.filter(o => o.status === 'active').length
  const trialCount = orgs.filter(o => o.status === 'trial').length
  const suspendedCount = orgs.filter(o => o.status === 'suspended').length
  const totalUsers = orgs.reduce((a, o) => a + (o.current_users || 0), 0)

  if (!isSuperAdmin) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="text-center">
          <Crown className="w-12 h-12 text-yellow-500/50 mx-auto mb-3" />
          <p className="text-gray-400">Only Super Admins can manage organizations.</p>
        </div>
      </div>
    )
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="p-2.5 rounded-xl bg-indigo-600/20"><Building2 className="w-5 h-5 text-indigo-400" /></div>
          <div>
            <h1 className="text-2xl font-bold text-white">Organizations</h1>
            <p className="text-sm text-gray-400">{orgs.length} organizations · {totalUsers} total users</p>
          </div>
        </div>
        <button onClick={() => setShowCreate(true)} className="flex items-center gap-2 px-4 py-2 rounded-lg bg-indigo-600 hover:bg-indigo-700 text-white text-sm font-medium transition">
          <Plus className="w-4 h-4" /> Create Organization
        </button>
      </div>

      {/* KPI Strip */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        {[
          { label: 'Total Orgs', value: orgs.length, color: 'text-white', bg: 'bg-white/5 border-white/10' },
          { label: 'Active', value: activeCount, color: 'text-green-400', bg: 'bg-green-500/10 border-green-500/20' },
          { label: 'Trial', value: trialCount, color: 'text-amber-400', bg: 'bg-amber-500/10 border-amber-500/20' },
          { label: 'Suspended', value: suspendedCount, color: 'text-red-400', bg: 'bg-red-500/10 border-red-500/20' },
        ].map((k, i) => (
          <div key={i} className={`rounded-xl border p-3 ${k.bg}`}>
            <p className="text-xs text-gray-500">{k.label}</p>
            <p className={`text-xl font-bold ${k.color}`}>{k.value}</p>
          </div>
        ))}
      </div>

      {/* Filters */}
      <div className="flex items-center gap-3 flex-wrap">
        <div className="relative flex-1 min-w-[200px] max-w-md">
          <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-500" />
          <input value={search} onChange={e => setSearch(e.target.value)} placeholder="Search by name, slug, or email..."
            className="w-full pl-10 pr-4 py-2 rounded-lg bg-gray-800 border border-gray-700 text-white text-sm placeholder-gray-500 focus:border-indigo-500 outline-none" />
        </div>
        <select value={statusFilter} onChange={e => setStatusFilter(e.target.value)}
          className="px-3 py-2 rounded-lg bg-gray-800 border border-gray-700 text-white text-sm">
          <option value="">All Status</option>
          <option value="active">Active</option>
          <option value="trial">Trial</option>
          <option value="suspended">Suspended</option>
          <option value="cancelled">Cancelled</option>
        </select>
        <select value={planFilter} onChange={e => setPlanFilter(e.target.value)}
          className="px-3 py-2 rounded-lg bg-gray-800 border border-gray-700 text-white text-sm">
          <option value="">All Plans</option>
          <option value="enterprise">Enterprise</option>
          <option value="professional">Professional</option>
          <option value="starter">Starter</option>
          <option value="free">Free</option>
        </select>
      </div>

      {error && <div className="flex items-center gap-2 p-3 rounded-lg bg-red-900/30 border border-red-700/50 text-red-300 text-sm"><AlertCircle className="w-4 h-4" />{error}</div>}

      {/* Org Cards */}
      {loading ? (
        <div className="text-center p-8 text-gray-500"><Loader2 className="w-5 h-5 animate-spin inline mr-2" />Loading organizations...</div>
      ) : filtered.length === 0 ? (
        <div className="text-center p-8 text-gray-500">No organizations found</div>
      ) : (
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          {filtered.map(org => (
            <OrgCard key={org.org_id} org={org} onRefresh={fetchOrgs} onViewDetail={() => setShowDetail(org.org_id)} />
          ))}
        </div>
      )}

      {showCreate && <CreateOrgModal onClose={() => setShowCreate(false)} onCreated={() => { setShowCreate(false); fetchOrgs() }} />}
      {showDetail && <OrgDetailModal orgId={showDetail} onClose={() => { setShowDetail(null); fetchOrgs() }} />}
    </div>
  )
}

function OrgCard({ org, onRefresh, onViewDetail }: { org: Organization; onRefresh: () => void; onViewDetail: () => void }) {
  const [acting, setActing] = useState(false)
  const isDefault = org.org_id === '00000000-0000-0000-0000-000000000001'

  const handleSuspend = async () => {
    setActing(true)
    try { await suspendTenant(org.org_id) } catch {}
    setActing(false); onRefresh()
  }
  const handleActivate = async () => {
    setActing(true)
    try { await activateTenant(org.org_id) } catch {}
    setActing(false); onRefresh()
  }
  const handleDelete = async () => {
    if (!confirm(`Cancel organization "${org.name}"? All users will be deactivated.`)) return
    setActing(true)
    try { await deleteTenant(org.org_id) } catch {}
    setActing(false); onRefresh()
  }

  return (
    <div className="bg-gray-800/50 border border-gray-700/50 rounded-xl p-5 hover:border-indigo-500/30 transition group">
      <div className="flex items-start justify-between mb-3">
        <div className="flex items-center gap-3">
          <div className="w-10 h-10 rounded-lg bg-indigo-600/20 flex items-center justify-center text-indigo-400 font-bold text-sm">
            {org.name.charAt(0).toUpperCase()}
          </div>
          <div>
            <h3 className="font-semibold text-white flex items-center gap-2">
              {org.name}
              {isDefault && <span className="text-[10px] px-1.5 py-0.5 rounded bg-yellow-600/20 text-yellow-400 border border-yellow-500/30">Default</span>}
            </h3>
            <p className="text-xs text-gray-500">slug: {org.slug} · {org.owner_email}</p>
          </div>
        </div>
        <div className="flex items-center gap-1.5">
          <span className={`px-2 py-0.5 rounded text-xs font-medium border ${PLAN_COLORS[org.plan] || PLAN_COLORS.free}`}>{org.plan}</span>
          <span className={`px-2 py-0.5 rounded text-xs font-medium ${STATUS_COLORS[org.status] || STATUS_COLORS.cancelled}`}>{org.status}</span>
        </div>
      </div>

      {/* Usage row */}
      <div className="grid grid-cols-4 gap-3 mb-3">
        <div className="text-center">
          <p className="text-xs text-gray-500">Users</p>
          <p className="text-sm font-semibold text-white">{org.current_users || 0}<span className="text-gray-500 font-normal">/{org.max_users}</span></p>
        </div>
        <div className="text-center">
          <p className="text-xs text-gray-500">Targets</p>
          <p className="text-sm font-semibold text-white">{org.current_targets || 0}<span className="text-gray-500 font-normal">/{org.max_targets}</span></p>
        </div>
        <div className="text-center">
          <p className="text-xs text-gray-500">Dashboards</p>
          <p className="text-sm font-semibold text-white">—<span className="text-gray-500 font-normal">/{org.max_dashboards}</span></p>
        </div>
        <div className="text-center">
          <p className="text-xs text-gray-500">Retention</p>
          <p className="text-sm font-semibold text-white">{org.retention_days}d</p>
        </div>
      </div>

      {org.trial_ends_at && (
        <p className="text-xs text-amber-400 mb-3">Trial ends: {new Date(org.trial_ends_at).toLocaleDateString()}</p>
      )}

      {/* Actions */}
      <div className="flex items-center gap-2 pt-3 border-t border-gray-700/50">
        <button onClick={onViewDetail} className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-xs bg-white/5 hover:bg-white/10 text-gray-300 transition">
          <Eye className="w-3.5 h-3.5" /> Details
        </button>
        {!isDefault && (
          <>
            {org.status === 'active' || org.status === 'trial' ? (
              <button onClick={handleSuspend} disabled={acting} className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-xs bg-orange-600/10 hover:bg-orange-600/20 text-orange-400 transition">
                <Pause className="w-3.5 h-3.5" /> Suspend
              </button>
            ) : org.status === 'suspended' ? (
              <button onClick={handleActivate} disabled={acting} className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-xs bg-green-600/10 hover:bg-green-600/20 text-green-400 transition">
                <Play className="w-3.5 h-3.5" /> Activate
              </button>
            ) : null}
            <button onClick={handleDelete} disabled={acting} className="flex items-center gap-1.5 px-2.5 py-1.5 rounded-lg text-xs bg-red-600/10 hover:bg-red-600/20 text-red-400 transition ml-auto">
              <Trash2 className="w-3.5 h-3.5" /> Cancel
            </button>
          </>
        )}
        {acting && <Loader2 className="w-3.5 h-3.5 animate-spin text-gray-400 ml-auto" />}
      </div>
    </div>
  )
}

function CreateOrgModal({ onClose, onCreated }: { onClose: () => void; onCreated: () => void }) {
  const [form, setForm] = useState({
    name: '', slug: '', plan: 'starter', owner_email: '', owner_username: 'admin', owner_password: '',
    max_targets: '', max_users: '', retention_days: '', region: 'default', notes: '',
  })
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')
  const [result, setResult] = useState<any>(null)

  const submit = async () => {
    setLoading(true); setError(''); setResult(null)
    try {
      const payload: any = { name: form.name, owner_email: form.owner_email, plan: form.plan }
      if (form.slug) payload.slug = form.slug
      if (form.owner_username) payload.owner_username = form.owner_username
      if (form.owner_password) payload.owner_password = form.owner_password
      if (form.max_targets) payload.max_targets = parseInt(form.max_targets)
      if (form.max_users) payload.max_users = parseInt(form.max_users)
      if (form.retention_days) payload.retention_days = parseInt(form.retention_days)
      if (form.region) payload.region = form.region
      if (form.notes) payload.notes = form.notes
      const res = await createTenant(payload)
      setResult(res)
    } catch (e: any) { setError(e.message) }
    setLoading(false)
  }

  if (result) {
    return (
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
        <div className="bg-gray-800 border border-gray-700 rounded-2xl p-6 w-full max-w-md shadow-2xl">
          <div className="text-center space-y-4">
            <div className="w-16 h-16 rounded-full bg-green-600/20 flex items-center justify-center mx-auto">
              <Building2 className="w-8 h-8 text-green-400" />
            </div>
            <h2 className="text-lg font-bold text-white">Organization Created</h2>
            <div className="bg-gray-900/50 rounded-lg p-4 text-left space-y-2 text-sm">
              <div className="flex justify-between"><span className="text-gray-400">Org ID</span><span className="text-white font-mono text-xs">{result.org_id}</span></div>
              <div className="flex justify-between"><span className="text-gray-400">Name</span><span className="text-white">{result.name}</span></div>
              <div className="flex justify-between"><span className="text-gray-400">Slug</span><span className="text-white">{result.slug}</span></div>
              <div className="flex justify-between"><span className="text-gray-400">Plan</span><span className="text-white">{result.plan}</span></div>
              <div className="flex justify-between"><span className="text-gray-400">Owner Email</span><span className="text-white">{result.owner_email}</span></div>
              <div className="flex justify-between"><span className="text-gray-400">Admin Username</span><span className="text-white">{result.admin_username}</span></div>
              <div className="flex justify-between"><span className="text-gray-400">Admin User ID</span><span className="text-white font-mono text-xs">{result.admin_user_id}</span></div>
            </div>
            <p className="text-xs text-amber-400">Save these credentials — the admin password was set during creation.</p>
            <button onClick={onCreated} className="px-6 py-2 rounded-lg bg-indigo-600 hover:bg-indigo-700 text-white text-sm font-medium">Done</button>
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-gray-800 border border-gray-700 rounded-2xl p-6 w-full max-w-lg shadow-2xl max-h-[85vh] overflow-y-auto">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-bold text-white">Create Organization</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-white"><X className="w-5 h-5" /></button>
        </div>
        {error && <div className="p-2 mb-3 rounded bg-red-900/30 text-red-300 text-sm">{error}</div>}
        <div className="space-y-4">
          {/* Org Details */}
          <div>
            <label className="block text-xs font-medium text-gray-400 mb-1.5">Organization Details</label>
            <div className="grid grid-cols-2 gap-3">
              <input placeholder="Organization Name *" value={form.name} onChange={e => setForm({ ...form, name: e.target.value })}
                className="w-full px-3 py-2 rounded-lg bg-gray-900 border border-gray-600 text-white text-sm placeholder-gray-500 outline-none focus:border-indigo-500" />
              <input placeholder="Slug (auto-generated)" value={form.slug} onChange={e => setForm({ ...form, slug: e.target.value })}
                className="w-full px-3 py-2 rounded-lg bg-gray-900 border border-gray-600 text-white text-sm placeholder-gray-500 outline-none focus:border-indigo-500" />
            </div>
          </div>

          {/* Plan */}
          <div>
            <label className="block text-xs font-medium text-gray-400 mb-1.5">Plan</label>
            <div className="flex gap-2">
              {['free', 'starter', 'professional', 'enterprise'].map(p => (
                <button key={p} onClick={() => setForm({ ...form, plan: p })}
                  className={`px-3 py-1.5 rounded-lg text-xs font-medium border transition capitalize ${form.plan === p ? 'bg-indigo-600/30 border-indigo-500/50 text-indigo-300' : 'bg-gray-900 border-gray-700 text-gray-400 hover:text-white'}`}>
                  {p}
                </button>
              ))}
            </div>
          </div>

          {/* Org Admin */}
          <div>
            <label className="block text-xs font-medium text-gray-400 mb-1.5">Organization Admin (initial user)</label>
            <div className="grid grid-cols-2 gap-3">
              <input placeholder="Admin Email *" value={form.owner_email} onChange={e => setForm({ ...form, owner_email: e.target.value })}
                className="w-full px-3 py-2 rounded-lg bg-gray-900 border border-gray-600 text-white text-sm placeholder-gray-500 outline-none focus:border-indigo-500" />
              <input placeholder="Admin Username" value={form.owner_username} onChange={e => setForm({ ...form, owner_username: e.target.value })}
                className="w-full px-3 py-2 rounded-lg bg-gray-900 border border-gray-600 text-white text-sm placeholder-gray-500 outline-none focus:border-indigo-500" />
            </div>
            <input type="password" placeholder="Admin Password *" value={form.owner_password} onChange={e => setForm({ ...form, owner_password: e.target.value })}
              className="w-full mt-2 px-3 py-2 rounded-lg bg-gray-900 border border-gray-600 text-white text-sm placeholder-gray-500 outline-none focus:border-indigo-500" />
          </div>

          {/* Limits */}
          <div>
            <label className="block text-xs font-medium text-gray-400 mb-1.5">Limits (leave blank for plan defaults)</label>
            <div className="grid grid-cols-3 gap-3">
              <input type="number" placeholder="Max Targets" value={form.max_targets} onChange={e => setForm({ ...form, max_targets: e.target.value })}
                className="w-full px-3 py-2 rounded-lg bg-gray-900 border border-gray-600 text-white text-sm placeholder-gray-500 outline-none focus:border-indigo-500" />
              <input type="number" placeholder="Max Users" value={form.max_users} onChange={e => setForm({ ...form, max_users: e.target.value })}
                className="w-full px-3 py-2 rounded-lg bg-gray-900 border border-gray-600 text-white text-sm placeholder-gray-500 outline-none focus:border-indigo-500" />
              <input type="number" placeholder="Retention (days)" value={form.retention_days} onChange={e => setForm({ ...form, retention_days: e.target.value })}
                className="w-full px-3 py-2 rounded-lg bg-gray-900 border border-gray-600 text-white text-sm placeholder-gray-500 outline-none focus:border-indigo-500" />
            </div>
          </div>

          {/* Notes */}
          <textarea placeholder="Notes (optional)" value={form.notes} onChange={e => setForm({ ...form, notes: e.target.value })} rows={2}
            className="w-full px-3 py-2 rounded-lg bg-gray-900 border border-gray-600 text-white text-sm placeholder-gray-500 outline-none focus:border-indigo-500 resize-none" />
        </div>

        <div className="flex justify-end gap-3 mt-5">
          <button onClick={onClose} className="px-4 py-2 rounded-lg text-sm text-gray-400 hover:text-white transition">Cancel</button>
          <button onClick={submit} disabled={loading || !form.name || !form.owner_email || !form.owner_password}
            className="px-4 py-2 rounded-lg bg-indigo-600 hover:bg-indigo-700 disabled:bg-indigo-800 text-white text-sm font-medium transition flex items-center gap-2">
            {loading && <Loader2 className="w-4 h-4 animate-spin" />} Create Organization
          </button>
        </div>
      </div>
    </div>
  )
}

function OrgDetailModal({ orgId, onClose }: { orgId: string; onClose: () => void }) {
  const [usage, setUsage] = useState<TenantUsage | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    (async () => {
      try { setUsage(await getTenantUsage(orgId)) } catch {}
      setLoading(false)
    })()
  }, [orgId])

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-gray-800 border border-gray-700 rounded-2xl p-6 w-full max-w-md shadow-2xl">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-bold text-white">Organization Usage</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-white"><X className="w-5 h-5" /></button>
        </div>
        {loading ? (
          <div className="text-center p-8 text-gray-500"><Loader2 className="w-5 h-5 animate-spin inline" /></div>
        ) : usage ? (
          <div className="space-y-4">
            <h3 className="text-white font-semibold">{usage.org_name}</h3>
            <div className="grid grid-cols-2 gap-3">
              <UsageBar label="Targets" used={usage.targets_used} limit={usage.targets_limit} icon={Server} />
              <UsageBar label="Users" used={usage.users_used} limit={usage.users_limit} icon={Users} />
            </div>
            <div className="p-3 rounded-lg bg-gray-900/50 border border-gray-700/50">
              <div className="flex items-center gap-2 text-sm">
                <BarChart3 className="w-4 h-4 text-blue-400" />
                <span className="text-gray-400">Alerts (30d):</span>
                <span className="text-white font-semibold">{usage.alerts_fired_30d}</span>
              </div>
            </div>
          </div>
        ) : (
          <p className="text-gray-500 text-sm">Failed to load usage data</p>
        )}
        <div className="flex justify-end mt-4">
          <button onClick={onClose} className="px-4 py-2 rounded-lg text-sm text-gray-400 hover:text-white">Close</button>
        </div>
      </div>
    </div>
  )
}

function UsageBar({ label, used, limit, icon: Icon }: { label: string; used: number; limit: number; icon: any }) {
  const pct = limit > 0 ? Math.min((used / limit) * 100, 100) : 0
  const color = pct >= 90 ? 'bg-red-500' : pct >= 70 ? 'bg-amber-500' : 'bg-green-500'
  return (
    <div className="p-3 rounded-lg bg-gray-900/50 border border-gray-700/50">
      <div className="flex items-center gap-2 mb-2">
        <Icon className="w-3.5 h-3.5 text-gray-400" />
        <span className="text-xs text-gray-400">{label}</span>
        <span className="ml-auto text-xs text-white font-semibold">{used}/{limit}</span>
      </div>
      <div className="w-full h-1.5 rounded-full bg-gray-700">
        <div className={`h-full rounded-full ${color} transition-all`} style={{ width: `${pct}%` }} />
      </div>
    </div>
  )
}
