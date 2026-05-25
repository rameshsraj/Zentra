import React from 'react'
import { ScatterChart, Scatter, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer, ZAxis } from 'recharts'

interface XYDataPoint {
  x: number
  y: number
  z?: number
  label?: string
}

interface XYSeries {
  name: string
  data: XYDataPoint[]
  color?: string
}

interface XYChartProps {
  series: XYSeries[]
  height?: number
  xLabel?: string
  yLabel?: string
  showBubble?: boolean
}

const COLORS = ['#3b82f6', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6', '#ec4899', '#06b6d4']

const XYChart = React.memo(function XYChart({ series, height = 300, xLabel, yLabel, showBubble = false }: XYChartProps) {
  return (
    <ResponsiveContainer width="100%" height={height}>
      <ScatterChart>
        <CartesianGrid strokeDasharray="3 3" stroke="#334155" />
        <XAxis
          dataKey="x" type="number" name="x"
          tick={{ fontSize: 10, fill: '#94a3b8' }}
          label={xLabel ? { value: xLabel, position: 'insideBottom', offset: -5, fontSize: 10, fill: '#94a3b8' } : undefined}
        />
        <YAxis
          dataKey="y" type="number" name="y"
          tick={{ fontSize: 10, fill: '#94a3b8' }}
          label={yLabel ? { value: yLabel, angle: -90, position: 'insideLeft', fontSize: 10, fill: '#94a3b8' } : undefined}
        />
        {showBubble && <ZAxis dataKey="z" type="number" range={[20, 400]} />}
        <Tooltip
          contentStyle={{ backgroundColor: '#1e293b', border: '1px solid #475569', borderRadius: 8, fontSize: 11 }}
          formatter={(value: number, name: string) => [value.toLocaleString(), name]}
        />
        {series.map((s, i) => (
          <Scatter
            key={s.name}
            name={s.name}
            data={s.data}
            fill={s.color || COLORS[i % COLORS.length]}
            fillOpacity={0.7}
          />
        ))}
      </ScatterChart>
    </ResponsiveContainer>
  )
})

export default XYChart
