import { useState, useCallback } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { FileText, Search, Bell, BarChart2, Database, Trash2, Plus, Download, Play } from 'lucide-react'
import { queryLogs, getLogLabels, getLogStreams, getLogAlerts, createLogAlert, deleteLogAlert, getLogAnalytics, exportLogs, type LogQueryResult, type LogStreamInfo, type LogAlert, type LogAnalyticsData } from '@/lib/api'
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer, PieChart, Pie, Cell } from 'recharts'

const LEVEL_COLORS: Record<string, string> = { error: 'text-red-400 bg-red-500/10', warn: 'text-yellow-400 bg-yellow-500/10', warning: 'text-yellow-400 bg-yellow-500/10', info: 'text-blue-400 bg-blue-500/10', debug: 'text-gray-400 bg-gray-500/10' }
const PIE_COLORS = ['#3b82f6', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6']

export default function LogExplorerPage() {
  const qc = useQueryClient()
  const [tab, setTab] = useState<'explorer' | 'streams' | 'alerts' | 'analytics'>('explorer')
  const [query, setQuery] = useState('{}')
  const [searchTriggered, setSearchTriggered] = useState(false)

  const { data: logs } = useQuery<LogQueryResult>({ queryKey: ['log-query', query], queryFn: () => queryLogs({ query, limit: '200' }), enabled: searchTriggered })
  const { data: labels = [] } = useQuery({ queryKey: ['log-labels'], queryFn: getLogLabels })
  const { data: streams = [] } = useQuery({ queryKey: ['log-streams'], queryFn: getLogStreams, enabled: tab === 'streams' })
  const { data: alerts = [] } = useQuery({ queryKey: ['log-alerts'], queryFn: getLogAlerts, enabled: tab === 'alerts' })
  const { data: analytics } = useQuery<LogAnalyticsData>({ queryKey: ['log-analytics'], queryFn: getLogAnalytics, enabled: tab === 'analytics' })
  const deleteAlertMut = useMutation({ mutationFn: deleteLogAlert, onSuccess: () => qc.invalidateQueries({ queryKey: ['log-alerts'] }) })
  const createAlertMut = useMutation({ mutationFn: createLogAlert, onSuccess: () => qc.invalidateQueries({ queryKey: ['log-alerts'] }) })

  const doSearch = useCallback(() => setSearchTriggered(true), [])

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-white flex items-center gap-2"><FileText className="w-6 h-6 text-blue-400" /> Log Explorer</h1>
        <div className="flex gap-2">
          {(['explorer', 'streams', 'alerts', 'analytics'] as const).map(t => (
            <button key={t} onClick={() => setTab(t)} className={`px-3 py-1.5 rounded text-sm capitalize ${tab === t ? 'bg-blue-600 text-white' : 'bg-gray-700 text-gray-300'}`}>
              {t === 'explorer' ? <><Search className="w-3.5 h-3.5 inline mr-1" />{t}</> : t === 'alerts' ? <><Bell className="w-3.5 h-3.5 inline mr-1" />{t}</> : t === 'analytics' ? <><BarChart2 className="w-3.5 h-3.5 inline mr-1" />{t}</> : <><Database className="w-3.5 h-3.5 inline mr-1" />{t}</>}
            </button>
          ))}
        </div>
      </div>

      {tab === 'explorer' && (
        <>
          <div className="flex gap-2">
            <input value={query} onChange={e => { setQuery(e.target.value); setSearchTriggered(false) }} onKeyDown={e => e.key === 'Enter' && doSearch()} placeholder='LogQL query, e.g. {job="nginx"} |= "error"' className="flex-1 bg-gray-800 text-white rounded px-4 py-2 text-sm border border-gray-700 font-mono" />
            <button onClick={doSearch} className="flex items-center gap-1 px-4 py-2 bg-blue-600 text-white rounded text-sm"><Play className="w-4 h-4" /> Run</button>
            <button onClick={() => exportLogs({ query })} className="flex items-center gap-1 px-3 py-2 bg-gray-700 text-gray-300 rounded text-sm"><Download className="w-4 h-4" /></button>
          </div>

          <div className="flex gap-2 flex-wrap">
            {labels.map(l => <button key={l} onClick={() => setQuery(`{${l}=""}`)} className="px-2 py-0.5 bg-gray-700 text-gray-300 hover:bg-gray-600 rounded text-xs">{l}</button>)}
          </div>

          {logs && (
            <div className="bg-gray-800 rounded-lg border border-gray-700 max-h-[60vh] overflow-y-auto">
              <div className="px-4 py-2 border-b border-gray-700 text-sm text-gray-400">{logs.total_entries} entries found across {logs.streams.length} streams</div>
              {logs.streams.map((stream, si) => (
                <div key={si}>
                  <div className="px-4 py-1.5 bg-gray-900 text-xs text-gray-500 font-mono">{JSON.stringify(stream.labels)}</div>
                  {stream.entries.map((entry, ei) => {
                    const level = stream.labels.level || ''
                    const cls = LEVEL_COLORS[level] || 'text-gray-300'
                    return (
                      <div key={ei} className={`px-4 py-1 font-mono text-xs border-b border-gray-700/30 hover:bg-gray-700/20 ${cls}`}>
                        <span className="text-gray-500 mr-3">{new Date(entry.timestamp).toLocaleTimeString()}</span>
                        {entry.line}
                      </div>
                    )
                  })}
                </div>
              ))}
            </div>
          )}
        </>
      )}

      {tab === 'streams' && (
        <div className="bg-gray-800 rounded-lg border border-gray-700 overflow-hidden">
          <table className="w-full text-sm">
            <thead><tr className="border-b border-gray-700 text-gray-400"><th className="px-4 py-3 text-left">Labels</th><th className="px-4 py-3 text-right">Entries</th><th className="px-4 py-3 text-right">Bytes</th></tr></thead>
            <tbody>
              {streams.map((s: LogStreamInfo) => (
                <tr key={s.fingerprint} className="border-b border-gray-700/50 hover:bg-gray-700/30">
                  <td className="px-4 py-3 font-mono text-xs text-gray-300">{JSON.stringify(s.labels)}</td>
                  <td className="px-4 py-3 text-right text-white">{s.entry_count}</td>
                  <td className="px-4 py-3 text-right text-gray-400">{(s.bytes / 1024).toFixed(1)} KB</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      {tab === 'alerts' && (
        <div className="space-y-4">
          <div className="flex justify-end"><button onClick={() => createAlertMut.mutate({ name: 'New Alert', query: '{level="error"}', threshold: 10, severity: 'warning' })} className="flex items-center gap-1 px-3 py-1.5 bg-blue-600 text-white rounded text-sm"><Plus className="w-4 h-4" /> Create Alert</button></div>
          {alerts.map((a: LogAlert) => (
            <div key={a.id} className="bg-gray-800 rounded-lg p-4 border border-gray-700 flex items-center justify-between">
              <div>
                <h3 className="text-white font-medium">{a.name}</h3>
                <p className="text-gray-400 text-sm font-mono mt-1">{a.query}</p>
                <div className="flex gap-4 text-xs text-gray-500 mt-2">
                  <span>Threshold: {a.comparison} {a.threshold}</span>
                  <span className={`px-1.5 rounded ${a.severity === 'critical' ? 'bg-red-500/20 text-red-400' : 'bg-yellow-500/20 text-yellow-400'}`}>{a.severity}</span>
                  {a.last_fired && <span>Last fired: {new Date(a.last_fired).toLocaleString()}</span>}
                </div>
              </div>
              <button onClick={() => deleteAlertMut.mutate(a.id)} className="p-1 text-red-400 hover:text-red-300"><Trash2 className="w-4 h-4" /></button>
            </div>
          ))}
        </div>
      )}

      {tab === 'analytics' && analytics && (
        <div className="space-y-6">
          <div className="grid grid-cols-4 gap-4">
            <div className="bg-gray-800 rounded-lg p-4 border border-gray-700"><p className="text-gray-400 text-xs">Total Streams</p><p className="text-2xl font-bold text-white">{analytics.total_streams}</p></div>
            <div className="bg-gray-800 rounded-lg p-4 border border-gray-700"><p className="text-gray-400 text-xs">Total Entries</p><p className="text-2xl font-bold text-white">{analytics.total_entries.toLocaleString()}</p></div>
            <div className="bg-gray-800 rounded-lg p-4 border border-gray-700"><p className="text-gray-400 text-xs">Ingestion Rate</p><p className="text-2xl font-bold text-blue-400">{analytics.ingestion_rate_lines_per_sec.toFixed(0)} lines/s</p></div>
            <div className="bg-gray-800 rounded-lg p-4 border border-gray-700"><p className="text-gray-400 text-xs">Total Bytes</p><p className="text-2xl font-bold text-white">{(analytics.total_bytes / 1024 / 1024).toFixed(1)} MB</p></div>
          </div>
          <div className="grid grid-cols-2 gap-6">
            <div className="bg-gray-800 rounded-lg p-4 border border-gray-700">
              <h3 className="text-gray-300 font-medium text-sm mb-3">Volume by Source</h3>
              <div className="h-48">
                <ResponsiveContainer><PieChart><Pie data={analytics.volume_by_source} dataKey="lines" nameKey="source" innerRadius={40} outerRadius={70}>{analytics.volume_by_source.map((_, i) => <Cell key={i} fill={PIE_COLORS[i % PIE_COLORS.length]} />)}</Pie><Tooltip /></PieChart></ResponsiveContainer>
              </div>
            </div>
            <div className="bg-gray-800 rounded-lg p-4 border border-gray-700">
              <h3 className="text-gray-300 font-medium text-sm mb-3">Error Rate Trend</h3>
              <div className="h-48">
                <ResponsiveContainer><BarChart data={analytics.error_rate_trend}><XAxis dataKey="timestamp" tickFormatter={(t: string) => new Date(t).getHours() + 'h'} tick={{ fill: '#9ca3af', fontSize: 10 }} /><YAxis tick={{ fill: '#9ca3af', fontSize: 10 }} /><Tooltip contentStyle={{ backgroundColor: '#1f2937', border: '1px solid #374151', borderRadius: '0.5rem' }} /><Bar dataKey="count" fill="#ef4444" radius={[2, 2, 0, 0]} /></BarChart></ResponsiveContainer>
              </div>
            </div>
          </div>
          <div className="bg-gray-800 rounded-lg p-4 border border-gray-700">
            <h3 className="text-gray-300 font-medium text-sm mb-3">Top Error Messages</h3>
            <div className="space-y-2">
              {analytics.top_error_messages.map((e, i) => (
                <div key={i} className="flex items-center justify-between text-sm"><span className="text-red-400">{e.message}</span><div className="flex gap-4"><span className="text-white font-mono">{e.count}</span><span className="text-gray-500 text-xs">{new Date(e.last_seen).toLocaleString()}</span></div></div>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
