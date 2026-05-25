import React from 'react'

interface GaugeChartProps {
  value: number
  max?: number
  size?: number
  thickness?: number
  label?: string
  color?: string
  warningAt?: number
  criticalAt?: number
  className?: string
}

const GaugeChart = React.memo(function GaugeChart({
  value,
  max = 100,
  size = 120,
  thickness = 10,
  label,
  color,
  warningAt = 70,
  criticalAt = 90,
  className = '',
}: GaugeChartProps) {
  const pct = Math.min((value / max) * 100, 100)
  const resolvedColor = color || (pct >= criticalAt ? '#ef4444' : pct >= warningAt ? '#f59e0b' : '#22c55e')

  const radius = (size - thickness) / 2
  const cy = size / 2
  const circumference = Math.PI * radius // semicircle
  const dash = circumference * (pct / 100)

  return (
    <div className={`flex flex-col items-center ${className}`}>
      <div className="relative" style={{ width: size, height: size / 2 + 10 }}>
        <svg width={size} height={size / 2 + 10} viewBox={`0 0 ${size} ${size / 2 + 10}`}>
          {/* Background arc */}
          <path
            d={`M ${thickness / 2} ${cy} A ${radius} ${radius} 0 0 1 ${size - thickness / 2} ${cy}`}
            fill="none"
            stroke="#334155"
            strokeWidth={thickness}
            strokeLinecap="round"
          />
          {/* Value arc */}
          <path
            d={`M ${thickness / 2} ${cy} A ${radius} ${radius} 0 0 1 ${size - thickness / 2} ${cy}`}
            fill="none"
            stroke={resolvedColor}
            strokeWidth={thickness}
            strokeLinecap="round"
            strokeDasharray={`${dash} ${circumference}`}
            className="transition-all duration-700 ease-out"
          />
        </svg>
        <div className="absolute inset-0 flex flex-col items-center justify-end pb-0">
          <span className="text-lg font-bold font-mono" style={{ color: resolvedColor }}>
            {pct.toFixed(0)}%
          </span>
        </div>
      </div>
      {label && <span className="text-[10px] text-gray-500 uppercase tracking-wider mt-1">{label}</span>}
    </div>
  )
})

export default GaugeChart
