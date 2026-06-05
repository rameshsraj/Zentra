import { useState, useEffect } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { Plus, Trash2, RefreshCw, X, Loader2, Server } from 'lucide-react'
import LiveModeButton from '@/components/LiveModeButton'
import { getMiddlewareInstances, createMiddlewareInstance, deleteMiddlewareInstance, getMiddlewareStatus, getMiddlewareHistory } from '@/lib/api'
import { AreaChart, Area, XAxis, YAxis, Tooltip, ResponsiveContainer } from 'recharts'

interface Props {
  serviceType: string
  title: string
  color?: string
  extraEndpoints?: { label: string; fetcher: (id: string) => Promise<any> }[]
}

const defaultPorts: Record<string, number> = {
  nginx: 80, apache: 80, haproxy: 1936, tomcat: 8080, phpfpm: 9000,
  rabbitmq: 15672, kafka: 9092, elasticsearch: 9200, redis: 6379,
  memcached: 11211, docker: 2375, kubernetes: 6443,
}

export default function MiddlewareServicePage({ serviceType, title, extraEndpoints }: Props) {
  const qc = useQueryClient()
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [tab, setTab] = useState<'status' | 'history' | 'extra'>('status')
  const [autoRefresh, setAutoRefresh] = useState(false)
  const [showAddModal, setShowAddModal] = useState(false)
  const [addForm, setAddForm] = useState({ name: '', host: '', port: defaultPorts[serviceType] || 0, description: '', environment: 'production' })
  const [addError, setAddError] = useState('')

  const { data: instances = [] } = useQuery({ queryKey: ['mw-instances', serviceType], queryFn: () => getMiddlewareInstances(serviceType) })
  const { data: status } = useQuery({ queryKey: ['mw-status', serviceType, selectedId], queryFn: () => selectedId ? getMiddlewareStatus(serviceType, selectedId) : null, enabled: !!selectedId, refetchInterval: autoRefresh ? 10000 : false })
  const { data: history = [] } = useQuery({ queryKey: ['mw-history', serviceType, selectedId], queryFn: () => selectedId ? getMiddlewareHistory(serviceType, selectedId) : [], enabled: !!selectedId && tab === 'history', refetchInterval: autoRefresh ? 10000 : false })
  const [extraData, setExtraData] = useState<any>(null)
  const createMut = useMutation({
    mutationFn: (data: any) => createMiddlewareInstance(serviceType, data),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['mw-instances', serviceType] })
      setShowAddModal(false)
      setAddForm({ name: '', host: '', port: defaultPorts[serviceType] || 0, description: '', environment: 'production' })
      setAddError('')
    },
    onError: (e: Error) => setAddError(e.message),
  })
  const deleteMut = useMutation({ mutationFn: (id: string) => deleteMiddlewareInstance(serviceType, id), onSuccess: () => { qc.invalidateQueries({ queryKey: ['mw-instances', serviceType] }); if (selectedId === deleteMut.variables) setSelectedId(null) } })

  useEffect(() => { if (instances.length > 0 && !selectedId) setSelectedId(instances[0].id) }, [instances, selectedId])

  const handleAddSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (!addForm.name || !addForm.host) {
      setAddError('Name and Host are required')
      return
    }
    setAddError('')
    createMut.mutate({ name: addForm.name, host: addForm.host, port: addForm.port, description: addForm.description, environment: addForm.environment })
  }

  const updateField = (field: string, value: string | number) => setAddForm(f => ({ ...f, [field]: value }))

  useEffect(() => {
    if (tab === 'extra' && selectedId && extraEndpoints?.[0]) {
      extraEndpoints[0].fetcher(selectedId).then(setExtraData).catch(() => setExtraData(null))
    }
  }, [tab, selectedId, extraEndpoints])

  const historyNumKeys = history.length > 0 ? Object.keys(history[0]).filter(k => k !== 'timestamp' && typeof history[0][k] === 'number').slice(0, 4) : []
  const COLORS = ['#3b82f6', '#10b981', '#f59e0b', '#ef4444']

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-white">{title} Monitoring</h1>
        <div className="flex items-center gap-2">
          <button onClick={() => setAutoRefresh(!autoRefresh)} className={`flex items-center gap-1.5 px-3 py-2 rounded-xl text-xs font-medium border transition-colors ${autoRefresh ? 'bg-emerald-500/20 text-emerald-400 border-emerald-500/30' : 'bg-white/5 text-slate-400 border-white/10 hover:text-white'}`}><RefreshCw className={`w-3.5 h-3.5 ${autoRefresh ? 'animate-spin' : ''}`} />{autoRefresh ? 'Auto' : 'Manual'}</button>
          <LiveModeButton><button onClick={() => setShowAddModal(true)} className="flex items-center gap-1.5 px-4 py-2 bg-brand-600 hover:bg-brand-700 rounded-xl text-xs font-medium text-white transition-colors"><Plus className="w-4 h-4" /> Add Instance</button></LiveModeButton>
        </div>
      </div>

      {/* Instance selector */}
      <div className="flex gap-2 flex-wrap">
        {instances.map((inst: any) => (
          <button key={inst.id} onClick={() => setSelectedId(inst.id)} className={`px-3 py-1.5 rounded text-sm flex items-center gap-2 ${selectedId === inst.id ? 'bg-blue-600 text-white' : 'bg-gray-700 text-gray-300 hover:bg-gray-600'}`}>
            <div className={`w-2 h-2 rounded-full ${inst.status === 'healthy' ? 'bg-green-500' : inst.status === 'warning' ? 'bg-yellow-500' : inst.status === 'critical' ? 'bg-red-500' : 'bg-gray-500'}`} />
            {inst.name}
            <span className="text-xs text-gray-400">{inst.host}:{inst.port}</span>
            <button onClick={e => { e.stopPropagation(); deleteMut.mutate(inst.id) }} className="ml-1 text-red-400 hover:text-red-300"><Trash2 className="w-3 h-3" /></button>
          </button>
        ))}
      </div>

      {selectedId && (
        <>
          {/* Tabs */}
          <div className="flex gap-2">
            {(['status', 'history'] as const).map(t => <button key={t} onClick={() => setTab(t)} className={`px-3 py-1.5 rounded text-sm capitalize ${tab === t ? 'bg-blue-600 text-white' : 'bg-gray-700 text-gray-300'}`}>{t}</button>)}
            {extraEndpoints?.map((ep, i) => <button key={i} onClick={() => setTab('extra')} className={`px-3 py-1.5 rounded text-sm ${tab === 'extra' ? 'bg-blue-600 text-white' : 'bg-gray-700 text-gray-300'}`}>{ep.label}</button>)}
          </div>

          {/* Status tab */}
          {tab === 'status' && status && (
            <div className="grid grid-cols-2 lg:grid-cols-4 gap-3">
              {Object.entries(status).filter(([, v]) => typeof v !== 'object' || v === null).map(([k, v]) => (
                <div key={k} className="bg-gray-800 rounded-lg p-4 border border-gray-700">
                  <p className="text-gray-400 text-xs capitalize">{k.replace(/_/g, ' ')}</p>
                  <p className="text-white font-bold text-lg mt-1">{typeof v === 'number' ? (v % 1 === 0 ? v.toLocaleString() : (v as number).toFixed(2)) : String(v)}</p>
                </div>
              ))}
            </div>
          )}

          {/* History tab */}
          {tab === 'history' && history.length > 0 && (
            <div className="space-y-6">
              {historyNumKeys.map((key, idx) => (
                <div key={key} className="bg-gray-800 rounded-lg p-4 border border-gray-700">
                  <h3 className="text-gray-300 text-sm font-medium mb-3 capitalize">{key.replace(/_/g, ' ')}</h3>
                  <div className="h-48">
                    <ResponsiveContainer>
                      <AreaChart data={history}>
                        <XAxis dataKey="timestamp" tickFormatter={(t: string) => new Date(t).toLocaleTimeString()} tick={{ fill: '#9ca3af', fontSize: 10 }} />
                        <YAxis tick={{ fill: '#9ca3af', fontSize: 10 }} />
                        <Tooltip contentStyle={{ backgroundColor: '#1f2937', border: '1px solid #374151', borderRadius: '0.5rem' }} labelFormatter={(l: string) => new Date(l).toLocaleString()} />
                        <Area type="monotone" dataKey={key} stroke={COLORS[idx % 4]} fill={`${COLORS[idx % 4]}20`} strokeWidth={2} />
                      </AreaChart>
                    </ResponsiveContainer>
                  </div>
                </div>
              ))}
            </div>
          )}

          {/* Extra tab */}
          {tab === 'extra' && extraData && (
            <div className="bg-gray-800 rounded-lg border border-gray-700 overflow-hidden">
              {Array.isArray(extraData) ? (
                <table className="w-full text-sm">
                  <thead><tr className="border-b border-gray-700 text-gray-400">{Object.keys(extraData[0] || {}).map(k => <th key={k} className="px-4 py-3 text-left capitalize">{k.replace(/_/g, ' ')}</th>)}</tr></thead>
                  <tbody>{extraData.map((row: any, i: number) => (
                    <tr key={i} className="border-b border-gray-700/50 hover:bg-gray-700/30">{Object.values(row).map((v: any, j: number) => <td key={j} className="px-4 py-2 text-gray-300 text-xs">{typeof v === 'object' ? JSON.stringify(v) : String(v)}</td>)}</tr>
                  ))}</tbody>
                </table>
              ) : (
                <pre className="p-4 text-gray-300 text-xs overflow-auto max-h-96">{JSON.stringify(extraData, null, 2)}</pre>
              )}
            </div>
          )}
        </>
      )}

      {instances.length === 0 && (
        <div className="bg-white/5 border border-white/10 rounded-xl text-center py-16">
          <Server className="w-12 h-12 text-slate-600 mx-auto mb-3" />
          <p className="text-slate-400">No {title} instances configured</p>
          <p className="text-xs text-slate-500 mt-1">Click "Add Instance" to get started</p>
        </div>
      )}

      {/* Add Instance Modal */}
      {showAddModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
          <div className="w-full max-w-lg bg-slate-900 border border-white/10 rounded-2xl shadow-2xl">
            <div className="flex items-center justify-between px-6 py-4 border-b border-white/10">
              <h2 className="text-lg font-bold text-white flex items-center gap-2">
                <Plus className="w-5 h-5 text-brand-400" />
                Add {title} Instance
              </h2>
              <button onClick={() => setShowAddModal(false)} className="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 transition-colors">
                <X className="w-5 h-5" />
              </button>
            </div>

            <form onSubmit={handleAddSubmit} className="p-6 space-y-4">
              {addError && (
                <div className="px-4 py-2.5 bg-red-500/10 border border-red-500/20 rounded-xl text-sm text-red-400">{addError}</div>
              )}

              <div className="grid grid-cols-2 gap-4">
                <div>
                  <label className="block text-xs font-medium text-slate-400 mb-1.5">Instance Name *</label>
                  <input
                    value={addForm.name}
                    onChange={e => updateField('name', e.target.value)}
                    placeholder={`e.g. ${serviceType}-prod-01`}
                    className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-brand-500/50"
                    autoFocus
                  />
                </div>
                <div>
                  <label className="block text-xs font-medium text-slate-400 mb-1.5">Environment</label>
                  <select
                    value={addForm.environment}
                    onChange={e => updateField('environment', e.target.value)}
                    className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white focus:outline-none focus:ring-2 focus:ring-brand-500/50 [&>option]:bg-slate-900 [&>option]:text-white"
                  >
                    <option value="production">Production</option>
                    <option value="staging">Staging</option>
                    <option value="development">Development</option>
                    <option value="testing">Testing</option>
                  </select>
                </div>
              </div>

              <div className="grid grid-cols-3 gap-4">
                <div className="col-span-2">
                  <label className="block text-xs font-medium text-slate-400 mb-1.5">Host / IP *</label>
                  <input
                    value={addForm.host}
                    onChange={e => updateField('host', e.target.value)}
                    placeholder="e.g. 10.0.1.10 or hostname"
                    className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-brand-500/50"
                  />
                </div>
                <div>
                  <label className="block text-xs font-medium text-slate-400 mb-1.5">Port</label>
                  <input
                    type="number"
                    value={addForm.port || ''}
                    onChange={e => updateField('port', parseInt(e.target.value) || 0)}
                    placeholder={String(defaultPorts[serviceType] || 0)}
                    className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-brand-500/50"
                  />
                </div>
              </div>

              <div>
                <label className="block text-xs font-medium text-slate-400 mb-1.5">Description</label>
                <textarea
                  value={addForm.description}
                  onChange={e => updateField('description', e.target.value)}
                  rows={2}
                  placeholder="Optional description..."
                  className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-brand-500/50 resize-none"
                />
              </div>

              <div className="flex justify-end gap-3 pt-2">
                <button
                  type="button"
                  onClick={() => setShowAddModal(false)}
                  className="px-4 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-slate-400 hover:text-white transition-colors"
                >
                  Cancel
                </button>
                <button
                  type="submit"
                  disabled={createMut.isPending}
                  className="flex items-center gap-2 px-5 py-2.5 bg-brand-600 hover:bg-brand-700 rounded-xl text-sm font-medium transition-colors disabled:opacity-50"
                >
                  {createMut.isPending ? <Loader2 className="w-4 h-4 animate-spin" /> : <Plus className="w-4 h-4" />}
                  {createMut.isPending ? 'Creating...' : 'Create Instance'}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  )
}
