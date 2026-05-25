interface HeatmapCell {
  label?: string
  value: number
  max?: number
}

interface HeatmapGridProps {
  cells: HeatmapCell[]
  cols?: number
  cellSize?: number
  className?: string
}

function getHeatColor(pct: number): string {
  if (pct >= 90) return 'bg-red-500/80'
  if (pct >= 75) return 'bg-amber-500/80'
  if (pct >= 50) return 'bg-yellow-500/50'
  if (pct >= 25) return 'bg-emerald-500/50'
  return 'bg-emerald-500/25'
}

export default function HeatmapGrid({ cells, cols = 8, cellSize = 28, className = '' }: HeatmapGridProps) {
  return (
    <div
      className={`grid gap-1 ${className}`}
      style={{ gridTemplateColumns: `repeat(${cols}, ${cellSize}px)` }}
    >
      {cells.map((cell, i) => {
        const pct = cell.max ? (cell.value / cell.max) * 100 : cell.value
        return (
          <div
            key={i}
            className={`rounded-sm transition-colors ${getHeatColor(pct)}`}
            style={{ width: cellSize, height: cellSize }}
            title={cell.label ? `${cell.label}: ${cell.value}${cell.max ? `/${cell.max}` : '%'}` : `${pct.toFixed(0)}%`}
          />
        )
      })}
    </div>
  )
}
