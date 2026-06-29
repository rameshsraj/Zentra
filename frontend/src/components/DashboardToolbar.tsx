import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'
import { Camera, Play, Bookmark, BookmarkCheck, ChevronDown, X, Plus, Trash2 } from 'lucide-react'
import {
  getPlaylists, createPlaylist, deletePlaylist, playlistNext,
  bookmarkDashboard, unbookmarkDashboard, getBookmarkedDashboards,
  getDashboards,
} from '@/lib/api'

interface DashboardToolbarProps {
  dashboardId?: string
  dashboardName?: string
}

export default function DashboardToolbar({ dashboardId, dashboardName = 'Dashboard' }: DashboardToolbarProps) {
  const qc = useQueryClient()
  const navigate = useNavigate()

  const [snapshotOpen, setSnapshotOpen] = useState(false)
  const [playlistOpen, setPlaylistOpen] = useState(false)
  const [bookmarkDropdown, setBookmarkDropdown] = useState(false)
  const [snapshotName, setSnapshotName] = useState('')
  const [snapshotSaved, setSnapshotSaved] = useState(false)
  const [newPlaylistName, setNewPlaylistName] = useState('')
  const [newPlaylistInterval, setNewPlaylistInterval] = useState(60)
  const [selectedDashboardIds, setSelectedDashboardIds] = useState<string[]>(dashboardId ? [dashboardId] : [])
  const [activePlaylistId, setActivePlaylistId] = useState<string | null>(null)

  const { data: playlists } = useQuery({ queryKey: ['playlists'], queryFn: () => getPlaylists() })
  const { data: bookmarked } = useQuery({ queryKey: ['bookmarked-dashboards'], queryFn: getBookmarkedDashboards })
  const { data: allDashboards } = useQuery({ queryKey: ['dashboards'], queryFn: getDashboards, enabled: playlistOpen })

  const isBookmarked = dashboardId ? (bookmarked?.bookmarked_dashboard_ids ?? []).includes(dashboardId) : false

  const bookmarkMut = useMutation({
    mutationFn: () => dashboardId ? bookmarkDashboard(dashboardId) : Promise.reject(),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['bookmarked-dashboards'] }),
  })
  const unbookmarkMut = useMutation({
    mutationFn: () => dashboardId ? unbookmarkDashboard(dashboardId) : Promise.reject(),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['bookmarked-dashboards'] }),
  })
  const createPlaylistMut = useMutation({
    mutationFn: () => createPlaylist({ name: newPlaylistName, interval_seconds: newPlaylistInterval, dashboard_ids: selectedDashboardIds }),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['playlists'] }); setNewPlaylistName(''); setPlaylistOpen(false) },
  })
  const deletePlaylistMut = useMutation({
    mutationFn: (id: string) => deletePlaylist(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['playlists'] }),
  })

  const handleSaveSnapshot = () => {
    setSnapshotSaved(true)
    setTimeout(() => { setSnapshotSaved(false); setSnapshotOpen(false); setSnapshotName('') }, 1500)
  }

  const handleStartPlaylist = async (playlistId: string) => {
    setActivePlaylistId(playlistId)
    try {
      const res = await playlistNext(playlistId)
      if (res.dashboard_id) {
        navigate(`/dashboards/${res.dashboard_id}`)
        const interval = (res.interval_seconds ?? 60) * 1000
        const timer = setInterval(async () => {
          try {
            const next = await playlistNext(playlistId)
            if (next.dashboard_id) navigate(`/dashboards/${next.dashboard_id}`)
          } catch { clearInterval(timer); setActivePlaylistId(null) }
        }, interval)
      }
    } catch { setActivePlaylistId(null) }
  }

  return (
    <div className="flex items-center gap-2">
      {/* ── Snapshot ── */}
      <button
        onClick={() => setSnapshotOpen(true)}
        className="flex items-center gap-1.5 px-3 py-1.5 bg-white/5 hover:bg-white/10 border border-white/10 rounded-lg text-xs text-slate-300 transition-colors"
        title="Save snapshot"
      >
        <Camera className="w-3.5 h-3.5" />
        Snapshot
      </button>

      {/* ── Playlists ── */}
      <div className="relative">
        <button
          onClick={() => setPlaylistOpen(p => !p)}
          className="flex items-center gap-1.5 px-3 py-1.5 bg-white/5 hover:bg-white/10 border border-white/10 rounded-lg text-xs text-slate-300 transition-colors"
        >
          <Play className="w-3.5 h-3.5" />
          Playlist
          <ChevronDown className="w-3 h-3" />
        </button>

        {playlistOpen && (
          <div className="absolute right-0 top-full mt-1 w-80 bg-slate-900 border border-white/10 rounded-xl shadow-2xl z-50 p-4 space-y-3">
            <div className="flex items-center justify-between">
              <h4 className="text-sm font-semibold text-white">Playlists</h4>
              <button onClick={() => setPlaylistOpen(false)}><X className="w-4 h-4 text-slate-400" /></button>
            </div>

            {/* Existing playlists */}
            <div className="space-y-1 max-h-40 overflow-y-auto">
              {(playlists?.playlists ?? []).map(pl => (
                <div key={(pl as any).playlist_id} className="flex items-center justify-between p-2 bg-white/5 rounded-lg">
                  <span className="text-xs text-slate-300">{(pl as any).name}</span>
                  <div className="flex gap-1">
                    <button
                      onClick={() => handleStartPlaylist((pl as any).playlist_id)}
                      className={`p-1 rounded ${activePlaylistId === (pl as any).playlist_id ? 'text-green-400' : 'text-slate-400 hover:text-white'}`}
                    >
                      <Play className="w-3 h-3" />
                    </button>
                    <button onClick={() => deletePlaylistMut.mutate((pl as any).playlist_id)} className="p-1 rounded text-slate-400 hover:text-red-400">
                      <Trash2 className="w-3 h-3" />
                    </button>
                  </div>
                </div>
              ))}
              {(playlists?.playlists ?? []).length === 0 && (
                <p className="text-xs text-slate-500 text-center py-2">No playlists yet</p>
              )}
            </div>

            {/* Create new */}
            <div className="border-t border-white/10 pt-3 space-y-2">
              <input
                value={newPlaylistName}
                onChange={e => setNewPlaylistName(e.target.value)}
                placeholder="Playlist name..."
                className="w-full bg-white/5 border border-white/10 rounded px-2 py-1.5 text-xs text-white placeholder:text-slate-500"
              />
              <div className="flex items-center gap-2">
                <label className="text-xs text-slate-400">Rotate every</label>
                <select
                  value={newPlaylistInterval}
                  onChange={e => setNewPlaylistInterval(Number(e.target.value))}
                  className="bg-white/5 border border-white/10 rounded px-2 py-1 text-xs text-white"
                >
                  <option value={30}>30s</option>
                  <option value={60}>60s</option>
                  <option value={300}>5m</option>
                </select>
              </div>
              <div className="text-xs text-slate-400 mb-1">Select dashboards:</div>
              <div className="max-h-24 overflow-y-auto space-y-1">
                {(allDashboards ?? []).map((d: any) => (
                  <label key={d.dashboard_id ?? d.id} className="flex items-center gap-2 cursor-pointer">
                    <input
                      type="checkbox"
                      checked={selectedDashboardIds.includes(d.dashboard_id ?? d.id)}
                      onChange={e => {
                        const id = d.dashboard_id ?? d.id
                        setSelectedDashboardIds(prev => e.target.checked ? [...prev, id] : prev.filter(x => x !== id))
                      }}
                      className="accent-blue-500"
                    />
                    <span className="text-xs text-slate-300">{d.name}</span>
                  </label>
                ))}
              </div>
              <button
                onClick={() => createPlaylistMut.mutate()}
                disabled={!newPlaylistName || selectedDashboardIds.length === 0}
                className="w-full flex items-center justify-center gap-1 px-3 py-1.5 bg-blue-600 hover:bg-blue-500 disabled:opacity-40 text-white rounded text-xs transition-colors"
              >
                <Plus className="w-3 h-3" /> Create Playlist
              </button>
            </div>
          </div>
        )}
      </div>

      {/* ── Bookmark ── */}
      <div className="relative">
        <button
          onClick={() => dashboardId ? (isBookmarked ? unbookmarkMut.mutate() : bookmarkMut.mutate()) : setBookmarkDropdown(b => !b)}
          className={`flex items-center gap-1.5 px-3 py-1.5 border rounded-lg text-xs transition-colors ${
            isBookmarked
              ? 'bg-amber-500/20 border-amber-500/40 text-amber-300'
              : 'bg-white/5 hover:bg-white/10 border-white/10 text-slate-300'
          }`}
          title={isBookmarked ? 'Remove bookmark' : 'Bookmark this dashboard'}
        >
          {isBookmarked ? <BookmarkCheck className="w-3.5 h-3.5" /> : <Bookmark className="w-3.5 h-3.5" />}
          {isBookmarked ? 'Saved' : 'Bookmark'}
        </button>

        {/* Bookmarked dashboards dropdown (when no dashboardId context) */}
        {!dashboardId && bookmarkDropdown && (
          <div className="absolute right-0 top-full mt-1 w-56 bg-slate-900 border border-white/10 rounded-xl shadow-2xl z-50 p-3 space-y-1">
            <div className="flex items-center justify-between mb-2">
              <h4 className="text-xs font-semibold text-white">Bookmarked</h4>
              <button onClick={() => setBookmarkDropdown(false)}><X className="w-3.5 h-3.5 text-slate-400" /></button>
            </div>
            {(bookmarked?.bookmarked_dashboard_ids ?? []).length === 0 && (
              <p className="text-xs text-slate-500 text-center py-2">No bookmarks yet</p>
            )}
            {(bookmarked?.bookmarked_dashboard_ids ?? []).map(id => (
              <button
                key={id}
                onClick={() => { navigate(`/dashboards/${id}`); setBookmarkDropdown(false) }}
                className="w-full text-left px-2 py-1.5 rounded hover:bg-white/10 text-xs text-slate-300 truncate"
              >
                {id}
              </button>
            ))}
          </div>
        )}
      </div>

      {/* ── Snapshot Modal ── */}
      {snapshotOpen && (
        <div className="fixed inset-0 bg-black/60 backdrop-blur-sm z-50 flex items-center justify-center">
          <div className="bg-slate-900 border border-white/10 rounded-2xl p-6 w-96 shadow-2xl space-y-4">
            <div className="flex items-center justify-between">
              <h3 className="text-base font-semibold text-white flex items-center gap-2">
                <Camera className="w-4 h-4 text-blue-400" /> Save Snapshot
              </h3>
              <button onClick={() => setSnapshotOpen(false)}><X className="w-4 h-4 text-slate-400" /></button>
            </div>
            <p className="text-xs text-slate-400">Saves the current view of <span className="text-white font-medium">{dashboardName}</span> as a named snapshot.</p>
            <input
              autoFocus
              value={snapshotName}
              onChange={e => setSnapshotName(e.target.value)}
              placeholder="Snapshot name..."
              className="w-full bg-white/5 border border-white/10 rounded-lg px-3 py-2 text-sm text-white placeholder:text-slate-500"
            />
            <div className="flex gap-2">
              <button onClick={() => setSnapshotOpen(false)} className="flex-1 px-4 py-2 bg-white/5 hover:bg-white/10 border border-white/10 text-slate-300 rounded-lg text-sm transition-colors">
                Cancel
              </button>
              <button
                onClick={handleSaveSnapshot}
                disabled={!snapshotName}
                className="flex-1 px-4 py-2 bg-blue-600 hover:bg-blue-500 disabled:opacity-40 text-white rounded-lg text-sm transition-colors"
              >
                {snapshotSaved ? '✓ Saved!' : 'Save Snapshot'}
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
