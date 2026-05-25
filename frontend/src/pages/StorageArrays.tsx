import { useState, useCallback } from 'react'
import { useQuery, useQueryClient } from '@tanstack/react-query'
import { HardDrive, Database, Activity, ArrowRight, ChevronDown, ChevronUp, Zap } from 'lucide-react'
import PageHeader from '@/components/PageHeader'
import AddTargetModal from '@/components/AddTargetModal'
import {
  getStorageArrays, getStorageArray, getStorageVolumes, getStorageReplications, getStorageSummary,
  StorageArray, StorageVolume, StorageReplication
} from '@/lib/api'

const TYPE_META: Record<string, { label: string; color: string }> = {
  netapp_ontap: { label: 'NetApp ONTAP', color: 'text-blue-400 bg-blue-900/30 border-blue-700' },
  pure_storage: { label: 'Pure Storage', color: 'text-orange-400 bg-orange-900/30 border-orange-700' },
  ceph: { label: 'Ceph', color: 'text-red-400 bg-red-900/30 border-red-700' },
  dellemc: { label: 'Dell EMC', color: 'text-green-400 bg-green-900/30 border-green-700' },
  veeam: { label: 'Veeam', color: 'text-teal-400 bg-teal-900/30 border-teal-700' },
  minio: { label: 'MinIO', color: 'text-yellow-400 bg-yellow-900/30 border-yellow-700' },
  zfs: { label: 'ZFS', color: 'text-purple-400 bg-purple-900/30 border-purple-700' },
  ibm_gpfs: { label: 'IBM GPFS', color: 'text-indigo-400 bg-indigo-900/30 border-indigo-700' },
}

function ArrayTypeBadge({ type }: { type: string }) {
  const m = TYPE_META[type] ?? { label: type, color: 'text-gray-400 bg-gray-800 border-gray-600' }
  return <span className={`text-xs px-2 py-0.5 rounded border ${m.color}`}>{m.label}</span>
}

function StatusDot({ status }: { status: string }) {
  const cls = status === 'healthy' ? 'bg-green-400' : status === 'warning' ? 'bg-yellow-400' : status === 'critical' ? 'bg-red-400' : 'bg-gray-500'
  return <span className={`inline-block w-2 h-2 rounded-full ${cls}`} />
}

function CapacityBar({ used, total, warn = 80 }: { used: number; total: number; warn?: number }) {
  const pct = total > 0 ? (used / total) * 100 : 0
  const color = pct >= 90 ? 'bg-red-500' : pct >= warn ? 'bg-yellow-500' : 'bg-blue-500'
  return (
    <div className="w-full bg-gray-700 rounded-full h-1.5">
      <div className={`h-1.5 rounded-full ${color} transition-all`} style={{ width: `${Math.min(100, pct)}%` }} />
    </div>
  )
}

function VolumeRow({ vol }: { vol: StorageVolume }) {
  return (
    <tr className="border-b border-gray-800 hover:bg-gray-800/50 text-xs">
      <td className="py-2 px-3 text-white font-mono">{vol.name}</td>
      <td className="py-2 px-3 text-gray-400">{vol.protocol}</td>
      <td className="py-2 px-3">
        <div className="text-gray-300">{vol.used_tb.toFixed(1)} / {vol.size_tb.toFixed(1)} TB</div>
        <CapacityBar used={vol.used_tb} total={vol.size_tb} />
      </td>
      <td className="py-2 px-3 text-gray-300">{vol.iops.toLocaleString()}</td>
      <td className="py-2 px-3 text-gray-300">{vol.latency_ms.toFixed(1)} ms</td>
      <td className="py-2 px-3 text-gray-300">{vol.snapshot_count}</td>
      <td className="py-2 px-3">
        <span className={`text-xs ${vol.status === 'online' ? 'text-green-400' : 'text-red-400'}`}>{vol.status}</span>
      </td>
    </tr>
  )
}

