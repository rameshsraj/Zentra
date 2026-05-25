import React from 'react'
import { AreaChart, Area, XAxis, YAxis, Tooltip, ResponsiveContainer, CartesianGrid } from 'recharts'

interface MetricAreaChartProps {
  data: Array<Record<string, unknown>>
  dataKey: string
  color?: string
  height?: number
  title?: string
  unit?: string
  className?: string
}

const MetricAreaChart = React.memo(function MetricAreaChart({
  data,
  dataKey,
  color = '#3b82f6',
  height = 200,
  title,
  unit = '',
  className = '',
}: MetricAreaChartProps) {
  const gradientId = `grad-${dataKey.replace(/\W/g, '')}`

  return (
    <div className={className}>
      {title && <h3 className="text-sm font-semibold text-gray-300 mb-2">{title}</h3>}
      <ResponsiveContainer width="100%" height={height}>
        <AreaChart data={data} margin={{ top: 5, right: 10, left: 0, bottom: 0 }}>
          <defs>
            <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
              <stop offset="5%" stopColor={color} stopOpacity={0.3} />
              <stop offset="95%" stopColor={color} stopOpacity={0} />
            </linearGradient>
          </defs>
          <CartesianGrid strokeDasharray="3 3" stroke="#334155" vertical={false} />
          <XAxis dataKey="time" axisLine={false} tickLine={false} tick={{ fill: '#64748b', fontSize: 10 }} />
          <YAxis axisLine={false} tickLine={false} tick={{ fill: '#64748b', fontSize: 10 }} />
          <Tooltip
            contentStyle={{
              backgroundColor: '#1e293b',
              border: '1px solid #475569',
              borderRadius: '8px',
              fontSize: '12px',
            }}
            formatter={(value: number) => [`${value.toLocaleString()}${unit}`, dataKey]}
          />
          <Area type="monotone" dataKey={dataKey} stroke={color} strokeWidth={2} fill={`url(#${gradientId})`} />
        </AreaChart>
      </ResponsiveContainer>
    </div>
  )
})

export default MetricAreaChart
