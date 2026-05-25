import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { fetchJSON } from '@/lib/api/_client'
import PageHeader from '@/components/PageHeader'
import { cn } from '@/lib/utils'
import {
  Shield, Activity, AlertTriangle, Network, Cpu, Radio,
  BarChart3, PieChart as PieChartIcon, Grid3X3, Settings,
} from 'lucide-react'

interface CiliumStatus {
  scrape_targets: { host: string; port: number; component: string }[]
  last_scrape_at: string | null
}

interface CiliumMetrics {
  endpoint_states: Record<string, number>
  drop_reasons: Record<string, number>
  bpf_map_pressure: Record<string, number>
  flow_verdicts: Record<string, number>
  policy_import_errors: number
  identity_count: number
  hubble_drops: Record<string, number>
}

interface RegisterForm {
  agent_host: string
  agent_port: number
  hubble_host: string
  hubble_port: number
  operator_host: string
  operator_port: number
}

const getCiliumStatus = () => fetchJSON<CiliumStatus>('/cilium/status')
const getCiliumMetrics = () => fetchJSON<CiliumMetrics>('/cilium/metrics/summary')
const registerCiliumTargets = (data: Partial<RegisterForm>) =>
  fetchJSON<{ registered: unknown[]; count: number }>('/cilium/register-scrape-targets', { method: 'POST', body: JSON.stringify(data) })

