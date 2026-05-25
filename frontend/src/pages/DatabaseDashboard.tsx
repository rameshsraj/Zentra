import { useCallback, useState } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { getTargets, type Target } from '@/lib/api'
import PageHeader from '@/components/PageHeader'
import AddDatabaseModal from '@/components/AddDatabaseModal'
import { Database, Activity, CheckCircle, AlertTriangle, XCircle } from 'lucide-react'
import LoadingSkeleton from '@/components/LoadingSkeleton'
import { useMemo } from 'react'
import { Link } from 'react-router-dom'

const DB_TYPES = [
  { key: 'oracle', label: 'Oracle', path: '/oracle', color: 'text-red-400', bgColor: 'bg-red-500/10', borderColor: 'border-red-500/20' },
]

function statusIcon(status: string) {
  switch (status) {
    case 'healthy': case 'connected': return <CheckCircle className="w-3.5 h-3.5 text-emerald-400" />
    case 'warning': return <AlertTriangle className="w-3.5 h-3.5 text-amber-400" />
    case 'critical': case 'unreachable': return <XCircle className="w-3.5 h-3.5 text-red-400" />
    default: return <Activity className="w-3.5 h-3.5 text-slate-400" />
  }
}

export default function DatabaseDashboardPage() {
  const qc = useQueryClient()
  const { data: targets, isLoading } = useQuery({ queryKey: ['targets'], queryFn: getTargets })
  const [showAddModal, setShowAddModal] = useState(false)

  const dbTargets = useMemo(() => (targets ?? []).filter(t => t.target_type === 'database'), [targets])

  const handleRefresh = useCallback(() => {
    qc.invalidateQueries({ queryKey: ['targets'] })
  }, [qc])

  const byType = useMemo(() => {
    const map: Record<string, Target[]> = {}
    for (const t of dbTargets) {
      const sub = (t.target_subtype || '').toLowerCase()
      const key = sub.includes('oracle') ? 'oracle' : sub || 'other'
      if (!map[key]) map[key] = []
      map[key].push(t)
    }
    return map
  }, [dbTargets])

  const healthy = dbTargets.filter(t => t.status === 'healthy' || t.status === 'connected').length
  const warning = dbTargets.filter(t => t.status === 'warning').length
  const critical = dbTargets.filter(t => t.status === 'critical' || t.status === 'unreachable').length

  return (
    <div className="space-y-6" role="main" aria-label="Database Dashboard">
      <PageHeader
        title="Database Dashboard"
        subtitle={`${dbTargets.length} databases monitored · ${healthy} healthy · ${warning} warning · ${critical} critical`}
        onRefresh={handleRefresh}
        showAddTarget
        onAddTarget={() => setShowAddModal(true)}
        addTargetLabel="Add Database"
      />
      <AddDatabaseModal open={showAddModal} onClose={() => setShowAddModal(false)} dbType="oracle" />

      {/* KPI Strip */}
      <div className="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-5 gap-3" role="region" aria-label="Database statistics">
        {[
          { label: 'Total Databases', value: String(dbTargets.length), color: 'text-white' },
          { label: 'Healthy', value: String(healthy), color: 'text-emerald-400' },
          { label: 'Warning', value: String(warning), color: 'text-amber-400' },
          { label: 'Critical', value: String(critical), color: 'text-red-400' },
          { label: 'DB Types', value: String(Object.keys(byType).length), color: 'text-purple-400' },
        ].map((kpi, i) => (
          <div key={i} className="bg-white/5 backdrop-blur-sm border border-white/10 p-3 rounded-xl">
            <div className="text-[10px] text-slate-400 uppercase tracking-wider mb-1">{kpi.label}</div>
            <div className={`text-2xl font-bold font-mono ${kpi.color}`}>{kpi.value}</div>
          </div>
        ))}
      </div>

      {/* Database Type Cards — only show types that have instances */}
      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
        {DB_TYPES.filter(dbType => (byType[dbType.key] || []).length > 0).map(dbType => {
          const items = byType[dbType.key] || []
          return (
            <Link
              key={dbType.key}
              to={dbType.path}
              className={`${dbType.bgColor} backdrop-blur-sm border ${dbType.borderColor} rounded-xl p-5 hover:bg-white/[0.08] transition-colors`}
            >
              <div className="flex items-center gap-2 mb-3">
                <Database className={`w-5 h-5 ${dbType.color}`} />
                <h3 className="text-sm font-bold text-white">{dbType.label}</h3>
                <span className={`ml-auto text-xs font-mono ${dbType.color}`}>{items.length} instance{items.length !== 1 ? 's' : ''}</span>
              </div>
              <div className="space-y-2">
                {items.slice(0, 5).map(t => (
                  <div key={t.target_id} className="flex items-center gap-2 text-xs">
                    {statusIcon(t.status)}
                    <span className="text-white truncate flex-1">{t.name}</span>
                    <span className="text-slate-500">{t.host}:{t.port}</span>
                  </div>
                ))}
                {items.length > 5 && <div className="text-[10px] text-slate-500">+{items.length - 5} more</div>}
              </div>
            </Link>
          )
        })}
      </div>

      {/* All Database Targets Table */}
      {dbTargets.length > 0 && (
        <div className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl overflow-hidden">
          <div className="px-5 py-3 border-b border-white/10">
            <h3 className="text-sm font-bold text-white">All Database Instances</h3>
          </div>
          <div className="overflow-x-auto">
            <table className="w-full text-xs" aria-label="Database instances">
              <thead>
                <tr className="border-b border-white/10 text-slate-400">
                  <th className="px-4 py-2 text-left">Status</th>
                  <th className="px-4 py-2 text-left">Name</th>
                  <th className="px-4 py-2 text-left">Type</th>
                  <th className="px-4 py-2 text-left">Host</th>
                  <th className="px-4 py-2 text-left">Port</th>
                </tr>
              </thead>
              <tbody>
                {dbTargets.map(t => (
                  <tr key={t.target_id} className="border-b border-white/5 hover:bg-white/5">
                    <td className="px-4 py-2">{statusIcon(t.status)}</td>
                    <td className="px-4 py-2 text-white font-medium">{t.name}</td>
                    <td className="px-4 py-2 text-slate-400 capitalize">{t.target_subtype || '—'}</td>
                    <td className="px-4 py-2 text-slate-400 font-mono">{t.host}</td>
                    <td className="px-4 py-2 text-slate-400 font-mono">{t.port}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {isLoading && <LoadingSkeleton variant="card" count={3} />}

      {!isLoading && dbTargets.length === 0 && (
        <div className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-12 text-center">
          <Database className="w-12 h-12 text-slate-600 mx-auto mb-3" />
          <h3 className="text-lg font-bold text-white mb-1">No databases configured</h3>
          <p className="text-sm text-slate-400">Add Oracle database targets to start monitoring.</p>
        </div>
      )}
    </div>
  )
}
