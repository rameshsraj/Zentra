import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { Bell, Plus, Trash2, ToggleLeft, ToggleRight } from 'lucide-react'
import { getLogAlerts, createLogAlert, updateLogAlert, deleteLogAlert, type LogAlert } from '@/lib/api'

export default function LogAlertsPage() {
  const qc = useQueryClient()
  const [showCreate, setShowCreate] = useState(false)
  const { data: alerts = [] } = useQuery({ queryKey: ['log-alerts'], queryFn: getLogAlerts })
  const createMut = useMutation({ mutationFn: createLogAlert, onSuccess: () => { qc.invalidateQueries({ queryKey: ['log-alerts'] }); setShowCreate(false) } })
  const updateMut = useMutation({ mutationFn: ({ id, data }: { id: string; data: any }) => updateLogAlert(id, data), onSuccess: () => qc.invalidateQueries({ queryKey: ['log-alerts'] }) })
  const deleteMut = useMutation({ mutationFn: deleteLogAlert, onSuccess: () => qc.invalidateQueries({ queryKey: ['log-alerts'] }) })

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <h1 className="text-2xl font-bold text-white flex items-center gap-2"><Bell className="w-6 h-6 text-blue-400" /> Log Alerts</h1>
        <button onClick={() => setShowCreate(true)} className="flex items-center gap-1 px-3 py-1.5 bg-blue-600 hover:bg-blue-700 text-white rounded text-sm"><Plus className="w-4 h-4" /> Create Alert</button>
      </div>

      <div className="grid gap-4">
        {alerts.map((a: LogAlert) => (
          <div key={a.id} className="bg-gray-800 rounded-lg p-5 border border-gray-700">
            <div className="flex items-center justify-between">
              <div className="flex items-center gap-3">
                <button onClick={() => updateMut.mutate({ id: a.id, data: { ...a, enabled: !a.enabled } })} className="text-gray-400 hover:text-white">
                  {a.enabled ? <ToggleRight className="w-6 h-6 text-green-400" /> : <ToggleLeft className="w-6 h-6 text-gray-500" />}
                </button>
                <div>
                  <h3 className="text-white font-medium">{a.name}</h3>
                  <p className="text-gray-400 text-sm font-mono mt-1">{a.query}</p>
                </div>
              </div>
              <div className="flex items-center gap-3">
                <span className={`px-2 py-0.5 rounded text-xs border ${a.severity === 'critical' ? 'bg-red-500/20 text-red-400 border-red-500/30' : a.severity === 'warning' ? 'bg-yellow-500/20 text-yellow-400 border-yellow-500/30' : 'bg-blue-500/20 text-blue-400 border-blue-500/30'}`}>{a.severity}</span>
                <span className="text-gray-400 text-sm">{a.comparison} {a.threshold}</span>
                <button onClick={() => deleteMut.mutate(a.id)} className="p-1 text-red-400 hover:text-red-300"><Trash2 className="w-4 h-4" /></button>
              </div>
            </div>
            <div className="flex gap-6 mt-3 text-xs text-gray-500">
              <span>Created: {new Date(a.created_at).toLocaleString()}</span>
              {a.last_fired && <span className="text-yellow-400">Last fired: {new Date(a.last_fired).toLocaleString()}</span>}
              <span>{a.enabled ? 'Active' : 'Disabled'}</span>
            </div>
          </div>
        ))}
        {alerts.length === 0 && <div className="text-center text-gray-500 py-12">No log alerts configured</div>}
      </div>

      {showCreate && (
        <div className="fixed inset-0 bg-black/50 flex items-center justify-center z-50" onClick={() => setShowCreate(false)}>
          <div className="bg-gray-800 rounded-lg p-6 w-[500px] border border-gray-700" onClick={e => e.stopPropagation()}>
            <h2 className="text-lg font-bold text-white mb-4">Create Log Alert</h2>
            <form onSubmit={e => { e.preventDefault(); const fd = new FormData(e.currentTarget); createMut.mutate({ name: fd.get('name'), query: fd.get('query'), threshold: parseFloat(fd.get('threshold') as string) || 10, comparison: fd.get('comparison'), severity: fd.get('severity') }) }} className="space-y-3">
              <input name="name" placeholder="Alert name" required className="w-full bg-gray-700 text-white rounded px-3 py-2 text-sm border border-gray-600" />
              <input name="query" placeholder='LogQL query, e.g. {job="api"} |= "error"' required className="w-full bg-gray-700 text-white rounded px-3 py-2 text-sm border border-gray-600 font-mono" />
              <div className="grid grid-cols-3 gap-2">
                <select name="comparison" className="bg-gray-700 text-white rounded px-3 py-2 text-sm border border-gray-600"><option value=">">&gt;</option><option value=">=">&gt;=</option><option value="<">&lt;</option><option value="=">=</option></select>
                <input name="threshold" type="number" defaultValue={10} placeholder="Threshold" className="bg-gray-700 text-white rounded px-3 py-2 text-sm border border-gray-600" />
                <select name="severity" className="bg-gray-700 text-white rounded px-3 py-2 text-sm border border-gray-600"><option value="warning">Warning</option><option value="critical">Critical</option><option value="info">Info</option></select>
              </div>
              <div className="flex gap-2 justify-end"><button type="button" onClick={() => setShowCreate(false)} className="px-4 py-2 bg-gray-700 text-gray-300 rounded text-sm">Cancel</button><button type="submit" className="px-4 py-2 bg-blue-600 text-white rounded text-sm">Create</button></div>
            </form>
          </div>
        </div>
      )}
    </div>
  )
}
