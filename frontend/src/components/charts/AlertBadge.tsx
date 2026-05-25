import { cn } from '@/lib/utils'
import { AlertTriangle, XCircle, AlertCircle, Info, CheckCircle } from 'lucide-react'

interface AlertBadgeProps {
  severity: 'critical' | 'high' | 'warning' | 'info' | 'success'
  count?: number
  label?: string
  size?: 'sm' | 'md' | 'lg'
  onClick?: () => void
  className?: string
}

const config = {
  critical: { icon: XCircle, bg: 'bg-red-500/10', border: 'border-red-500/30', text: 'text-red-400', pulse: true },
  high: { icon: AlertTriangle, bg: 'bg-orange-500/10', border: 'border-orange-500/30', text: 'text-orange-400', pulse: false },
  warning: { icon: AlertCircle, bg: 'bg-amber-500/10', border: 'border-amber-500/30', text: 'text-amber-400', pulse: false },
  info: { icon: Info, bg: 'bg-blue-500/10', border: 'border-blue-500/30', text: 'text-blue-400', pulse: false },
  success: { icon: CheckCircle, bg: 'bg-emerald-500/10', border: 'border-emerald-500/30', text: 'text-emerald-400', pulse: false },
}

export default function AlertBadge({ severity, count, label, size = 'md', onClick, className }: AlertBadgeProps) {
  const c = config[severity]
  const Icon = c.icon
  const sizes = { sm: 'px-2 py-1 text-xs', md: 'px-3 py-1.5 text-sm', lg: 'px-4 py-2 text-sm' }

  return (
    <div
      onClick={onClick}
      className={cn(
        'inline-flex items-center gap-1.5 rounded-lg border font-medium',
        c.bg, c.border, c.text,
        sizes[size],
        onClick && 'cursor-pointer hover:bg-white/10',
        className
      )}
    >
      <Icon className={cn('w-3.5 h-3.5', c.pulse && 'animate-pulse')} />
      {count !== undefined && <span className="font-mono font-bold">{count}</span>}
      {label && <span>{label}</span>}
    </div>
  )
}
