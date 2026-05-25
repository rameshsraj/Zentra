import React from 'react'
import { cn } from '@/lib/utils'

interface Series {
  key: string
  label: string
  color: string
}

interface StackedAreaChartProps {
  data: Record<string, number | string>[]
  timeKey?: string
  series: Series[]
  height?: number
  showLegend?: boolean
  className?: string
}

const StackedAreaChart = React.memo(function StackedAreaChart({
  data,
  series,
  height = 200,
  showLegend = true,
  className,
}: StackedAreaChartProps) {
  if (data.length === 0) return null

  const maxSum = Math.max(...data.map(row =>
    series.reduce((sum, s) => sum + (Number(row[s.key]) || 0), 0)
  ), 1)

  const w = 600
  const h = height
  const padX = 0
  const padY = 4
  const plotW = w - padX * 2
  const plotH = h - padY * 2

  const points = data.map((row, i) => {
    const x = padX + (i / Math.max(data.length - 1, 1)) * plotW
    let cumY = 0
    const layers = series.map((_s, sIdx) => {
      const v = Number(row[series[sIdx].key]) || 0
      cumY += v
      return { key: series[sIdx].key, y: padY + plotH - (cumY / maxSum) * plotH }
    })
    return { x, layers }
  })

  const paths = series.map((_s, si) => {
    const topLine = points.map(p => `${p.x},${p.layers[si].y}`).join(' L ')
    const bottomLine = si === 0
      ? points.map(p => `${p.x},${padY + plotH}`).reverse().join(' L ')
      : points.map(p => `${p.x},${p.layers[si - 1].y}`).reverse().join(' L ')
    return `M ${topLine} L ${bottomLine} Z`
  })

  return (
    <div className={cn('w-full', className)}>
      <svg width="100%" viewBox={`0 0 ${w} ${h}`} preserveAspectRatio="none">
        {paths.map((d, i) => (
          <path key={series[i].key} d={d} fill={series[i].color} opacity={0.6} />
        ))}
        {series.map((s, si) => {
          const line = points.map(p => `${p.x},${p.layers[si].y}`).join(' L ')
          return <path key={s.key + '-line'} d={`M ${line}`} fill="none" stroke={s.color} strokeWidth={1.5} />
        })}
      </svg>
      {showLegend && (
        <div className="flex items-center justify-center gap-4 mt-2">
          {series.map(s => (
            <div key={s.key} className="flex items-center gap-1.5 text-[10px]">
              <div className="w-2.5 h-2.5 rounded-sm" style={{ backgroundColor: s.color }} />
              <span className="text-slate-400">{s.label}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  )
})

export default StackedAreaChart
