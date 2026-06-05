import { useState, useEffect, useCallback } from 'react'
import { Link } from 'react-router-dom'
import { getUserGroups, createUserGroup, deleteUserGroup, getUserGroup, addGroupMembers, removeGroupMember, getUsers, UserGroupItem } from '../../lib/api'
import { Users, Plus, Trash2, Eye, X, Loader2, UserPlus, Shield, Search, ExternalLink } from 'lucide-react'

export default function UserGroupManagement() {
  const [groups, setGroups] = useState<UserGroupItem[]>([])
  const [loading, setLoading] = useState(true)
  const [showCreate, setShowCreate] = useState(false)
  const [detail, setDetail] = useState<any>(null)
  const [search, setSearch] = useState('')

  const fetchGroups = useCallback(async () => {
    setLoading(true)
    try { setGroups(await getUserGroups()) } catch {}
    setLoading(false)
  }, [])

  useEffect(() => { fetchGroups() }, [fetchGroups])

  const handleDelete = async (id: string) => {
    if (!confirm('Delete this group?')) return
    await deleteUserGroup(id)
    fetchGroups()
  }

  const totalMembers = groups.reduce((a, g) => a + g.member_count, 0)
  const filtered = groups.filter(g => !search || g.name.toLowerCase().includes(search.toLowerCase()) || (g.description || '').toLowerCase().includes(search.toLowerCase()))

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="p-2.5 rounded-xl bg-purple-600/20"><Users className="w-5 h-5 text-purple-400" /></div>
          <div>
            <h1 className="text-2xl font-bold text-white">User Groups</h1>
            <p className="text-sm text-gray-400">{groups.length} groups · {totalMembers} total members</p>
          </div>
        </div>
        <div className="flex items-center gap-2">
          <Link to="/admin/users" className="flex items-center gap-2 px-3 py-2 rounded-lg bg-blue-600/20 text-blue-400 hover:bg-blue-600/30 text-sm font-medium transition">
            <Users className="w-4 h-4" /> Users
          </Link>
          <Link to="/admin/roles" className="flex items-center gap-2 px-3 py-2 rounded-lg bg-red-600/20 text-red-400 hover:bg-red-600/30 text-sm font-medium transition">
            <Shield className="w-4 h-4" /> Roles
          </Link>
          <button onClick={() => setShowCreate(true)} className="flex items-center gap-2 px-4 py-2 rounded-lg bg-purple-600 hover:bg-purple-700 text-white text-sm font-medium transition">
            <Plus className="w-4 h-4" /> Create Group
          </button>
        </div>
      </div>

      {/* KPI Strip */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        {[
          { label: 'Total Groups', value: groups.length, color: 'text-white', bg: 'bg-white/5 border-white/10' },
          { label: 'Total Members', value: totalMembers, color: 'text-purple-400', bg: 'bg-purple-500/10 border-purple-500/20' },
          { label: 'Avg Members', value: groups.length > 0 ? Math.round(totalMembers / groups.length) : 0, color: 'text-blue-400', bg: 'bg-blue-500/10 border-blue-500/20' },
          { label: 'Empty Groups', value: groups.filter(g => g.member_count === 0).length, color: 'text-amber-400', bg: 'bg-amber-500/10 border-amber-500/20' },
        ].map((k, i) => (
          <div key={i} className={`rounded-xl border p-3 ${k.bg}`}>
            <p className="text-xs text-gray-500">{k.label}</p>
            <p className={`text-xl font-bold ${k.color}`}>{k.value}</p>
          </div>
        ))}
      </div>

      {/* Search */}
      <div className="relative max-w-md">
        <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-gray-500" />
        <input value={search} onChange={e => setSearch(e.target.value)} placeholder="Search groups..."
          className="w-full pl-10 pr-4 py-2 rounded-lg bg-gray-800 border border-gray-700 text-white text-sm placeholder-gray-500 focus:border-purple-500 outline-none" />
      </div>

      {loading ? (
        <div className="text-center p-8 text-gray-500"><Loader2 className="w-5 h-5 animate-spin inline mr-2" />Loading...</div>
      ) : filtered.length === 0 ? (
        <div className="text-center p-8 text-gray-500">{search ? 'No groups match your search' : 'No groups yet. Create one to get started.'}</div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {filtered.map(g => (
            <div key={g.id} className="bg-gray-800/50 border border-gray-700/50 rounded-xl p-4 hover:border-purple-500/30 transition group">
              <div className="flex items-center justify-between mb-3">
                <h3 className="font-semibold text-white">{g.name}</h3>
                <div className="flex gap-1 opacity-50 group-hover:opacity-100 transition">
                  <button onClick={async () => setDetail(await getUserGroup(g.id))} className="p-1.5 rounded hover:bg-gray-700 text-gray-400" title="View Members"><Eye className="w-3.5 h-3.5" /></button>
                  <button onClick={() => handleDelete(g.id)} className="p-1.5 rounded hover:bg-gray-700 text-red-400" title="Delete"><Trash2 className="w-3.5 h-3.5" /></button>
                </div>
              </div>
              {g.description && <p className="text-xs text-gray-500 mb-3 line-clamp-2">{g.description}</p>}
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2 text-xs text-gray-400">
                  <Users className="w-3.5 h-3.5" />
                  <span>{g.member_count} member{g.member_count !== 1 ? 's' : ''}</span>
                </div>
                <button onClick={async () => setDetail(await getUserGroup(g.id))}
                  className="text-xs text-purple-400 hover:text-purple-300 flex items-center gap-1">
                  <UserPlus className="w-3 h-3" /> Manage
                </button>
              </div>
            </div>
          ))}
        </div>
      )}

      {showCreate && <CreateGroupModal onClose={() => setShowCreate(false)} onCreated={() => { setShowCreate(false); fetchGroups() }} />}
      {detail && <GroupDetailModal group={detail} onClose={() => { setDetail(null); fetchGroups() }} onRefresh={async () => setDetail(await getUserGroup(detail.id))} />}
    </div>
  )
}

