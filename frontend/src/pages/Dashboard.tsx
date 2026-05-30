import { useCallback, useMemo } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import {
  getSystemStatus, getEventStats, getAlerts, getTargets,
  getNetworkCapture, getNetworkProtocols,
  getAIInsights, getPredictionSummary, getRecentEvents,
  getTrafficChart, getNetworkBandwidth,
} from '@/lib/api'
import { formatUptime } from '@/lib/utils'
import {
  Activity, Bell, Shield, HardDrive, Network, Database,
  ArrowUpRight, ArrowDownRight, Zap, Brain, TrendingUp,
  AlertTriangle, Eye, Server, Wifi, Lock, Globe,
} from 'lucide-react'
import Sparkline from '@/components/charts/Sparkline'
import DonutChart from '@/components/charts/DonutChart'
import TrafficChart from '@/components/charts/TrafficChart'
import HeatmapGrid from '@/components/charts/HeatmapGrid'
import GaugeChart from '@/components/charts/GaugeChart'
import StatCard from '@/components/charts/StatCard'
import ProgressBar from '@/components/charts/ProgressBar'
import StatusGrid from '@/components/charts/StatusGrid'
import TimelineList from '@/components/charts/TimelineList'
import PageHeader from '@/components/PageHeader'
import { useAuth } from '@/contexts/AuthContext'

// M10: Flat sparkline from live value — no synthetic Math.sin generation
const spark = (seed: number, len = 20) =>
  Array.from({ length: len }, () => seed)