export default function CiliumMonitoring() {
  const [showRegister, setShowRegister] = useState(false)
  const [form, setForm] = useState<RegisterForm>({
    agent_host: '', agent_port: 9962, hubble_host: '', hubble_port: 9965, operator_host: '', operator_port: 9963,
  })

  const { data: status } = useQuery({ queryKey: ['cilium-status'], queryFn: getCiliumStatus, refetchInterval: 15000 })
  const { data: metrics } = useQuery({ queryKey: ['cilium-metrics'], queryFn: getCiliumMetrics, refetchInterval: 15000, enabled: (status?.scrape_targets?.length ?? 0) > 0 })

  const hasTargets = (status?.scrape_targets?.length ?? 0) > 0

  const handleRegister = async () => {
    const body: Record<string, unknown> = { agent_host: form.agent_host, agent_port: form.agent_port }
    if (form.hubble_host) { body.hubble_host = form.hubble_host; body.hubble_port = form.hubble_port }
    if (form.operator_host) { body.operator_host = form.operator_host; body.operator_port = form.operator_port }
    await registerCiliumTargets(body as Partial<RegisterForm>)
    setShowRegister(false)
  }

  return (
    <div className="space-y-6">
      <PageHeader
        title="eBPF Network Monitoring"
        subtitle="Cilium endpoint states, policy health, BPF map pressure, and Hubble flow statistics"
      />

      {!hasTargets ? (
        <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-8 text-center">
          <Network className="w-12 h-12 text-cyan-400 mx-auto mb-4" />
          <h3 className="text-lg font-semibold text-white mb-2">No eBPF Targets Configured</h3>
          <p className="text-slate-400 text-sm mb-4">Register your Cilium agent, Hubble relay, and operator endpoints to start monitoring.</p>
          <button onClick={() => setShowRegister(true)} className="px-4 py-2 bg-cyan-600 hover:bg-cyan-500 text-white text-sm rounded-lg transition">
            Register Targets
          </button>
        </div>
      ) : (
        <div className="space-y-6">
          {/* Status bar */}
          <div className="flex items-center gap-4 text-xs text-slate-400">
            <span className="flex items-center gap-1.5">
              <div className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse" />
              {status?.scrape_targets.length} target(s) configured
            </span>
            {status?.last_scrape_at && (
              <span>Last scrape: {new Date(status.last_scrape_at).toLocaleTimeString()}</span>
            )}
            <button onClick={() => setShowRegister(true)} className="ml-auto text-cyan-400 hover:text-cyan-300 transition">
              <Settings className="w-3.5 h-3.5" />
            </button>
          </div>

          {/* Stat cards */}
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
            <StatCard icon={Shield} label="Policy Errors" value={metrics?.policy_import_errors ?? 0} color="red" />
            <StatCard icon={Cpu} label="Identities" value={metrics?.identity_count ?? 0} color="cyan" />
            <StatCard icon={Activity} label="Endpoint States" value={Object.keys(metrics?.endpoint_states ?? {}).length} color="emerald" />
            <StatCard icon={AlertTriangle} label="Drop Reasons" value={Object.keys(metrics?.drop_reasons ?? {}).length} color="amber" />
          </div>

          {/* Grid panels */}
          <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
            <MetricPanel title="Endpoint States" icon={Grid3X3} data={metrics?.endpoint_states} color="emerald" />
            <MetricPanel title="Flow Verdicts" icon={PieChartIcon} data={metrics?.flow_verdicts} color="blue" />
            <MetricPanel title="Drop Reasons" icon={BarChart3} data={metrics?.drop_reasons} color="red" />
            <MetricPanel title="BPF Map Pressure" icon={Cpu} data={metrics?.bpf_map_pressure} color="amber" />
            <MetricPanel title="Hubble Drops" icon={Radio} data={metrics?.hubble_drops} color="orange" />
          </div>
        </div>
      )}

      {/* Register modal */}
      {showRegister && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
          <div className="bg-slate-800 border border-slate-700 rounded-xl p-6 w-full max-w-lg space-y-4">
            <h3 className="text-lg font-semibold text-white">Register eBPF Targets</h3>
            <InputField label="Agent Host *" value={form.agent_host} onChange={v => setForm(f => ({ ...f, agent_host: v }))} placeholder="10.0.0.1" />
            <InputField label="Agent Port" value={String(form.agent_port)} onChange={v => setForm(f => ({ ...f, agent_port: Number(v) }))} />
            <InputField label="Hubble Host" value={form.hubble_host} onChange={v => setForm(f => ({ ...f, hubble_host: v }))} placeholder="optional" />
            <InputField label="Hubble Port" value={String(form.hubble_port)} onChange={v => setForm(f => ({ ...f, hubble_port: Number(v) }))} />
            <InputField label="Operator Host" value={form.operator_host} onChange={v => setForm(f => ({ ...f, operator_host: v }))} placeholder="optional" />
            <InputField label="Operator Port" value={String(form.operator_port)} onChange={v => setForm(f => ({ ...f, operator_port: Number(v) }))} />
            <div className="flex justify-end gap-2">
              <button onClick={() => setShowRegister(false)} className="px-4 py-2 text-sm text-slate-400 hover:text-white transition">Cancel</button>
              <button onClick={handleRegister} disabled={!form.agent_host} className="px-4 py-2 bg-cyan-600 hover:bg-cyan-500 disabled:opacity-50 text-white text-sm rounded-lg transition">Register</button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

function StatCard({ icon: Icon, label, value, color }: { icon: React.ElementType; label: string; value: number; color: string }) {
  return (
    <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-4">
      <div className="flex items-center gap-2 mb-2">
        <Icon className={cn('w-4 h-4', `text-${color}-400`)} />
        <span className="text-xs text-slate-400">{label}</span>
      </div>
      <div className="text-2xl font-bold text-white">{value.toLocaleString()}</div>
    </div>
  )
}

function MetricPanel({ title, icon: Icon, data, color }: { title: string; icon: React.ElementType; data?: Record<string, number>; color: string }) {
  const entries = Object.entries(data ?? {})
  return (
    <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-4">
      <div className="flex items-center gap-2 mb-3">
        <Icon className={cn('w-4 h-4', `text-${color}-400`)} />
        <span className="text-sm font-medium text-white">{title}</span>
      </div>
      {entries.length === 0 ? (
        <p className="text-xs text-slate-500">No data</p>
      ) : (
        <div className="space-y-1.5">
          {entries.map(([key, val]) => (
            <div key={key} className="flex items-center justify-between text-xs">
              <span className="text-slate-400 truncate mr-2">{key}</span>
              <span className="text-white font-mono">{typeof val === 'number' ? val.toLocaleString() : String(val)}</span>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

function InputField({ label, value, onChange, placeholder }: { label: string; value: string; onChange: (v: string) => void; placeholder?: string }) {
  return (
    <div>
      <label className="text-xs text-slate-400 block mb-1">{label}</label>
      <input
        value={value} onChange={e => onChange(e.target.value)} placeholder={placeholder}
        className="w-full bg-slate-900 border border-slate-600 rounded-lg px-3 py-2 text-sm text-white focus:border-cyan-500 focus:outline-none"
      />
    </div>
  )
}
