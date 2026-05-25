import React from 'react'
import { cn } from '@/lib/utils'

interface BarChartItem {
  label: string
  value: number
  color?: string
  sub?: string
}

interface BarChartProps {
  data: BarChartItem[]
  maxValue?: number
  showValues?: boolean
  horizontal?: boolean
  height?: number
  className?: string
}

const BarChart = React.memo(function BarChart({
  data,
  maxValue,
  showValues = true,
  horizontal = true,
  height = 24,
  className,
}: BarChartProps) {
  const max = maxValue ?? Math.max(...data.map(d => d.value), 1)

  if (horizontal) {
    return (
      <div className={cn('space-y-3', className)}>
        {data.map((item, i) => (
          <div key={i}>
            <div className="flex items-center justify-between mb-1">
              <div className="flex items-center gap-2">
                <span className="text-xs font-medium text-white">{item.label}</span>
                {item.sub && <span className="text-[10px] text-slate-500">{item.sub}</span>}
              </div>
              {showValues && (
                <span className="text-xs font-mono text-slate-300">{item.value.toLocaleString()}</span>
              )}
            </div>
            <div className="h-2 bg-slate-800 rounded-full overflow-hidden">
              <div
                className="h-full rounded-full transition-all duration-700"
                style={{
                  width: `${Math.min((item.value / max) * 100, 100)}%`,
                  backgroundColor: item.color ?? '#3b82f6',
                }}
              />
            </div>
          </div>
        ))}
      </div>
    )
  }

  return (
    <div className={cn('flex items-end gap-2', className)} style={{ height }}>
      {data.map((item, i) => (
        <div key={i} className="flex-1 flex flex-col items-center gap-1">
          <div className="w-full relative" style={{ height: height - 20 }}>
            <div
              className="absolute bottom-0 w-full rounded-t transition-all duration-700"
              style={{
                height: `${Math.min((item.value / max) * 100, 100)}%`,
                backgroundColor: item.color ?? '#3b82f6',
              }}
            />
          </div>
          <span className="text-[10px] text-slate-500 truncate max-w-full">{item.label}</span>
        </div>
      ))}
    </div>
  )
})

export default BarChart
