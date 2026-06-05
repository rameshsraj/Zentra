import { useState, useEffect, useCallback } from 'react'
import { Link } from 'react-router-dom'
import { getUsers, createUser, deleteUser, lockUser, unlockUser, resetUserPassword, bulkUserAction, getRoles, assignUserRoles, getUserGroups, addGroupMembers, getUserModules, setUserModules, UserListItem, RoleItem, UserGroupItem } from '../../lib/api'
import { Users, Plus, Search, Lock, Unlock, Trash2, Key, MoreVertical, X, Loader2, AlertCircle, Shield, UserPlus, ExternalLink, LayoutGrid, Crown } from 'lucide-react'
import ModuleAccessTree from '../../components/ModuleAccessTree'
import { useAuth } from '../../contexts/AuthContext'

export default function UserManagement() {
  const { isSuperAdmin: currentUserIsSuperAdmin } = useAuth()
  const [users, setUsers] = useState<UserListItem[]>([])
  const [total, setTotal] = useState(0)
  const [loading, setLoading] = useState(true)
  const [search, setSearch] = useState('')
  const [statusFilter, setStatusFilter] = useState('')
  const [roleFilter, setRoleFilter] = useState('')
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [showCreate, setShowCreate] = useState(false)
  const [showPasswordReset, setShowPasswordReset] = useState<string | null>(null)
  const [showRoleAssign, setShowRoleAssign] = useState<string | null>(null)
  const [showGroupAssign, setShowGroupAssign] = useState<string | null>(null)
  const [showModuleAccess, setShowModuleAccess] = useState<string | null>(null)
  const [error, setError] = useState('')
  const [roles, setRoles] = useState<RoleItem[]>([])
  const [groups, setGroups] = useState<UserGroupItem[]>([])

  const fetchUsers = useCallback(async () => {
    setLoading(true)
    try {
      const params: Record<string, string> = {}
      if (search) params.search = search
      if (statusFilter) params.status = statusFilter
      if (roleFilter) params.role = roleFilter
      const res = await getUsers(params)
      setUsers(res.users)
      setTotal(res.total)
    } catch (e: any) { setError(e.message) }
    setLoading(false)
  }, [search, statusFilter, roleFilter])

  useEffect(() => { fetchUsers() }, [fetchUsers])
  useEffect(() => { getRoles().then(setRoles).catch(() => {}); getUserGroups().then(setGroups).catch(() => {}) }, [])

  const handleBulk = async (action: string) => {
    if (selected.size === 0) return
    await bulkUserAction({ action, user_ids: Array.from(selected) })
    setSelected(new Set())
    fetchUsers()
  }

  const toggleSelect = (id: string) => {
    const next = new Set(selected)
    next.has(id) ? next.delete(id) : next.add(id)
    setSelected(next)
  }

  const toggleAll = () => {
    if (selected.size === users.length) setSelected(new Set())
    else setSelected(new Set(users.map(u => u.id)))
  }

  const activeCount = users.filter(u => u.is_active && !u.is_locked).length
  const lockedCount = users.filter(u => u.is_locked).length
  const disabledCount = users.filter(u => !u.is_active).length

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="p-2.5 rounded-xl bg-blue-600/20"><Users className="w-5 h-5 text-blue-400" /></div>
          <div>
            <h1 className="text-2xl font-bold text-white">User Management</h1>
            <p className="text-sm text-gray-400">{total} users total</p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Link to="/admin/groups" className="flex items-center gap-2 px-3 py-2 rounded-lg bg-purple-600/20 text-purple-400 hover:bg-purple-600/30 text-sm font-medium transition">
            <Users className="w-4 h-4" /> Groups
          </Link>
          <Link to="/admin/roles" className="flex items-center gap-2 px-3 py-2 rounded-lg bg-red-600/20 text-red-400 hover:bg-red-600/30 text-sm font-medium transition">
            <Shield className="w-4 h-4" /> Roles
          </Link>
          <button onClick={() => setShowCreate(true)} className="flex items-center gap-2 px-4 py-2 rounded-lg bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium transition">
            <Plus className="w-4 h-4" /> Create User
          </button>
        </div>
      </div>

      {/* KPI Strip */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        {[
          { label: 'Total Users', value: total, color: 'text-white', bg: 'bg-white/5 border-white/10' },
          { label: 'Active', value: activeCount, color: 'text-green-400', bg: 'bg-green-500/10 border-green-500/20' },
          { label: 'Locked', value: lockedCount, color: 'text-red-400', bg: 'bg-red-500/10 border-red-500/20' },
          { label: 'Disabled', value: disabledCount, color: 'text-gray-400', bg: 'bg-gray-500/10 border-gray-500/20' },
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
          <input value={search} onChange={e => setSearch(e.target.value)} placeholder="Search users by name, email, username..."
            className="w-full pl-10 pr-4 py-2 rounded-lg bg-gray-800 border border-gray-700 text-white text-sm placeholder-gray-500 focus:border-blue-500 outline-none" />
        </div>
        <select value={statusFilter} onChange={e => setStatusFilter(e.target.value)}
          className="px-3 py-2 rounded-lg bg-gray-800 border border-gray-700 text-white text-sm">
          <option value="">All Status</option>
          <option value="active">Active</option>
          <option value="locked">Locked</option>
          <option value="disabled">Disabled</option>
        </select>
        <select value={roleFilter} onChange={e => setRoleFilter(e.target.value)}
          className="px-3 py-2 rounded-lg bg-gray-800 border border-gray-700 text-white text-sm">
          <option value="">All Roles</option>
          {roles.map(r => <option key={r.role_id} value={r.name}>{r.name}</option>)}
        </select>
      </div>

      {/* Bulk Actions */}
      {selected.size > 0 && (
        <div className="flex items-center gap-3 p-3 rounded-lg bg-blue-900/30 border border-blue-700/30">
          <span className="text-sm text-blue-300">{selected.size} selected</span>
          <button onClick={() => handleBulk('enable')} className="px-3 py-1 text-xs rounded bg-green-600/20 text-green-400 hover:bg-green-600/30">Enable</button>
          <button onClick={() => handleBulk('disable')} className="px-3 py-1 text-xs rounded bg-yellow-600/20 text-yellow-400 hover:bg-yellow-600/30">Disable</button>
          <button onClick={() => handleBulk('lock')} className="px-3 py-1 text-xs rounded bg-orange-600/20 text-orange-400 hover:bg-orange-600/30">Lock</button>
          <button onClick={() => handleBulk('delete')} className="px-3 py-1 text-xs rounded bg-red-600/20 text-red-400 hover:bg-red-600/30">Delete</button>
          <button onClick={() => setSelected(new Set())} className="ml-auto text-gray-400 hover:text-white"><X className="w-4 h-4" /></button>
        </div>
      )}

      {error && <div className="flex items-center gap-2 p-3 rounded-lg bg-red-900/30 border border-red-700/50 text-red-300 text-sm"><AlertCircle className="w-4 h-4" />{error}</div>}

      {/* Table */}
      <div className="bg-gray-800/50 border border-gray-700/50 rounded-xl overflow-hidden">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-gray-700/50 text-gray-400 text-left">
              <th className="p-3 w-10"><input type="checkbox" checked={selected.size === users.length && users.length > 0} onChange={toggleAll} className="rounded" /></th>
              <th className="p-3">User</th>
              <th className="p-3">Email</th>
              <th className="p-3">Roles</th>
              <th className="p-3">Status</th>
              <th className="p-3">Last Login</th>
              <th className="p-3 w-28">Actions</th>
            </tr>
          </thead>
          <tbody>
            {loading ? (
              <tr><td colSpan={7} className="p-8 text-center text-gray-500"><Loader2 className="w-5 h-5 animate-spin inline mr-2" />Loading...</td></tr>
            ) : users.length === 0 ? (
              <tr><td colSpan={7} className="p-8 text-center text-gray-500">No users found</td></tr>
            ) : users.map(u => (
              <tr key={u.id} className="border-b border-gray-700/30 hover:bg-gray-700/20 transition">
                <td className="p-3"><input type="checkbox" checked={selected.has(u.id)} onChange={() => toggleSelect(u.id)} className="rounded" /></td>
                <td className="p-3">
                  <div className="flex items-center gap-2">
                    <div className="w-8 h-8 rounded-full bg-blue-600/20 flex items-center justify-center text-blue-400 text-xs font-bold">
                      {(u.display_name || u.username).charAt(0).toUpperCase()}
                    </div>
                    <div>
                      <p className="font-medium text-white">{u.display_name || u.username}</p>
                      <p className="text-xs text-gray-500">@{u.username}</p>
                    </div>
                  </div>
                </td>
                <td className="p-3 text-gray-300">{u.email}</td>
                <td className="p-3">
                  <div className="flex gap-1 flex-wrap items-center">
                    {u.roles.map(r => (
                      <span key={r} className={`px-2 py-0.5 rounded text-xs font-medium inline-flex items-center gap-1 ${
                        r === 'Super Admin' ? 'bg-yellow-600/20 text-yellow-400 border border-yellow-500/30' :
                        r === 'Admin' ? 'bg-red-600/20 text-red-400' :
                        r === 'Operator' ? 'bg-orange-600/20 text-orange-400' :
                        r === 'Editor' ? 'bg-blue-600/20 text-blue-400' : 'bg-gray-600/20 text-gray-400'
                      }`}>{r === 'Super Admin' && <Crown className="w-3 h-3" />}{r}</span>
                    ))}
                    {currentUserIsSuperAdmin && (
                      <button onClick={() => setShowRoleAssign(u.id)} className="p-0.5 rounded hover:bg-gray-600 text-gray-500 hover:text-blue-400" title="Assign Roles">
                        <Shield className="w-3 h-3" />
                      </button>
                    )}
                  </div>
                </td>
                <td className="p-3">
                  {!u.is_active ? <span className="px-2 py-0.5 rounded text-xs bg-gray-600/20 text-gray-400">Disabled</span> :
                   u.is_locked ? <span className="px-2 py-0.5 rounded text-xs bg-red-600/20 text-red-400">Locked</span> :
                   <span className="px-2 py-0.5 rounded text-xs bg-green-600/20 text-green-400">Active</span>}
                </td>
                <td className="p-3 text-gray-400 text-xs">{u.last_login ? new Date(u.last_login).toLocaleString() : 'Never'}</td>
                <td className="p-3">
                  <div className="flex items-center gap-1">
                    {!u.roles.includes('Super Admin') && <button onClick={() => setShowModuleAccess(u.id)} className="p-1 rounded hover:bg-gray-700 text-emerald-400" title="Module Access"><LayoutGrid className="w-3.5 h-3.5" /></button>}
                    {u.roles.includes('Super Admin') && <span className="text-[10px] text-yellow-500/60 italic">Full Access</span>}
                    <button onClick={() => setShowGroupAssign(u.id)} className="p-1 rounded hover:bg-gray-700 text-purple-400" title="Assign to Group"><UserPlus className="w-3.5 h-3.5" /></button>
                    <UserActions user={u} onRefresh={fetchUsers} onResetPassword={() => setShowPasswordReset(u.id)} isSuperAdminRow={u.roles.includes('Super Admin')} currentUserIsSuperAdmin={currentUserIsSuperAdmin} />
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {showCreate && <CreateUserModal roles={roles} groups={groups} onClose={() => setShowCreate(false)} onCreated={() => { setShowCreate(false); fetchUsers() }} />}
      {showPasswordReset && <PasswordResetModal userId={showPasswordReset} onClose={() => setShowPasswordReset(null)} />}
      {showRoleAssign && <RoleAssignModal userId={showRoleAssign} userRoles={users.find(u => u.id === showRoleAssign)?.roles || []} allRoles={roles} onClose={() => setShowRoleAssign(null)} onSaved={() => { setShowRoleAssign(null); fetchUsers() }} />}
      {showGroupAssign && <GroupAssignModal userId={showGroupAssign} allGroups={groups} onClose={() => setShowGroupAssign(null)} onSaved={() => { setShowGroupAssign(null); fetchUsers() }} />}
      {showModuleAccess && <ModuleAccessModal userId={showModuleAccess} userName={users.find(u => u.id === showModuleAccess)?.display_name || users.find(u => u.id === showModuleAccess)?.username || ''} onClose={() => setShowModuleAccess(null)} onSaved={() => setShowModuleAccess(null)} />}
    </div>
  )
}

function UserActions({ user, onRefresh, onResetPassword, isSuperAdminRow, currentUserIsSuperAdmin }: { user: UserListItem; onRefresh: () => void; onResetPassword: () => void; isSuperAdminRow?: boolean; currentUserIsSuperAdmin?: boolean }) {
  const [open, setOpen] = useState(false)

  const handle = async (action: () => Promise<any>) => {
    setOpen(false)
    await action()
    onRefresh()
  }

  return (
    <div className="relative">
      <button onClick={() => setOpen(!open)} className="p-1 rounded hover:bg-gray-700 text-gray-400"><MoreVertical className="w-4 h-4" /></button>
      {open && (
        <>
          <div className="fixed inset-0 z-10" onClick={(e) => { e.stopPropagation(); setOpen(false) }} />
          <div className="absolute right-0 top-8 z-20 w-44 bg-gray-800 border border-gray-700 rounded-lg shadow-xl py-1 text-sm">
            {isSuperAdminRow ? (
              <div className="px-3 py-2 text-yellow-500/70 text-xs flex items-center gap-2">
                <Crown className="w-3.5 h-3.5" />Super Admin — Protected
              </div>
            ) : (
              <>
                {user.is_locked
                  ? <button onClick={() => handle(() => unlockUser(user.id))} className="w-full flex items-center gap-2 px-3 py-2 text-left hover:bg-gray-700 text-green-400"><Unlock className="w-3.5 h-3.5" />Unlock</button>
                  : <button onClick={() => handle(() => lockUser(user.id))} className="w-full flex items-center gap-2 px-3 py-2 text-left hover:bg-gray-700 text-orange-400"><Lock className="w-3.5 h-3.5" />Lock</button>
                }
              </>
            )}
            {currentUserIsSuperAdmin && <button onClick={() => { setOpen(false); onResetPassword() }} className="w-full flex items-center gap-2 px-3 py-2 text-left hover:bg-gray-700 text-blue-400"><Key className="w-3.5 h-3.5" />Reset Password</button>}
            {!isSuperAdminRow && <button onClick={() => handle(() => deleteUser(user.id))} className="w-full flex items-center gap-2 px-3 py-2 text-left hover:bg-gray-700 text-red-400"><Trash2 className="w-3.5 h-3.5" />Deactivate</button>}
          </div>
        </>
      )}
    </div>
  )
}

function CreateUserModal({ roles, groups, onClose, onCreated }: { roles: RoleItem[]; groups: UserGroupItem[]; onClose: () => void; onCreated: () => void }) {
  const [form, setForm] = useState({ email: '', username: '', password: '', display_name: '' })
  const [selectedRoles, setSelectedRoles] = useState<Set<string>>(new Set())
  const [selectedGroups, setSelectedGroups] = useState<Set<string>>(new Set())
  const [selectedModules, setSelectedModules] = useState<Set<string>>(new Set(['command_center']))
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')

  const toggleRole = (id: string) => { const s = new Set(selectedRoles); s.has(id) ? s.delete(id) : s.add(id); setSelectedRoles(s) }
  const toggleGroup = (id: string) => { const s = new Set(selectedGroups); s.has(id) ? s.delete(id) : s.add(id); setSelectedGroups(s) }

  const submit = async () => {
    setLoading(true); setError('')
    try {
      const res = await createUser({ ...form, role_ids: Array.from(selectedRoles) })
      // Set module access for the newly created user
      if (selectedModules.size > 0 && res?.id) {
        await setUserModules(res.id, Array.from(selectedModules)).catch(() => {})
      }
      onCreated()
    } catch (e: any) { setError(e.message) }
    setLoading(false)
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-gray-800 border border-gray-700 rounded-2xl p-6 w-full max-w-lg shadow-2xl max-h-[85vh] overflow-y-auto">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-bold text-white">Create User</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-white"><X className="w-5 h-5" /></button>
        </div>
        {error && <div className="p-2 mb-3 rounded bg-red-900/30 text-red-300 text-sm">{error}</div>}
        <div className="space-y-3">
          <div className="grid grid-cols-2 gap-3">
            <input placeholder="Email *" value={form.email} onChange={e => setForm({ ...form, email: e.target.value })}
              className="w-full px-3 py-2 rounded-lg bg-gray-900 border border-gray-600 text-white text-sm placeholder-gray-500 outline-none focus:border-blue-500" />
            <input placeholder="Username *" value={form.username} onChange={e => setForm({ ...form, username: e.target.value })}
              className="w-full px-3 py-2 rounded-lg bg-gray-900 border border-gray-600 text-white text-sm placeholder-gray-500 outline-none focus:border-blue-500" />
          </div>
          <div className="grid grid-cols-2 gap-3">
            <input placeholder="Display Name" value={form.display_name} onChange={e => setForm({ ...form, display_name: e.target.value })}
              className="w-full px-3 py-2 rounded-lg bg-gray-900 border border-gray-600 text-white text-sm placeholder-gray-500 outline-none focus:border-blue-500" />
            <input type="password" placeholder="Password *" value={form.password} onChange={e => setForm({ ...form, password: e.target.value })}
              className="w-full px-3 py-2 rounded-lg bg-gray-900 border border-gray-600 text-white text-sm placeholder-gray-500 outline-none focus:border-blue-500" />
          </div>

          {/* Role Selection — Super Admin excluded (only 1 allowed system-wide) */}
          <div>
            <label className="block text-xs font-medium text-gray-400 mb-1.5">Assign Roles</label>
            <div className="flex flex-wrap gap-2">
              {roles.filter(r => r.name !== 'Super Admin').map(r => (
                <button key={r.role_id} onClick={() => toggleRole(r.role_id)}
                  className={`px-2.5 py-1 rounded-lg text-xs font-medium border transition ${selectedRoles.has(r.role_id) ? 'bg-blue-600/30 border-blue-500/50 text-blue-300' : 'bg-gray-900 border-gray-700 text-gray-400 hover:text-white'}`}>
                  <Shield className="w-3 h-3 inline mr-1" />{r.name}
                </button>
              ))}
            </div>
          </div>

          {/* Group Selection */}
          <div>
            <label className="block text-xs font-medium text-gray-400 mb-1.5">Add to Groups</label>
            <div className="flex flex-wrap gap-2">
              {groups.map(g => (
                <button key={g.id} onClick={() => toggleGroup(g.id)}
                  className={`px-2.5 py-1 rounded-lg text-xs font-medium border transition ${selectedGroups.has(g.id) ? 'bg-purple-600/30 border-purple-500/50 text-purple-300' : 'bg-gray-900 border-gray-700 text-gray-400 hover:text-white'}`}>
                  <Users className="w-3 h-3 inline mr-1" />{g.name}
                </button>
              ))}
              {groups.length === 0 && <span className="text-xs text-gray-500">No groups available</span>}
            </div>
          </div>

          {/* Module Access Tree */}
          <div>
            <label className="block text-xs font-medium text-gray-400 mb-1.5 flex items-center gap-1.5">
              <LayoutGrid className="w-3.5 h-3.5 text-emerald-400" /> Module Access
              <span className="text-gray-600 font-normal">— select which screens/modules this user can access</span>
            </label>
            <div className="bg-gray-900/50 border border-gray-700/50 rounded-lg p-2 max-h-64 overflow-y-auto custom-scrollbar">
              <ModuleAccessTree selected={selectedModules} onChange={setSelectedModules} />
            </div>
          </div>
        </div>
        <div className="flex justify-end gap-3 mt-5">
          <button onClick={onClose} className="px-4 py-2 rounded-lg text-sm text-gray-400 hover:text-white transition">Cancel</button>
          <button onClick={submit} disabled={loading || !form.email || !form.username || !form.password}
            className="px-4 py-2 rounded-lg bg-blue-600 hover:bg-blue-700 disabled:bg-blue-800 text-white text-sm font-medium transition flex items-center gap-2">
            {loading && <Loader2 className="w-4 h-4 animate-spin" />} Create User
          </button>
        </div>
      </div>
    </div>
  )
}

function RoleAssignModal({ userId, userRoles, allRoles, onClose, onSaved }: { userId: string; userRoles: string[]; allRoles: RoleItem[]; onClose: () => void; onSaved: () => void }) {
  const [selected, setSelected] = useState<Set<string>>(new Set(allRoles.filter(r => userRoles.includes(r.name)).map(r => r.role_id)))
  const [saving, setSaving] = useState(false)

  const toggle = (id: string) => { const s = new Set(selected); s.has(id) ? s.delete(id) : s.add(id); setSelected(s) }

  const save = async () => {
    setSaving(true)
    await assignUserRoles(userId, Array.from(selected)).catch(() => {})
    setSaving(false)
    onSaved()
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-gray-800 border border-gray-700 rounded-2xl p-6 w-full max-w-sm shadow-2xl">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-bold text-white">Assign Roles</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-white"><X className="w-5 h-5" /></button>
        </div>
        <div className="space-y-2">
          {allRoles.map(r => (
            <label key={r.role_id} className="flex items-center gap-3 px-3 py-2.5 rounded-lg bg-gray-900/50 hover:bg-gray-900 cursor-pointer transition">
              <input type="checkbox" checked={selected.has(r.role_id)} onChange={() => toggle(r.role_id)} className="rounded" />
              <div className="flex-1">
                <p className="text-sm text-white font-medium">{r.name}</p>
                {r.description && <p className="text-xs text-gray-500">{r.description}</p>}
              </div>
              <span className="text-xs text-gray-500">{r.permission_count} perms</span>
            </label>
          ))}
        </div>
        <div className="flex items-center justify-between mt-4">
          <Link to="/admin/roles" className="text-xs text-blue-400 hover:underline flex items-center gap-1"><ExternalLink className="w-3 h-3" /> Manage Roles</Link>
          <div className="flex gap-2">
            <button onClick={onClose} className="px-3 py-1.5 rounded-lg text-sm text-gray-400 hover:text-white">Cancel</button>
            <button onClick={save} disabled={saving}
              className="px-3 py-1.5 rounded-lg bg-blue-600 hover:bg-blue-700 text-white text-sm font-medium flex items-center gap-1">
              {saving && <Loader2 className="w-3 h-3 animate-spin" />} Save
            </button>
          </div>
        </div>
      </div>
    </div>
  )
}

function GroupAssignModal({ userId, allGroups, onClose, onSaved }: { userId: string; allGroups: UserGroupItem[]; onClose: () => void; onSaved: () => void }) {
  const [saving, setSaving] = useState<string | null>(null)

  const addToGroup = async (groupId: string) => {
    setSaving(groupId)
    await addGroupMembers(groupId, [userId]).catch(() => {})
    setSaving(null)
    onSaved()
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-gray-800 border border-gray-700 rounded-2xl p-6 w-full max-w-sm shadow-2xl">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-bold text-white">Add to Group</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-white"><X className="w-5 h-5" /></button>
        </div>
        <div className="space-y-2">
          {allGroups.map(g => (
            <div key={g.id} className="flex items-center justify-between px-3 py-2.5 rounded-lg bg-gray-900/50">
              <div>
                <p className="text-sm text-white font-medium">{g.name}</p>
                <p className="text-xs text-gray-500">{g.member_count} members</p>
              </div>
              <button onClick={() => addToGroup(g.id)} disabled={saving === g.id}
                className="px-2 py-1 rounded text-xs bg-purple-600/20 text-purple-400 hover:bg-purple-600/30 flex items-center gap-1">
                {saving === g.id ? <Loader2 className="w-3 h-3 animate-spin" /> : <UserPlus className="w-3 h-3" />} Add
              </button>
            </div>
          ))}
          {allGroups.length === 0 && <p className="text-sm text-gray-500 text-center py-4">No groups exist yet</p>}
        </div>
        <div className="flex items-center justify-between mt-4">
          <Link to="/admin/groups" className="text-xs text-purple-400 hover:underline flex items-center gap-1"><ExternalLink className="w-3 h-3" /> Manage Groups</Link>
          <button onClick={onClose} className="px-3 py-1.5 rounded-lg text-sm text-gray-400 hover:text-white">Close</button>
        </div>
      </div>
    </div>
  )
}

function PasswordResetModal({ userId, onClose }: { userId: string; onClose: () => void }) {
  const [password, setPassword] = useState('')
  const [loading, setLoading] = useState(false)
  const [done, setDone] = useState(false)

  const submit = async () => {
    setLoading(true)
    await resetUserPassword(userId, password)
    setLoading(false); setDone(true)
    setTimeout(onClose, 1500)
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-gray-800 border border-gray-700 rounded-2xl p-6 w-full max-w-sm shadow-2xl">
        <h2 className="text-lg font-bold text-white mb-4">Reset Password</h2>
        {done ? <p className="text-green-400 text-sm">Password reset successfully!</p> : (
          <>
            <input type="password" placeholder="New password" value={password} onChange={e => setPassword(e.target.value)}
              className="w-full px-3 py-2 rounded-lg bg-gray-900 border border-gray-600 text-white text-sm placeholder-gray-500 outline-none focus:border-blue-500" />
            <div className="flex justify-end gap-3 mt-4">
              <button onClick={onClose} className="px-4 py-2 rounded-lg text-sm text-gray-400 hover:text-white">Cancel</button>
              <button onClick={submit} disabled={loading || !password}
                className="px-4 py-2 rounded-lg bg-blue-600 hover:bg-blue-700 disabled:bg-blue-800 text-white text-sm font-medium flex items-center gap-2">
                {loading && <Loader2 className="w-4 h-4 animate-spin" />} Reset
              </button>
            </div>
          </>
        )}
      </div>
    </div>
  )
}

function ModuleAccessModal({ userId, userName, onClose, onSaved }: { userId: string; userName: string; onClose: () => void; onSaved: () => void }) {
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState('')

  useEffect(() => {
    getUserModules(userId)
      .then(res => { setSelected(new Set(res.modules)); setLoading(false) })
      .catch(e => { setError(e.message); setLoading(false) })
  }, [userId])

  const save = async () => {
    setSaving(true); setError('')
    try {
      await setUserModules(userId, Array.from(selected))
      onSaved()
    } catch (e: any) { setError(e.message) }
    setSaving(false)
  }

  const moduleCount = Array.from(selected).filter(m => !m.includes('.')).length
  const subCount = Array.from(selected).filter(m => m.includes('.')).length

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-gray-800 border border-gray-700 rounded-2xl p-6 w-full max-w-lg shadow-2xl max-h-[85vh] flex flex-col">
        <div className="flex items-center justify-between mb-4">
          <div>
            <h2 className="text-lg font-bold text-white flex items-center gap-2">
              <LayoutGrid className="w-5 h-5 text-emerald-400" /> Module Access
            </h2>
            <p className="text-xs text-gray-400 mt-0.5">
              {userName} — {moduleCount} modules, {subCount} sub-items selected
            </p>
          </div>
          <button onClick={onClose} className="text-gray-400 hover:text-white"><X className="w-5 h-5" /></button>
        </div>
        {error && <div className="p-2 mb-3 rounded bg-red-900/30 text-red-300 text-sm">{error}</div>}
        <div className="flex-1 overflow-y-auto custom-scrollbar pr-1">
          {loading ? (
            <div className="flex items-center justify-center py-8 text-gray-500"><Loader2 className="w-5 h-5 animate-spin mr-2" />Loading...</div>
          ) : (
            <ModuleAccessTree selected={selected} onChange={setSelected} />
          )}
        </div>
        <div className="flex justify-end gap-3 mt-4 pt-3 border-t border-gray-700/50">
          <button onClick={onClose} className="px-4 py-2 rounded-lg text-sm text-gray-400 hover:text-white transition">Cancel</button>
          <button onClick={save} disabled={saving}
            className="px-4 py-2 rounded-lg bg-emerald-600 hover:bg-emerald-700 disabled:bg-emerald-800 text-white text-sm font-medium transition flex items-center gap-2">
            {saving && <Loader2 className="w-4 h-4 animate-spin" />} Save Module Access
          </button>
        </div>
      </div>
    </div>
  )
}
