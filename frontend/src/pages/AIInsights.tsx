import { useState, useCallback } from 'react'
import PageHeader from '@/components/PageHeader'
import DrilldownModal, { type DrilldownNode } from '@/components/DrilldownModal'
import Sparkline from '@/components/charts/Sparkline'
import {
  Brain, TrendingUp, AlertTriangle, DollarSign,
  Activity, Target, Lightbulb, ChevronRight,
} from 'lucide-react'
import { AreaChart, Area, XAxis, YAxis, Tooltip, ResponsiveContainer, BarChart, Bar, LineChart, Line, Legend } from 'recharts'

const spark = (base: number, _v?: number, len = 20) =>
  Array.from({ length: len }, (_, i) => Math.max(0, base + Math.sin(base * 0.1 + i * 0.5) * (base * 0.05)))

const mlModels = [
  { name: 'Anomaly Detector', algorithm: 'IsolationForest', metric: 'Precision', value: 84.3, samples: 5000, status: 'active', color: 'text-purple-400' },
  { name: 'Classifier', algorithm: 'RandomForest', metric: 'Accuracy', value: 99.7, samples: 1600, status: 'active', color: 'text-blue-400' },
  { name: 'Clusterer', algorithm: 'KMeans', metric: 'Clusters', value: 4, samples: 1000, status: 'active', color: 'text-emerald-400' },
  { name: 'Cost Optimizer', algorithm: 'GradientBoosting', metric: 'R²', value: 79.1, samples: 1600, status: 'active', color: 'text-amber-400' },
  { name: 'Forecaster', algorithm: 'GradientBoosting', metric: 'R²', value: 83.4, samples: 800, status: 'training', color: 'text-cyan-400' },
]

// Static representative data — will be replaced by real API calls once ML engine is live
const perfData = Array.from({ length: 24 }, (_, i) => ({
  hour: `${i}:00`,
  cpu: Math.round(40 + Math.sin(i * 0.6) * 15 + (i >= 9 && i <= 17 ? 15 : 0)),
  memory: Math.round(50 + Math.sin(i * 0.3 + 1) * 10),
  latency: Math.round(20 + Math.sin(i * 0.4 + 2) * 15 + (i >= 12 && i <= 14 ? 20 : 0)),
}))

const anomalyData = Array.from({ length: 7 }, (_, i) => ({
  day: ['Mon', 'Tue', 'Wed', 'Thu', 'Fri', 'Sat', 'Sun'][i],
  detected: 2 + Math.round(Math.abs(Math.sin(i * 0.9)) * 13),
  resolved: 1 + Math.round(Math.abs(Math.sin(i * 0.7 + 1)) * 10),
  falsePositives: Math.round(Math.abs(Math.sin(i * 0.5)) * 2),
}))

const capacityData = Array.from({ length: 30 }, (_, i) => ({
  day: i + 1,
  current: Math.round(65 + i * 0.5 + Math.sin(i * 0.4) * 2),
  predicted: Math.round(65 + i * 0.8 + Math.sin(i * 0.3) * 1.5),
  threshold: 85,
}))

const costData = [
  { category: 'Compute', current: 4500, optimized: 3200, savings: 1300 },
  { category: 'Storage', current: 2100, optimized: 1800, savings: 300 },
  { category: 'Network', current: 1200, optimized: 1000, savings: 200 },
  { category: 'Database', current: 3200, optimized: 2400, savings: 800 },
]

const insights = [
  { id: '1', severity: 'critical', title: 'Memory pressure detected on prod-web-03', description: 'Memory utilization has been above 90% for 2 hours. Consider scaling up or optimizing memory-heavy processes.', recommendation: 'Add 4GB RAM or restart the nginx worker pool', confidence: 0.94 },
  { id: '2', severity: 'warning', title: 'Anomalous CPU spike pattern on db-primary', description: 'CPU usage shows unusual periodic spikes every 15 minutes, correlating with a cron job running VACUUM on all tables.', recommendation: 'Schedule VACUUM during off-peak hours (2-4 AM)', confidence: 0.87 },
  { id: '3', severity: 'info', title: 'Storage growth exceeds forecast by 12%', description: 'Current storage consumption rate will reach 85% threshold in 18 days, 4 days earlier than predicted.', recommendation: 'Review log retention policies and archive old data', confidence: 0.91 },
  { id: '4', severity: 'warning', title: 'Network latency anomaly in US-East region', description: 'P95 latency increased from 45ms to 120ms in the last 30 minutes. 3 hosts affected.', recommendation: 'Check ISP peering or switch to backup route', confidence: 0.82 },
]

