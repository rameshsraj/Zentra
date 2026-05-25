import React from 'react'
import { ComposedChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer } from 'recharts'

interface CandlestickData {
  time: string
  open: number
  high: number
  low: number
  close: number
  volume?: number
}

interface CandlestickChartProps {
  data: CandlestickData[]
  height?: number
}

const CandlestickChart = React.memo(function CandlestickChart({ data, height = 300 }: CandlestickChartProps) {
  // Transform OHLC data into bar chart compatible format
  const transformed = data.map(d => {
    const isUp = d.close >= d.open
    return {
      time: d.time,
      // Body of the candle
      bodyBase: Math.min(d.open, d.close),
      bodyHeight: Math.abs(d.close - d.open) || 0.01,
      // Wicks
      wickHigh: d.high - Math.max(d.open, d.close),
      wickLow: Math.min(d.open, d.close) - d.low,
      color: isUp ? '#10b981' : '#ef4444',
      open: d.open,
      high: d.high,
      low: d.low,
      close: d.close,
      volume: d.volume,
    }
  })

  return (
    <ResponsiveContainer width="100%" height={height}>
      <ComposedChart data={transformed}>
        <CartesianGrid strokeDasharray="3 3" stroke="#334155" />
        <XAxis dataKey="time" tick={{ fontSize: 9, fill: '#94a3b8' }} />
        <YAxis
          domain={['dataMin - 1', 'dataMax + 1']}
          tick={{ fontSize: 10, fill: '#94a3b8' }}
        />
        <Tooltip
          contentStyle={{ backgroundColor: '#1e293b', border: '1px solid #475569', borderRadius: 8, fontSize: 11 }}
          formatter={(_: unknown, _name: string, props: any) => {
            const d = props.payload
            return [
              <div key="ohlc" className="text-xs space-y-0.5">
                <div>O: {d.open} H: {d.high} L: {d.low} C: {d.close}</div>
                {d.volume != null && <div>Vol: {d.volume.toLocaleString()}</div>}
              </div>,
              '',
            ]
          }}
          labelFormatter={(label) => label}
        />
        <Bar
          dataKey="bodyHeight"
          stackId="candle"
          fill="transparent"
          stroke="none"
          barSize={8}
          shape={(props: any) => {
            const { x, y, width: w, height: h, payload } = props
            return (
              <g>
                {/* Wick line */}
                <line
                  x1={x + w / 2} y1={y - payload.wickHigh * (h / (payload.bodyHeight || 1))}
                  x2={x + w / 2} y2={y + h + payload.wickLow * (h / (payload.bodyHeight || 1))}
                  stroke={payload.color} strokeWidth={1}
                />
                {/* Body rect */}
                <rect x={x} y={y} width={w} height={Math.max(h, 1)} fill={payload.color} rx={1} />
              </g>
            )
          }}
        />
      </ComposedChart>
    </ResponsiveContainer>
  )
})

export default CandlestickChart
