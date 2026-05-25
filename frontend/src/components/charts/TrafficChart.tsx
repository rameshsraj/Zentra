import React from 'react'
import { AreaChart, Area, XAxis, YAxis, Tooltip, ResponsiveContainer, CartesianGrid } from 'recharts'

interface TrafficDataPoint {
  time: string
  inbound: number
  outbound: number
}

interface TrafficChartProps {
  data: TrafficDataPoint[]
  title?: string
  height?: number
  className?: string
}

const TrafficChart = React.memo(function TrafficChart({ data, title, height = 280, className = '' }: TrafficChartProps) {
  return (
    <div className={className}>
      {title && <h3 className="text-sm font-semibold text-gray-300 mb-2">{title}</h3>}
      <ResponsiveContainer width="100%" height={height}>
        <AreaChart data={data} margin={{ top: 5, right: 10, left: 0, bottom: 0 }}>
          <defs>
            <linearGradient id="gradIn" x1="0" y1="0" x2="0" y2="1">
              <stop offset="5%" stopColor="#3b82f6" stopOpacity={0.3} />
              <stop offset="95%" stopColor="#3b82f6" stopOpacity={0} />
            </linearGradient>
            <linearGradient id="gradOut" x1="0" y1="0" x2="0" y2="1">
              <stop offset="5%" stopColor="#f59e0b" stopOpacity={0.3} />
              <stop offset="95%" stopColor="#f59e0b" stopOpacity={0} />
            </linearGradient>
          </defs>
          <CartesianGrid strokeDasharray="3 3" stroke="#334155" vertical={false} />
          <XAxis
            dataKey="time"
            axisLine={false}
            tickLine={false}
            tick={{ fill: '#64748b', fontSize: 10 }}
          />
          <YAxis
            axisLine={false}
            tickLine={false}
            tick={{ fill: '#64748b', fontSize: 10 }}
            tickFormatter={(v: number) => {
              if (v >= 1e9) return `${(v / 1e9).toFixed(1)}G`
              if (v >= 1e6) return `${(v / 1e6).toFixed(1)}M`
              if (v >= 1e3) return `${(v / 1e3).toFixed(1)}K`
              return String(v)
            }}
          />
          <Tooltip
            contentStyle={{
              backgroundColor: '#1e293b',
              border: '1px solid #475569',
              borderRadius: '8px',
              fontSize: '12px',
            }}
            labelStyle={{ color: '#94a3b8' }}
          />
          <Area
            type="monotone"
            dataKey="inbound"
            stroke="#3b82f6"
            strokeWidth={2}
            fill="url(#gradIn)"
            name="Inbound"
          />
          <Area
            type="monotone"
            dataKey="outbound"
            stroke="#f59e0b"
            strokeWidth={2}
            fill="url(#gradOut)"
            name="Outbound"
          />
        </AreaChart>
      </ResponsiveContainer>
    </div>
  )
})

export default TrafficChart
