import { useQuery } from '@tanstack/react-query'
import { BarChart2 } from 'lucide-react'
import { getLogAnalytics, type LogAnalyticsData } from '@/lib/api'
import { BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer, PieChart, Pie, Cell, AreaChart, Area } from 'recharts'

const COLORS = ['#3b82f6', '#10b981', '#f59e0b', '#ef4444', '#8b5cf6', '#ec4899']

export default function LogAnalyticsPage() {
  const { data: analytics } = useQuery<LogAnalyticsData>({ queryKey: ['log-analytics'], queryFn: getLogAnalytics })

  if (!analytics) return <div className="text-center text-gray-500 py-20">Loading analytics...</div>

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold text-white flex items-center gap-2"><BarChart2 className="w-6 h-6 text-blue-400" /> Log Analytics</h1>

      <div className="grid grid-cols-4 gap-4">
        <div className="bg-gray-800 rounded-lg p-4 border border-gray-700"><p className="text-gray-400 text-xs">Total Streams</p><p className="text-2xl font-bold text-white">{analytics.total_streams}</p></div>
        <div className="bg-gray-800 rounded-lg p-4 border border-gray-700"><p className="text-gray-400 text-xs">Total Entries</p><p className="text-2xl font-bold text-white">{analytics.total_entries.toLocaleString()}</p></div>
        <div className="bg-gray-800 rounded-lg p-4 border border-gray-700"><p className="text-gray-400 text-xs">Ingestion Rate</p><p className="text-2xl font-bold text-blue-400">{analytics.ingestion_rate_lines_per_sec.toFixed(0)} lines/s</p><p className="text-xs text-gray-500 mt-1">{(analytics.ingestion_rate_bytes_per_sec / 1024).toFixed(1)} KB/s</p></div>
        <div className="bg-gray-800 rounded-lg p-4 border border-gray-700"><p className="text-gray-400 text-xs">Total Size</p><p className="text-2xl font-bold text-white">{(analytics.total_bytes / 1024 / 1024).toFixed(1)} MB</p></div>
      </div>

      <div className="grid grid-cols-2 gap-6">
        <div className="bg-gray-800 rounded-lg p-5 border border-gray-700">
          <h3 className="text-gray-300 font-medium text-sm mb-4">Volume by Source</h3>
          <div className="h-56">
            <ResponsiveContainer>
              <PieChart>
                <Pie data={analytics.volume_by_source} dataKey="lines" nameKey="source" innerRadius={50} outerRadius={80} label={({ source, percent }: any) => `${source} ${(percent * 100).toFixed(0)}%`}>
                  {analytics.volume_by_source.map((_, i) => <Cell key={i} fill={COLORS[i % COLORS.length]} />)}
                </Pie>
                <Tooltip />
              </PieChart>
            </ResponsiveContainer>
          </div>
          <div className="grid grid-cols-2 gap-2 mt-3">
            {analytics.volume_by_source.map((s, i) => (
              <div key={i} className="flex items-center gap-2 text-xs">
                <div className="w-3 h-3 rounded-sm" style={{ backgroundColor: COLORS[i % COLORS.length] }} />
                <span className="text-gray-300">{s.source}</span>
                <span className="text-gray-500 ml-auto">{s.lines.toLocaleString()} lines</span>
              </div>
            ))}
          </div>
        </div>

        <div className="bg-gray-800 rounded-lg p-5 border border-gray-700">
          <h3 className="text-gray-300 font-medium text-sm mb-4">Error Rate Trend</h3>
          <div className="h-56">
            <ResponsiveContainer>
              <AreaChart data={analytics.error_rate_trend}>
                <XAxis dataKey="timestamp" tickFormatter={(t: string) => new Date(t).getHours() + ':00'} tick={{ fill: '#9ca3af', fontSize: 10 }} />
                <YAxis tick={{ fill: '#9ca3af', fontSize: 10 }} />
                <Tooltip contentStyle={{ backgroundColor: '#1f2937', border: '1px solid #374151', borderRadius: '0.5rem' }} labelFormatter={(l: string) => new Date(l).toLocaleString()} />
                <Area type="monotone" dataKey="count" stroke="#ef4444" fill="#ef444420" strokeWidth={2} name="Errors" />
              </AreaChart>
            </ResponsiveContainer>
          </div>
        </div>
      </div>

      <div className="bg-gray-800 rounded-lg p-5 border border-gray-700">
        <h3 className="text-gray-300 font-medium text-sm mb-4">Top Error Messages</h3>
        <div className="space-y-3">
          {analytics.top_error_messages.map((e, i) => (
            <div key={i} className="flex items-center gap-4">
              <div className="flex-1 min-w-0">
                <p className="text-red-400 text-sm truncate">{e.message}</p>
                <p className="text-gray-500 text-xs">Last seen: {new Date(e.last_seen).toLocaleString()}</p>
              </div>
              <div className="flex-shrink-0 text-right">
                <p className="text-white font-bold">{e.count.toLocaleString()}</p>
                <div className="w-24 h-1.5 bg-gray-700 rounded-full mt-1"><div className="h-full bg-red-500 rounded-full" style={{ width: `${Math.min((e.count / (analytics.top_error_messages[0]?.count || 1)) * 100, 100)}%` }} /></div>
              </div>
            </div>
          ))}
        </div>
      </div>

      <div className="bg-gray-800 rounded-lg p-5 border border-gray-700">
        <h3 className="text-gray-300 font-medium text-sm mb-4">Log Volume Over Time</h3>
        <div className="h-48">
          <ResponsiveContainer>
            <BarChart data={analytics.error_rate_trend}>
              <XAxis dataKey="timestamp" tickFormatter={(t: string) => new Date(t).getHours() + ':00'} tick={{ fill: '#9ca3af', fontSize: 10 }} />
              <YAxis tick={{ fill: '#9ca3af', fontSize: 10 }} />
              <Tooltip contentStyle={{ backgroundColor: '#1f2937', border: '1px solid #374151', borderRadius: '0.5rem' }} />
              <Bar dataKey="bytes" fill="#3b82f6" radius={[2, 2, 0, 0]} name="Bytes" />
            </BarChart>
          </ResponsiveContainer>
        </div>
      </div>
    </div>
  )
}
