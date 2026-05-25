import { cn } from '@/lib/utils'

interface TimelineEvent {
  id: string
  title: string
  description?: string
  timestamp: string
  iconColor?: string
  severity?: 'critical' | 'high' | 'warning' | 'info' | 'success'
}

interface TimelineListProps {
  events: TimelineEvent[]
  maxItems?: number
  className?: string
}

const severityColors: Record<string, string> = {
  critical: 'bg-red-500',
  high: 'bg-orange-500',
  warning: 'bg-amber-500',
  info: 'bg-blue-500',
  success: 'bg-emerald-500',
}

export default function TimelineList({ events, maxItems = 10, className }: TimelineListProps) {
  const items = events.slice(0, maxItems)

  return (
    <div className={cn('relative', className)}>
      {items.map((event, i) => {
        const dotColor = severityColors[event.severity ?? 'info'] ?? severityColors.info
        return (
          <div key={event.id} className="flex gap-3 relative">
            {/* Vertical line */}
            {i < items.length - 1 && (
              <div className="absolute left-[9px] top-6 w-px h-[calc(100%-12px)] bg-white/10" />
            )}
            {/* Dot */}
            <div className={cn('w-[18px] h-[18px] rounded-full flex items-center justify-center flex-shrink-0 mt-0.5', dotColor + '/20')}>
              <div className={cn('w-2 h-2 rounded-full', dotColor)} />
            </div>
            {/* Content */}
            <div className="flex-1 pb-4">
              <div className="flex items-center justify-between">
                <span className="text-xs font-medium text-white">{event.title}</span>
                <span className="text-[10px] text-slate-500">{formatTime(event.timestamp)}</span>
              </div>
              {event.description && (
                <p className="text-[11px] text-slate-400 mt-0.5">{event.description}</p>
              )}
            </div>
          </div>
        )
      })}
    </div>
  )
}

function formatTime(ts: string): string {
  try {
    const d = new Date(ts)
    const now = new Date()
    const diffMs = now.getTime() - d.getTime()
    if (diffMs < 60000) return 'just now'
    if (diffMs < 3600000) return `${Math.floor(diffMs / 60000)}m ago`
    if (diffMs < 86400000) return `${Math.floor(diffMs / 3600000)}h ago`
    return d.toLocaleDateString()
  } catch {
    return ts
  }
}