function ArrayDetail({ arrayId }: { arrayId: string }) {
  const { data: array } = useQuery({ queryKey: ['storage-array', arrayId], queryFn: () => getStorageArray(arrayId) })
  const { data: volumes = [] } = useQuery({ queryKey: ['storage-volumes', arrayId], queryFn: () => getStorageVolumes(arrayId) })
  if (!array) return <div className="p-4 text-gray-500 text-sm">Loading...</div>

  return (
    <div className="border-t border-gray-700 p-4 space-y-4 bg-gray-950/50">
      <div className="grid grid-cols-2 md:grid-cols-4 gap-4 text-xs">
        {[
          { label: 'Firmware', value: array.firmware },
          { label: 'Protocol', value: array.protocol },
          { label: 'Location', value: array.location },
          { label: 'Data Reduction', value: `${array.data_reduction_ratio}:1` },
        ].map(({ label, value }) => (
          <div key={label}>
            <div className="text-gray-500">{label}</div>
            <div className="text-gray-200 font-mono">{value}</div>
          </div>
        ))}
      </div>

      <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
        {[
          { label: 'IOPS', value: array.iops.toLocaleString(), sub: 'current' },
          { label: 'Throughput', value: `${array.throughput_mb.toFixed(0)} MB/s`, sub: 'current' },
          { label: 'Latency', value: `${array.latency_ms.toFixed(1)} ms`, sub: 'avg' },
          { label: 'Snapshots', value: array.snapshot_count.toString(), sub: `${array.volume_count} volumes` },
        ].map(({ label, value, sub }) => (
          <div key={label} className="bg-gray-800 rounded p-2.5 text-xs">
            <div className="text-gray-400">{label}</div>
            <div className="text-white font-semibold text-base">{value}</div>
            <div className="text-gray-500">{sub}</div>
          </div>
        ))}
      </div>

      {array.replication_enabled && (
        <div className="flex items-center gap-2 text-xs text-green-400 bg-green-900/20 border border-green-800 rounded px-3 py-2">
          <ArrowRight className="w-3 h-3" />
          Replication active
          {array.replication_lag_secs != null && (
            <span className="text-gray-400 ml-1">— lag: <span className="text-white">{array.replication_lag_secs.toFixed(1)}s</span></span>
          )}
        </div>
      )}

      {volumes.length > 0 && (
        <div>
          <div className="text-xs font-medium text-gray-300 mb-2">Volumes / Pools</div>
          <div className="bg-gray-900 border border-gray-700 rounded overflow-hidden">
            <table className="w-full">
              <thead>
                <tr className="text-xs text-gray-500 border-b border-gray-700 bg-gray-800/50">
                  <th className="text-left py-2 px-3">Name</th>
                  <th className="text-left py-2 px-3">Protocol</th>
                  <th className="text-left py-2 px-3">Capacity</th>
                  <th className="text-left py-2 px-3">IOPS</th>
                  <th className="text-left py-2 px-3">Latency</th>
                  <th className="text-left py-2 px-3">Snaps</th>
                  <th className="text-left py-2 px-3">Status</th>
                </tr>
              </thead>
              <tbody>
                {volumes.map(v => <VolumeRow key={v.id} vol={v} />)}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  )
}

function ArrayRow({ array }: { array: StorageArray }) {
  const [expanded, setExpanded] = useState(false)
  const pct = array.utilization_pct

  return (
    <div className="bg-gray-900 border border-gray-700 rounded-lg overflow-hidden">
      <div className="p-4 cursor-pointer hover:bg-gray-800/30 transition-colors" onClick={() => setExpanded(e => !e)}>
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-3">
            <StatusDot status={array.status} />
            <div>
              <div className="flex items-center gap-2">
                <span className="font-medium text-white text-sm">{array.name}</span>
                <ArrayTypeBadge type={array.array_type} />
              </div>
              <div className="text-xs text-gray-400 mt-0.5">{array.vendor} {array.model} · {array.ip}</div>
            </div>
          </div>

          <div className="flex items-center gap-6 text-xs text-right">
            <div>
              <div className="text-gray-500">Capacity</div>
              <div className="text-white">{array.used_tb.toFixed(1)} / {array.total_tb.toFixed(0)} TB</div>
              <div className="w-24 mt-1">
                <CapacityBar used={array.used_tb} total={array.total_tb} />
                <div className={`text-right mt-0.5 ${pct >= 90 ? 'text-red-400' : pct >= 80 ? 'text-yellow-400' : 'text-gray-500'}`}>
                  {pct.toFixed(0)}%
                </div>
              </div>
            </div>
            <div className="hidden md:block">
              <div className="text-gray-500">IOPS</div>
              <div className="text-white">{array.iops.toLocaleString()}</div>
            </div>
            <div className="hidden md:block">
              <div className="text-gray-500">Latency</div>
              <div className={array.latency_ms > 5 ? 'text-yellow-400' : 'text-white'}>{array.latency_ms.toFixed(1)} ms</div>
            </div>
            <div className="hidden lg:block">
              <div className="text-gray-500">Throughput</div>
              <div className="text-white">{array.throughput_mb.toFixed(0)} MB/s</div>
            </div>
            {expanded ? <ChevronUp className="w-4 h-4 text-gray-400" /> : <ChevronDown className="w-4 h-4 text-gray-400" />}
          </div>
        </div>
      </div>
      {expanded && <ArrayDetail arrayId={array.id} />}
    </div>
  )
}

function ReplicationTable({ replications, arrays }: { replications: StorageReplication[]; arrays: StorageArray[] }) {
  const arrayName = (id: string) => arrays.find(a => a.id === id)?.name ?? id
  return (
    <div className="bg-gray-900 border border-gray-700 rounded-lg overflow-hidden">
      <table className="w-full text-sm">
        <thead>
          <tr className="text-xs text-gray-400 border-b border-gray-700 bg-gray-800/50">
            <th className="text-left py-2 px-4">Source</th>
            <th className="text-left py-2 px-4">Target</th>
            <th className="text-left py-2 px-4">Type</th>
            <th className="text-left py-2 px-4">Status</th>
            <th className="text-right py-2 px-4">Lag</th>
            <th className="text-right py-2 px-4">Rate</th>
            <th className="text-right py-2 px-4">Last Transfer</th>
          </tr>
        </thead>
        <tbody>
          {replications.map(r => (
            <tr key={r.id} className="border-b border-gray-800 hover:bg-gray-800/50 text-xs">
              <td className="py-2.5 px-4 text-white font-mono">{arrayName(r.source_array_id)}</td>
              <td className="py-2.5 px-4 text-gray-300 font-mono">{arrayName(r.target_array_id)}</td>
              <td className="py-2.5 px-4">
                <span className={`capitalize ${r.relationship_type === 'sync' ? 'text-green-400' : 'text-blue-400'}`}>{r.relationship_type}</span>
              </td>
              <td className="py-2.5 px-4 text-green-400 capitalize">{r.status}</td>
              <td className="py-2.5 px-4 text-right text-gray-300">{r.lag_secs.toFixed(1)}s</td>
              <td className="py-2.5 px-4 text-right text-gray-300">{r.transfer_rate_mb.toFixed(0)} MB/s</td>
              <td className="py-2.5 px-4 text-right text-gray-400">{r.last_transfer_size_gb.toFixed(1)} GB</td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

const ALL_TYPES = ['all', 'netapp_ontap', 'pure_storage', 'ceph', 'dellemc', 'veeam', 'minio']

export default function StorageArraysPage() {
  const qc = useQueryClient()
  const [typeFilter, setTypeFilter] = useState('all')
  const [tab, setTab] = useState<'arrays' | 'replication'>('arrays')
  const [showAddModal, setShowAddModal] = useState(false)
  const handleRefresh = useCallback(() => {
    qc.invalidateQueries({ queryKey: ['storage-arrays'] })
    qc.invalidateQueries({ queryKey: ['storage-summary'] })
    qc.invalidateQueries({ queryKey: ['storage-replications'] })
  }, [qc])

  const { data: arrays = [], isLoading } = useQuery({
    queryKey: ['storage-arrays'],
    queryFn: () => getStorageArrays(),
    refetchInterval: 30000,
  })
  const { data: replications = [] } = useQuery({
    queryKey: ['storage-replications'],
    queryFn: getStorageReplications,
  })
  const { data: summary } = useQuery({
    queryKey: ['storage-summary'],
    queryFn: getStorageSummary,
  })

  const filtered = typeFilter === 'all' ? arrays : arrays.filter(a => a.array_type === typeFilter)

  if (isLoading) return <div className="p-6 text-gray-500">Loading storage arrays...</div>

  return (
    <div className="p-6 space-y-6">
      <PageHeader
        title="Enterprise Storage Arrays"
        subtitle="NetApp ONTAP, Pure Storage, Ceph, Dell EMC, Veeam, MinIO — capacity & replication"
        onRefresh={handleRefresh}
        showAddTarget
        onAddTarget={() => setShowAddModal(true)}
        addTargetLabel="Add Storage Array"
      />

      {summary && (
        <div className="grid grid-cols-2 md:grid-cols-5 gap-4">
          {[
            { label: 'Arrays', value: summary.total_arrays, icon: Database, sub: `${summary.healthy} healthy` },
            { label: 'Total Capacity', value: `${(summary.total_tb / 1024).toFixed(1)} PB`, icon: HardDrive, sub: `${summary.utilization_pct}% used` },
            { label: 'Total IOPS', value: summary.total_iops?.toLocaleString(), icon: Zap, sub: 'across all arrays' },
            { label: 'Throughput', value: `${(summary.total_throughput_mb / 1024).toFixed(1)} GB/s`, icon: Activity, sub: 'aggregate' },
            { label: 'Replication', value: summary.replication_relationships, icon: ArrowRight, sub: 'active links' },
          ].map(({ label, value, icon: Icon, sub }) => (
            <div key={label} className="bg-gray-900 border border-gray-700 rounded-lg p-4">
              <div className="flex items-center gap-2 text-gray-400 text-xs mb-1">
                <Icon className="w-4 h-4" />{label}
              </div>
              <div className="text-xl font-bold text-white">{value}</div>
              <div className="text-xs text-gray-500 mt-0.5">{sub}</div>
            </div>
          ))}
        </div>
      )}

      <div className="flex items-center justify-between">
        <div className="flex gap-1">
          {ALL_TYPES.map(t => (
            <button key={t} onClick={() => setTypeFilter(t)}
              className={`px-3 py-1.5 text-xs rounded capitalize transition-colors ${typeFilter === t ? 'bg-blue-600 text-white' : 'bg-gray-800 text-gray-400 hover:text-white'}`}>
              {t === 'all' ? 'All' : (TYPE_META[t]?.label ?? t)}
              {t !== 'all' && ` (${arrays.filter(a => a.array_type === t).length})`}
            </button>
          ))}
        </div>
        <div className="flex gap-2">
          {(['arrays', 'replication'] as const).map(v => (
            <button key={v} onClick={() => setTab(v)}
              className={`px-3 py-1.5 text-xs capitalize rounded ${tab === v ? 'bg-gray-700 text-white' : 'text-gray-400 hover:text-white'}`}>
              {v}
            </button>
          ))}
        </div>
      </div>

      {tab === 'arrays' && (
        <div className="space-y-3">
          {filtered.map(a => <ArrayRow key={a.id} array={a} />)}
          {filtered.length === 0 && (
            <div className="text-center py-12 text-gray-500">
              <HardDrive className="w-10 h-10 mx-auto mb-3 opacity-30" />
              <div>No storage arrays found</div>
            </div>
          )}
        </div>
      )}

      {tab === 'replication' && (
        <div>
          <h2 className="font-semibold text-white mb-3 flex items-center gap-2">
            <ArrowRight className="w-4 h-4" /> Replication Relationships
          </h2>
          {replications.length > 0
            ? <ReplicationTable replications={replications} arrays={arrays} />
            : <div className="text-center py-12 text-gray-500">No replication relationships configured</div>
          }
        </div>
      )}

      <AddTargetModal
        open={showAddModal}
        onClose={() => setShowAddModal(false)}
        defaultType="storage"
        title="Add Storage Array"
      />
    </div>
  )
}
