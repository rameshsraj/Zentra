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

function countByStatus(items: Target[]) {
  return {
    healthy: items.filter(t => t.status === 'healthy' || t.status === 'connected').length,
    warning: items.filter(t => t.status === 'warning').length,
    critical: items.filter(t => t.status === 'critical' || t.status === 'unreachable').length,
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

      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
        {DB_TYPES.filter(dbType => (byType[dbType.key] || []).length > 0).map(dbType => {
          const items = byType[dbType.key] || []
          const status = countByStatus(items)
          return (
            <Link
              key={dbType.key}
              to={dbType.path}
              className={`${dbType.bgColor} backdrop-blur-sm border ${dbType.borderColor} rounded-xl p-5 hover:bg-white/[0.08] transition-colors`}
            >
              <div className="flex items-center gap-2 mb-4">
                <Database className={`w-5 h-5 ${dbType.color}`} />
                <h3 className="text-sm font-bold text-white">{dbType.label}</h3>
                <span className={`ml-auto text-xs font-mono ${dbType.color}`}>{items.length} instance{items.length !== 1 ? 's' : ''}</span>
              </div>
              <div className="grid grid-cols-3 gap-2 text-xs">
                <div className="bg-black/20 rounded-lg p-2">
                  <div className="flex items-center gap-1.5 text-slate-400">{statusIcon('healthy')}Healthy</div>
                  <div className="text-lg font-bold font-mono text-emerald-400 mt-1">{status.healthy}</div>
                </div>
                <div className="bg-black/20 rounded-lg p-2">
                  <div className="flex items-center gap-1.5 text-slate-400">{statusIcon('warning')}Warning</div>
                  <div className="text-lg font-bold font-mono text-amber-400 mt-1">{status.warning}</div>
                </div>
                <div className="bg-black/20 rounded-lg p-2">
                  <div className="flex items-center gap-1.5 text-slate-400">{statusIcon('critical')}Critical</div>
                  <div className="text-lg font-bold font-mono text-red-400 mt-1">{status.critical}</div>
                </div>
              </div>
            </Link>
          )
        })}
      </div>

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
