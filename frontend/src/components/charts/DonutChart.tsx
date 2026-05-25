import React from 'react'

interface Segment {
  label: string
  value: number
  color: string
}

interface DonutChartProps {
  segments: Segment[]
  size?: number
  thickness?: number
  centerLabel?: string
  centerValue?: string
  className?: string
}

const DonutChart = React.memo(function DonutChart({
  segments,
  size = 160,
  thickness = 18,
  centerLabel,
  centerValue,
  className = '',
}: DonutChartProps) {
  const total = segments.reduce((sum, s) => sum + s.value, 0)
  if (total === 0) {
    return (
      <div className={`flex items-center justify-center ${className}`} style={{ width: size, height: size }}>
        <span className="text-gray-600 text-xs">No data</span>
      </div>
    )
  }

  const radius = (size - thickness) / 2
  const cx = size / 2
  const cy = size / 2
  const circumference = 2 * Math.PI * radius

  let offset = -circumference / 4 // start from top

  return (
    <div className={`relative ${className}`} style={{ width: size, height: size }}>
      <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`}>
        {segments.map((seg, i) => {
          const pct = seg.value / total
          const dash = circumference * pct
          const gap = circumference - dash
          const currentOffset = offset
          offset += dash

          return (
            <circle
              key={i}
              cx={cx}
              cy={cy}
              r={radius}
              fill="none"
              stroke={seg.color}
              strokeWidth={thickness}
              strokeDasharray={`${dash} ${gap}`}
              strokeDashoffset={-currentOffset}
              strokeLinecap="butt"
              className="transition-all duration-500"
            />
          )
        })}
      </svg>
      {(centerLabel || centerValue) && (
        <div className="absolute inset-0 flex flex-col items-center justify-center">
          {centerValue && <span className="text-xl font-bold text-white font-mono">{centerValue}</span>}
          {centerLabel && <span className="text-[10px] text-gray-400 uppercase tracking-wider">{centerLabel}</span>}
        </div>
      )}
    </div>
  )
})

export default DonutChart
