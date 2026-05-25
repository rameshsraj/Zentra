import { cn } from '@/lib/utils'

interface ProgressBarProps {
  value: number
  max?: number
  label?: string
  showPercentage?: boolean
  size?: 'xs' | 'sm' | 'md' | 'lg'
  color?: string
  warningThreshold?: number
  criticalThreshold?: number
  className?: string
}

export default function ProgressBar({
  value,
  max = 100,
  label,
  showPercentage = true,
  size = 'md',
  color,
  warningThreshold = 70,
  criticalThreshold = 85,
  className,
}: ProgressBarProps) {
  const pct = Math.min((value / max) * 100, 100)
  const barColor = color ?? (pct >= criticalThreshold ? '#ef4444' : pct >= warningThreshold ? '#f59e0b' : '#22c55e')
  const heights = { xs: 'h-1', sm: 'h-1.5', md: 'h-2', lg: 'h-3' }

  return (
    <div className={cn('w-full', className)}>
      {(label || showPercentage) && (
        <div className="flex items-center justify-between mb-1">
          {label && <span className="text-xs text-slate-400">{label}</span>}
          {showPercentage && <span className="text-xs font-mono text-slate-300">{pct.toFixed(0)}%</span>}
        </div>
      )}
      <div className={cn('w-full bg-slate-800 rounded-full overflow-hidden', heights[size])}>
        <div
          className="h-full rounded-full transition-all duration-700"
          style={{ width: `${pct}%`, backgroundColor: barColor }}
        />
      </div>
    </div>
  )
}
