interface SparklineProps {
  data: number[]
  width?: number
  height?: number
  color?: string
  fillColor?: string
  strokeWidth?: number
  showDots?: boolean
  className?: string
}

export default function Sparkline({
  data,
  width = 80,
  height = 24,
  color = '#3b82f6',
  fillColor,
  strokeWidth = 1.5,
  showDots = false,
  className = '',
}: SparklineProps) {
  if (!data || data.length === 0) {
    return <div className={className} style={{ width, height }} />
  }

  const min = Math.min(...data)
  const max = Math.max(...data)
  const range = max - min || 1

  const points = data.map((value, index) => ({
    x: (index / (data.length - 1)) * width,
    y: height - ((value - min) / range) * (height - 4) - 2,
    value,
  }))

  const pathData = points
    .map((p, i) => (i === 0 ? `M ${p.x},${p.y}` : `L ${p.x},${p.y}`))
    .join(' ')

  const fillPath = fillColor ? `${pathData} L ${width},${height} L 0,${height} Z` : ''

  return (
    <svg width={width} height={height} className={className} viewBox={`0 0 ${width} ${height}`} preserveAspectRatio="none">
      {fillColor && <path d={fillPath} fill={fillColor} opacity={0.15} />}
      <path d={pathData} fill="none" stroke={color} strokeWidth={strokeWidth} strokeLinecap="round" strokeLinejoin="round" />
      {showDots &&
        points.map((p, i) => (
          <circle key={i} cx={p.x} cy={p.y} r={1.5} fill={color} />
        ))}
    </svg>
  )
}
