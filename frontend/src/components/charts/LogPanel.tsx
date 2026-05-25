import { useState, useEffect, useRef } from 'react'
import { cn } from '@/lib/utils'
import { Search, ArrowDown } from 'lucide-react'

interface LogLine {
  timestamp: string
  level: 'debug' | 'info' | 'warn' | 'error' | 'fatal' | string
  message: string
  source?: string
  fields?: Record<string, string>
}

interface LogPanelProps {
  logs: LogLine[]
  height?: number
  liveTail?: boolean
  onSearch?: (query: string) => void
}

const LEVEL_COLORS: Record<string, string> = {
  debug: 'text-slate-400 bg-slate-500/10',
  info: 'text-blue-400 bg-blue-500/10',
  warn: 'text-amber-400 bg-amber-500/10',
  warning: 'text-amber-400 bg-amber-500/10',
  error: 'text-red-400 bg-red-500/10',
  fatal: 'text-red-300 bg-red-600/20',
  critical: 'text-red-300 bg-red-600/20',
}

export default function LogPanel({ logs, height = 400, liveTail = false, onSearch }: LogPanelProps) {
  const [filter, setFilter] = useState('')
  const [levelFilter, setLevelFilter] = useState<string | null>(null)
  const endRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (liveTail && endRef.current) {
      endRef.current.scrollIntoView({ behavior: 'smooth' })
    }
  }, [logs.length, liveTail])

  const filtered = logs.filter(log => {
    if (levelFilter && log.level !== levelFilter) return false
    if (filter && !log.message.toLowerCase().includes(filter.toLowerCase())) return false
    return true
  })

  return (
    <div className="flex flex-col" style={{ height }}>
      {/* Toolbar */}
      <div className="flex items-center gap-2 px-3 py-2 border-b border-white/10 flex-shrink-0">
        <div className="relative flex-1">
          <Search className="absolute left-2 top-1/2 -translate-y-1/2 w-3.5 h-3.5 text-slate-500" />
          <input
            type="text"
            value={filter}
            onChange={e => { setFilter(e.target.value); onSearch?.(e.target.value) }}
            placeholder="Filter logs..."
            className="w-full pl-7 pr-3 py-1 bg-slate-900 border border-white/10 rounded text-xs text-white placeholder-slate-500 focus:outline-none focus:border-cyan-500/30"
          />
        </div>
        <div className="flex gap-0.5">
          {['debug', 'info', 'warn', 'error'].map(lv => (
            <button
              key={lv}
              onClick={() => setLevelFilter(levelFilter === lv ? null : lv)}
              className={cn(
                'px-2 py-0.5 rounded text-[10px] font-bold uppercase transition-colors',
                levelFilter === lv ? LEVEL_COLORS[lv] : 'text-slate-500 hover:text-slate-300'
              )}
            >
              {lv}
            </button>
          ))}
        </div>
        <span className="text-[10px] text-slate-500">{filtered.length} / {logs.length}</span>
      </div>

      {/* Log Lines */}
      <div className="flex-1 overflow-y-auto font-mono text-[11px] leading-5">
        {filtered.map((log, i) => (
          <div key={i} className="flex items-start gap-2 px-3 py-0.5 hover:bg-white/5 border-b border-white/[0.02]">
            <span className="text-slate-500 flex-shrink-0 w-20">{log.timestamp}</span>
            <span className={cn('px-1 py-0 rounded text-[9px] font-bold uppercase flex-shrink-0 w-10 text-center', LEVEL_COLORS[log.level] || 'text-slate-400')}>
              {log.level}
            </span>
            {log.source && <span className="text-cyan-500/70 flex-shrink-0 w-16 truncate">{log.source}</span>}
            <span className={cn('flex-1', log.level === 'error' || log.level === 'fatal' ? 'text-red-300' : 'text-slate-300')}>
              {log.message}
            </span>
          </div>
        ))}
        <div ref={endRef} />
      </div>

      {/* Live tail indicator */}
      {liveTail && (
        <div className="flex items-center gap-1 px-3 py-1 border-t border-white/10 text-[10px] text-green-400 flex-shrink-0">
          <ArrowDown className="w-3 h-3 animate-bounce" /> Live tail active
        </div>
      )}
    </div>
  )
}
