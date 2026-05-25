import { useState, useEffect, useCallback } from 'react'
import { getOracleMetrics, createOracleMetric, updateOracleMetric, deleteOracleMetric, testOracleMetric, reloadOracleMetrics } from '@/lib/api'
import { Database, Plus, Trash2, Play, RefreshCw, Code, Eye, ToggleLeft, ToggleRight } from 'lucide-react'
import { cn } from '@/lib/utils'
import LiveModeButton from '@/components/LiveModeButton'

export default function OracleMetricEditor() {
  const [metrics, setMetrics] = useState<any[]>([])
  const [selected, setSelected] = useState<any>(null)
  const [testResult, setTestResult] = useState<any>(null)
  const [showAdd, setShowAdd] = useState(false)
  const [filter, setFilter] = useState<'all' | 'default' | 'custom'>('all')

  const load = useCallback(async () => {
    const data = await getOracleMetrics().catch(() => ({ metrics: [] }))
    setMetrics(data.metrics || [])
  }, [])
  useEffect(() => { load() }, [load])

  const filtered = metrics.filter(m =>
    filter === 'all' ? true : filter === 'default' ? m.is_default : !m.is_default
  )

  const handleTest = async (id: string) => {
    const res = await testOracleMetric(id).catch(() => null)
    setTestResult(res)
  }

  const handleToggle = async (m: any) => {
    await updateOracleMetric(m.id, { is_active: !m.is_active })
    load()
  }

  const handleDelete = async (id: string) => {
    await deleteOracleMetric(id)
    if (selected?.id === id) setSelected(null)
    load()
  }

  const handleReload = async () => { await reloadOracleMetrics(); load() }

  const handleCreate = async (e: React.FormEvent<HTMLFormElement>) => {
    e.preventDefault()
    const fd = new FormData(e.currentTarget)
    await createOracleMetric({
      context: fd.get('context'),
      request: fd.get('request'),
      query_timeout: Number(fd.get('query_timeout')) || 30,
      scrape_interval: Number(fd.get('scrape_interval')) || 60,
    })
    setShowAdd(false); load()
  }

  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-3">
          <div className="p-2 rounded-lg bg-orange-600/20"><Database className="w-6 h-6 text-orange-400" /></div>
          <div><h1 className="text-2xl font-bold text-white">Oracle Metric Editor</h1><p className="text-sm text-gray-400">Manage metric definitions — TOML/YAML, test, toggle defaults</p></div>
        </div>
        <div className="flex gap-2">
          <button onClick={handleReload} className="px-3 py-1.5 bg-gray-700 text-gray-300 rounded text-xs flex items-center gap-1"><RefreshCw className="w-3 h-3" /> Reload</button>
          <LiveModeButton><button onClick={() => setShowAdd(true)} className="px-3 py-1.5 bg-orange-600 text-white rounded text-xs flex items-center gap-1"><Plus className="w-3 h-3" /> Custom Metric</button></LiveModeButton>
        </div>
      </div>

      {/* Filter */}
      <div className="flex gap-1 bg-gray-800/50 rounded-lg p-1 w-fit">
        {(['all', 'default', 'custom'] as const).map(f => (
          <button key={f} onClick={() => setFilter(f)} className={cn('px-3 py-1 rounded text-xs capitalize', filter === f ? 'bg-orange-600 text-white' : 'text-gray-400 hover:text-white')}>{f} ({f === 'all' ? metrics.length : metrics.filter(m => f === 'default' ? m.is_default : !m.is_default).length})</button>
        ))}
      </div>

      {showAdd && (
        <form onSubmit={handleCreate} className="bg-gray-800/50 border border-gray-700/50 rounded-xl p-4 space-y-3">
          <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
            <input name="context" placeholder="Context name" required className="bg-gray-700 text-sm text-white rounded px-3 py-2 border border-gray-600" />
            <input name="query_timeout" placeholder="Timeout (30s)" type="number" defaultValue={30} className="bg-gray-700 text-sm text-white rounded px-3 py-2 border border-gray-600" />
            <input name="scrape_interval" placeholder="Interval (60s)" type="number" defaultValue={60} className="bg-gray-700 text-sm text-white rounded px-3 py-2 border border-gray-600" />
          </div>
          <textarea name="request" placeholder="SELECT ... FROM ..." rows={3} required className="w-full bg-gray-700 text-sm text-white rounded px-3 py-2 border border-gray-600 font-mono resize-none" />
          <div className="flex gap-2 justify-end">
            <button type="button" onClick={() => setShowAdd(false)} className="px-3 py-1.5 bg-gray-700 text-gray-300 rounded text-xs">Cancel</button>
            <button type="submit" className="px-3 py-1.5 bg-orange-600 text-white rounded text-xs">Create</button>
          </div>
        </form>
      )}

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        {/* Metric List */}
        <div className="space-y-2 max-h-[70vh] overflow-y-auto pr-1">
          {filtered.map((m: any) => (
            <div key={m.id} onClick={() => { setSelected(m); setTestResult(null) }}
              className={cn('bg-gray-800/50 border rounded-xl p-3 cursor-pointer transition-colors',
                selected?.id === m.id ? 'border-orange-500/50' : 'border-gray-700/50 hover:border-gray-600/50')}>
              <div className="flex items-center justify-between mb-1">
                <div className="flex items-center gap-2">
                  <Code className="w-4 h-4 text-orange-400" />
                  <span className="text-sm font-medium text-white">{m.context}</span>
                  {m.is_default && <span className="text-[9px] px-1.5 py-0.5 rounded bg-blue-500/20 text-blue-400">default</span>}
                  {!m.is_default && <span className="text-[9px] px-1.5 py-0.5 rounded bg-purple-500/20 text-purple-400">custom</span>}
                </div>
                <div className="flex items-center gap-1">
                  <button onClick={(e) => { e.stopPropagation(); handleToggle(m) }} className="p-1 text-gray-400 hover:text-white">
                    {m.is_active ? <ToggleRight className="w-4 h-4 text-green-400" /> : <ToggleLeft className="w-4 h-4 text-gray-600" />}
                  </button>
                  {!m.is_default && <button onClick={(e) => { e.stopPropagation(); handleDelete(m.id) }} className="p-1 text-gray-400 hover:text-red-400"><Trash2 className="w-3.5 h-3.5" /></button>}
                </div>
              </div>
              <p className="text-[10px] text-gray-500 font-mono truncate">{m.request?.slice(0, 80)}</p>
              <div className="flex gap-3 mt-1 text-[9px] text-gray-600">
                {m.labels?.length > 0 && <span>Labels: {m.labels.join(', ')}</span>}
                {m.scrape_interval && <span>Every {m.scrape_interval}s</span>}
                {m.source_file && <span>{m.source_file}</span>}
              </div>
            </div>
          ))}
        </div>

        {/* Detail / Editor */}
        <div className="space-y-3">
          {selected ? (
            <div className="bg-gray-800/50 border border-gray-700/50 rounded-xl p-4 space-y-3">
              <div className="flex items-center justify-between">
                <h3 className="text-sm font-bold text-white flex items-center gap-2"><Eye className="w-4 h-4" /> {selected.context}</h3>
                <button onClick={() => handleTest(selected.id)} className="px-3 py-1.5 bg-green-600 hover:bg-green-700 text-white rounded text-xs flex items-center gap-1"><Play className="w-3 h-3" /> Test</button>
              </div>
              <div className="bg-gray-900 rounded-lg p-3">
                <pre className="text-xs text-green-400 font-mono whitespace-pre-wrap">{selected.request}</pre>
              </div>
              <div className="grid grid-cols-2 gap-3 text-xs">
                <div><span className="text-gray-500">Labels:</span> <span className="text-white">{selected.labels?.join(', ') || 'none'}</span></div>
                <div><span className="text-gray-500">Timeout:</span> <span className="text-white">{selected.query_timeout || 30}s</span></div>
                <div><span className="text-gray-500">Interval:</span> <span className="text-white">{selected.scrape_interval || 60}s</span></div>
                <div><span className="text-gray-500">Source:</span> <span className="text-white">{selected.source_file || 'custom'}</span></div>
                <div><span className="text-gray-500">Status:</span> <span className={selected.is_active ? 'text-green-400' : 'text-red-400'}>{selected.is_active ? 'Active' : 'Disabled'}</span></div>
                <div><span className="text-gray-500">Type:</span> <span className="text-white">{selected.is_default ? 'Default' : 'Custom'}</span></div>
              </div>

              {testResult && (
                <div className="bg-gray-900 rounded-lg p-3 space-y-2">
                  <div className="flex justify-between text-xs"><span className="text-gray-400">Test Results</span><span className="text-gray-500">{testResult.duration_ms}ms · {testResult.row_count} rows</span></div>
                  <pre className="text-[10px] text-cyan-400 font-mono max-h-[200px] overflow-auto whitespace-pre-wrap">{JSON.stringify(testResult.sample_rows, null, 2)}</pre>
                </div>
              )}
            </div>
          ) : (
            <div className="bg-gray-800/50 border border-gray-700/50 rounded-xl p-8 text-center text-gray-500 text-sm">Select a metric to view details and test</div>
          )}
        </div>
      </div>
    </div>
  )
}
