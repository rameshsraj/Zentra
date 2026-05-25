import { useState, useEffect, useCallback, useRef } from 'react'
import { fetchJSON } from '@/lib/api'
import { Play, Download, Clock, ChevronRight, ChevronDown, Table2, Columns3, Search } from 'lucide-react'
import { cn } from '@/lib/utils'

interface SqlWorkspaceTabProps {
  dbType: 'oracle'
  connectionId?: string
}

interface QueryResult {
  success: boolean
  columns?: string[]
  rows?: string[][]
  row_count?: number
  execution_time_ms?: number
  error?: string
}

interface HistoryEntry {
  id: string
  sql_text: string
  row_count?: number
  execution_time_ms: number
  status: string
  executed_at: string
}

interface SchemaItem {
  schema_name: string
}

interface TableItem {
  table_name: string
  num_rows?: number
  type?: string
}

export default function SqlWorkspaceTab({ dbType, connectionId }: SqlWorkspaceTabProps) {
  const [sql, setSql] = useState('')
  const [result, setResult] = useState<QueryResult | null>(null)
  const [running, setRunning] = useState(false)
  const [history, setHistory] = useState<HistoryEntry[]>([])
  const [schemas, setSchemas] = useState<SchemaItem[]>([])
  const [tables, setTables] = useState<TableItem[]>([])
  const [selectedSchema, setSelectedSchema] = useState<string | null>(null)
  const [schemaOpen, setSchemaOpen] = useState(true)
  const [historyOpen, setHistoryOpen] = useState(false)
  const [page, setPage] = useState(0)
  const editorRef = useRef<HTMLTextAreaElement>(null)
  const PAGE_SIZE = 50

  // Load schemas on mount
  useEffect(() => {
    const params = connectionId ? `?connection_id=${connectionId}` : ''
    fetchJSON<any>(`/db/${dbType}/schemas${params}`).then(d => setSchemas(d.schemas || [])).catch(() => {})
  }, [dbType, connectionId])

  // Load tables when schema selected
  useEffect(() => {
    if (!selectedSchema) return
    const params = connectionId ? `&connection_id=${connectionId}` : ''
    fetchJSON<any>(`/db/${dbType}/tables?schema=${selectedSchema}${params}`).then(d => setTables(d.tables || [])).catch(() => {})
  }, [dbType, selectedSchema, connectionId])

  // Load history
  const loadHistory = useCallback(async () => {
    const d = await fetchJSON<any>(`/db/history?db_type=${dbType}&limit=50`).catch(() => ({ history: [] }))
    setHistory(d.history || [])
  }, [dbType])
  useEffect(() => { loadHistory() }, [loadHistory])

  const executeQuery = async () => {
    if (!sql.trim()) return
    setRunning(true)
    setResult(null)
    setPage(0)
    try {
      const res = await fetchJSON<QueryResult>('/db/execute', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ db_type: dbType, connection_id: connectionId, sql: sql.trim(), limit: 500 }),
      })
      setResult(res)
      loadHistory()
    } catch (e: any) {
      setResult({ success: false, error: e.message || 'Query failed' })
    } finally {
      setRunning(false)
    }
  }

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if ((e.ctrlKey || e.metaKey) && e.key === 'Enter') {
      e.preventDefault()
      executeQuery()
    }
  }

  const insertTableQuery = (table: string) => {
    const q = `SELECT * FROM ${table} FETCH FIRST 10 ROWS ONLY`
    setSql(q)
    editorRef.current?.focus()
  }

  const exportCSV = () => {
    if (!result?.columns || !result?.rows) return
    const header = result.columns.join(',')
    const rows = result.rows.map(r => r.map(c => `"${c.replace(/"/g, '""')}"`).join(',')).join('\n')
    const blob = new Blob([header + '\n' + rows], { type: 'text/csv' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url; a.download = `query_result_${Date.now()}.csv`; a.click()
    URL.revokeObjectURL(url)
  }

  const exportJSON = () => {
    if (!result?.columns || !result?.rows) return
    const data = result.rows.map(r => {
      const obj: Record<string, string> = {}
      result.columns!.forEach((c, i) => { obj[c] = r[i] })
      return obj
    })
    const blob = new Blob([JSON.stringify(data, null, 2)], { type: 'application/json' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url; a.download = `query_result_${Date.now()}.json`; a.click()
    URL.revokeObjectURL(url)
  }

  const pagedRows = result?.rows?.slice(page * PAGE_SIZE, (page + 1) * PAGE_SIZE) || []
  const totalPages = result?.rows ? Math.ceil(result.rows.length / PAGE_SIZE) : 0

  return (
    <div className="flex h-[calc(100vh-12rem)] gap-3">
      {/* Left: Schema Browser */}
      <div className="w-56 flex-shrink-0 bg-gray-800/50 border border-gray-700/50 rounded-xl overflow-hidden flex flex-col">
        <div className="p-2 border-b border-gray-700/50">
          <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wide">Schema Browser</h3>
        </div>
        <div className="flex-1 overflow-y-auto p-2 space-y-1 text-xs">
          <button onClick={() => setSchemaOpen(!schemaOpen)} className="flex items-center gap-1 text-gray-300 hover:text-white w-full">
            {schemaOpen ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
            <Columns3 className="w-3 h-3 text-yellow-400" /> Schemas ({schemas.length})
          </button>
          {schemaOpen && schemas.map(s => (
            <div key={s.schema_name}>
              <button onClick={() => setSelectedSchema(selectedSchema === s.schema_name ? null : s.schema_name)}
                className={cn('flex items-center gap-1 pl-4 py-0.5 w-full rounded hover:bg-gray-700/50',
                  selectedSchema === s.schema_name ? 'text-white bg-gray-700/50' : 'text-gray-400')}>
                {selectedSchema === s.schema_name ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
                {s.schema_name}
              </button>
              {selectedSchema === s.schema_name && tables.map(t => (
                <button key={t.table_name} onClick={() => insertTableQuery(t.table_name)}
                  className="flex items-center gap-1 pl-8 py-0.5 w-full text-gray-500 hover:text-white hover:bg-gray-700/30 rounded">
                  <Table2 className="w-3 h-3 text-blue-400" />
                  <span className="truncate">{t.table_name}</span>
                  {t.num_rows != null && <span className="text-gray-600 ml-auto text-[9px]">{t.num_rows}</span>}
                </button>
              ))}
            </div>
          ))}
        </div>

        {/* History */}
        <div className="border-t border-gray-700/50">
          <button onClick={() => { setHistoryOpen(!historyOpen); if (!historyOpen) loadHistory() }}
            className="flex items-center gap-1 p-2 text-xs text-gray-400 hover:text-white w-full">
            <Clock className="w-3 h-3" /> History ({history.length})
            {historyOpen ? <ChevronDown className="w-3 h-3 ml-auto" /> : <ChevronRight className="w-3 h-3 ml-auto" />}
          </button>
          {historyOpen && (
            <div className="max-h-40 overflow-y-auto px-2 pb-2 space-y-1">
              {history.slice(0, 20).map(h => (
                <button key={h.id} onClick={() => setSql(h.sql_text)}
                  className="w-full text-left p-1 rounded hover:bg-gray-700/50 text-[10px]">
                  <div className="text-gray-400 font-mono truncate">{h.sql_text.slice(0, 60)}</div>
                  <div className="flex gap-2 text-gray-600">
                    <span className={h.status === 'success' ? 'text-green-500' : 'text-red-500'}>{h.status}</span>
                    <span>{h.execution_time_ms}ms</span>
                    {h.row_count != null && <span>{h.row_count} rows</span>}
                  </div>
                </button>
              ))}
            </div>
          )}
        </div>
      </div>

      {/* Right: Editor + Results */}
      <div className="flex-1 flex flex-col gap-3 min-w-0">
        {/* Editor */}
        <div className="bg-gray-800/50 border border-gray-700/50 rounded-xl overflow-hidden">
          <div className="flex items-center justify-between px-3 py-1.5 border-b border-gray-700/50">
            <span className="text-xs text-gray-500">SQL Editor — {dbType.toUpperCase()} · Ctrl+Enter to run</span>
            <button onClick={executeQuery} disabled={running || !sql.trim()}
              className={cn('px-3 py-1 rounded text-xs flex items-center gap-1 transition-colors',
                running ? 'bg-gray-600 text-gray-400' : 'bg-green-600 hover:bg-green-700 text-white')}>
              <Play className="w-3 h-3" /> {running ? 'Running...' : 'Run'}
            </button>
          </div>
          <textarea ref={editorRef} value={sql} onChange={e => setSql(e.target.value)} onKeyDown={handleKeyDown}
            rows={6} placeholder="SELECT * FROM dual ..."
            className="w-full bg-gray-900 text-green-400 font-mono text-sm p-3 resize-none focus:outline-none border-none" spellCheck={false} />
        </div>

        {/* Results */}
        <div className="flex-1 bg-gray-800/50 border border-gray-700/50 rounded-xl overflow-hidden flex flex-col min-h-0">
          {result ? (
            <>
              <div className="flex items-center justify-between px-3 py-1.5 border-b border-gray-700/50 flex-shrink-0">
                <div className="flex items-center gap-3 text-xs">
                  {result.success ? (
                    <>
                      <span className="text-green-400">✓ {result.row_count} rows</span>
                      <span className="text-gray-500">{result.execution_time_ms}ms</span>
                    </>
                  ) : (
                    <span className="text-red-400">✗ Error</span>
                  )}
                </div>
                {result.success && (
                  <div className="flex gap-1">
                    <button onClick={exportCSV} className="px-2 py-0.5 bg-gray-700 text-gray-300 rounded text-[10px] flex items-center gap-1 hover:bg-gray-600"><Download className="w-3 h-3" /> CSV</button>
                    <button onClick={exportJSON} className="px-2 py-0.5 bg-gray-700 text-gray-300 rounded text-[10px] flex items-center gap-1 hover:bg-gray-600"><Download className="w-3 h-3" /> JSON</button>
                  </div>
                )}
              </div>
              {result.success && result.columns && result.rows ? (
                <div className="flex-1 overflow-auto">
                  <table className="w-full text-xs">
                    <thead className="sticky top-0 bg-gray-800">
                      <tr>{result.columns.map((c, i) => <th key={i} className="text-left px-2 py-1.5 text-gray-400 font-medium border-b border-gray-700/50">{c}</th>)}</tr>
                    </thead>
                    <tbody>
                      {pagedRows.map((row, ri) => (
                        <tr key={ri} className="hover:bg-gray-700/30 border-b border-gray-800/50">
                          {row.map((cell, ci) => <td key={ci} className="px-2 py-1 text-gray-300 max-w-xs truncate">{cell}</td>)}
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              ) : (
                <div className="p-3 text-red-400 text-sm font-mono">{result.error}</div>
              )}
              {totalPages > 1 && (
                <div className="flex items-center justify-center gap-2 px-3 py-1.5 border-t border-gray-700/50 flex-shrink-0">
                  <button onClick={() => setPage(Math.max(0, page - 1))} disabled={page === 0} className="px-2 py-0.5 bg-gray-700 text-gray-300 rounded text-[10px] disabled:opacity-30">Prev</button>
                  <span className="text-[10px] text-gray-500">Page {page + 1} / {totalPages}</span>
                  <button onClick={() => setPage(Math.min(totalPages - 1, page + 1))} disabled={page >= totalPages - 1} className="px-2 py-0.5 bg-gray-700 text-gray-300 rounded text-[10px] disabled:opacity-30">Next</button>
                </div>
              )}
            </>
          ) : (
            <div className="flex-1 flex items-center justify-center text-gray-500 text-sm">
              <div className="text-center">
                <Search className="w-8 h-8 mx-auto mb-2 opacity-30" />
                <p>Run a query to see results</p>
                <p className="text-[10px] text-gray-600 mt-1">Ctrl+Enter to execute · Click table names to generate SELECT</p>
              </div>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
