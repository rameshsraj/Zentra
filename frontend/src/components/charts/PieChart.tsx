import React from 'react'
import { cn } from '@/lib/utils'

interface PieSlice {
  label: string
  value: number
  color: string
}

interface PieChartProps {
  data: PieSlice[]
  size?: number
  showLegend?: boolean
  className?: string
}

const PieChart = React.memo(function PieChart({ data, size = 160, showLegend = true, className }: PieChartProps) {
  const total = data.reduce((sum, d) => sum + d.value, 0)
  if (total === 0) return null

  const radius = size / 2
  const cx = radius
  const cy = radius
  const r = radius - 4

  let startAngle = -90

  const slices = data.map(slice => {
    const pct = slice.value / total
    const angle = pct * 360
    const endAngle = startAngle + angle

    const start = polarToCartesian(cx, cy, r, startAngle)
    const end = polarToCartesian(cx, cy, r, endAngle)
    const largeArc = angle > 180 ? 1 : 0

    const d = `M ${cx} ${cy} L ${start.x} ${start.y} A ${r} ${r} 0 ${largeArc} 1 ${end.x} ${end.y} Z`

    startAngle = endAngle
    return { ...slice, d, pct }
  })

  return (
    <div className={cn('flex items-center gap-4', className)}>
      <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`}>
        {slices.map((s, i) => (
          <path key={i} d={s.d} fill={s.color} stroke="rgba(0,0,0,0.3)" strokeWidth={1}>
            <title>{s.label}: {s.value.toLocaleString()} ({(s.pct * 100).toFixed(1)}%)</title>
          </path>
        ))}
      </svg>
      {showLegend && (
        <div className="space-y-1.5">
          {slices.map((s, i) => (
            <div key={i} className="flex items-center gap-2 text-xs">
              <div className="w-2.5 h-2.5 rounded-sm flex-shrink-0" style={{ backgroundColor: s.color }} />
              <span className="text-slate-300">{s.label}</span>
              <span className="text-slate-500 ml-auto font-mono">{(s.pct * 100).toFixed(0)}%</span>
            </div>
          ))}
        </div>
      )}
    </div>
  )
})

export default PieChart

function polarToCartesian(cx: number, cy: number, r: number, angleInDeg: number) {
  const rad = (angleInDeg * Math.PI) / 180
  return { x: cx + r * Math.cos(rad), y: cy + r * Math.sin(rad) }
}
