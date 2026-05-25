import { useState, useEffect, useCallback } from 'react'
import { Link } from 'react-router-dom'
import { getRoles, createRole, deleteRole, getRole, updateRole, getPermissions, RoleItem, PermissionItem } from '../../lib/api'
import { Shield, Plus, Trash2, Eye, X, Loader2, Save, Users, Lock, CheckCircle } from 'lucide-react'

export default function RoleManagement() {
  const [roles, setRoles] = useState<RoleItem[]>([])
  const [loading, setLoading] = useState(true)
  const [showCreate, setShowCreate] = useState(false)
  const [editing, setEditing] = useState<string | null>(null)

  const fetchRoles = useCallback(async () => { setLoading(true); try { setRoles(await getRoles()) } catch {} setLoading(false) }, [])
  useEffect(() => { fetchRoles() }, [fetchRoles])

  const systemCount = roles.filter(r => r.is_system).length
  const customCount = roles.filter(r => !r.is_system).length
  const totalPerms = roles.reduce((a, r) => a + r.permission_count, 0)

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="p-2.5 rounded-xl bg-red-600/20"><Shield className="w-5 h-5 text-red-400" /></div>
          <div><h1 className="text-2xl font-bold text-white">Role Management</h1><p className="text-sm text-gray-400">{roles.length} roles · {totalPerms} permissions assigned</p></div>
        </div>
        <div className="flex items-center gap-2">
          <Link to="/admin/users" className="flex items-center gap-2 px-3 py-2 rounded-lg bg-blue-600/20 text-blue-400 hover:bg-blue-600/30 text-sm font-medium transition">
            <Users className="w-4 h-4" /> Users
          </Link>
          <Link to="/admin/groups" className="flex items-center gap-2 px-3 py-2 rounded-lg bg-purple-600/20 text-purple-400 hover:bg-purple-600/30 text-sm font-medium transition">
            <Users className="w-4 h-4" /> Groups
          </Link>
          <button onClick={() => setShowCreate(true)} className="flex items-center gap-2 px-4 py-2 rounded-lg bg-red-600 hover:bg-red-700 text-white text-sm font-medium transition">
            <Plus className="w-4 h-4" /> Create Role
          </button>
        </div>
      </div>

      {/* KPI Strip */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        {[
          { label: 'Total Roles', value: roles.length, color: 'text-white', bg: 'bg-white/5 border-white/10', icon: Shield },
          { label: 'System Roles', value: systemCount, color: 'text-blue-400', bg: 'bg-blue-500/10 border-blue-500/20', icon: Lock },
          { label: 'Custom Roles', value: customCount, color: 'text-green-400', bg: 'bg-green-500/10 border-green-500/20', icon: CheckCircle },
          { label: 'Total Permissions', value: totalPerms, color: 'text-purple-400', bg: 'bg-purple-500/10 border-purple-500/20', icon: Shield },
        ].map((k, i) => (
          <div key={i} className={`rounded-xl border p-3 ${k.bg}`}>
            <div className="flex items-center gap-2">
              <k.icon className={`w-3.5 h-3.5 ${k.color}`} />
              <p className="text-xs text-gray-500">{k.label}</p>
            </div>
            <p className={`text-xl font-bold mt-1 ${k.color}`}>{k.value}</p>
          </div>
        ))}
      </div>

      {loading ? <div className="text-center p-8 text-gray-500"><Loader2 className="w-5 h-5 animate-spin inline" /></div> : (
        <div className="bg-gray-800/50 border border-gray-700/50 rounded-xl overflow-hidden">
          <table className="w-full text-sm">
            <thead><tr className="border-b border-gray-700/50 text-gray-400 text-left">
              <th className="p-3">Role</th><th className="p-3">Description</th><th className="p-3">Type</th><th className="p-3">Permissions</th><th className="p-3">Created</th><th className="p-3 w-20">Actions</th>
            </tr></thead>
            <tbody>{roles.map(r => (
              <tr key={r.role_id} className="border-b border-gray-700/30 hover:bg-gray-700/20 transition">
                <td className="p-3">
                  <div className="flex items-center gap-2">
                    <Shield className={`w-4 h-4 ${r.is_system ? 'text-blue-400' : 'text-red-400'}`} />
                    <span className="font-medium text-white">{r.name}</span>
                  </div>
                </td>
                <td className="p-3 text-gray-400 max-w-xs truncate">{r.description || '—'}</td>
                <td className="p-3"><span className={`px-2 py-0.5 rounded text-xs ${r.is_system ? 'bg-blue-600/20 text-blue-400' : 'bg-gray-600/20 text-gray-400'}`}>{r.is_system ? 'System' : 'Custom'}</span></td>
                <td className="p-3"><span className="text-gray-300 font-mono text-xs">{r.permission_count}</span></td>
                <td className="p-3 text-gray-500 text-xs">{new Date(r.created_at).toLocaleDateString()}</td>
                <td className="p-3">
                  <div className="flex gap-1">
                    <button onClick={() => setEditing(r.role_id)} className="p-1 rounded hover:bg-gray-700 text-gray-400" title="Edit"><Eye className="w-3.5 h-3.5" /></button>
                    {!r.is_system && <button onClick={async () => { if (confirm(`Delete role "${r.name}"?`)) { await deleteRole(r.role_id); fetchRoles() } }} className="p-1 rounded hover:bg-gray-700 text-red-400" title="Delete"><Trash2 className="w-3.5 h-3.5" /></button>}
                  </div>
                </td>
              </tr>
            ))}</tbody>
          </table>
        </div>
      )}

      {showCreate && <RoleEditor onClose={() => setShowCreate(false)} onSaved={() => { setShowCreate(false); fetchRoles() }} />}
      {editing && <RoleEditor roleId={editing} onClose={() => setEditing(null)} onSaved={() => { setEditing(null); fetchRoles() }} />}
    </div>
  )
}

