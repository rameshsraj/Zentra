import { useState, useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import { fetchJSON } from '@/lib/api/_client'
import PageHeader from '@/components/PageHeader'
import { cn } from '@/lib/utils'
import {
  Settings, Copy, Download, CheckCircle, Terminal, Globe,
  Activity, ScrollText, Cpu, Radio, Server,
} from 'lucide-react'

interface OtelStatus {
  signals: string[]
  endpoints: { http: Record<string, string>; grpc: { port: number; status: string } }
  encodings: string[]
  auth: string[]
}

const getOtelStatus = () => fetchJSON<OtelStatus>('/otel/status')

export default function OTelCollectorConfig() {
  const [host, setHost] = useState('localhost')
  const [port, setPort] = useState('3001')
  const [protocol, setProtocol] = useState('http/json')
  const [traces, setTraces] = useState(true)
  const [metrics, setMetrics] = useState(true)
  const [logs, setLogs] = useState(true)
  const [profiles, setProfiles] = useState(false)
  const [copied, setCopied] = useState(false)

  const { data: status } = useQuery({ queryKey: ['otel-status'], queryFn: getOtelStatus })

  const yaml = useMemo(() => {
    let pipelines = ''
    if (traces) pipelines += '      traces:\n        receivers: [otlp]\n        exporters: [otlp/zentra]\n'
    if (metrics) pipelines += '      metrics:\n        receivers: [otlp]\n        exporters: [otlp/zentra]\n'
    if (logs) pipelines += '      logs:\n        receivers: [otlp]\n        exporters: [otlp/zentra]\n'
    if (profiles) pipelines += '      profiles:\n        receivers: [otlp]\n        exporters: [otlp/zentra]\n'

    return `receivers:
  otlp:
    protocols:
      grpc:
        endpoint: 0.0.0.0:4317
      http:
        endpoint: 0.0.0.0:4318

exporters:
  otlp/zentra:
    endpoint: ${host}:${port}
    tls:
      insecure: true
    encoding: ${protocol}

service:
  pipelines:
${pipelines}`
  }, [host, port, protocol, traces, metrics, logs, profiles])

  const dockerCmd = `docker run -d --name otel-collector \\
  -p 4317:4317 -p 4318:4318 \\
  -v $(pwd)/otel-config.yaml:/etc/otelcol/config.yaml \\
  otel/opentelemetry-collector-contrib:latest`

  const handleCopy = () => {
    navigator.clipboard.writeText(yaml)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  const handleDownload = () => {
    const blob = new Blob([yaml], { type: 'text/yaml' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = 'otel-config.yaml'
    a.click()
    URL.revokeObjectURL(url)
  }

  return (
    <div className="space-y-6">
      <PageHeader
        title="Collector Setup"
        subtitle="Generate a working OpenTelemetry Collector configuration pointing at Zentra"
      />

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Left: Configuration */}
        <div className="space-y-4">
          <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-5 space-y-4">
            <h3 className="text-sm font-semibold text-white flex items-center gap-2">
              <Settings className="w-4 h-4 text-cyan-400" />
              Connection Settings
            </h3>
            <div className="grid grid-cols-2 gap-3">
              <InputField label="Host" value={host} onChange={setHost} placeholder="localhost" />
              <InputField label="Port" value={port} onChange={setPort} placeholder="3001" />
            </div>
            <div>
              <label className="text-xs text-slate-400 block mb-1">Protocol</label>
              <select
                value={protocol} onChange={e => setProtocol(e.target.value)}
                className="w-full bg-slate-900 border border-slate-600 rounded-lg px-3 py-2 text-sm text-white focus:border-cyan-500 focus:outline-none"
              >
                <option value="http/json">HTTP/JSON</option>
                <option value="http/protobuf">HTTP/Protobuf</option>
              </select>
            </div>
          </div>

          <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-5 space-y-3">
            <h3 className="text-sm font-semibold text-white flex items-center gap-2">
              <Radio className="w-4 h-4 text-emerald-400" />
              Signals
            </h3>
            <ToggleRow icon={Activity} label="Traces" checked={traces} onChange={setTraces} color="blue" />
            <ToggleRow icon={Cpu} label="Metrics" checked={metrics} onChange={setMetrics} color="emerald" />
            <ToggleRow icon={ScrollText} label="Logs" checked={logs} onChange={setLogs} color="amber" />
            <ToggleRow icon={Globe} label="Profiles" checked={profiles} onChange={setProfiles} color="purple" />
          </div>

          {/* Endpoint reference */}
          <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-5 space-y-2">
            <h3 className="text-sm font-semibold text-white flex items-center gap-2">
              <Server className="w-4 h-4 text-orange-400" />
              OTLP Endpoint Reference
            </h3>
            <div className="space-y-1 text-xs">
              {status?.endpoints?.http && Object.entries(status.endpoints.http).map(([signal, path]) => (
                <div key={signal} className="flex justify-between text-slate-400">
                  <span className="capitalize">{signal}</span>
                  <code className="text-cyan-300 bg-slate-900 px-2 py-0.5 rounded">POST {path}</code>
                </div>
              ))}
              <div className="flex justify-between text-slate-400 pt-1 border-t border-slate-700 mt-1">
                <span>Encodings</span>
                <span className="text-slate-300">{status?.encodings?.join(', ') || 'JSON, Protobuf'}</span>
              </div>
              <div className="flex justify-between text-slate-400">
                <span>Auth</span>
                <span className="text-slate-300">{status?.auth?.join(', ') || 'JWT, API Key'}</span>
              </div>
            </div>
          </div>
        </div>

        {/* Right: YAML preview + Docker */}
        <div className="space-y-4">
          <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-5">
            <div className="flex items-center justify-between mb-3">
              <h3 className="text-sm font-semibold text-white">Generated Config</h3>
              <div className="flex gap-2">
                <button onClick={handleCopy} className="flex items-center gap-1 px-3 py-1.5 text-xs bg-cyan-600 hover:bg-cyan-500 text-white rounded-lg transition">
                  {copied ? <CheckCircle className="w-3 h-3" /> : <Copy className="w-3 h-3" />}
                  {copied ? 'Copied!' : 'Copy'}
                </button>
                <button onClick={handleDownload} className="flex items-center gap-1 px-3 py-1.5 text-xs bg-slate-700 hover:bg-slate-600 text-white rounded-lg transition">
                  <Download className="w-3 h-3" />
                  Download
                </button>
              </div>
            </div>
            <pre className="bg-slate-950 border border-slate-700 rounded-lg p-4 text-xs text-green-300 font-mono overflow-auto max-h-[400px] whitespace-pre-wrap">
              {yaml}
            </pre>
          </div>

          <div className="bg-slate-800/60 border border-slate-700 rounded-xl p-5">
            <h3 className="text-sm font-semibold text-white flex items-center gap-2 mb-3">
              <Terminal className="w-4 h-4 text-emerald-400" />
              Quick Start — Docker
            </h3>
            <p className="text-xs text-slate-400 mb-3">
              Save the config above as <code className="text-cyan-300">otel-config.yaml</code>, then run:
            </p>
            <pre className="bg-slate-950 border border-slate-700 rounded-lg p-4 text-xs text-amber-300 font-mono overflow-auto whitespace-pre-wrap">
              {dockerCmd}
            </pre>
          </div>
        </div>
      </div>
    </div>
  )
}

function ToggleRow({ icon: Icon, label, checked, onChange, color }: {
  icon: React.ElementType; label: string; checked: boolean; onChange: (v: boolean) => void; color: string
}) {
  return (
    <label className="flex items-center gap-3 cursor-pointer group">
      <div className={cn(
        'w-9 h-5 rounded-full transition-colors relative',
        checked ? `bg-${color}-600` : 'bg-slate-700'
      )}>
        <div className={cn(
          'absolute top-0.5 w-4 h-4 rounded-full bg-white transition-transform',
          checked ? 'left-[18px]' : 'left-0.5'
        )} />
      </div>
      <input type="checkbox" checked={checked} onChange={e => onChange(e.target.checked)} className="sr-only" />
      <Icon className={cn('w-4 h-4', checked ? `text-${color}-400` : 'text-slate-500')} />
      <span className={cn('text-sm', checked ? 'text-white' : 'text-slate-500')}>{label}</span>
    </label>
  )
}

function InputField({ label, value, onChange, placeholder }: {
  label: string; value: string; onChange: (v: string) => void; placeholder?: string
}) {
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
