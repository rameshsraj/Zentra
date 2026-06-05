import { useState, useCallback } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import PageHeader from '@/components/PageHeader'
import { getLogSources, createLogSource, deleteLogSource } from '@/lib/api'
import {
  FileInput, Server, Database, Cloud, Terminal, Trash2, CheckCircle2,
  XCircle, Clock, Settings, Pause,
} from 'lucide-react'
import { cn } from '@/lib/utils'

interface LogSourceItem {
  id: string
  name: string
  source_type: string
  host: string
  port: number
  path: string
  status: 'active' | 'error' | 'paused'
  last_ingested: string | null
  log_count: number
  created_at: string
}


const sourceTypeIcon = (type: string) => {
  switch (type) {
    case 'file': return Server
    case 'syslog': return Terminal
    case 'docker': return Database
    case 'cloudwatch': return Cloud
    default: return FileInput
  }
}

const statusConfig = (status: string) => {
  switch (status) {
    case 'active': return { color: 'text-emerald-400', bg: 'bg-emerald-500/20', icon: CheckCircle2 }
    case 'error': return { color: 'text-red-400', bg: 'bg-red-500/20', icon: XCircle }
    case 'paused': return { color: 'text-amber-400', bg: 'bg-amber-500/20', icon: Pause }
    default: return { color: 'text-slate-400', bg: 'bg-slate-500/20', icon: Clock }
  }
}

