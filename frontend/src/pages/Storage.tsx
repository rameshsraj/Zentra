import { useMemo, useState, useCallback } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { getStorageArrays } from '@/lib/api'
import DonutChart from '@/components/charts/DonutChart'
import Sparkline from '@/components/charts/Sparkline'
import PageHeader from '@/components/PageHeader'
import AddTargetModal from '@/components/AddTargetModal'
import { HardDrive } from 'lucide-react'

const spark = (base: number, _v?: number, len = 20) =>
  Array.from({ length: len }, (_, i) => Math.max(0, base + Math.sin(base * 0.1 + i * 0.5) * (base * 0.08)))

type SortField = 'name' | 'capacity' | 'used' | 'iops' | 'latency'

export default function StoragePage() {
  const qc = useQueryClient()
  const { data: rawArrays = [] } = useQuery({ queryKey: ['storage-arrays'], queryFn: () => getStorageArrays() })
  const [sortField, setSortField] = useState<SortField>('name')
  const [sortOrder, setSortOrder] = useState<'asc' | 'desc'>('asc')
  const [showAddModal, setShowAddModal] = useState(false)
  const handleRefresh = useCallback(() => qc.invalidateQueries({ queryKey: ['storage-arrays'] }), [qc])

  const arrays = rawArrays.map((a, i) => ({
    id: a.id || String(i),
    name: a.name,
    vendor: `${a.vendor} ${a.model}`.trim(),
    type: a.protocol || a.array_type,
    capacity: a.total_tb || 0,
    used: a.utilization_pct || 0,
    iops: a.iops || 0,
    latency: a.latency_ms || 0,
    status: a.status,
  }))
  const totalCap = arrays.reduce((a, s) => a + s.capacity, 0)
  const usedCap = arrays.reduce((a, s) => a + (s.capacity * s.used / 100), 0)
  const totalIops = arrays.reduce((a, s) => a + s.iops, 0)

  const sorted = useMemo(() => {
    const arr = [...arrays]
    arr.sort((a, b) => {
      const av = a[sortField]
      const bv = b[sortField]
      if (typeof av === 'string' && typeof bv === 'string') return sortOrder === 'asc' ? av.localeCompare(bv) : bv.localeCompare(av)
      return sortOrder === 'asc' ? (av as number) - (bv as number) : (bv as number) - (av as number)
    })
    return arr
  }, [arrays, sortField, sortOrder])

  const toggleSort = (field: SortField) => {
    if (sortField === field) setSortOrder(o => o === 'asc' ? 'desc' : 'asc')
    else { setSortField(field); setSortOrder('asc') }
  }

  return (
    <div className="space-y-6">
      <PageHeader
        title="Storage Infrastructure"
        subtitle={`SAN, NAS, DAS monitoring \u00b7 ${(totalCap / 1000).toFixed(1)} PB total capacity \u00b7 ${arrays.length} arrays`}
        onRefresh={handleRefresh}
        onSearch={() => {}}
        searchPlaceholder="Search storage arrays..."
        showAddTarget
        onAddTarget={() => setShowAddModal(true)}
        addTargetLabel="Add Storage Array"
      />

      {/* KPI Strip */}
      <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-5 gap-3">
        {[
          { label: 'Total Capacity', value: `${(totalCap / 1000).toFixed(1)} PB`, sub: `Raw: ${(totalCap * 1.25 / 1000).toFixed(1)} PB`, color: 'text-white' },
          { label: 'Used', value: `${(usedCap / 1000).toFixed(1)} PB`, sub: totalCap > 0 ? `${Math.round(usedCap / totalCap * 100)}% utilization` : '—', color: 'text-blue-400' },
          { label: 'Available', value: `${((totalCap - usedCap) / 1000).toFixed(1)} PB`, sub: totalCap > 0 ? `${Math.round((1 - usedCap / totalCap) * 100)}% remaining` : '—', color: 'text-emerald-400' },
          { label: 'Storage Arrays', value: String(arrays.length), sub: arrays.length > 0 ? 'All online' : 'None configured', color: 'text-white' },
          { label: 'Total IOPS', value: `${(totalIops / 1000).toFixed(0)}K`, sub: 'Aggregate', color: 'text-purple-400' },
        ].map((kpi, i) => (
          <div key={i} className="bg-white/5 backdrop-blur-sm border border-white/10 p-3 rounded-xl">
            <div className="text-[10px] text-slate-400 uppercase tracking-wider mb-1">{kpi.label}</div>
            <div className={`text-xl font-bold font-mono ${kpi.color}`}>{kpi.value}</div>
            <div className="text-[10px] text-slate-500 mt-0.5">{kpi.sub}</div>
          </div>
        ))}
      </div>

      {/* Capacity Overview */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        <div className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-5">
          <h3 className="text-sm font-bold text-white uppercase tracking-wider mb-4">Capacity Distribution</h3>
          <div className="flex justify-center">
            <DonutChart
              size={180}
              thickness={20}
              centerValue={`${totalCap > 0 ? Math.round(usedCap / totalCap * 100) : 0}%`}
              centerLabel="Used"
              segments={arrays.map((a, i) => ({
                label: a.name,
                value: a.capacity * a.used / 100,
                color: ['#3b82f6', '#8b5cf6', '#f59e0b', '#22c55e', '#ec4899'][i],
              }))}
            />
          </div>
          <div className="flex flex-wrap gap-2 mt-4 justify-center">
            {arrays.map((a, i) => (
              <div key={a.id} className="flex items-center gap-1 text-[10px]">
                <div className="w-2 h-2 rounded-full" style={{ backgroundColor: ['#3b82f6', '#8b5cf6', '#f59e0b', '#22c55e', '#ec4899'][i] }} />
                <span className="text-slate-400">{a.name}</span>
              </div>
            ))}
          </div>
        </div>

        <div className="lg:col-span-2 bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-5">
          <h3 className="text-sm font-bold text-white uppercase tracking-wider mb-4">IOPS by Array</h3>
          <div className="space-y-4">
            {arrays.map(a => (
              <div key={a.id} className="flex items-center gap-3">
                <div className="w-28 flex-shrink-0">
                  <div className="text-xs font-medium text-white">{a.name}</div>
                  <div className="text-[10px] text-slate-500">{a.type}</div>
                </div>
                <div className="flex-1">
                  <div className="h-5 bg-slate-900/50 rounded-full overflow-hidden">
                    <div
                      className="h-full bg-gradient-to-r from-brand-600 to-brand-400 rounded-full transition-all duration-700"
                      style={{ width: `${(a.iops / 200000) * 100}%` }}
                    />
                  </div>
                </div>
                <div className="w-20 text-right">
                  <span className="text-xs font-mono text-white">{(a.iops / 1000).toFixed(0)}K</span>
                  <span className="text-[10px] text-slate-500 ml-1">IOPS</span>
                </div>
                <Sparkline data={spark(a.iops / 1000, a.iops / 5000)} width={60} height={16} color="#3b82f6" />
              </div>
            ))}
          </div>
        </div>
      </div>

      {/* Storage Array Table */}
      <div className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl overflow-hidden">
        <div className="px-5 py-4 border-b border-white/10 flex items-center gap-2">
          <HardDrive className="w-4 h-4 text-blue-400" />
          <h3 className="text-sm font-bold text-white uppercase tracking-wider">Storage Arrays</h3>
        </div>
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="bg-white/5">
                {[
                  { key: 'name' as SortField, label: 'Array' },
                  { key: 'capacity' as SortField, label: 'Capacity (TB)' },
                  { key: 'used' as SortField, label: 'Used %' },
                  { key: 'iops' as SortField, label: 'IOPS' },
                  { key: 'latency' as SortField, label: 'Latency (ms)' },
                ].map(col => (
                  <th
                    key={col.key}
                    onClick={() => toggleSort(col.key)}
                    className="px-5 py-3 text-left text-xs font-semibold text-slate-400 uppercase tracking-wider cursor-pointer hover:text-white transition-colors"
                  >
                    {col.label} {sortField === col.key ? (sortOrder === 'asc' ? '↑' : '↓') : ''}
                  </th>
                ))}
                <th className="px-5 py-3 text-left text-xs font-semibold text-slate-400 uppercase tracking-wider">Status</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-white/5">
              {sorted.map(a => (
                <tr key={a.id} className="hover:bg-white/5 transition-colors">
                  <td className="px-5 py-3">
                    <div className="text-white font-medium">{a.name}</div>
                    <div className="text-[10px] text-slate-500">{a.vendor} &bull; {a.type}</div>
                  </td>
                  <td className="px-5 py-3 font-mono text-slate-300">{a.capacity}</td>
                  <td className="px-5 py-3">
                    <div className="flex items-center gap-2">
                      <div className="w-16 h-2 bg-slate-800 rounded-full overflow-hidden">
                        <div
                          className={`h-full rounded-full ${a.used >= 85 ? 'bg-red-500' : a.used >= 70 ? 'bg-amber-500' : 'bg-emerald-500'}`}
                          style={{ width: `${a.used}%` }}
                        />
                      </div>
                      <span className="text-xs font-mono text-slate-300">{a.used}%</span>
                    </div>
                  </td>
                  <td className="px-5 py-3 font-mono text-slate-300">{(a.iops / 1000).toFixed(0)}K</td>
                  <td className="px-5 py-3 font-mono text-slate-300">{a.latency}ms</td>
                  <td className="px-5 py-3">
                    <span className={`inline-flex items-center px-2.5 py-0.5 rounded-full text-[10px] font-semibold border ${
                      a.status === 'healthy' ? 'bg-emerald-500/20 text-emerald-400 border-emerald-500/30' :
                      a.status === 'warning' ? 'bg-amber-500/20 text-amber-400 border-amber-500/30' :
                      'bg-red-500/20 text-red-400 border-red-500/30'
                    }`}>{a.status}</span>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      </div>
      <AddTargetModal open={showAddModal} onClose={() => setShowAddModal(false)} defaultType="storage" title="Add Storage Array" />
    </div>
  )
}
