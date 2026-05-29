import { useState, useEffect, useCallback, useRef } from 'react'
import { fetchJSON } from '@/lib/api'
import {
  Play, Download, Clock, ChevronRight, ChevronDown, Table2, Columns3, Search,
  Eye, Hash, Zap, Box, Code2, Layers, Link2, Database, Plus, FileText, Trash2, RefreshCw
} from 'lucide-react'
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

type ObjectType = 'tables' | 'views' | 'indexes' | 'sequences' | 'triggers' | 'types' | 'procedures' | 'mviews' | 'synonyms'

interface ObjectCategory {
  key: ObjectType
  label: string
  icon: React.ReactNode
  color: string
  nameField: string
  createTemplate: string
  dropPrefix: string
  selectTemplate?: string
}

const OBJECT_CATEGORIES: ObjectCategory[] = [
  { key: 'tables', label: 'Tables', icon: <Table2 className="w-3 h-3" />, color: 'text-blue-400', nameField: 'table_name', createTemplate: 'CREATE TABLE {schema}.new_table (\n  id NUMBER PRIMARY KEY,\n  name VARCHAR2(100) NOT NULL,\n  created_at TIMESTAMP DEFAULT SYSTIMESTAMP\n);', dropPrefix: 'DROP TABLE', selectTemplate: 'SELECT * FROM {schema}.{name} FETCH FIRST 100 ROWS ONLY' },
  { key: 'views', label: 'Views', icon: <Eye className="w-3 h-3" />, color: 'text-purple-400', nameField: 'view_name', createTemplate: 'CREATE OR REPLACE VIEW {schema}.new_view AS\nSELECT * FROM {schema}.table_name\nWHERE 1=1;', dropPrefix: 'DROP VIEW', selectTemplate: 'SELECT * FROM {schema}.{name} FETCH FIRST 100 ROWS ONLY' },
  { key: 'indexes', label: 'Indexes', icon: <Hash className="w-3 h-3" />, color: 'text-yellow-400', nameField: 'index_name', createTemplate: 'CREATE INDEX {schema}.idx_new ON {schema}.table_name (column_name);', dropPrefix: 'DROP INDEX' },
  { key: 'sequences', label: 'Sequences', icon: <Layers className="w-3 h-3" />, color: 'text-cyan-400', nameField: 'sequence_name', createTemplate: 'CREATE SEQUENCE {schema}.new_seq\n  START WITH 1\n  INCREMENT BY 1\n  NOCACHE\n  NOCYCLE;', dropPrefix: 'DROP SEQUENCE', selectTemplate: 'SELECT {schema}.{name}.NEXTVAL FROM dual' },
  { key: 'triggers', label: 'Triggers', icon: <Zap className="w-3 h-3" />, color: 'text-orange-400', nameField: 'trigger_name', createTemplate: 'CREATE OR REPLACE TRIGGER {schema}.new_trigger\n  BEFORE INSERT ON {schema}.table_name\n  FOR EACH ROW\nBEGIN\n  NULL;\nEND;\n/', dropPrefix: 'DROP TRIGGER' },
  { key: 'types', label: 'Types', icon: <Box className="w-3 h-3" />, color: 'text-pink-400', nameField: 'type_name', createTemplate: 'CREATE OR REPLACE TYPE {schema}.new_type AS OBJECT (\n  id NUMBER,\n  name VARCHAR2(100)\n);', dropPrefix: 'DROP TYPE' },
  { key: 'procedures', label: 'Procedures / Functions', icon: <Code2 className="w-3 h-3" />, color: 'text-green-400', nameField: 'object_name', createTemplate: 'CREATE OR REPLACE PROCEDURE {schema}.new_proc\nIS\nBEGIN\n  NULL;\nEND;\n/', dropPrefix: 'DROP PROCEDURE' },
  { key: 'mviews', label: 'Materialized Views', icon: <Layers className="w-3 h-3" />, color: 'text-teal-400', nameField: 'mview_name', createTemplate: 'CREATE MATERIALIZED VIEW {schema}.new_mview\nBUILD IMMEDIATE\nREFRESH FAST ON DEMAND\nAS\nSELECT * FROM {schema}.table_name;', dropPrefix: 'DROP MATERIALIZED VIEW', selectTemplate: 'SELECT * FROM {schema}.{name} FETCH FIRST 100 ROWS ONLY' },
  { key: 'synonyms', label: 'Synonyms', icon: <Link2 className="w-3 h-3" />, color: 'text-indigo-400', nameField: 'synonym_name', createTemplate: 'CREATE SYNONYM {schema}.new_syn FOR {schema}.table_name;', dropPrefix: 'DROP SYNONYM' },
]