export default function LogSourcesPage() {
  const qc = useQueryClient()
  const { data: sources = [] } = useQuery<LogSourceItem[]>({ queryKey: ['log-sources'], queryFn: getLogSources })
  const createMut = useMutation({ mutationFn: createLogSource, onSuccess: () => qc.invalidateQueries({ queryKey: ['log-sources'] }) })
  const deleteMut = useMutation({ mutationFn: deleteLogSource, onSuccess: () => qc.invalidateQueries({ queryKey: ['log-sources'] }) })

  const [search, setSearch] = useState('')
  const [showAddModal, setShowAddModal] = useState(false)
  const [newSource, setNewSource] = useState({ name: '', source_type: 'file', host: '', port: 22, path: '' })

  const handleRefresh = useCallback(() => {
    qc.invalidateQueries({ queryKey: ['log-sources'] })
  }, [qc])

  const filteredSources = sources.filter(s =>
    !search || `${s.name} ${s.host} ${s.path} ${s.source_type}`.toLowerCase().includes(search.toLowerCase())
  )

  const handleAdd = () => {
    if (!newSource.name || !newSource.host || !newSource.path) return
    createMut.mutate(newSource as any)
    setShowAddModal(false)
    setNewSource({ name: '', source_type: 'file', host: '', port: 22, path: '' })
  }

  return (
    <div className="space-y-6">
      <PageHeader
        title="Log Sources"
        subtitle="Configure log ingestion from multiple sources · File, Syslog, Docker, CloudWatch"
        onRefresh={handleRefresh}
        onSearch={setSearch}
        searchPlaceholder="Search sources..."
        showAddTarget
        onAddTarget={() => setShowAddModal(true)}
        addTargetLabel="Add Log Source"
      />

      {/* KPI Strip */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
        {[
          { label: 'Total Sources', value: sources.length, color: 'text-blue-400' },
          { label: 'Active', value: sources.filter(s => s.status === 'active').length, color: 'text-emerald-400' },
          { label: 'Errors', value: sources.filter(s => s.status === 'error').length, color: 'text-red-400' },
          { label: 'Total Logs', value: `${(sources.reduce((s, src) => s + src.log_count, 0) / 1000).toFixed(0)}K`, color: 'text-purple-400' },
        ].map((kpi, i) => (
          <div key={i} className="bg-white/5 border border-white/10 rounded-xl p-3">
            <div className="text-[10px] text-slate-500 uppercase tracking-wider">{kpi.label}</div>
            <div className={`text-2xl font-bold font-mono ${kpi.color}`}>{kpi.value}</div>
          </div>
        ))}
      </div>

      {/* Sources Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
        {filteredSources.map(src => {
          const Icon = sourceTypeIcon(src.source_type)
          const status = statusConfig(src.status)
          const StatusIcon = status.icon
          return (
            <div key={src.id} className="bg-white/5 border border-white/10 rounded-xl p-4 hover:bg-white/[0.08] transition-colors">
              <div className="flex items-start justify-between mb-3">
                <div className="flex items-center gap-2">
                  <div className="w-8 h-8 rounded-lg bg-brand-600/20 flex items-center justify-center">
                    <Icon className="w-4 h-4 text-brand-400" />
                  </div>
                  <div>
                    <div className="text-sm font-bold text-white">{src.name}</div>
                    <div className="text-[10px] text-slate-500 uppercase">{src.source_type}</div>
                  </div>
                </div>
                <span className={cn('flex items-center gap-1 px-2 py-0.5 rounded text-[10px] font-semibold', status.bg, status.color)}>
                  <StatusIcon className="w-3 h-3" />{src.status}
                </span>
              </div>
              <div className="space-y-1.5 text-[10px]">
                <div className="flex justify-between"><span className="text-slate-500">Host</span><span className="text-slate-300 font-mono">{src.host}:{src.port}</span></div>
                <div className="flex justify-between"><span className="text-slate-500">Path</span><span className="text-slate-300 font-mono truncate ml-4">{src.path}</span></div>
                <div className="flex justify-between"><span className="text-slate-500">Logs</span><span className="text-slate-300 font-mono">{src.log_count.toLocaleString()}</span></div>
                <div className="flex justify-between"><span className="text-slate-500">Last Ingested</span><span className="text-slate-300">{src.last_ingested ? new Date(src.last_ingested).toLocaleString() : 'Never'}</span></div>
              </div>
              <div className="flex gap-1.5 mt-3 pt-3 border-t border-white/10">
                <button className="flex-1 flex items-center justify-center gap-1 px-2 py-1.5 bg-white/5 border border-white/10 rounded-lg text-[10px] text-slate-400 hover:text-white transition-colors">
                  <Settings className="w-3 h-3" />Configure
                </button>
                <button onClick={() => deleteMut.mutate(src.id)} className="flex items-center justify-center gap-1 px-2 py-1.5 bg-white/5 border border-white/10 rounded-lg text-[10px] text-slate-400 hover:text-red-400 transition-colors">
                  <Trash2 className="w-3 h-3" />
                </button>
              </div>
            </div>
          )
        })}
      </div>

      {/* Add Source Modal */}
      {showAddModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
          <div className="bg-slate-900 border border-white/10 rounded-2xl shadow-2xl w-full max-w-lg p-6">
            <h3 className="text-lg font-bold text-white mb-4">Add Log Source</h3>
            <div className="space-y-3">
              <div>
                <label className="text-[10px] text-slate-500 uppercase tracking-wider block mb-1">Name</label>
                <input type="text" value={newSource.name} onChange={e => setNewSource(p => ({ ...p, name: e.target.value }))}
                  placeholder="e.g., Production Nginx Logs" className="w-full bg-black/20 border border-white/10 rounded-xl px-4 py-2.5 text-sm text-white placeholder-slate-600 focus:outline-none focus:ring-2 focus:ring-brand-500/50" />
              </div>
              <div>
                <label className="text-[10px] text-slate-500 uppercase tracking-wider block mb-1">Source Type</label>
                <div className="flex gap-2">
                  {['file', 'syslog', 'docker', 'cloudwatch'].map(t => (
                    <button key={t} onClick={() => setNewSource(p => ({ ...p, source_type: t }))}
                      className={cn('px-3 py-2 rounded-xl text-xs border transition-colors',
                        newSource.source_type === t ? 'bg-brand-600/20 text-brand-400 border-brand-500/30' : 'bg-white/5 text-slate-500 border-white/10 hover:text-white'
                      )}>{t}</button>
                  ))}
                </div>
              </div>
              <div className="grid grid-cols-3 gap-3">
                <div className="col-span-2">
                  <label className="text-[10px] text-slate-500 uppercase tracking-wider block mb-1">Host</label>
                  <input type="text" value={newSource.host} onChange={e => setNewSource(p => ({ ...p, host: e.target.value }))}
                    placeholder="10.88.11.162" className="w-full bg-black/20 border border-white/10 rounded-xl px-4 py-2.5 text-sm text-white placeholder-slate-600 focus:outline-none focus:ring-2 focus:ring-brand-500/50" />
                </div>
                <div>
                  <label className="text-[10px] text-slate-500 uppercase tracking-wider block mb-1">Port</label>
                  <input type="number" value={newSource.port} onChange={e => setNewSource(p => ({ ...p, port: parseInt(e.target.value) || 22 }))}
                    className="w-full bg-black/20 border border-white/10 rounded-xl px-4 py-2.5 text-sm text-white focus:outline-none focus:ring-2 focus:ring-brand-500/50" />
                </div>
              </div>
              <div>
                <label className="text-[10px] text-slate-500 uppercase tracking-wider block mb-1">Path</label>
                <input type="text" value={newSource.path} onChange={e => setNewSource(p => ({ ...p, path: e.target.value }))}
                  placeholder="/var/log/nginx/error.log" className="w-full bg-black/20 border border-white/10 rounded-xl px-4 py-2.5 text-sm text-white placeholder-slate-600 focus:outline-none focus:ring-2 focus:ring-brand-500/50" />
              </div>
            </div>
            <div className="flex gap-2 mt-5">
              <button onClick={() => setShowAddModal(false)} className="flex-1 px-4 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-slate-400 hover:text-white transition-colors">Cancel</button>
              <button onClick={handleAdd} className="flex-1 px-4 py-2.5 bg-brand-600 text-white rounded-xl text-sm font-medium hover:bg-brand-700 transition-colors shadow-lg shadow-brand-600/20">Add Source</button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