const totalSavings = costData.reduce((sum, d) => sum + d.savings, 0)

export default function AIInsightsPage() {
  const [search, setSearch] = useState('')
  const [drilldown, setDrilldown] = useState<{ open: boolean; title: string; nodes: DrilldownNode[] }>({ open: false, title: '', nodes: [] })

  const handleRefresh = useCallback(() => {}, [])

  const liveInsights = insights
  const liveModels = mlModels
  const livePerfData = perfData
  const liveAnomalyData = anomalyData
  const liveCapacityData = capacityData
  const liveCostData = costData
  const liveTotalSavings = totalSavings
  const filteredInsights = liveInsights.filter(i =>
    !search || `${i.title} ${i.description}`.toLowerCase().includes(search.toLowerCase())
  )

  const sevColor = (s: string) => {
    switch (s) {
      case 'critical': return 'bg-red-500/20 text-red-400 border-red-500/30'
      case 'warning': return 'bg-amber-500/20 text-amber-400 border-amber-500/30'
      default: return 'bg-blue-500/20 text-blue-400 border-blue-500/30'
    }
  }

  return (
    <div className="space-y-6">
      <PageHeader
        title="AI Insights"
        subtitle="ML-powered anomaly detection · Capacity forecasting · Cost optimization"
        onRefresh={handleRefresh}
        onSearch={setSearch}
        searchPlaceholder="Search insights..."
      />

      {/* KPI Strip */}
      <div className="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-5 gap-3">
        {[
          { label: 'ML Models', value: String(liveModels.length), icon: Brain, color: 'text-purple-400', sparkData: spark(5, 1) },
          { label: 'Anomalies (7d)', value: String(liveAnomalyData.reduce((s, d) => s + d.detected, 0)), icon: AlertTriangle, color: 'text-amber-400', sparkData: spark(8, 4) },
          { label: 'Avg Confidence', value: liveInsights.length ? `${Math.round(liveInsights.reduce((s, i) => s + i.confidence, 0) / liveInsights.length * 100)}%` : '—', icon: Target, color: 'text-emerald-400', sparkData: spark(88, 5) },
          { label: 'Cost Savings', value: `$${liveTotalSavings.toLocaleString()}`, icon: DollarSign, color: 'text-cyan-400', sparkData: spark(2600, 500) },
          { label: 'Active Insights', value: String(liveInsights.length), icon: Lightbulb, color: 'text-blue-400', sparkData: spark(4, 2) },
        ].map((kpi, i) => (
          <div key={i} className="bg-white/5 backdrop-blur-sm border border-white/10 p-3 rounded-xl cursor-pointer hover:bg-white/[0.08] transition-colors"
            onClick={() => setDrilldown({ open: true, title: kpi.label, nodes: [{ id: '1', label: `${kpi.label} detail`, value: kpi.value }] })}>
            <div className="flex items-center gap-1.5 text-[10px] text-slate-400 uppercase tracking-wider mb-1">
              <kpi.icon className="w-3 h-3" />{kpi.label}
            </div>
            <div className={`text-xl font-bold font-mono ${kpi.color}`}>{kpi.value}</div>
            <Sparkline data={kpi.sparkData} width={80} height={14} color="#8b5cf6" className="mt-1" />
          </div>
        ))}
      </div>

      {/* ML Model Cards */}
      <div>
        <h2 className="text-sm font-bold text-white uppercase tracking-wider mb-3 flex items-center gap-2"><Brain className="w-4 h-4 text-purple-400" />ML Models</h2>
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-5 gap-3">
          {liveModels.map(m => (
            <div key={m.name} className="bg-white/5 border border-white/10 rounded-xl p-4">
              <div className="flex items-center justify-between mb-2">
                <span className="text-xs font-bold text-white">{m.name}</span>
                <span className={`px-2 py-0.5 rounded text-[10px] font-semibold ${m.status === 'active' ? 'bg-emerald-500/20 text-emerald-400' : 'bg-amber-500/20 text-amber-400'}`}>{m.status}</span>
              </div>
              <div className="text-[10px] text-slate-500 mb-1">{m.algorithm}</div>
              <div className={`text-2xl font-bold font-mono ${m.color}`}>{m.value}{m.metric === 'Clusters' ? '' : '%'}</div>
              <div className="text-[10px] text-slate-500 mt-1">{m.metric} · {m.samples.toLocaleString()} samples</div>
            </div>
          ))}
        </div>
      </div>

      {/* Charts Row */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Performance Prediction */}
        <div className="bg-white/5 border border-white/10 rounded-xl p-5">
          <h3 className="text-sm font-bold text-white uppercase tracking-wider mb-4 flex items-center gap-2">
            <Activity className="w-4 h-4 text-blue-400" />Performance Prediction (24h)
          </h3>
          <ResponsiveContainer width="100%" height={220}>
            <LineChart data={livePerfData}>
              <XAxis dataKey="hour" tick={{ fill: '#64748b', fontSize: 10 }} axisLine={false} tickLine={false} interval={3} />
              <YAxis tick={{ fill: '#64748b', fontSize: 10 }} axisLine={false} tickLine={false} />
              <Tooltip contentStyle={{ background: '#0f172a', border: '1px solid rgba(255,255,255,0.1)', borderRadius: '12px', fontSize: 11 }} />
              <Line type="monotone" dataKey="cpu" stroke="#3b82f6" strokeWidth={2} dot={false} name="CPU %" />
              <Line type="monotone" dataKey="memory" stroke="#8b5cf6" strokeWidth={2} dot={false} name="Memory %" />
              <Line type="monotone" dataKey="latency" stroke="#f59e0b" strokeWidth={2} dot={false} name="Latency ms" />
              <Legend wrapperStyle={{ fontSize: 10 }} />
            </LineChart>
          </ResponsiveContainer>
        </div>

        {/* Anomaly Detection */}
        <div className="bg-white/5 border border-white/10 rounded-xl p-5">
          <h3 className="text-sm font-bold text-white uppercase tracking-wider mb-4 flex items-center gap-2">
            <AlertTriangle className="w-4 h-4 text-amber-400" />Anomaly Detection (7d)
          </h3>
          <ResponsiveContainer width="100%" height={220}>
            <BarChart data={liveAnomalyData}>
              <XAxis dataKey="day" tick={{ fill: '#64748b', fontSize: 10 }} axisLine={false} tickLine={false} />
              <YAxis tick={{ fill: '#64748b', fontSize: 10 }} axisLine={false} tickLine={false} />
              <Tooltip contentStyle={{ background: '#0f172a', border: '1px solid rgba(255,255,255,0.1)', borderRadius: '12px', fontSize: 11 }} />
              <Bar dataKey="detected" fill="#f59e0b" radius={[4, 4, 0, 0]} name="Detected" />
              <Bar dataKey="resolved" fill="#22c55e" radius={[4, 4, 0, 0]} name="Resolved" />
              <Bar dataKey="falsePositives" fill="#64748b" radius={[4, 4, 0, 0]} name="False Positives" />
              <Legend wrapperStyle={{ fontSize: 10 }} />
            </BarChart>
          </ResponsiveContainer>
        </div>

        {/* Capacity Forecast */}
        <div className="bg-white/5 border border-white/10 rounded-xl p-5">
          <h3 className="text-sm font-bold text-white uppercase tracking-wider mb-4 flex items-center gap-2">
            <TrendingUp className="w-4 h-4 text-cyan-400" />Capacity Forecast (30d)
          </h3>
          <ResponsiveContainer width="100%" height={220}>
            <AreaChart data={liveCapacityData}>
              <XAxis dataKey="day" tick={{ fill: '#64748b', fontSize: 10 }} axisLine={false} tickLine={false} />
              <YAxis tick={{ fill: '#64748b', fontSize: 10 }} axisLine={false} tickLine={false} domain={[50, 100]} />
              <Tooltip contentStyle={{ background: '#0f172a', border: '1px solid rgba(255,255,255,0.1)', borderRadius: '12px', fontSize: 11 }} />
              <Area type="monotone" dataKey="current" stroke="#3b82f6" fill="#3b82f620" strokeWidth={2} name="Current %" />
              <Area type="monotone" dataKey="predicted" stroke="#8b5cf6" fill="#8b5cf620" strokeWidth={2} strokeDasharray="5 5" name="Predicted %" />
              <Area type="monotone" dataKey="threshold" stroke="#ef4444" fill="none" strokeWidth={1} strokeDasharray="3 3" name="Threshold" />
              <Legend wrapperStyle={{ fontSize: 10 }} />
            </AreaChart>
          </ResponsiveContainer>
        </div>

        {/* Cost Optimization */}
        <div className="bg-white/5 border border-white/10 rounded-xl p-5">
          <h3 className="text-sm font-bold text-white uppercase tracking-wider mb-4 flex items-center gap-2">
            <DollarSign className="w-4 h-4 text-emerald-400" />Cost Optimization
          </h3>
          <div className="space-y-3">
            {liveCostData.map(d => (
              <div key={d.category}>
                <div className="flex items-center justify-between text-xs mb-1">
                  <span className="text-white font-medium">{d.category}</span>
                  <span className="text-emerald-400 font-mono font-bold">-${d.savings.toLocaleString()}</span>
                </div>
                <div className="flex gap-1 h-3">
                  <div className="bg-blue-500/30 rounded-l" style={{ width: `${(d.optimized / d.current) * 100}%` }}>
                    <div className="h-full bg-blue-500 rounded-l" style={{ width: '100%' }} />
                  </div>
                  <div className="bg-emerald-500/30 rounded-r" style={{ width: `${(d.savings / d.current) * 100}%` }}>
                    <div className="h-full bg-emerald-500/50 rounded-r" style={{ width: '100%' }} />
                  </div>
                </div>
                <div className="flex justify-between text-[10px] text-slate-500 mt-0.5">
                  <span>${d.optimized.toLocaleString()} optimized</span>
                  <span>${d.current.toLocaleString()} current</span>
                </div>
              </div>
            ))}
            <div className="border-t border-white/10 pt-2 flex items-center justify-between">
              <span className="text-sm font-bold text-white">Total Savings</span>
              <span className="text-lg font-bold font-mono text-emerald-400">${liveTotalSavings.toLocaleString()}/mo</span>
            </div>
          </div>
        </div>
      </div>

      {/* AI Insights Feed */}
      <div>
        <h2 className="text-sm font-bold text-white uppercase tracking-wider mb-3 flex items-center gap-2"><Lightbulb className="w-4 h-4 text-amber-400" />Active Insights</h2>
        <div className="space-y-3">
          {filteredInsights.map(insight => (
            <div key={insight.id} className="bg-white/5 border border-white/10 rounded-xl p-4 hover:bg-white/[0.08] transition-colors cursor-pointer"
              onClick={() => setDrilldown({ open: true, title: insight.title, nodes: [
                { id: '1', label: 'Description', value: insight.description },
                { id: '2', label: 'Recommendation', value: insight.recommendation },
                { id: '3', label: 'Confidence', value: `${Math.round(insight.confidence * 100)}%` },
              ] })}>
              <div className="flex items-start gap-3">
                <span className={`px-2 py-0.5 rounded border text-[10px] font-semibold uppercase ${sevColor(insight.severity)}`}>
                  {insight.severity}
                </span>
                <div className="flex-1 min-w-0">
                  <div className="text-sm font-semibold text-white">{insight.title}</div>
                  <div className="text-xs text-slate-400 mt-1 line-clamp-2">{insight.description}</div>
                  <div className="flex items-center gap-3 mt-2">
                    <span className="text-[10px] text-slate-500 flex items-center gap-1"><Target className="w-3 h-3" />{Math.round(insight.confidence * 100)}% confidence</span>
                    <span className="text-[10px] text-brand-400 flex items-center gap-1"><Lightbulb className="w-3 h-3" />{insight.recommendation}</span>
                  </div>
                </div>
                <ChevronRight className="w-4 h-4 text-slate-600 flex-shrink-0 mt-1" />
              </div>
            </div>
          ))}
        </div>
      </div>

      <DrilldownModal open={drilldown.open} onClose={() => setDrilldown(d => ({ ...d, open: false }))} title={drilldown.title} root={drilldown.nodes} />
    </div>
  )
}