interface ContextMenuState {
  x: number
  y: number
  items: { label: string; icon?: React.ReactNode; action: () => void; danger?: boolean }[]
}

interface ObjectDetail {
  type: ObjectType
  name: string
  schema: string
  properties: Record<string, any>
  columns?: any[]
}

export default function SqlWorkspaceTab({ dbType, connectionId }: SqlWorkspaceTabProps) {
  const [sql, setSql] = useState('')
  const [result, setResult] = useState<QueryResult | null>(null)
  const [running, setRunning] = useState(false)
  const [history, setHistory] = useState<HistoryEntry[]>([])
  const [schemas, setSchemas] = useState<SchemaItem[]>([])
  const [selectedSchema, setSelectedSchema] = useState<string | null>(null)
  const [schemaOpen, setSchemaOpen] = useState(true)
  const [historyOpen, setHistoryOpen] = useState(false)
  const [expandedCategories, setExpandedCategories] = useState<Set<string>>(new Set())
  const [objectData, setObjectData] = useState<Record<string, any[]>>({})
  const [objectCounts, setObjectCounts] = useState<Record<string, number>>({})
  const [contextMenu, setContextMenu] = useState<ContextMenuState | null>(null)
  const [objectDetail, setObjectDetail] = useState<ObjectDetail | null>(null)
  const [page, setPage] = useState(0)
  const editorRef = useRef<HTMLTextAreaElement>(null)
  const PAGE_SIZE = 50

  // Close context menu on click outside
  useEffect(() => {
    const close = () => setContextMenu(null)
    document.addEventListener('click', close)
    return () => document.removeEventListener('click', close)
  }, [])

  // Load schemas on mount
  useEffect(() => {
    const params = connectionId ? `?connection_id=${connectionId}` : ''
    fetchJSON<any>(`/db/${dbType}/schemas${params}`).then(d => setSchemas(d.schemas || [])).catch(() => {})
  }, [dbType, connectionId])

  // Load object counts when schema selected
  useEffect(() => {
    if (!selectedSchema) return
    const connParam = connectionId ? `&connection_id=${connectionId}` : ''
    fetchJSON<any>(`/db/${dbType}/schema-objects?schema=${selectedSchema}${connParam}`)
      .then(d => {
        const counts: Record<string, number> = {}
        for (const item of (d.object_counts || [])) {
          const t = (item.object_type || '').toLowerCase()
          const n = parseInt(item.cnt) || 0
          if (t === 'table') counts.tables = n
          else if (t === 'view') counts.views = n
          else if (t === 'index') counts.indexes = n
          else if (t === 'sequence') counts.sequences = n
          else if (t === 'trigger') counts.triggers = n
          else if (t === 'type') counts.types = n
          else if (t === 'procedure' || t === 'function' || t === 'package') counts.procedures = (counts.procedures || 0) + n
          else if (t === 'materialized view') counts.mviews = n
          else if (t === 'synonym') counts.synonyms = n
        }
        setObjectCounts(counts)
      }).catch(() => {})
  }, [dbType, selectedSchema, connectionId])

  const loadCategory = useCallback(async (cat: ObjectType) => {
    if (!selectedSchema) return
    const connParam = connectionId ? `&connection_id=${connectionId}` : ''
    try {
      const d = await fetchJSON<any>(`/db/${dbType}/${cat}?schema=${selectedSchema}${connParam}`)
      setObjectData(prev => ({ ...prev, [`${selectedSchema}.${cat}`]: d[cat] || [] }))
    } catch { /* ignore */ }
  }, [dbType, selectedSchema, connectionId])

  const toggleCategory = (cat: ObjectType) => {
    const key = `${selectedSchema}.${cat}`
    setExpandedCategories(prev => {
      const next = new Set(prev)
      if (next.has(key)) { next.delete(key) } else { next.add(key); loadCategory(cat) }
      return next
    })
  }

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
    setObjectDetail(null)
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

  const setAndFocus = (q: string) => {
    setSql(q)
    editorRef.current?.focus()
  }

  const loadObjectDetail = async (cat: ObjectCategory, objName: string, objData: any) => {
    const schema = selectedSchema || ''
    const detail: ObjectDetail = {
      type: cat.key,
      name: objName,
      schema,
      properties: objData,
    }
    // Load columns for tables and views
    if (cat.key === 'tables' || cat.key === 'views') {
      try {
        const connParam = connectionId ? `&connection_id=${connectionId}` : ''
        const d = await fetchJSON<any>(`/db/${dbType}/columns?schema=${schema}&table=${objName}${connParam}`)
        detail.columns = d.columns || []
      } catch { /* ignore */ }
    }
    setObjectDetail(detail)
    setResult(null)
  }

  // Context menu for category headers (e.g., right-click on "Tables" -> "Create Table...")
  const handleCategoryContextMenu = (e: React.MouseEvent, cat: ObjectCategory) => {
    e.preventDefault()
    e.stopPropagation()
    const schema = selectedSchema || 'SCHEMA'
    setContextMenu({
      x: e.clientX, y: e.clientY,
      items: [
        { label: `Create ${cat.label.replace(/s$/, '').replace(/ \/ Function/, '')}...`, icon: <Plus className="w-3 h-3" />, action: () => setAndFocus(cat.createTemplate.replace(/\{schema\}/g, schema)) },
        { label: `Refresh ${cat.label}`, icon: <RefreshCw className="w-3 h-3" />, action: () => loadCategory(cat.key) },
      ]
    })
  }

  // Context menu for individual objects
  const handleObjectContextMenu = (e: React.MouseEvent, cat: ObjectCategory, objName: string) => {
    e.preventDefault()
    e.stopPropagation()
    const schema = selectedSchema || 'SCHEMA'
    const items: ContextMenuState['items'] = []

    if (cat.selectTemplate) {
      items.push({ label: `SELECT from ${objName}`, icon: <Play className="w-3 h-3" />, action: () => setAndFocus(cat.selectTemplate!.replace(/\{schema\}/g, schema).replace(/\{name\}/g, objName)) })
    }
    if (cat.key === 'tables' || cat.key === 'views') {
      items.push({ label: `Show Columns`, icon: <Columns3 className="w-3 h-3" />, action: () => setAndFocus(`SELECT column_name, data_type, data_length, nullable FROM all_tab_columns WHERE owner = '${schema}' AND table_name = '${objName}' ORDER BY column_id`) })
    }
    if (cat.key === 'tables') {
      items.push({ label: `Row Count`, icon: <Hash className="w-3 h-3" />, action: () => setAndFocus(`SELECT COUNT(*) AS row_count FROM ${schema}.${objName}`) })
      items.push({ label: `Show Indexes`, icon: <Hash className="w-3 h-3" />, action: () => setAndFocus(`SELECT index_name, column_name, column_position FROM all_ind_columns WHERE table_owner = '${schema}' AND table_name = '${objName}' ORDER BY index_name, column_position`) })
      items.push({ label: `Show Triggers`, icon: <Zap className="w-3 h-3" />, action: () => setAndFocus(`SELECT trigger_name, trigger_type, triggering_event, status FROM all_triggers WHERE owner = '${schema}' AND table_name = '${objName}'`) })
    }
    if (cat.key === 'procedures') {
      items.push({ label: `View Source`, icon: <FileText className="w-3 h-3" />, action: () => setAndFocus(`SELECT line, text FROM all_source WHERE owner = '${schema}' AND name = '${objName}' ORDER BY line`) })
    }
    if (cat.key === 'triggers') {
      items.push({ label: `View Source`, icon: <FileText className="w-3 h-3" />, action: () => setAndFocus(`SELECT line, text FROM all_source WHERE owner = '${schema}' AND name = '${objName}' AND type = 'TRIGGER' ORDER BY line`) })
    }
    items.push({ label: `Generate DDL`, icon: <Code2 className="w-3 h-3" />, action: () => setAndFocus(`SELECT DBMS_METADATA.GET_DDL('${cat.key === 'tables' ? 'TABLE' : cat.key === 'views' ? 'VIEW' : cat.key === 'indexes' ? 'INDEX' : cat.key === 'sequences' ? 'SEQUENCE' : cat.key === 'triggers' ? 'TRIGGER' : cat.key === 'types' ? 'TYPE' : cat.key === 'procedures' ? 'PROCEDURE' : cat.key === 'mviews' ? 'MATERIALIZED_VIEW' : 'SYNONYM'}', '${objName}', '${schema}') AS ddl FROM dual`) })
    items.push({ label: `Drop ${objName}`, icon: <Trash2 className="w-3 h-3" />, danger: true, action: () => setAndFocus(`${cat.dropPrefix} ${schema}.${objName};`) })

    setContextMenu({ x: e.clientX, y: e.clientY, items })
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

  // ── Render helpers ────────────────────────────────────────────────────

  const renderObjectDetail = () => {
    if (!objectDetail) return null
    const cat = OBJECT_CATEGORIES.find(c => c.key === objectDetail.type)
    return (
      <div className="flex-1 bg-gray-800/50 border border-gray-700/50 rounded-xl overflow-hidden flex flex-col min-h-0">
        <div className="flex items-center gap-2 px-3 py-2 border-b border-gray-700/50 flex-shrink-0">
          <span className={cat?.color}>{cat?.icon}</span>
          <span className="text-sm font-medium text-white">{objectDetail.name}</span>
          <span className="text-xs text-gray-500">{objectDetail.schema} &middot; {cat?.label}</span>
        </div>

        {/* Properties */}
        <div className="p-3 space-y-3 overflow-auto flex-1">
          <div>
            <h4 className="text-xs font-semibold text-gray-400 uppercase mb-2">Properties</h4>
            <div className="grid grid-cols-2 gap-1 text-xs">
              {Object.entries(objectDetail.properties).filter(([k]) => k !== objectDetail.type).map(([k, v]) => (
                <div key={k} className="flex">
                  <span className="text-gray-500 w-36 flex-shrink-0">{k.replace(/_/g, ' ')}</span>
                  <span className="text-gray-300 truncate">{String(v ?? '-')}</span>
                </div>
              ))}
            </div>
          </div>

          {/* Columns for tables/views */}
          {objectDetail.columns && objectDetail.columns.length > 0 && (
            <div>
              <h4 className="text-xs font-semibold text-gray-400 uppercase mb-2">Columns ({objectDetail.columns.length})</h4>
              <table className="w-full text-xs">
                <thead>
                  <tr className="border-b border-gray-700/50">
                    <th className="text-left px-2 py-1 text-gray-500">#</th>
                    <th className="text-left px-2 py-1 text-gray-500">Name</th>
                    <th className="text-left px-2 py-1 text-gray-500">Type</th>
                    <th className="text-left px-2 py-1 text-gray-500">Length</th>
                    <th className="text-left px-2 py-1 text-gray-500">Nullable</th>
                  </tr>
                </thead>
                <tbody>
                  {objectDetail.columns.map((col: any, i: number) => (
                    <tr key={i} className="border-b border-gray-800/50 hover:bg-gray-700/20">
                      <td className="px-2 py-1 text-gray-600">{i + 1}</td>
                      <td className="px-2 py-1 text-gray-300 font-mono">{col.column_name}</td>
                      <td className="px-2 py-1 text-blue-300">{col.data_type}</td>
                      <td className="px-2 py-1 text-gray-400">{col.data_length}</td>
                      <td className="px-2 py-1">{col.nullable === 'Y' ? <span className="text-yellow-400">Yes</span> : <span className="text-gray-500">No</span>}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}

          {/* Quick Actions */}
          <div>
            <h4 className="text-xs font-semibold text-gray-400 uppercase mb-2">Quick Actions</h4>
            <div className="flex flex-wrap gap-1.5">
              {cat?.selectTemplate && (
                <button onClick={() => setAndFocus(cat.selectTemplate!.replace(/\{schema\}/g, objectDetail.schema).replace(/\{name\}/g, objectDetail.name))}
                  className="px-2 py-1 bg-gray-700/50 text-gray-300 rounded text-[10px] hover:bg-gray-600/50 flex items-center gap-1">
                  <Play className="w-3 h-3" /> SELECT
                </button>
              )}
              {(objectDetail.type === 'tables' || objectDetail.type === 'views') && (
                <button onClick={() => setAndFocus(`SELECT COUNT(*) AS row_count FROM ${objectDetail.schema}.${objectDetail.name}`)}
                  className="px-2 py-1 bg-gray-700/50 text-gray-300 rounded text-[10px] hover:bg-gray-600/50 flex items-center gap-1">
                  <Hash className="w-3 h-3" /> COUNT
                </button>
              )}
              <button onClick={() => setAndFocus(`SELECT DBMS_METADATA.GET_DDL('${objectDetail.type === 'tables' ? 'TABLE' : objectDetail.type === 'views' ? 'VIEW' : objectDetail.type === 'mviews' ? 'MATERIALIZED_VIEW' : objectDetail.type.toUpperCase().replace(/S$/, '')}', '${objectDetail.name}', '${objectDetail.schema}') AS ddl FROM dual`)}
                className="px-2 py-1 bg-gray-700/50 text-gray-300 rounded text-[10px] hover:bg-gray-600/50 flex items-center gap-1">
                <Code2 className="w-3 h-3" /> DDL
              </button>
            </div>
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className="flex h-[calc(100vh-12rem)] gap-3">
      {/* Left: Schema Browser */}
      <div className="w-60 flex-shrink-0 bg-gray-800/50 border border-gray-700/50 rounded-xl overflow-hidden flex flex-col">
        <div className="flex items-center justify-between p-2 border-b border-gray-700/50">
          <h3 className="text-xs font-semibold text-gray-400 uppercase tracking-wide flex items-center gap-1.5">
            <Database className="w-3.5 h-3.5 text-blue-400" /> Schema Browser
          </h3>
        </div>
        <div className="flex-1 overflow-y-auto p-1.5 space-y-0.5 text-xs select-none">
          <button onClick={() => setSchemaOpen(!schemaOpen)} className="flex items-center gap-1 text-gray-300 hover:text-white w-full px-1 py-0.5 rounded hover:bg-gray-700/30">
            {schemaOpen ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
            <Columns3 className="w-3 h-3 text-yellow-400" /> Schemas ({schemas.length})
          </button>

          {schemaOpen && schemas.map(s => {
            const isSelected = selectedSchema === s.schema_name
            return (
              <div key={s.schema_name}>
                <button onClick={() => { setSelectedSchema(isSelected ? null : s.schema_name); setExpandedCategories(new Set()); setObjectData({}); setObjectCounts({}) }}
                  className={cn('flex items-center gap-1 pl-4 py-0.5 w-full rounded hover:bg-gray-700/50',
                    isSelected ? 'text-white bg-gray-700/50' : 'text-gray-400')}>
                  {isSelected ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
                  <Database className="w-3 h-3 text-orange-400" />
                  {s.schema_name}
                </button>

                {/* Object type categories under schema */}
                {isSelected && OBJECT_CATEGORIES.map(cat => {
                  const catKey = `${s.schema_name}.${cat.key}`
                  const isExpanded = expandedCategories.has(catKey)
                  const items = objectData[catKey] || []
                  const count = objectCounts[cat.key] ?? (isExpanded ? items.length : '...')

                  return (
                    <div key={cat.key}>
                      <button
                        onClick={() => toggleCategory(cat.key)}
                        onContextMenu={e => handleCategoryContextMenu(e, cat)}
                        className="flex items-center gap-1 pl-7 py-0.5 w-full rounded hover:bg-gray-700/40 text-gray-400 hover:text-white"
                      >
                        {isExpanded ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
                        <span className={cat.color}>{cat.icon}</span>
                        <span className="truncate">{cat.label}</span>
                        <span className="ml-auto text-[9px] text-gray-600 pr-1">({count})</span>
                      </button>

                      {isExpanded && items.map((item: any) => {
                        const name = item[cat.nameField] || item.object_name || 'unknown'
                        return (
                          <button key={name}
                            onClick={() => loadObjectDetail(cat, name, item)}
                            onContextMenu={e => handleObjectContextMenu(e, cat, name)}
                            className="flex items-center gap-1 pl-11 py-0.5 w-full text-gray-500 hover:text-white hover:bg-gray-700/30 rounded group"
                          >
                            <span className={cn('w-3 h-3 flex-shrink-0', cat.color)}>{cat.icon}</span>
                            <span className="truncate">{name}</span>
                            {item.status && item.status !== 'VALID' && <span className="ml-auto text-[8px] text-red-400 pr-1">INVALID</span>}
                            {item.object_type && cat.key === 'procedures' && <span className="ml-auto text-[8px] text-gray-600 pr-1">{item.object_type}</span>}
                          </button>
                        )
                      })}

                      {isExpanded && items.length === 0 && (
                        <div className="pl-12 py-0.5 text-[10px] text-gray-600 italic">No {cat.label.toLowerCase()}</div>
                      )}
                    </div>
                  )
                })}
              </div>
            )
          })}
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

      {/* Right: Editor + Results / Object Detail */}
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

        {/* Results or Object Detail */}
        {objectDetail && !result ? renderObjectDetail() : (
          <div className="flex-1 bg-gray-800/50 border border-gray-700/50 rounded-xl overflow-hidden flex flex-col min-h-0">
            {result ? (
              <>
                <div className="flex items-center justify-between px-3 py-1.5 border-b border-gray-700/50 flex-shrink-0">
                  <div className="flex items-center gap-3 text-xs">
                    {result.success ? (
                      <>
                        <span className="text-green-400">&#10003; {result.row_count} rows</span>
                        <span className="text-gray-500">{result.execution_time_ms}ms</span>
                      </>
                    ) : (
                      <span className="text-red-400">&#10007; Error</span>
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
                  <p className="text-[10px] text-gray-600 mt-1">Ctrl+Enter to execute &middot; Click objects to inspect &middot; Right-click for actions</p>
                </div>
              </div>
            )}
          </div>
        )}
      </div>

      {/* Context Menu */}
      {contextMenu && (
        <div className="fixed z-[9999] bg-gray-800 border border-gray-600 rounded-lg shadow-2xl py-1 min-w-[180px]"
          style={{ left: contextMenu.x, top: contextMenu.y }}
          onClick={e => e.stopPropagation()}>
          {contextMenu.items.map((item, i) => (
            <button key={i}
              onClick={() => { item.action(); setContextMenu(null) }}
              className={cn('flex items-center gap-2 w-full px-3 py-1.5 text-xs hover:bg-gray-700/60 text-left',
                item.danger ? 'text-red-400 hover:text-red-300' : 'text-gray-300 hover:text-white')}>
              {item.icon}
              {item.label}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}
