interface StateTimelineRow {
  label: string
  states: { start: number; end: number; state: string; color?: string }[]
}

interface StateTimelineProps {
  rows: StateTimelineRow[]
  startTime: number
  endTime: number
  height?: number
  stateColors?: Record<string, string>
}

const DEFAULT_COLORS: Record<string, string> = {
  ok: '#10b981', up: '#10b981', healthy: '#10b981', active: '#10b981',
  warning: '#f59e0b', degraded: '#f59e0b',
  critical: '#ef4444', down: '#ef4444', error: '#ef4444',
  unknown: '#6b7280', maintenance: '#8b5cf6',
}

export default function StateTimeline({ rows, startTime, endTime, height = 200, stateColors }: StateTimelineProps) {
  const totalDuration = endTime - startTime || 1
  const colors = { ...DEFAULT_COLORS, ...stateColors }
  const rowHeight = Math.min(30, (height - 20) / Math.max(rows.length, 1))

  return (
    <div style={{ height }} className="overflow-y-auto">
      {rows.map((row, ri) => (
        <div key={ri} className="flex items-center gap-2 mb-1">
          <span className="text-[10px] text-slate-400 w-20 truncate flex-shrink-0">{row.label}</span>
          <div className="flex-1 relative bg-slate-700/50 rounded overflow-hidden" style={{ height: rowHeight }}>
            {row.states.map((seg, si) => {
              const left = ((seg.start - startTime) / totalDuration) * 100
              const width = ((seg.end - seg.start) / totalDuration) * 100
              const bg = seg.color || colors[seg.state.toLowerCase()] || '#6b7280'
              return (
                <div
                  key={si}
                  className="absolute top-0 h-full transition-all"
                  style={{ left: `${left}%`, width: `${Math.max(width, 0.5)}%`, backgroundColor: bg }}
                  title={`${seg.state}: ${new Date(seg.start).toLocaleTimeString()} – ${new Date(seg.end).toLocaleTimeString()}`}
                />
              )
            })}
          </div>
        </div>
      ))}
    </div>
  )
}
