interface BarGaugeItem {
  label: string
  value: number
  max?: number
  color?: string
}

interface BarGaugeProps {
  items: BarGaugeItem[]
  orientation?: 'horizontal' | 'vertical'
  thresholds?: { value: number; color: string }[]
  height?: number
  showValue?: boolean
  unit?: string
}

function getThresholdColor(value: number, thresholds?: { value: number; color: string }[]): string {
  if (!thresholds || thresholds.length === 0) return '#3b82f6'
  const sorted = [...thresholds].sort((a, b) => b.value - a.value)
  for (const t of sorted) {
    if (value >= t.value) return t.color
  }
  return sorted[sorted.length - 1]?.color || '#3b82f6'
}

export default function BarGauge({ items, orientation = 'horizontal', thresholds, height = 200, showValue = true, unit = '' }: BarGaugeProps) {
  if (orientation === 'vertical') {
    return (
      <div className="flex items-end gap-2 justify-center" style={{ height }}>
        {items.map((item, i) => {
          const max = item.max || Math.max(...items.map(it => it.value)) * 1.2
          const pct = Math.min((item.value / max) * 100, 100)
          const color = item.color || getThresholdColor(item.value, thresholds)
          return (
            <div key={i} className="flex flex-col items-center gap-1" style={{ height: '100%' }}>
              {showValue && <span className="text-[10px] font-mono text-white">{item.value}{unit}</span>}
              <div className="flex-1 w-6 bg-slate-700 rounded-t relative overflow-hidden flex items-end">
                <div className="w-full rounded-t transition-all duration-500" style={{ height: `${pct}%`, backgroundColor: color }} />
              </div>
              <span className="text-[9px] text-slate-400 truncate max-w-[60px]">{item.label}</span>
            </div>
          )
        })}
      </div>
    )
  }

  return (
    <div className="space-y-2">
      {items.map((item, i) => {
        const max = item.max || Math.max(...items.map(it => it.value)) * 1.2
        const pct = Math.min((item.value / max) * 100, 100)
        const color = item.color || getThresholdColor(item.value, thresholds)
        return (
          <div key={i} className="flex items-center gap-3">
            <span className="text-xs text-slate-400 w-24 truncate">{item.label}</span>
            <div className="flex-1 h-4 bg-slate-700 rounded overflow-hidden">
              <div className="h-full rounded transition-all duration-500" style={{ width: `${pct}%`, backgroundColor: color }} />
            </div>
            {showValue && <span className="text-xs font-mono text-white w-16 text-right">{item.value.toLocaleString()}{unit}</span>}
          </div>
        )
      })}
    </div>
  )
}
