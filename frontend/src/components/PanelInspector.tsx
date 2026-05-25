import { useState, useCallback } from 'react'
import { cn } from '@/lib/utils'
import { X, Download, Copy, Table, BarChart3, Code, Info } from 'lucide-react'

interface PanelInspectorProps {
  panelTitle: string
  data: Record<string, unknown>[]
  query?: string
  stats?: { key: string; value: string | number }[]
  rawJson?: unknown
  onClose: () => void
}

type TabId = 'data' | 'stats' | 'query' | 'json'

const TABS: { id: TabId; label: string; icon: typeof Table }[] = [
  { id: 'data', label: 'Data', icon: Table },
  { id: 'stats', label: 'Stats', icon: Info },
  { id: 'query', label: 'Query', icon: BarChart3 },
  { id: 'json', label: 'JSON', icon: Code },
]

export default function PanelInspector({ panelTitle, data, query, stats, rawJson, onClose }: PanelInspectorProps) {
  const [tab, setTab] = useState<TabId>('data')

  const columns = data.length > 0 ? Object.keys(data[0]) : []

  const exportCSV = useCallback(() => {
    if (data.length === 0) return
    const headers = columns.join(',')
    const rows = data.map(row => columns.map(c => `"${String(row[c] ?? '').replace(/"/g, '""')}"`).join(','))
    const csv = [headers, ...rows].join('\n')
    downloadFile(csv, `${panelTitle.replace(/\s+/g, '_')}.csv`, 'text/csv')
  }, [data, columns, panelTitle])

  const exportJSON = useCallback(() => {
    const json = JSON.stringify(rawJson ?? data, null, 2)
    downloadFile(json, `${panelTitle.replace(/\s+/g, '_')}.json`, 'application/json')
  }, [data, rawJson, panelTitle])

  const copyToClipboard = useCallback(() => {
    const json = JSON.stringify(rawJson ?? data, null, 2)
    navigator.clipboard.writeText(json)
  }, [data, rawJson])

  // Auto-compute stats if not provided
  const computedStats = stats || (() => {
    const result: { key: string; value: string | number }[] = [
      { key: 'Rows', value: data.length },
      { key: 'Columns', value: columns.length },
    ]
    for (const col of columns) {
      const nums = data.map(r => Number(r[col])).filter(n => !isNaN(n))
      if (nums.length > 0) {
        result.push({ key: `${col} (min)`, value: Math.min(...nums).toFixed(2) })
        result.push({ key: `${col} (max)`, value: Math.max(...nums).toFixed(2) })
        result.push({ key: `${col} (avg)`, value: (nums.reduce((a, b) => a + b, 0) / nums.length).toFixed(2) })
      }
    }
    return result
  })()

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
      <div className="bg-slate-900 border border-white/10 rounded-xl shadow-2xl w-[90vw] max-w-[1200px] h-[80vh] flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between px-4 py-3 border-b border-white/10 flex-shrink-0">
          <div className="flex items-center gap-3">
            <Info className="w-5 h-5 text-cyan-400" />
            <span className="font-bold text-white">Inspect: {panelTitle}</span>
          </div>
          <div className="flex items-center gap-2">
            <button onClick={exportCSV} className="flex items-center gap-1 px-2 py-1 bg-white/5 hover:bg-white/10 border border-white/10 rounded text-xs text-slate-300" title="Export CSV">
              <Download className="w-3.5 h-3.5" /> CSV
            </button>
            <button onClick={exportJSON} className="flex items-center gap-1 px-2 py-1 bg-white/5 hover:bg-white/10 border border-white/10 rounded text-xs text-slate-300" title="Export JSON">
              <Download className="w-3.5 h-3.5" /> JSON
            </button>
            <button onClick={copyToClipboard} className="flex items-center gap-1 px-2 py-1 bg-white/5 hover:bg-white/10 border border-white/10 rounded text-xs text-slate-300" title="Copy to clipboard">
              <Copy className="w-3.5 h-3.5" /> Copy
            </button>
            <button onClick={onClose} className="p-1 hover:bg-white/10 rounded text-slate-400 hover:text-white">
              <X className="w-5 h-5" />
            </button>
          </div>
        </div>

        {/* Tab Bar */}
        <div className="flex gap-1 px-4 py-2 border-b border-white/10 flex-shrink-0">
          {TABS.map(t => {
            const Icon = t.icon
            return (
              <button
                key={t.id}
                onClick={() => setTab(t.id)}
                className={cn(
                  'flex items-center gap-1.5 px-3 py-1.5 rounded text-xs font-medium transition-colors',
                  tab === t.id ? 'bg-cyan-600/20 text-cyan-400 border border-cyan-500/30' : 'text-slate-400 hover:text-white hover:bg-white/5'
                )}
              >
                <Icon className="w-3.5 h-3.5" /> {t.label}
              </button>
            )
          })}
        </div>

        {/* Content */}
        <div className="flex-1 overflow-auto p-4">
          {/* DATA TAB */}
          {tab === 'data' && (
            <div className="overflow-auto">
              {data.length === 0 ? (
                <p className="text-sm text-slate-500 text-center py-8">No data available</p>
              ) : (
                <table className="w-full text-xs">
                  <thead className="sticky top-0 bg-slate-900">
                    <tr className="border-b border-white/10">
                      <th className="px-3 py-2 text-left text-slate-400 uppercase">#</th>
                      {columns.map(c => (
                        <th key={c} className="px-3 py-2 text-left text-slate-400 uppercase">{c}</th>
                      ))}
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-white/5">
                    {data.map((row, i) => (
                      <tr key={i} className="hover:bg-white/5">
                        <td className="px-3 py-1.5 text-slate-500">{i + 1}</td>
                        {columns.map(c => (
                          <td key={c} className="px-3 py-1.5 font-mono text-slate-300">{String(row[c] ?? '')}</td>
                        ))}
                      </tr>
                    ))}
                  </tbody>
                </table>
              )}
            </div>
          )}

          {/* STATS TAB */}
          {tab === 'stats' && (
            <div className="grid grid-cols-2 md:grid-cols-3 gap-3">
              {computedStats.map((s, i) => (
                <div key={i} className="bg-white/5 border border-white/10 rounded-lg p-3">
                  <p className="text-[10px] text-slate-400 uppercase">{s.key}</p>
                  <p className="text-lg font-mono text-white mt-0.5">{typeof s.value === 'number' ? s.value.toLocaleString() : s.value}</p>
                </div>
              ))}
            </div>
          )}

          {/* QUERY TAB */}
          {tab === 'query' && (
            <div>
              <p className="text-[10px] text-slate-400 uppercase mb-2">Query Expression</p>
              <pre className="bg-slate-800 border border-white/10 rounded-lg p-4 text-xs font-mono text-cyan-300 whitespace-pre-wrap">
                {query || 'No query associated with this panel'}
              </pre>
            </div>
          )}

          {/* JSON TAB */}
          {tab === 'json' && (
            <pre className="bg-slate-800 border border-white/10 rounded-lg p-4 text-xs font-mono text-slate-300 whitespace-pre-wrap overflow-auto">
              {JSON.stringify(rawJson ?? data, null, 2)}
            </pre>
          )}
        </div>
      </div>
    </div>
  )
}

function downloadFile(content: string, filename: string, mimeType: string) {
  const blob = new Blob([content], { type: mimeType })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.click()
  URL.revokeObjectURL(url)
}
