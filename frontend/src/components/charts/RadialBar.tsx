import { cn } from '@/lib/utils'

interface RadialBarProps {
  value: number
  max?: number
  size?: number
  thickness?: number
  color?: string
  bgColor?: string
  label?: string
  centerText?: string
  className?: string
}

export default function RadialBar({
  value,
  max = 100,
  size = 100,
  thickness = 8,
  color = '#3b82f6',
  bgColor = 'rgba(255,255,255,0.05)',
  label,
  centerText,
  className,
}: RadialBarProps) {
  const pct = Math.min(value / max, 1)
  const radius = (size - thickness) / 2
  const circumference = 2 * Math.PI * radius
  const dashOffset = circumference * (1 - pct)

  return (
    <div className={cn('flex flex-col items-center', className)}>
      <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`}>
        <circle
          cx={size / 2}
          cy={size / 2}
          r={radius}
          fill="none"
          stroke={bgColor}
          strokeWidth={thickness}
        />
        <circle
          cx={size / 2}
          cy={size / 2}
          r={radius}
          fill="none"
          stroke={color}
          strokeWidth={thickness}
          strokeLinecap="round"
          strokeDasharray={circumference}
          strokeDashoffset={dashOffset}
          transform={`rotate(-90 ${size / 2} ${size / 2})`}
          className="transition-all duration-700"
        />
        <text
          x={size / 2}
          y={size / 2}
          textAnchor="middle"
          dominantBaseline="central"
          className="fill-white text-sm font-bold"
          fontSize={size * 0.18}
        >
          {centerText ?? `${Math.round(pct * 100)}%`}
        </text>
      </svg>
      {label && <span className="text-[10px] text-slate-400 mt-1">{label}</span>}
    </div>
  )
}