function CreateGroupModal({ onClose, onCreated }: { onClose: () => void; onCreated: () => void }) {
  const [name, setName] = useState('')
  const [description, setDescription] = useState('')
  const [loading, setLoading] = useState(false)

  const submit = async () => {
    setLoading(true)
    await createUserGroup({ name, description: description || undefined })
    setLoading(false)
    onCreated()
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-gray-800 border border-gray-700 rounded-2xl p-6 w-full max-w-md shadow-2xl">
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-lg font-bold text-white">Create Group</h2>
          <button onClick={onClose} className="text-gray-400 hover:text-white"><X className="w-5 h-5" /></button>
        </div>
        <div className="space-y-3">
          <input placeholder="Group Name *" value={name} onChange={e => setName(e.target.value)}
            className="w-full px-3 py-2 rounded-lg bg-gray-900 border border-gray-600 text-white text-sm placeholder-gray-500 outline-none focus:border-purple-500" />
          <textarea placeholder="Description (optional)" value={description} onChange={e => setDescription(e.target.value)} rows={3}
            className="w-full px-3 py-2 rounded-lg bg-gray-900 border border-gray-600 text-white text-sm placeholder-gray-500 outline-none focus:border-purple-500 resize-none" />
        </div>
        <div className="flex justify-end gap-3 mt-5">
          <button onClick={onClose} className="px-4 py-2 rounded-lg text-sm text-gray-400 hover:text-white">Cancel</button>
          <button onClick={submit} disabled={loading || !name}
            className="px-4 py-2 rounded-lg bg-purple-600 hover:bg-purple-700 disabled:bg-purple-800 text-white text-sm font-medium flex items-center gap-2">
            {loading && <Loader2 className="w-4 h-4 animate-spin" />} Create Group
          </button>
        </div>
      </div>
    </div>
  )
}