function RoleEditor({ roleId, onClose, onSaved }: { roleId?: string; onClose: () => void; onSaved: () => void }) {
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [selectedPerms, setSelectedPerms] = useState<Set<string>>(new Set())
  const [allPerms, setAllPerms] = useState<Record<string, PermissionItem[]>>({})
  const [loading, setLoading] = useState(true)
  const [saving, setSaving] = useState(false)

  useEffect(() => {
    (async () => {
      const perms = await getPermissions()
      setAllPerms(perms)
      if (roleId) {
        const detail = await getRole(roleId)
        setName(detail.name)
        setDescription(detail.description || '')
        setSelectedPerms(new Set(detail.permissions.map(p => p.permission_id)))
      }
      setLoading(false)
    })()
  }, [roleId])

  const togglePerm = (id: string) => {
    const next = new Set(selectedPerms)
    next.has(id) ? next.delete(id) : next.add(id)
    setSelectedPerms(next)
  }

  const toggleAll = (resource: string) => {
    const perms = allPerms[resource] || []
    const allSelected = perms.every(p => selectedPerms.has(p.permission_id))
    const next = new Set(selectedPerms)
    perms.forEach(p => allSelected ? next.delete(p.permission_id) : next.add(p.permission_id))
    setSelectedPerms(next)
  }

  const save = async () => {
    setSaving(true)
    if (roleId) {
      await updateRole(roleId, { name, description, permission_ids: Array.from(selectedPerms) })
    } else {
      await createRole({ name, description, permission_ids: Array.from(selectedPerms) })
    }
    setSaving(false)
    onSaved()
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60" onClick={onClose}>
      <div className="bg-gray-800 border border-gray-700 rounded-2xl p-6 w-full max-w-2xl shadow-2xl max-h-[85vh] overflow-y-auto" onClick={e => e.stopPropagation()}>
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-bold text-white">{roleId ? 'Edit Role' : 'Create Role'}</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-white"><X className="w-5 h-5" /></button>
        </div>

        {loading ? <div className="text-center p-8"><Loader2 className="w-5 h-5 animate-spin inline text-gray-500" /></div> : (
          <div className="space-y-4">
            <div className="grid grid-cols-2 gap-3">
              <input placeholder="Role Name *" value={name} onChange={e => setName(e.target.value)}
                className="w-full px-3 py-2 rounded-lg bg-gray-900 border border-gray-600 text-white text-sm outline-none focus:border-blue-500" />
              <input placeholder="Description" value={description} onChange={e => setDescription(e.target.value)}
                className="w-full px-3 py-2 rounded-lg bg-gray-900 border border-gray-600 text-white text-sm outline-none focus:border-blue-500" />
            </div>

            <div>
              <div className="flex items-center justify-between mb-2">
                <h3 className="text-sm font-medium text-gray-300">Permission Matrix ({selectedPerms.size} selected)</h3>
              </div>
              <div className="space-y-3">
                {Object.entries(allPerms).map(([resource, perms]) => {
                  const allSelected = perms.every(p => selectedPerms.has(p.permission_id))
                  const someSelected = perms.some(p => selectedPerms.has(p.permission_id))
                  return (
                    <div key={resource} className="p-3 rounded-lg bg-gray-900/50 border border-gray-700/50">
                      <div className="flex items-center justify-between mb-2">
                        <h4 className="text-xs font-semibold text-gray-400 uppercase">{resource}</h4>
                        <button onClick={() => toggleAll(resource)}
                          className={`text-[10px] px-2 py-0.5 rounded ${allSelected ? 'bg-blue-600/30 text-blue-300' : someSelected ? 'bg-amber-600/20 text-amber-300' : 'bg-gray-700 text-gray-400'}`}>
                          {allSelected ? 'Deselect All' : 'Select All'}
                        </button>
                      </div>
                      <div className="flex flex-wrap gap-2">
                        {perms.map(p => (
                          <label key={p.permission_id} className={`flex items-center gap-1.5 cursor-pointer px-2 py-1 rounded border text-xs transition ${selectedPerms.has(p.permission_id) ? 'bg-blue-600/20 border-blue-500/30 text-blue-300' : 'bg-transparent border-gray-700 text-gray-400 hover:text-white'}`}>
                            <input type="checkbox" checked={selectedPerms.has(p.permission_id)} onChange={() => togglePerm(p.permission_id)} className="rounded w-3 h-3" />
                            <span>{p.action}</span>
                          </label>
                        ))}
                      </div>
                    </div>
                  )
                })}
              </div>
            </div>

            <div className="flex justify-end gap-3 pt-2 border-t border-gray-700/50">
              <button onClick={onClose} className="px-4 py-2 rounded-lg text-sm text-gray-400 hover:text-white">Cancel</button>
              <button onClick={save} disabled={saving || !name}
                className="px-4 py-2 rounded-lg bg-blue-600 hover:bg-blue-700 disabled:bg-blue-800 text-white text-sm font-medium flex items-center gap-2">
                {saving ? <Loader2 className="w-4 h-4 animate-spin" /> : <Save className="w-4 h-4" />} {roleId ? 'Update Role' : 'Create Role'}
              </button>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
