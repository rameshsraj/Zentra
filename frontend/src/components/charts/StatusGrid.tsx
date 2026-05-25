import { cn } from '@/lib/utils'
import { type LucideIcon, CheckCircle, XCircle, AlertTriangle, HelpCircle } from 'lucide-react'

interface StatusItem {
  label: string
  status: 'healthy' | 'warning' | 'critical' | 'unknown' | 'offline'
  subtitle?: string
  icon?: LucideIcon
  onClick?: () => void
}

interface StatusGridProps {
  items: StatusItem[]
  columns?: number
  className?: string
}

const statusConfig: Record<string, { icon: LucideIcon; bg: string; text: string; border: string; dot: string }> = {
  healthy: { icon: CheckCircle, bg: 'bg-emerald-500/10', text: 'text-emerald-400', border: 'border-emerald-500/20', dot: 'bg-emerald-500' },
  warning: { icon: AlertTriangle, bg: 'bg-amber-500/10', text: 'text-amber-400', border: 'border-amber-500/20', dot: 'bg-amber-500' },
  critical: { icon: XCircle, bg: 'bg-red-500/10', text: 'text-red-400', border: 'border-red-500/20', dot: 'bg-red-500' },
  unknown: { icon: HelpCircle, bg: 'bg-slate-500/10', text: 'text-slate-400', border: 'border-slate-500/20', dot: 'bg-slate-500' },
  offline: { icon: XCircle, bg: 'bg-slate-500/10', text: 'text-slate-500', border: 'border-slate-500/20', dot: 'bg-slate-600' },
}

export default function StatusGrid({ items, columns = 4, className }: StatusGridProps) {
  return (
    <div
      className={cn('grid gap-3', className)}
      style={{ gridTemplateColumns: `repeat(${columns}, minmax(0, 1fr))` }}
    >
      {items.map((item, i) => {
        const cfg = statusConfig[item.status] ?? statusConfig.unknown
        const Icon = item.icon ?? cfg.icon
        return (
          <div
            key={i}
            onClick={item.onClick}
            className={cn(
              'rounded-xl p-3 border flex items-center gap-3 transition-all',
              cfg.bg, cfg.border,
              item.onClick && 'cursor-pointer hover:bg-white/[0.08]'
            )}
          >
            <div className={cn('w-2 h-2 rounded-full flex-shrink-0', cfg.dot)} />
            <div className="flex-1 min-w-0">
              <div className="text-xs font-medium text-white truncate">{item.label}</div>
              {item.subtitle && <div className="text-[10px] text-slate-500 truncate">{item.subtitle}</div>}
            </div>
            <Icon className={cn('w-4 h-4 flex-shrink-0', cfg.text)} />
          </div>
        )
      })}
    </div>
  )
}
