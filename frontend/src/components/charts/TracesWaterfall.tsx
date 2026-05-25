import { cn } from '@/lib/utils'

interface Span {
  spanId: string
  operationName: string
  serviceName: string
  startTime: number
  duration: number
  status?: 'ok' | 'error' | 'unset'
  depth?: number
  tags?: Record<string, string>
}

interface TracesWaterfallProps {
  spans: Span[]
  traceStartTime: number
  traceDuration: number
  onSpanClick?: (span: Span) => void
  selectedSpanId?: string
}

const SERVICE_COLORS = [
  '#3b82f6', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6',
  '#ec4899', '#06b6d4', '#84cc16', '#f97316',
]

export default function TracesWaterfall({ spans, traceStartTime, traceDuration, onSpanClick, selectedSpanId }: TracesWaterfallProps) {
  const serviceColorMap = new Map<string, string>()
  let colorIdx = 0
  spans.forEach(s => {
    if (!serviceColorMap.has(s.serviceName)) {
      serviceColorMap.set(s.serviceName, SERVICE_COLORS[colorIdx % SERVICE_COLORS.length])
      colorIdx++
    }
  })

  const totalDuration = traceDuration || 1

  return (
    <div className="space-y-0.5">
      {/* Header */}
      <div className="flex items-center gap-2 px-2 py-1 text-[10px] text-slate-500 uppercase border-b border-white/10">
        <span className="w-48 flex-shrink-0">Operation</span>
        <span className="flex-1 text-right">{(totalDuration / 1000).toFixed(1)}ms total</span>
      </div>

      {/* Spans */}
      {spans.map((span) => {
        const left = ((span.startTime - traceStartTime) / totalDuration) * 100
        const width = Math.max((span.duration / totalDuration) * 100, 0.5)
        const color = serviceColorMap.get(span.serviceName) || '#6b7280'
        const depth = span.depth || 0
        const isSelected = span.spanId === selectedSpanId

        return (
          <div
            key={span.spanId}
            className={cn(
              'flex items-center gap-2 px-2 py-1 cursor-pointer hover:bg-white/5 transition-colors rounded',
              isSelected && 'bg-white/10 ring-1 ring-cyan-500/30'
            )}
            style={{ paddingLeft: `${8 + depth * 16}px` }}
            onClick={() => onSpanClick?.(span)}
          >
            <span className="w-44 flex-shrink-0 truncate text-xs text-slate-300 flex items-center gap-1.5">
              <span className="w-2 h-2 rounded-full flex-shrink-0" style={{ backgroundColor: color }} />
              <span className="truncate">{span.operationName}</span>
            </span>
            <div className="flex-1 h-5 relative bg-slate-700/30 rounded overflow-hidden">
              <div
                className="absolute top-0.5 bottom-0.5 rounded transition-all"
                style={{
                  left: `${left}%`,
                  width: `${Math.max(width, 0.3)}%`,
                  backgroundColor: span.status === 'error' ? '#ef4444' : color,
                }}
              />
            </div>
            <span className="text-[10px] font-mono text-slate-400 w-16 text-right flex-shrink-0">
              {span.duration < 1000 ? `${span.duration}µs` : `${(span.duration / 1000).toFixed(1)}ms`}
            </span>
          </div>
        )
      })}

      {/* Service Legend */}
      <div className="flex flex-wrap gap-3 pt-2 px-2 border-t border-white/5">
        {Array.from(serviceColorMap.entries()).map(([svc, color]) => (
          <span key={svc} className="flex items-center gap-1 text-[10px] text-slate-400">
            <span className="w-2 h-2 rounded-full" style={{ backgroundColor: color }} />
            {svc}
          </span>
        ))}
      </div>
    </div>
  )
}
