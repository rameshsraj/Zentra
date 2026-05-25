import { useState, useCallback } from 'react'
import PageHeader from '@/components/PageHeader'
import {
  SearchCode, Upload, FileText, ExternalLink,
  ChevronRight, Server, Loader2, ClipboardCopy, Lightbulb,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { analyzeRCA } from '@/lib/api'

interface RCAError {
  message: string
  count: number
  severity: 'critical' | 'error' | 'warning'
  pattern: string
  solutions: { title: string; description: string; source: string; url: string; confidence: number }[]
}

interface RCAResult {
  id: string
  source: string
  total_errors: number
  unique_errors: number
  errors: RCAError[]
  analyzed_at: string
}

const mockHistory: RCAResult[] = [
  {
    id: 'rca-1', source: '/var/log/nginx/error.log', total_errors: 42, unique_errors: 8,
    analyzed_at: new Date(Date.now() - 3600000).toISOString(),
    errors: [
      {
        message: 'upstream timed out (110: Connection timed out) while connecting to upstream',
        count: 18, severity: 'critical', pattern: 'Upstream Timeout',
        solutions: [
          { title: 'Increase proxy_read_timeout', description: 'Set proxy_read_timeout to 120s in nginx.conf upstream block', source: 'Stack Overflow', url: 'https://stackoverflow.com/q/18740635', confidence: 0.92 },
          { title: 'Check backend health', description: 'Verify the upstream backend server is responsive and not overloaded', source: 'Nginx Docs', url: 'https://nginx.org/en/docs/http/ngx_http_upstream_module.html', confidence: 0.88 },
        ],
      },
      {
        message: 'SSL_do_handshake() failed (SSL: error:14094438)',
        count: 7, severity: 'error', pattern: 'SSL Handshake Failure',
        solutions: [
          { title: 'Update SSL certificate chain', description: 'Ensure the full certificate chain is included in ssl_certificate directive', source: 'Documentation', url: 'https://nginx.org/en/docs/http/configuring_https_servers.html', confidence: 0.95 },
        ],
      },
      {
        message: 'client intended to send too large body: 10485760 bytes',
        count: 12, severity: 'warning', pattern: 'Request Body Too Large',
        solutions: [
          { title: 'Increase client_max_body_size', description: 'Set client_max_body_size 50m; in the server or location block', source: 'Stack Overflow', url: 'https://stackoverflow.com/q/2056124', confidence: 0.97 },
        ],
      },
    ],
  },
  {
    id: 'rca-2', source: '/var/log/postgresql/postgresql-15-main.log', total_errors: 15, unique_errors: 4,
    analyzed_at: new Date(Date.now() - 7200000).toISOString(),
    errors: [
      {
        message: 'FATAL: remaining connection slots are reserved for non-replication superuser connections',
        count: 8, severity: 'critical', pattern: 'Connection Exhaustion',
        solutions: [
          { title: 'Increase max_connections', description: 'Increase max_connections in postgresql.conf and restart. Consider using pgbouncer for connection pooling.', source: 'PostgreSQL Docs', url: 'https://www.postgresql.org/docs/current/runtime-config-connection.html', confidence: 0.90 },
          { title: 'Use PgBouncer', description: 'Deploy PgBouncer as a connection pooler to manage connection limits more efficiently', source: 'PgBouncer Docs', url: 'https://www.pgbouncer.org/config.html', confidence: 0.85 },
        ],
      },
      {
        message: 'ERROR: deadlock detected',
        count: 3, severity: 'error', pattern: 'Deadlock',
        solutions: [
          { title: 'Review transaction ordering', description: 'Ensure all transactions acquire locks in consistent order to prevent deadlocks', source: 'PostgreSQL Wiki', url: 'https://wiki.postgresql.org/wiki/Lock_Monitoring', confidence: 0.78 },
        ],
      },
    ],
  },
]

const sevColor = (s: string) => {
  switch (s) {
    case 'critical': return 'bg-red-500/20 text-red-400 border-red-500/30'
    case 'error': return 'bg-orange-500/20 text-orange-400 border-orange-500/30'
    case 'warning': return 'bg-amber-500/20 text-amber-400 border-amber-500/30'
    default: return 'bg-slate-500/20 text-slate-400 border-slate-500/30'
  }
}

export default function RCAPage() {
  const [logInput, setLogInput] = useState('')
  const [sourceType, setSourceType] = useState<'paste' | 'file' | 'remote'>('paste')
  const [remoteHost, setRemoteHost] = useState('')
  const [remotePath, setRemotePath] = useState('')
  const [isAnalyzing, setIsAnalyzing] = useState(false)
  const [results, setResults] = useState<RCAResult[]>(mockHistory)
  const [selectedResult, setSelectedResult] = useState<RCAResult | null>(null)
  const [selectedError, setSelectedError] = useState<RCAError | null>(null)
  const [search, setSearch] = useState('')

  const handleRefresh = useCallback(() => {}, [])

  const handleAnalyze = async () => {
    if (sourceType === 'paste' && !logInput.trim()) return
    if (sourceType === 'remote' && (!remoteHost || !remotePath)) return

    setIsAnalyzing(true)
    try {
      const logContent = sourceType === 'paste' ? logInput : sourceType === 'remote' ? `remote://${remoteHost}${remotePath}` : logInput
      const source = sourceType === 'paste' ? 'Pasted log content' : sourceType === 'remote' ? `${remoteHost}:${remotePath}` : 'Uploaded file'
      const newResult = await analyzeRCA(logContent, source) as unknown as RCAResult
      setResults(prev => [newResult, ...prev])
      setSelectedResult(newResult)
      setLogInput('')
    } catch (err) {
      console.error('RCA analysis failed', err)
    } finally {
      setIsAnalyzing(false)
    }
  }

  const filteredResults = results.filter(r =>
    !search || `${r.source} ${r.errors.map(e => e.message).join(' ')}`.toLowerCase().includes(search.toLowerCase())
  )

  return (
    <div className="space-y-6">
      <PageHeader
        title="Root Cause Analysis"
        subtitle="Upload logs from any source · AI-powered error extraction · Automated solution search"
        onRefresh={handleRefresh}
        onSearch={setSearch}
        searchPlaceholder="Search RCA history..."
      />

      {/* Input Section */}
      <div className="bg-white/5 border border-white/10 rounded-xl p-5">
        <h3 className="text-sm font-bold text-white uppercase tracking-wider mb-4 flex items-center gap-2">
          <Upload className="w-4 h-4 text-brand-400" />Log Source
        </h3>

        {/* Source Type Selector */}
        <div className="flex gap-2 mb-4">
          {([
            { key: 'paste', label: 'Paste Log', icon: ClipboardCopy },
            { key: 'file', label: 'Upload File', icon: FileText },
            { key: 'remote', label: 'Remote Server', icon: Server },
          ] as const).map(t => (
            <button key={t.key} onClick={() => setSourceType(t.key)}
              className={cn('flex items-center gap-1.5 px-4 py-2 rounded-xl text-xs font-medium border transition-colors',
                sourceType === t.key ? 'bg-brand-600/20 text-brand-400 border-brand-500/30' : 'bg-white/5 text-slate-500 border-white/10 hover:text-white'
              )}>
              <t.icon className="w-3.5 h-3.5" />{t.label}
            </button>
          ))}
        </div>

        {/* Paste */}
        {sourceType === 'paste' && (
          <textarea
            value={logInput}
            onChange={e => setLogInput(e.target.value)}
            placeholder="Paste your log content here...&#10;&#10;Example:&#10;2024-01-15 10:23:45 ERROR: upstream timed out (110: Connection timed out)&#10;2024-01-15 10:24:01 FATAL: remaining connection slots are reserved"
            className="w-full h-40 bg-black/20 border border-white/10 rounded-xl p-4 text-xs font-mono text-slate-300 placeholder-slate-600 focus:outline-none focus:ring-2 focus:ring-brand-500/50 resize-none"
          />
        )}

        {/* File Upload */}
        {sourceType === 'file' && (
          <div className="border-2 border-dashed border-white/10 rounded-xl p-8 text-center">
            <Upload className="w-8 h-8 text-slate-600 mx-auto mb-2" />
            <p className="text-sm text-slate-400">Drop log files here or click to browse</p>
            <p className="text-[10px] text-slate-600 mt-1">Supports .log, .txt, .gz files up to 50MB</p>
            <input type="file" className="hidden" accept=".log,.txt,.gz" />
            <button className="mt-3 px-4 py-2 bg-white/5 border border-white/10 rounded-xl text-xs text-slate-400 hover:text-white transition-colors">Browse Files</button>
          </div>
        )}

        {/* Remote */}
        {sourceType === 'remote' && (
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            <div>
              <label className="text-[10px] text-slate-500 uppercase tracking-wider mb-1 block">Server Host</label>
              <input type="text" value={remoteHost} onChange={e => setRemoteHost(e.target.value)}
                placeholder="e.g., 10.88.11.162" className="w-full bg-black/20 border border-white/10 rounded-xl px-4 py-2.5 text-sm text-white placeholder-slate-600 focus:outline-none focus:ring-2 focus:ring-brand-500/50" />
            </div>
            <div>
              <label className="text-[10px] text-slate-500 uppercase tracking-wider mb-1 block">Log File Path</label>
              <input type="text" value={remotePath} onChange={e => setRemotePath(e.target.value)}
                placeholder="e.g., /var/log/nginx/error.log" className="w-full bg-black/20 border border-white/10 rounded-xl px-4 py-2.5 text-sm text-white placeholder-slate-600 focus:outline-none focus:ring-2 focus:ring-brand-500/50" />
            </div>
          </div>
        )}

        <button onClick={handleAnalyze} disabled={isAnalyzing}
          className={cn('mt-4 flex items-center gap-2 px-6 py-2.5 rounded-xl text-sm font-medium transition-colors',
            isAnalyzing ? 'bg-slate-700 text-slate-400 cursor-not-allowed' : 'bg-brand-600 text-white hover:bg-brand-700 shadow-lg shadow-brand-600/20'
          )}>
          {isAnalyzing ? <><Loader2 className="w-4 h-4 animate-spin" />Analyzing...</> : <><SearchCode className="w-4 h-4" />Analyze Logs</>}
        </button>
      </div>

      {/* Results */}
      <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
        {/* History List */}
        <div className="lg:col-span-4 space-y-3">
          <h3 className="text-sm font-bold text-white uppercase tracking-wider">Analysis History</h3>
          {filteredResults.map(r => (
            <div key={r.id} onClick={() => { setSelectedResult(r); setSelectedError(null) }}
              className={cn('bg-white/5 border rounded-xl p-4 cursor-pointer transition-colors',
                selectedResult?.id === r.id ? 'border-brand-500/30 bg-brand-600/10' : 'border-white/10 hover:bg-white/[0.08]'
              )}>
              <div className="flex items-center justify-between mb-1">
                <span className="text-xs font-mono text-slate-300 truncate flex-1">{r.source}</span>
                <ChevronRight className="w-3.5 h-3.5 text-slate-600 flex-shrink-0" />
              </div>
              <div className="flex items-center gap-3 text-[10px] text-slate-500">
                <span className="text-red-400 font-semibold">{r.total_errors} errors</span>
                <span>{r.unique_errors} unique</span>
                <span className="ml-auto">{new Date(r.analyzed_at).toLocaleString()}</span>
              </div>
            </div>
          ))}
        </div>

        {/* Error Detail */}
        <div className="lg:col-span-8">
          {selectedResult ? (
            <div className="space-y-4">
              <div className="bg-white/5 border border-white/10 rounded-xl p-4">
                <div className="flex items-center gap-2 mb-1">
                  <FileText className="w-4 h-4 text-brand-400" />
                  <span className="text-sm font-bold text-white">{selectedResult.source}</span>
                </div>
                <div className="flex gap-4 text-[10px] text-slate-500">
                  <span>Total: <strong className="text-red-400">{selectedResult.total_errors}</strong></span>
                  <span>Unique: <strong className="text-amber-400">{selectedResult.unique_errors}</strong></span>
                  <span>Analyzed: {new Date(selectedResult.analyzed_at).toLocaleString()}</span>
                </div>
              </div>

              {/* Error Cards */}
              <div className="space-y-3">
                {selectedResult.errors.map((err, i) => (
                  <div key={i} className={cn('bg-white/5 border rounded-xl overflow-hidden transition-colors',
                    selectedError === err ? 'border-brand-500/30' : 'border-white/10'
                  )}>
                    <div className="p-4 cursor-pointer" onClick={() => setSelectedError(selectedError === err ? null : err)}>
                      <div className="flex items-start gap-3">
                        <span className={cn('px-2 py-0.5 rounded border text-[10px] font-semibold uppercase flex-shrink-0', sevColor(err.severity))}>
                          {err.severity}
                        </span>
                        <div className="flex-1 min-w-0">
                          <code className="text-xs font-mono text-white block truncate">{err.message}</code>
                          <div className="flex items-center gap-3 mt-1 text-[10px] text-slate-500">
                            <span>Pattern: <strong className="text-slate-300">{err.pattern}</strong></span>
                            <span>Count: <strong className="text-red-400">{err.count}</strong></span>
                            <span>{err.solutions.length} solutions</span>
                          </div>
                        </div>
                        <ChevronRight className={cn('w-4 h-4 text-slate-600 transition-transform flex-shrink-0', selectedError === err && 'rotate-90')} />
                      </div>
                    </div>

                    {/* Solutions */}
                    {selectedError === err && (
                      <div className="border-t border-white/10 bg-black/20 p-4 space-y-3">
                        <h4 className="text-xs font-bold text-white uppercase tracking-wider flex items-center gap-1.5">
                          <Lightbulb className="w-3.5 h-3.5 text-amber-400" />Recommended Solutions
                        </h4>
                        {err.solutions.map((sol, j) => (
                          <div key={j} className="bg-white/5 border border-white/10 rounded-lg p-3">
                            <div className="flex items-center justify-between mb-1">
                              <span className="text-xs font-semibold text-white">{sol.title}</span>
                              <div className="flex items-center gap-2">
                                <span className="text-[10px] text-emerald-400 font-mono">{Math.round(sol.confidence * 100)}%</span>
                                {sol.url !== '#' && (
                                  <a href={sol.url} target="_blank" rel="noopener" className="text-brand-400 hover:text-brand-300">
                                    <ExternalLink className="w-3 h-3" />
                                  </a>
                                )}
                              </div>
                            </div>
                            <p className="text-[10px] text-slate-400">{sol.description}</p>
                            <span className="text-[10px] text-slate-600 mt-1 block">Source: {sol.source}</span>
                          </div>
                        ))}
                      </div>
                    )}
                  </div>
                ))}
              </div>
            </div>
          ) : (
            <div className="bg-white/5 border border-white/10 rounded-xl text-center py-16">
              <SearchCode className="w-12 h-12 text-slate-600 mx-auto mb-3" />
              <p className="text-slate-400">Select an analysis from the history or submit new logs for analysis</p>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