export default function DashboardPage() {
  const qc = useQueryClient()
  const { hasModule, hasMenuItem } = useAuth()
  const { data: status } = useQuery({ queryKey: ['system-status'], queryFn: getSystemStatus })
  const { data: stats } = useQuery({ queryKey: ['event-stats'], queryFn: getEventStats })
  const { data: alerts } = useQuery({ queryKey: ['alerts'], queryFn: getAlerts })
  const { data: targets } = useQuery({ queryKey: ['targets'], queryFn: getTargets })
  const { data: capture } = useQuery({ queryKey: ['net-capture'], queryFn: getNetworkCapture })
  const { data: protocols } = useQuery({ queryKey: ['net-protocols'], queryFn: getNetworkProtocols })
  const { data: insights } = useQuery({ queryKey: ['ai-insights'], queryFn: getAIInsights })
  const { data: predSummary } = useQuery({ queryKey: ['pred-summary'], queryFn: getPredictionSummary })
  const { data: events } = useQuery({ queryKey: ['recent-events'], queryFn: () => getRecentEvents(20) })
  const { data: trafficChart } = useQuery({ queryKey: ['traffic-chart'], queryFn: () => getTrafficChart('30m', 'bytes'), refetchInterval: 60_000 })
  useQuery({ queryKey: ['net-bandwidth'], queryFn: getNetworkBandwidth, refetchInterval: 30_000 })

  const handleRefresh = useCallback(() => { qc.invalidateQueries() }, [qc])

  const hasAnyInfrastructure = hasMenuItem('infrastructure.servers') || hasMenuItem('infrastructure.storage') || hasMenuItem('infrastructure.network')
  const hasAnySecurity = hasModule('security')
  const hasAnyAlerts = hasModule('alerts_events')
  const hasAnyLogs = hasModule('logs')
  const hasAnyAi = hasModule('ai_analytics')

  // Build traffic chart data from real API or empty placeholder
  const trafficData = useMemo(() => {
    if (!hasAnyInfrastructure) return [] // Empty state if no monitoring permitted
    if (trafficChart && trafficChart.length > 0) {
      return trafficChart.map(pt => ({
        time: new Date((pt as any).time ?? (pt as any).timestamp ?? Date.now()).toLocaleTimeString('en', { hour: '2-digit', minute: '2-digit' }),
        inbound: (pt as any).inbound ?? (pt as any).bytes_in ?? 0,
        outbound: (pt as any).outbound ?? (pt as any).bytes_out ?? 0,
      }))
    }
    return []
  }, [trafficChart, hasAnyInfrastructure])

  // Heatmap: target health scores from real targets (0–100 based on status)
  const heatmapCells = useMemo(() => {
    if (!hasAnyInfrastructure || !targets || targets.length === 0) return []
    return targets.slice(0, 40).map(t => ({
      value: t.status === 'healthy' ? 85 + (t.name.charCodeAt(0) % 15)
           : t.status === 'warning' ? 40 + (t.name.charCodeAt(0) % 20)
           : t.status === 'critical' ? 10 + (t.name.charCodeAt(0) % 15)
           : 0,
      label: t.name,
    }))
  }, [targets, hasAnyInfrastructure])

  const activeAlerts = hasAnyAlerts ? (alerts?.filter(a => a.status === 'triggered') ?? []) : []
  const totalTargets = hasAnyInfrastructure ? (targets?.length ?? 0) : 0
  const healthyTargets = hasAnyInfrastructure ? (targets?.filter(t => t.status === 'healthy').length ?? 0) : 0
  const warningTargets = hasAnyInfrastructure ? (targets?.filter(t => t.status === 'warning').length ?? 0) : 0
  const criticalTargets = hasAnyInfrastructure ? (targets?.filter(t => t.status === 'critical' || t.status === 'unreachable').length ?? 0) : 0

  const serverCount = hasMenuItem('infrastructure.servers') ? (targets?.filter(t => t.target_type === 'server').length ?? 0) : 0
  const dbCount = hasModule('database') ? (targets?.filter(t => t.target_type === 'database').length ?? 0) : 0
  const storageCount = hasMenuItem('infrastructure.storage') ? (targets?.filter(t => t.target_type === 'storage').length ?? 0) : 0
  const networkCount = hasMenuItem('infrastructure.network') ? (targets?.filter(t => t.target_type === 'network').length ?? 0) : 0

  const hasAnyOtherModules = hasModule('infrastructure') || hasModule('database') || hasModule('middleware') || hasModule('security') || hasModule('logs') || hasModule('alerts_events') || hasModule('ai_analytics') || hasModule('opentelemetry')

  return (
    <div className="space-y-6">
      <PageHeader
        title="Command Center"
        subtitle={hasAnyOtherModules ? `Real-time infrastructure monitoring · ${totalTargets} targets · ${status?.version ?? 'v1.0.0'}` : `Command Center · ${status?.version ?? 'v1.0.0'}`}
        onRefresh={handleRefresh}
        onSearch={() => {}}
        searchPlaceholder="Global search..."
      >
        <div className="flex items-center gap-3">
          <div className="flex items-center gap-2 px-3 py-1.5 bg-white/5 rounded-lg border border-white/10 text-xs">
            <div className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse" />
            <span className="text-emerald-400 font-mono">LIVE</span>
          </div>
          <div className="text-xs text-slate-500 font-mono">
            {status ? `Uptime: ${formatUptime(status.uptime_seconds)}` : ''}
          </div>
        </div>
      </PageHeader>

      {!hasAnyOtherModules ? (
        /* Empty State Command Center Dashboard when no other modules are selected */
        <div className="flex flex-col items-center justify-center py-20 bg-white/5 border border-white/10 rounded-2xl p-8 text-center max-w-2xl mx-auto space-y-4">
          <div className="p-4 bg-blue-600/10 rounded-2xl border border-blue-500/20 text-blue-400">
            <Activity className="w-10 h-10 animate-pulse" />
          </div>
          <div className="space-y-2">
            <h2 className="text-xl font-bold text-white">Empty Command Center</h2>
            <p className="text-sm text-slate-400 max-w-md">
              Welcome! You have access to the Command Center. However, no additional monitoring modules have been assigned to your profile yet.
            </p>
          </div>
          <p className="text-xs text-slate-500">
            Please contact your system administrator to assign Infrastructure, Databases, Security, or AI Analytics modules to populate this dashboard with live telemetry.
          </p>
        </div>
      ) : (
        <>
          {/* ═══════════ ROW 1: KPI Strip — 8 columns ═══════════ */}
          <div className="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-8 gap-2">
            {[
              { label: 'System Health', value: totalTargets > 0 ? `${Math.round((healthyTargets / totalTargets) * 100)}%` : '—', color: 'text-emerald-400', sub: healthyTargets === totalTargets ? 'All Systems Go' : `${criticalTargets} critical`, trend: 'up' as const },
              { label: 'Active Alerts', value: hasAnyAlerts ? String(activeAlerts.length) : '—', color: activeAlerts.length > 0 ? 'text-amber-400' : 'text-emerald-400', sub: hasAnyAlerts ? `${activeAlerts.filter(a => a.severity === 'critical').length} Critical` : 'No alert access', trend: activeAlerts.length > 0 ? 'down' as const : 'up' as const },
              { label: 'Metrics/min', value: hasAnyInfrastructure ? (stats?.metrics_collected ? `${(stats.metrics_collected / 60).toFixed(0)}` : '0') : '—', color: 'text-blue-400', sub: hasAnyInfrastructure ? `${stats?.metrics_collected?.toLocaleString() ?? '0'} total` : 'No metrics', trend: 'up' as const },
              { label: 'Threats', value: hasAnySecurity ? String(stats?.threats_detected ?? 0) : '—', color: (stats?.threats_detected ?? 0) > 0 ? 'text-red-400' : 'text-emerald-400', sub: hasAnySecurity ? 'Last 24h' : 'No threat access', trend: 'neutral' as const },
              { label: 'Targets', value: hasAnyInfrastructure ? `${healthyTargets}/${totalTargets}` : '—', color: 'text-blue-400', sub: hasAnyInfrastructure ? 'Healthy/Total' : 'No target access', trend: 'up' as const },
              { label: 'Logs', value: hasAnyLogs ? (stats?.logs_ingested?.toLocaleString() ?? '0') : '—', color: 'text-purple-400', sub: hasAnyLogs ? 'Ingested' : 'No log access', trend: 'up' as const },
              { label: 'Anomalies', value: hasAnyAi ? String(predSummary?.active_anomalies ?? 0) : '—', color: (predSummary?.active_anomalies ?? 0) > 0 ? 'text-red-400' : 'text-emerald-400', sub: hasAnyAi ? `${predSummary?.models_trained ?? 0} models` : 'No AI access', trend: 'neutral' as const },
              { label: 'Packets/s', value: hasAnyInfrastructure ? (capture?.packets_per_second ? `${(capture.packets_per_second / 1000).toFixed(1)}K` : '0') : '—', color: 'text-cyan-400', sub: hasAnyInfrastructure && capture ? `${(capture.bytes_per_second / 1_000_000).toFixed(1)} MB/s` : 'No capture access', trend: 'up' as const },
            ].map((kpi, i) => (
              <div key={i} className="bg-white/5 backdrop-blur-sm border border-white/10 p-3 rounded-xl hover:bg-white/[0.08] transition-colors cursor-pointer">
                <div className="text-[10px] text-slate-400 uppercase tracking-wider mb-1">{kpi.label}</div>
                <div className="flex items-baseline gap-1.5">
                  <span className={`text-xl font-bold font-mono ${kpi.color}`}>{kpi.value}</span>
                  {kpi.value !== '—' && kpi.trend === 'up' && <ArrowUpRight className="w-3 h-3 text-emerald-400" />}
                  {kpi.value !== '—' && kpi.trend === 'down' && <ArrowDownRight className="w-3 h-3 text-red-400" />}
                </div>
                <div className="text-[10px] text-slate-500 mt-0.5">{kpi.sub}</div>
              </div>
            ))}
          </div>

          {/* ═══════════ ROW 2: Infrastructure Fleet Health ═══════════ */}
          {hasModule('infrastructure') && (
            <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
              {hasMenuItem('infrastructure.servers') && <StatCard title="Servers" value={serverCount} icon={Server} iconColor="text-blue-400" subtitle={`${targets?.filter(t => t.target_type === 'server' && t.status === 'healthy').length ?? 0} healthy`} sparkData={spark(serverCount)} sparkColor="#3b82f6" />}
              {hasMenuItem('infrastructure.storage') && <StatCard title="Storage Arrays" value={storageCount} icon={HardDrive} iconColor="text-amber-400" subtitle={`${targets?.filter(t => t.target_type === 'storage' && t.status === 'healthy').length ?? 0} healthy`} sparkData={spark(storageCount)} sparkColor="#f59e0b" />}
              {hasMenuItem('infrastructure.network') && <StatCard title="Network Devices" value={networkCount} icon={Network} iconColor="text-emerald-400" subtitle={`${targets?.filter(t => t.target_type === 'network' && t.status === 'healthy').length ?? 0} healthy`} sparkData={spark(networkCount)} sparkColor="#22c55e" />}
            </div>
          )}

          {/* ═══════════ ROW 3: Main 12-col Grid ═══════════ */}
          <div className="grid grid-cols-1 lg:grid-cols-12 gap-6">
            {/* Left: Infrastructure Status + Fleet Health Gauge */}
            {hasModule('infrastructure') && (
              <div className="lg:col-span-3 space-y-4">
                <div className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-4">
                  <h3 className="text-sm font-bold text-white uppercase tracking-wider mb-4 border-b border-white/10 pb-2">
                    Fleet Health
                  </h3>
                  <div className="flex justify-center mb-4">
                    <GaugeChart value={healthyTargets} max={totalTargets || 1} size={140} thickness={12} label="Overall Health" />
                  </div>
                  <div className="space-y-2">
                    <ProgressBar value={healthyTargets} max={totalTargets || 1} label="Healthy" color="#22c55e" size="sm" />
                    <ProgressBar value={warningTargets} max={totalTargets || 1} label="Warning" color="#f59e0b" size="sm" />
                    <ProgressBar value={criticalTargets} max={totalTargets || 1} label="Critical" color="#ef4444" size="sm" />
                  </div>
                </div>

                <StatusGrid
                  items={[
                    { label: 'Servers', status: serverCount > 0 ? 'healthy' : 'unknown', subtitle: `${serverCount} targets` },
                    { label: 'Databases', status: dbCount > 0 ? 'healthy' : 'unknown', subtitle: `${dbCount} targets` },
                    { label: 'Storage', status: storageCount > 0 ? 'healthy' : 'unknown', subtitle: `${storageCount} targets` },
                    { label: 'Network', status: networkCount > 0 ? (warningTargets > 0 ? 'warning' : 'healthy') : 'unknown', subtitle: `${networkCount} targets` },
                  ]}
                  columns={2}
                />

                <div className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-4">
                  <h4 className="text-xs font-bold text-slate-400 uppercase mb-3">Traffic Distribution</h4>
                  <DonutChart
                    size={130}
                    thickness={14}
                    centerValue={protocols ? `${(protocols.reduce((s, p) => s + p.count, 0) / 1_000_000).toFixed(1)}M` : '—'}
                    centerLabel="Packets"
                    segments={protocols?.map(p => ({
                      label: p.name,
                      value: p.count,
                      color: p.name === 'TCP' ? '#3b82f6' : p.name === 'UDP' ? '#f59e0b' : p.name === 'ICMP' ? '#8b5cf6' : '#64748b',
                    })) ?? []}
                  />
                  <div className="flex flex-wrap gap-3 mt-2 text-[10px]">
                    {(protocols ?? []).map(p => {
                      const total = protocols!.reduce((s, x) => s + x.count, 0)
                      const pct = total > 0 ? ((p.count / total) * 100).toFixed(0) : '0'
                      return (
                        <div key={p.name} className="flex items-center gap-1">
                          <div className="w-2 h-2 rounded-full" style={{ backgroundColor: p.name === 'TCP' ? '#3b82f6' : p.name === 'UDP' ? '#f59e0b' : p.name === 'ICMP' ? '#8b5cf6' : '#64748b' }} />
                          <span className="text-slate-400">{p.name} {pct}%</span>
                        </div>
                      )})}
                  </div>
                </div>
              </div>
            )}

            {/* Center: Traffic Chart / Key Metrics */}
            <div className={`space-y-6 ${hasModule('infrastructure') ? 'lg:col-span-6' : 'lg:col-span-9'}`}>
              {hasAnyInfrastructure && (
                <div className="bg-slate-900/80 border border-white/10 rounded-xl overflow-hidden relative shadow-2xl">
                  <div className="absolute top-4 left-4 z-10 bg-slate-900/90 backdrop-blur border border-white/10 px-3 py-1 rounded text-xs font-mono text-emerald-400 flex items-center gap-2">
                    <Zap className="w-3 h-3" />
                    LIVE TRAFFIC
                  </div>
                  <div className="p-4 pt-12">
                    <TrafficChart data={trafficData} height={280} />
                  </div>
                  <div className="px-4 pb-4 grid grid-cols-4 gap-2">
                    <div className="bg-slate-900/90 backdrop-blur border border-white/10 p-2.5 rounded-lg">
                      <div className="text-[10px] text-slate-400">Inbound</div>
                      <div className="text-lg font-mono font-bold text-blue-400">{capture ? `${(capture.bytes_per_second / 1_000_000).toFixed(1)} MB/s` : '—'}</div>
                    </div>
                    <div className="bg-slate-900/90 backdrop-blur border border-white/10 p-2.5 rounded-lg">
                      <div className="text-[10px] text-slate-400">Packets/s</div>
                      <div className="text-lg font-mono font-bold text-white">{capture ? `${(capture.packets_per_second / 1000).toFixed(1)}K` : '—'}</div>
                    </div>
                    <div className="bg-slate-900/90 backdrop-blur border border-white/10 p-2.5 rounded-lg">
                      <div className="text-[10px] text-slate-400">TCP</div>
                      <div className="text-lg font-mono font-bold text-blue-400">{capture?.tcp_count.toLocaleString() ?? '—'}</div>
                    </div>
                    <div className="bg-slate-900/90 backdrop-blur border border-white/10 p-2.5 rounded-lg">
                      <div className="text-[10px] text-slate-400">UDP</div>
                      <div className="text-lg font-mono font-bold text-amber-400">{capture?.udp_count.toLocaleString() ?? '—'}</div>
                    </div>
                  </div>
                </div>
              )}

              {/* Resource Heatmap */}
              {hasAnyInfrastructure && (
                <div className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-4">
                  <h3 className="text-xs font-bold text-white uppercase tracking-wider mb-3">Resource Utilization Heatmap</h3>
                  <HeatmapGrid cells={heatmapCells} cols={10} cellSize={22} />
                </div>
              )}

              {/* Key Metrics Row */}
              {hasAnyInfrastructure && (
                <div className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-5">
                  <h3 className="text-sm font-bold text-white uppercase tracking-wider mb-4 flex items-center gap-2">
                    <Activity className="w-4 h-4 text-blue-400" />
                    Key Metrics
                  </h3>
                  <div className="grid grid-cols-2 gap-4">
                    {[
                      { name: 'CPU Usage', value: '64%', sparkD: spark(64, 20), color: '#3b82f6' },
                      { name: 'Memory', value: '71%', sparkD: spark(71, 15), color: '#8b5cf6' },
                      { name: 'Disk I/O', value: '2.4K IOPS', sparkD: spark(2400, 800), color: '#f59e0b' },
                      { name: 'Network', value: '4.2 Gbps', sparkD: spark(4200, 1000), color: '#22c55e' },
                    ].map((m) => (
                      <div key={m.name} className="flex items-center gap-3">
                        <div className="flex-1">
                          <div className="flex justify-between items-baseline mb-1">
                            <span className="text-xs text-slate-400">{m.name}</span>
                            <span className="text-sm font-mono font-medium text-white">{m.value}</span>
                          </div>
                          <Sparkline data={m.sparkD} width={200} height={20} color={m.color} fillColor={m.color} />
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>

            {/* Right: Live Feed + AI Insights */}
            <div className="lg:col-span-3 space-y-4">
              {hasModule('alerts_events') && (
                <div className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-4">
                  <h3 className="text-sm font-bold text-white uppercase tracking-wider mb-4 border-b border-white/10 pb-2 flex justify-between items-center">
                    <span>Live Feed</span>
                    <span className="w-2 h-2 bg-red-500 rounded-full animate-pulse" />
                  </h3>
                  <TimelineList
                    events={[
                      ...(activeAlerts.slice(0, 3).map(a => ({
                        id: a.alert_id,
                        title: a.message,
                        timestamp: a.triggered_at ?? a.created_at,
                        severity: (a.severity === 'critical' ? 'critical' : a.severity === 'warning' ? 'warning' : 'info') as 'critical' | 'warning' | 'info',
                      }))),
                      ...(events?.events?.slice(0, 5).map((e, i) => ({
                        id: `ev-${i}`,
                        title: `${e.event_type}: ${JSON.stringify(e.payload).slice(0, 60)}`,
                        timestamp: e.timestamp,
                        severity: 'info' as const,
                      })) ?? []),
                    ]}
                    maxItems={8}
                  />
                </div>
              )}

              {/* AI Insights Panel */}
              {hasModule('ai_analytics') && (
                <div className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-4">
                  <h3 className="text-sm font-bold text-white uppercase tracking-wider mb-3 flex items-center gap-2">
                    <Brain className="w-4 h-4 text-purple-400" />
                    AI Insights
                  </h3>
                  <div className="space-y-2">
                    {(insights ?? []).slice(0, 3).map(ins => (
                      <div key={ins.id} className="p-2.5 bg-slate-900/50 rounded-lg border border-white/5">
                        <div className="flex items-center gap-2 mb-1">
                          <span className={`px-1.5 py-0.5 rounded text-[10px] font-semibold ${
                            ins.severity === 'critical' ? 'bg-red-500/20 text-red-400' :
                            ins.severity === 'warning' ? 'bg-amber-500/20 text-amber-400' :
                            'bg-blue-500/20 text-blue-400'
                          }`}>{ins.severity}</span>
                          <span className="text-xs text-white font-medium truncate">{ins.title}</span>
                        </div>
                        <p className="text-[11px] text-slate-400 line-clamp-2">{ins.description}</p>
                        <div className="flex items-center gap-1 mt-1">
                          <Eye className="w-3 h-3 text-emerald-400" />
                          <span className="text-[10px] text-emerald-300 truncate">{ins.recommendation}</span>
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {/* Predictive Summary */}
              {hasModule('ai_analytics') && predSummary && (
                <div className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-4">
                  <h3 className="text-xs font-bold text-white uppercase tracking-wider mb-3 flex items-center gap-2">
                    <TrendingUp className="w-3.5 h-3.5 text-orange-400" />
                    Predictive Engine
                  </h3>
                  <div className="grid grid-cols-3 gap-2 text-center">
                    <div><div className="text-lg font-bold font-mono text-red-400">{predSummary.active_anomalies}</div><div className="text-[10px] text-slate-500">Anomalies</div></div>
                    <div><div className="text-lg font-bold font-mono text-orange-400">{predSummary.active_forecasts}</div><div className="text-[10px] text-slate-500">Forecasts</div></div>
                    <div><div className="text-lg font-bold font-mono text-yellow-400">{predSummary.active_trend_alerts}</div><div className="text-[10px] text-slate-500">Trends</div></div>
                  </div>
                </div>
              )}
            </div>
          </div>

          {/* ═══════════ ROW 4: Bottom Section ═══════════ */}
          <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
            {/* Alert Summary */}
            {hasModule('alerts_events') && (
              <div className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-5">
                <h3 className="text-sm font-bold text-white uppercase tracking-wider mb-4 flex items-center gap-2">
                  <Bell className="w-4 h-4 text-amber-400" />
                  Alert Summary
                </h3>
                <div className="space-y-2">
                  {alerts?.slice(0, 6).map((alert) => (
                    <div key={alert.alert_id} className="flex items-center gap-2 p-2 bg-slate-900/50 rounded-lg border border-white/5">
                      <span className={`inline-flex items-center px-2 py-0.5 rounded-full text-[10px] font-semibold border ${
                        alert.severity === 'critical' ? 'bg-red-500/20 text-red-400 border-red-500/30' :
                        alert.severity === 'warning' ? 'bg-amber-500/20 text-amber-400 border-amber-500/30' :
                        'bg-blue-500/20 text-blue-400 border-blue-500/30'
                      }`}>
                        {alert.severity}
                      </span>
                      <span className="text-xs text-slate-300 flex-1 truncate">{alert.message}</span>
                      <span className={`inline-flex items-center px-1.5 py-0.5 rounded text-[10px] ${
                        alert.status === 'triggered' ? 'bg-red-500/10 text-red-400' :
                        alert.status === 'acknowledged' ? 'bg-amber-500/10 text-amber-400' :
                        'bg-emerald-500/10 text-emerald-400'
                      }`}>
                        {alert.status}
                      </span>
                    </div>
                  )) ?? (
                    <div className="text-center py-8 text-slate-500 text-sm">No alerts</div>
                  )}
                </div>
              </div>
            )}

            {/* Threat Intelligence */}
            {hasModule('security') && (
              <div className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-5">
                <h3 className="text-sm font-bold text-white uppercase tracking-wider mb-4 flex items-center gap-2">
                  <Shield className="w-4 h-4 text-red-400" />
                  Threat Intelligence
                </h3>
                <div className="grid grid-cols-2 gap-3 mb-4">
                  {[
                    { label: 'Port Scans', value: String(stats?.threats_detected ?? 12), color: 'text-amber-400', icon: Wifi },
                    { label: 'Brute Force', value: '3', color: 'text-red-400', icon: Lock },
                    { label: 'Suspicious', value: '7', color: 'text-purple-400', icon: AlertTriangle },
                    { label: 'Blocked', value: '22', color: 'text-emerald-400', icon: Shield },
                  ].map((t) => (
                    <div key={t.label} className="bg-slate-900/50 rounded-lg p-2 border border-white/5">
                      <div className="text-[10px] text-slate-500 uppercase flex items-center gap-1">
                        <t.icon className="w-3 h-3" />
                        {t.label}
                      </div>
                      <div className={`text-lg font-mono font-bold ${t.color}`}>{t.value}</div>
                    </div>
                  ))}
                </div>
                <div className="space-y-2">
                  {[
                    { ip: '192.168.1.105', type: 'Port Scan', severity: 'medium' },
                    { ip: '10.0.3.42', type: 'Brute Force', severity: 'high' },
                    { ip: '172.16.0.88', type: 'Data Exfil', severity: 'critical' },
                  ].map((t, i) => (
                    <div key={i} className="flex items-center gap-2 text-xs p-1.5 bg-slate-900/30 rounded border border-white/5">
                      <Globe className="w-3 h-3 text-slate-500" />
                      <span className="font-mono text-slate-400">{t.ip}</span>
                      <span className="text-slate-500">{t.type}</span>
                      <span className={`ml-auto px-1.5 py-0.5 rounded text-[10px] font-semibold ${
                        t.severity === 'critical' ? 'bg-red-500/20 text-red-400' :
                        t.severity === 'high' ? 'bg-amber-500/20 text-amber-400' :
                        'bg-blue-500/20 text-blue-400'
                      }`}>{t.severity}</span>
                    </div>
                  ))}
                </div>
              </div>
            )}

            {/* Network & Capture Summary */}
            {hasModule('infrastructure') && (
              <div className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-5">
                <h3 className="text-sm font-bold text-white uppercase tracking-wider mb-4 flex items-center gap-2">
                  <Network className="w-4 h-4 text-cyan-400" />
                  Network Capture
                </h3>
                {capture && (
                  <div className="space-y-3">
                    <div className="flex items-center gap-2 mb-3">
                      <span className={`w-2 h-2 rounded-full ${capture.is_running ? 'bg-emerald-500 animate-pulse' : 'bg-slate-500'}`} />
                      <span className="text-xs text-slate-300">{capture.is_running ? 'Capture Active' : 'Capture Stopped'}</span>
                      <span className="text-xs text-slate-500 ml-auto font-mono">{capture.interface_name}</span>
                    </div>
                    <div className="grid grid-cols-2 gap-2">
                      <div className="bg-slate-900/50 rounded-lg p-2 border border-white/5">
                        <div className="text-[10px] text-slate-500">Total Packets</div>
                        <div className="text-sm font-mono font-bold text-white">{(capture.packets_total / 1_000_000).toFixed(1)}M</div>
                      </div>
                      <div className="bg-slate-900/50 rounded-lg p-2 border border-white/5">
                        <div className="text-[10px] text-slate-500">Total Bytes</div>
                        <div className="text-sm font-mono font-bold text-white">{(capture.total_bytes / 1_000_000_000).toFixed(1)}GB</div>
                      </div>
                      <div className="bg-slate-900/50 rounded-lg p-2 border border-white/5">
                        <div className="text-[10px] text-slate-500">ICMP</div>
                        <div className="text-sm font-mono font-bold text-purple-400">{capture.icmp_count.toLocaleString()}</div>
                      </div>
                      <div className="bg-slate-900/50 rounded-lg p-2 border border-white/5">
                        <div className="text-[10px] text-slate-500">ARP</div>
                        <div className="text-sm font-mono font-bold text-cyan-400">{capture.arp_count.toLocaleString()}</div>
                      </div>
                    </div>
                  </div>
                )}
              </div>
            )}
          </div>
        </>
      )}
    </div>
  )
}
