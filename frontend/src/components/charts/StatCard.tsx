import { type LucideIcon, TrendingUp, TrendingDown, Minus } from 'lucide-react'
import Sparkline from './Sparkline'
import { cn } from '@/lib/utils'

interface StatCardProps {
  title: string
  value: string | number
  subtitle?: string
  icon?: LucideIcon
  iconColor?: string
  trend?: number
  trendLabel?: string
  sparkData?: number[]
  sparkColor?: string
  onClick?: () => void
  className?: string
  size?: 'sm' | 'md' | 'lg'
}

export default function StatCard({
  title,
  value,
  subtitle,
  icon: Icon,
  iconColor = 'text-brand-400',
  trend,
  trendLabel,
  sparkData,
  sparkColor = '#3b82f6',
  onClick,
  className,
  size = 'md',
}: StatCardProps) {
  const TrendIcon = trend && trend > 0 ? TrendingUp : trend && trend < 0 ? TrendingDown : Minus
  const trendColor = trend && trend > 0 ? 'text-emerald-400' : trend && trend < 0 ? 'text-red-400' : 'text-slate-400'

  return (
    <div
      className={cn(
        'bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl transition-all hover:bg-white/[0.08]',
        onClick && 'cursor-pointer hover:border-brand-500/30',
        size === 'sm' ? 'p-3' : size === 'lg' ? 'p-6' : 'p-4',
        className
      )}
      onClick={onClick}
    >
      <div className="flex items-start justify-between mb-2">
        <div className="text-[10px] text-slate-400 uppercase tracking-wider font-medium">{title}</div>
        {Icon && (
          <div className="w-8 h-8 rounded-lg bg-white/5 flex items-center justify-center">
            <Icon className={cn('w-4 h-4', iconColor)} />
          </div>
        )}
      </div>

      <div className={cn('font-bold font-mono text-white', size === 'lg' ? 'text-3xl' : size === 'sm' ? 'text-lg' : 'text-2xl')}>
        {value}
      </div>

      <div className="flex items-center justify-between mt-1">
        <div className="flex items-center gap-2">
          {trend !== undefined && (
            <span className={cn('flex items-center gap-0.5 text-xs font-medium', trendColor)}>
              <TrendIcon className="w-3 h-3" />
              {Math.abs(trend)}%
            </span>
          )}
          {(subtitle || trendLabel) && (
            <span className="text-[10px] text-slate-500">{trendLabel || subtitle}</span>
          )}
        </div>
        {sparkData && sparkData.length > 0 && (
          <Sparkline data={sparkData} width={60} height={16} color={sparkColor} />
        )}
      </div>
    </div>
  )
}
