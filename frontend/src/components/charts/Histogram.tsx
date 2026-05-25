import { BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer } from 'recharts'

interface HistogramProps {
  data: number[]
  bins?: number
  height?: number
  color?: string
  xLabel?: string
  yLabel?: string
}

function computeBins(data: number[], numBins: number) {
  if (data.length === 0) return []
  const min = Math.min(...data)
  const max = Math.max(...data)
  const binWidth = (max - min) / numBins || 1
  const bins = Array.from({ length: numBins }, (_, i) => ({
    range: `${(min + i * binWidth).toFixed(1)}-${(min + (i + 1) * binWidth).toFixed(1)}`,
    count: 0,
    start: min + i * binWidth,
    end: min + (i + 1) * binWidth,
  }))
  for (const v of data) {
    const idx = Math.min(Math.floor((v - min) / binWidth), numBins - 1)
    bins[idx].count++
  }
  return bins
}

export default function Histogram({ data, bins = 10, height = 250, color = '#3b82f6', xLabel, yLabel }: HistogramProps) {
  const binData = computeBins(data, bins)

  return (
    <ResponsiveContainer width="100%" height={height}>
      <BarChart data={binData}>
        <CartesianGrid strokeDasharray="3 3" stroke="#334155" />
        <XAxis dataKey="range" tick={{ fontSize: 9, fill: '#94a3b8' }} label={xLabel ? { value: xLabel, position: 'insideBottom', offset: -5, fontSize: 10, fill: '#94a3b8' } : undefined} />
        <YAxis tick={{ fontSize: 10, fill: '#94a3b8' }} label={yLabel ? { value: yLabel, angle: -90, position: 'insideLeft', fontSize: 10, fill: '#94a3b8' } : undefined} />
        <Tooltip contentStyle={{ backgroundColor: '#1e293b', border: '1px solid #475569', borderRadius: 8, fontSize: 11 }} />
        <Bar dataKey="count" fill={color} radius={[2, 2, 0, 0]} />
      </BarChart>
    </ResponsiveContainer>
  )
}