function GroupDetailModal({ group, onClose, onRefresh }: { group: any; onClose: () => void; onRefresh: () => void }) {
  const [showAdd, setShowAdd] = useState(false)
  const [availableUsers, setAvailableUsers] = useState<any[]>([])
  const [userSearch, setUserSearch] = useState('')

  const loadAvailable = async () => {
    const res = await getUsers({ limit: '200' })
    const memberIds = new Set(group.members.map((m: any) => m.id))
    setAvailableUsers(res.users.filter((u: any) => !memberIds.has(u.id)))
    setShowAdd(true)
  }

  const handleAdd = async (userId: string) => {
    await addGroupMembers(group.id, [userId])
    onRefresh()
    setShowAdd(false)
  }

  const handleRemove = async (userId: string) => {
    await removeGroupMember(group.id, userId)
    onRefresh()
  }

  const filteredAvailable = availableUsers.filter(u =>
    !userSearch || (u.display_name || u.username || '').toLowerCase().includes(userSearch.toLowerCase()) || u.email.toLowerCase().includes(userSearch.toLowerCase())
  )

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-gray-800 border border-gray-700 rounded-2xl p-6 w-full max-w-lg shadow-2xl max-h-[80vh] overflow-y-auto">
        <div className="flex items-center justify-between mb-4">
          <div>
            <h2 className="text-lg font-bold text-white">{group.name}</h2>
            {group.description && <p className="text-sm text-gray-400">{group.description}</p>}
          </div>
          <button onClick={onClose} className="text-gray-400 hover:text-white"><X className="w-5 h-5" /></button>
        </div>

        <div className="flex items-center justify-between mb-3">
          <h3 className="text-sm font-medium text-gray-300">Members ({group.members?.length || 0})</h3>
          <button onClick={loadAvailable} className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs bg-purple-600 hover:bg-purple-700 text-white font-medium">
            <UserPlus className="w-3 h-3" /> Add Members
          </button>
        </div>

        <div className="space-y-2">
          {group.members?.map((m: any) => (
            <div key={m.id} className="flex items-center justify-between px-3 py-2.5 rounded-lg bg-gray-900/50 hover:bg-gray-900 transition">
              <div className="flex items-center gap-2">
                <div className="w-7 h-7 rounded-full bg-purple-600/20 flex items-center justify-center text-purple-400 text-xs font-bold">
                  {(m.display_name || m.username || '?').charAt(0).toUpperCase()}
                </div>
                <div>
                  <p className="text-sm text-white">{m.display_name || m.username}</p>
                  <p className="text-xs text-gray-500">{m.email}</p>
                </div>
              </div>
              <button onClick={() => handleRemove(m.id)} className="p-1 rounded hover:bg-gray-700 text-red-400 hover:text-red-300" title="Remove"><X className="w-4 h-4" /></button>
            </div>
          ))}
          {(!group.members || group.members.length === 0) && <p className="text-sm text-gray-500 text-center py-4">No members yet. Add users to this group.</p>}
        </div>

        {showAdd && (
          <div className="mt-4 p-3 rounded-lg bg-gray-900/50 border border-gray-700">
            <div className="flex items-center justify-between mb-2">
              <p className="text-xs font-medium text-gray-400">Select user to add:</p>
              <button onClick={() => setShowAdd(false)} className="text-gray-500 hover:text-white"><X className="w-3.5 h-3.5" /></button>
            </div>
            <div className="relative mb-2">
              <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-gray-500" />
              <input value={userSearch} onChange={e => setUserSearch(e.target.value)} placeholder="Search users..."
                className="w-full pl-8 pr-3 py-1.5 rounded bg-gray-800 border border-gray-600 text-white text-xs outline-none focus:border-purple-500" />
            </div>
            <div className="max-h-48 overflow-y-auto space-y-1">
              {filteredAvailable.map(u => (
                <button key={u.id} onClick={() => handleAdd(u.id)}
                  className="w-full flex items-center gap-2 px-2 py-1.5 rounded text-sm text-left hover:bg-gray-700 text-gray-300">
                  <div className="w-6 h-6 rounded-full bg-blue-600/20 flex items-center justify-center text-blue-400 text-[10px] font-bold">
                    {(u.display_name || u.username || '?').charAt(0).toUpperCase()}
                  </div>
                  <span className="flex-1">{u.display_name || u.username}</span>
                  <span className="text-xs text-gray-500">{u.email}</span>
                </button>
              ))}
              {filteredAvailable.length === 0 && <p className="text-xs text-gray-500 text-center py-2">No users available to add</p>}
            </div>
          </div>
        )}

        <div className="flex items-center justify-between mt-4 pt-3 border-t border-gray-700/50">
          <Link to="/admin/users" className="text-xs text-blue-400 hover:underline flex items-center gap-1" onClick={onClose}><ExternalLink className="w-3 h-3" /> Manage Users</Link>
          <button onClick={onClose} className="px-3 py-1.5 rounded-lg text-sm text-gray-400 hover:text-white">Close</button>
        </div>
      </div>
    </div>
  )
}
