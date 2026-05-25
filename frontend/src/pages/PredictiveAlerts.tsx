import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { getForecasts, getAnomalies, getTrendAlerts, getPredictionSummary } from '@/lib/api'
import PageHeader from '@/components/PageHeader'
import { Brain, TrendingUp, AlertTriangle, Eye, BarChart3, Zap } from 'lucide-react'

const severityColor: Record<string, string> = {
  critical: 'text-red-400 bg-red-900/30',
  high: 'text-orange-400 bg-orange-900/30',
  warning: 'text-yellow-400 bg-yellow-900/30',
  medium: 'text-yellow-400 bg-yellow-900/30',
  low: 'text-blue-400 bg-blue-900/30',
  info: 'text-gray-400 bg-gray-700/50',
}

export default function PredictiveAlertsPage() {
  const { data: summary } = useQuery({ queryKey: ['prediction-summary'], queryFn: getPredictionSummary })
  const { data: forecasts } = useQuery({ queryKey: ['forecasts'], queryFn: getForecasts })
  const { data: anomalies } = useQuery({ queryKey: ['anomalies'], queryFn: getAnomalies })
  const { data: trendAlerts } = useQuery({ queryKey: ['trend-alerts'], queryFn: getTrendAlerts })

  const [tab, setTab] = useState<'anomalies' | 'forecasts' | 'trends'>('anomalies')
  const [search, setSearch] = useState('')

  return (
    <div className="space-y-6">
      <PageHeader
        title="Predictive Alerts"
        subtitle="AI-powered anomaly detection, forecasting & trend analysis"
        onSearch={setSearch}
      />

      {/* Summary KPIs */}
      {summary && (
        <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-4">
          <div className="bg-gray-800 rounded-lg p-4">
            <div className="text-xs text-gray-400 flex items-center gap-1"><AlertTriangle className="w-3 h-3 text-red-400" /> Anomalies</div>
            <div className="text-2xl font-bold text-red-400">{summary.active_anomalies}</div>
          </div>
          <div className="bg-gray-800 rounded-lg p-4">
            <div className="text-xs text-gray-400 flex items-center gap-1"><TrendingUp className="w-3 h-3 text-orange-400" /> Forecasts</div>
            <div className="text-2xl font-bold text-orange-400">{summary.active_forecasts}</div>
          </div>
          <div className="bg-gray-800 rounded-lg p-4">
            <div className="text-xs text-gray-400 flex items-center gap-1"><Zap className="w-3 h-3 text-yellow-400" /> Trend Alerts</div>
            <div className="text-2xl font-bold text-yellow-400">{summary.active_trend_alerts}</div>
          </div>
          <div className="bg-gray-800 rounded-lg p-4">
            <div className="text-xs text-gray-400 flex items-center gap-1"><Brain className="w-3 h-3 text-purple-400" /> Models</div>
            <div className="text-2xl font-bold text-purple-400">{summary.models_trained}</div>
          </div>
          <div className="bg-gray-800 rounded-lg p-4">
            <div className="text-xs text-gray-400 flex items-center gap-1"><BarChart3 className="w-3 h-3 text-blue-400" /> Metrics</div>
            <div className="text-2xl font-bold text-blue-400">{summary.metrics_monitored}</div>
          </div>
          <div className="bg-gray-800 rounded-lg p-4">
            <div className="text-xs text-gray-400">Last Eval</div>
            <div className="text-sm font-medium text-gray-300 mt-1">{new Date(summary.last_evaluation).toLocaleTimeString()}</div>
          </div>
        </div>
      )}

      {/* Tabs */}
      <div className="flex gap-2 border-b border-gray-700 pb-2">
        <button onClick={() => setTab('anomalies')} className={`px-4 py-2 rounded-t text-sm font-medium ${tab === 'anomalies' ? 'bg-gray-700 text-white' : 'text-gray-400 hover:text-white'}`}>
          <AlertTriangle className="w-4 h-4 inline mr-1" /> Anomalies ({(anomalies ?? []).length})
        </button>
        <button onClick={() => setTab('forecasts')} className={`px-4 py-2 rounded-t text-sm font-medium ${tab === 'forecasts' ? 'bg-gray-700 text-white' : 'text-gray-400 hover:text-white'}`}>
          <TrendingUp className="w-4 h-4 inline mr-1" /> Forecasts ({(forecasts ?? []).length})
        </button>
        <button onClick={() => setTab('trends')} className={`px-4 py-2 rounded-t text-sm font-medium ${tab === 'trends' ? 'bg-gray-700 text-white' : 'text-gray-400 hover:text-white'}`}>
          <Zap className="w-4 h-4 inline mr-1" /> Trend Alerts ({(trendAlerts ?? []).length})
        </button>
      </div>

      {/* Anomalies Tab */}
      {tab === 'anomalies' && (
        <div className="space-y-3">
          {(anomalies ?? []).filter(a => !search || a.metric_name.includes(search) || a.description.toLowerCase().includes(search.toLowerCase())).map(a => (
            <div key={a.anomaly_id} className="bg-gray-800 rounded-lg p-4 border-l-4 border-red-500/50">
              <div className="flex items-start justify-between">
                <div>
                  <div className="flex items-center gap-2 mb-1">
                    <span className={`px-2 py-0.5 rounded text-xs font-medium ${severityColor[a.severity] ?? severityColor.info}`}>{a.severity}</span>
                    <span className="font-mono text-sm text-white">{a.metric_name}</span>
                    <span className="text-xs text-gray-500">on {a.resource_id}</span>
                  </div>
                  <p className="text-sm text-gray-300">{a.description}</p>
                  <div className="flex gap-4 mt-2 text-xs text-gray-500">
                    <span>Value: <span className="text-white">{(a.value ?? 0).toFixed(2)}</span></span>
                    <span>Baseline: <span className="text-gray-300">{(a.baseline_mean ?? 0).toFixed(2)} ± {(a.baseline_stddev ?? 0).toFixed(2)}</span></span>
                    <span>Z-score: <span className="text-orange-300">{(a.z_score ?? 0).toFixed(2)}</span></span>
                    <span>Direction: <span className="text-gray-300">{a.direction}</span></span>
                  </div>
                </div>
                <span className="text-xs text-gray-500">{new Date(a.detected_at).toLocaleString()}</span>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Forecasts Tab */}
      {tab === 'forecasts' && (
        <div className="space-y-3">
          {(forecasts ?? []).filter(f => !search || f.metric_name.includes(search)).map(f => (
            <div key={f.forecast_id} className="bg-gray-800 rounded-lg p-4 border-l-4 border-orange-500/50">
              <div className="flex items-start justify-between">
                <div>
                  <div className="flex items-center gap-2 mb-1">
                    <span className={`px-2 py-0.5 rounded text-xs font-medium ${severityColor[f.severity] ?? severityColor.info}`}>{f.severity}</span>
                    <span className="font-mono text-sm text-white">{f.metric_name}</span>
                  </div>
                  <p className="text-sm text-gray-300">{f.description}</p>
                  <div className="flex gap-4 mt-2 text-xs text-gray-500">
                    <span>Current: <span className="text-white">{(f.current_value ?? 0).toFixed(1)}</span></span>
                    <span>Threshold: <span className="text-red-300">{f.threshold ?? 'N/A'}</span></span>
                    <span>Rate: <span className="text-orange-300">{(f.rate_per_hour ?? 0).toFixed(3)}/hr</span></span>
                    {f.hours_to_threshold != null && <span>Breach in: <span className="text-yellow-300">{f.hours_to_threshold.toFixed(1)}h</span></span>}
                    <span>Confidence: <span className="text-green-300">{((f.confidence ?? 0) * 100).toFixed(0)}%</span></span>
                  </div>
                </div>
              </div>
            </div>
          ))}
        </div>
      )}

      {/* Trend Alerts Tab */}
      {tab === 'trends' && (
        <div className="space-y-3">
          {(trendAlerts ?? []).filter(t => !search || t.title.toLowerCase().includes(search.toLowerCase())).map(t => (
            <div key={t.alert_id} className="bg-gray-800 rounded-lg p-4 border-l-4 border-yellow-500/50">
              <div className="flex items-start justify-between">
                <div>
                  <div className="flex items-center gap-2 mb-1">
                    <span className={`px-2 py-0.5 rounded text-xs font-medium ${severityColor[t.severity] ?? severityColor.info}`}>{t.severity}</span>
                    <span className="text-white font-medium">{t.title}</span>
                  </div>
                  <p className="text-sm text-gray-300">{t.description}</p>
                  <div className="mt-2 flex items-start gap-1">
                    <Eye className="w-3 h-3 text-green-400 mt-0.5 flex-shrink-0" />
                    <span className="text-xs text-green-300">{t.recommendation}</span>
                  </div>
                </div>
                <span className="text-xs text-gray-500">{new Date(t.created_at).toLocaleString()}</span>
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
