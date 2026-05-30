import { useState, useCallback } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import PageHeader from '@/components/PageHeader'
import DrilldownModal, { type DrilldownNode } from '@/components/DrilldownModal'
import { getAIInsights } from '@/lib/api'
import {
  Brain, AlertTriangle, Target, Lightbulb, ChevronRight, Loader2,
} from 'lucide-react'

interface AIInsight {
  id: string
  severity: string
  title: string
  description: string
  recommendation: string
  confidence: number
}

export default function AIInsightsPage() {
  const qc = useQueryClient()
  const [search, setSearch] = useState('')
  const [drilldown, setDrilldown] = useState<{ open: boolean; title: string; nodes: DrilldownNode[] }>({ open: false, title: '', nodes: [] })

  const { data: liveInsights = [], isLoading } = useQuery<AIInsight[]>({
    queryKey: ['ai-insights'],
    queryFn: getAIInsights,
  })

  const handleRefresh = useCallback(() => {
    qc.invalidateQueries({ queryKey: ['ai-insights'] })
  }, [qc])

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

  const buildKpiDrilldown = (label: string): DrilldownNode[] => {
    switch (label) {
      case 'Avg Confidence':
        return liveInsights.map((ins, i) => ({
          id: `conf-${i}`,
          label: ins.title,
          value: `${Math.round(ins.confidence * 100)}%`,
          details: { Severity: ins.severity, Recommendation: ins.recommendation },
        }))
      case 'Active Insights':
      case 'Critical':
      case 'Warnings':
        return liveInsights
          .filter(ins => label === 'Active Insights' || ins.severity === label.toLowerCase().replace(/s$/, ''))
          .map((ins, i) => ({
            id: `ins-${i}`,
            label: ins.title,
            value: ins.severity,
            details: { Confidence: `${Math.round(ins.confidence * 100)}%`, Description: ins.description, Recommendation: ins.recommendation },
          }))
      default:
        return [{ id: '1', label: `${label} detail`, value: '—' }]
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

      {isLoading && (
        <div className="flex items-center justify-center py-12">
          <Loader2 className="w-6 h-6 text-brand-400 animate-spin" />
          <span className="ml-2 text-sm text-slate-400">Loading insights...</span>
        </div>
      )}

      {/* KPI Strip */}
      <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-3">
        {[
          { label: 'Active Insights', value: String(liveInsights.length), icon: Lightbulb, color: 'text-blue-400' },
          { label: 'Critical', value: String(liveInsights.filter(i => i.severity === 'critical').length), icon: AlertTriangle, color: 'text-red-400' },
          { label: 'Warnings', value: String(liveInsights.filter(i => i.severity === 'warning').length), icon: AlertTriangle, color: 'text-amber-400' },
          { label: 'Avg Confidence', value: liveInsights.length ? `${Math.round(liveInsights.reduce((s, i) => s + i.confidence, 0) / liveInsights.length * 100)}%` : '—', icon: Target, color: 'text-emerald-400' },
        ].map((kpi, i) => (
          <div key={i} className="bg-white/5 backdrop-blur-sm border border-white/10 p-3 rounded-xl cursor-pointer hover:bg-white/[0.08] transition-colors"
            onClick={() => setDrilldown({ open: true, title: kpi.label, nodes: buildKpiDrilldown(kpi.label) })}>
            <div className="flex items-center gap-1.5 text-[10px] text-slate-400 uppercase tracking-wider mb-1">
              <kpi.icon className="w-3 h-3" />{kpi.label}
            </div>
            <div className={`text-xl font-bold font-mono ${kpi.color}`}>{kpi.value}</div>
          </div>
        ))}
      </div>

      {/* ML Engine — not yet enabled, show explicit empty state */}
      <div className="bg-white/5 border border-white/10 rounded-xl p-5">
        <h2 className="text-sm font-bold text-white uppercase tracking-wider mb-3 flex items-center gap-2">
          <Brain className="w-4 h-4 text-purple-400" />ML Engine
        </h2>
        <div className="text-center py-8">
          <Brain className="w-10 h-10 text-slate-600 mx-auto mb-2" />
          <p className="text-sm text-slate-400">ML engine not yet enabled</p>
          <p className="text-[10px] text-slate-600 mt-1">Performance prediction, anomaly detection, capacity forecasting, and cost optimization will appear here once the ML engine is configured.</p>
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
