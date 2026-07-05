import { useState, useEffect, useCallback, useRef } from 'react'
import { useNavigate } from 'react-router-dom'
import { useMutation, useQuery } from '@tanstack/react-query'
import {
  registerAgent, listAgents, getAgentLogsByDc,
  getComponentsByDc, confirmComponents,
  type AgentLogLine, type DiscoveredComponent,
} from '@/lib/api/fleet'
import PageHeader from '@/components/PageHeader'
import {
  Server, Copy, Check, Loader2, CheckCircle2,
  AlertTriangle, ChevronRight, ChevronDown, Terminal,
  RefreshCw, Package, Database, Globe, Cpu, Network, FolderOpen,
} from 'lucide-react'

type Step = 1 | 2 | 3 | 4

const STEP_LABELS = ['Register Datacenter', 'Install Agent', 'Waiting for Connection', 'Targets Discovered']

// ── Agent Log Tail (live) ──────────────────────────────────────────

function levelColor(level: string): string {
  const lvl = level.toUpperCase()
  if (lvl === 'ERROR') return 'text-red-400'
  if (lvl === 'WARN' || lvl === 'WARNING') return 'text-amber-400'
  if (lvl === 'DEBUG' || lvl === 'TRACE') return 'text-zinc-500'
  return 'text-emerald-400'
}

function AgentLogTail({ dcName, defaultOpen = true }: { dcName: string; defaultOpen?: boolean }) {
  const [open, setOpen] = useState(defaultOpen)
  const [paused, setPaused] = useState(false)
  const [lines, setLines] = useState<AgentLogLine[]>([])
  const scrollRef = useRef<HTMLDivElement>(null)
  const seenRef = useRef<Set<string>>(new Set())

  const { data } = useQuery({
    queryKey: ['agent-logs', dcName],
    queryFn: () => getAgentLogsByDc(dcName),
    refetchInterval: paused || !open ? false : 3_000,
    enabled: !!dcName,
  })

  useEffect(() => {
    if (!data?.lines) return
    const seen = seenRef.current
    const fresh: AgentLogLine[] = []
    for (const l of data.lines) {
      const key = `${l.timestamp}|${l.message}`
      if (seen.has(key)) continue
      seen.add(key)
      fresh.push(l)
    }
    if (fresh.length > 0) {
      setLines(prev => {
        const next = [...prev, ...fresh]
        return next.length > 500 ? next.slice(next.length - 500) : next
      })
    }
  }, [data])

  useEffect(() => {
    if (paused) return
    const el = scrollRef.current
    if (el) el.scrollTop = el.scrollHeight
  }, [lines, paused])

  return (
    <div className="rounded-lg border border-border bg-card">
      <button
        onClick={() => setOpen(o => !o)}
        className="w-full flex items-center gap-2 px-4 py-2.5 text-sm font-medium hover:bg-muted/40 transition-colors"
      >
        <Terminal className="h-4 w-4 text-blue-400" />
        <span className="flex-1 text-left">Agent Logs (live tail)</span>
        <span className="text-xs text-muted-foreground">{lines.length} lines</span>
        {open ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}
      </button>
      {open && (
        <div className="border-t border-border">
          <div className="flex items-center justify-between px-4 py-2 text-xs border-b border-border">
            <span className="text-muted-foreground">Polling every 3s · datacenter <code className="text-xs">{dcName}</code></span>
            <div className="flex items-center gap-2">
              <button
                onClick={() => setPaused(p => !p)}
                className="rounded px-2 py-0.5 border border-border hover:bg-muted/40"
              >
                {paused ? 'Resume' : 'Pause'}
              </button>
              <button
                onClick={() => { setLines([]); seenRef.current = new Set() }}
                className="rounded px-2 py-0.5 border border-border hover:bg-muted/40"
              >
                Clear
              </button>
            </div>
          </div>
          <div
            ref={scrollRef}
            className="h-64 overflow-y-auto bg-zinc-950 px-3 py-2 font-mono text-xs leading-relaxed"
          >
            {lines.length === 0 ? (
              <div className="text-zinc-500 italic">Waiting for agent log lines… (first heartbeat arrives within 30s of agent start)</div>
            ) : (
              lines.map((l, i) => (
                <div key={i} className="flex gap-2 whitespace-pre-wrap break-words">
                  <span className="text-zinc-600 shrink-0">{new Date(l.timestamp).toLocaleTimeString()}</span>
                  <span className={`shrink-0 w-12 ${levelColor(l.level)}`}>{l.level.toUpperCase()}</span>
                  <span className="text-zinc-500 shrink-0">{l.target}</span>
                  <span className="text-zinc-200">{l.message}</span>
                </div>
              ))
            )}
          </div>
        </div>
      )}
    </div>
  )
}

// ── Step 1: Register Datacenter ────────────────────────────────────

function StepRegister({ onComplete }: { onComplete: (apiKey: string, dc: string) => void }) {
  const [dcName, setDcName] = useState('')
  const [region, setRegion] = useState('')
  const [apiKey, setApiKey] = useState('')
  const [copied, setCopied] = useState(false)
  const [confirmed, setConfirmed] = useState(false)

  const registerMut = useMutation({
    mutationFn: () => registerAgent({ datacenter_name: dcName, region_label: region || undefined }),
    onSuccess: (data) => setApiKey(data.api_key),
  })

  const handleCopy = () => {
    navigator.clipboard.writeText(apiKey)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  if (apiKey) {
    return (
      <div className="space-y-6">
        <div className="rounded-lg border border-yellow-500/30 bg-yellow-500/10 p-4">
          <div className="flex items-center gap-2 text-yellow-400 font-medium mb-2">
            <AlertTriangle className="h-5 w-5" />
            This key will not be shown again. Copy it now.
          </div>
        </div>
        <div className="relative">
          <pre className="rounded-lg bg-zinc-900 p-4 pr-12 text-sm font-mono text-green-400 overflow-x-auto">
            {apiKey}
          </pre>
          <button
            onClick={handleCopy}
            className="absolute top-3 right-3 rounded p-1.5 hover:bg-zinc-700 transition-colors"
            title="Copy API key"
          >
            {copied ? <Check className="h-4 w-4 text-green-400" /> : <Copy className="h-4 w-4 text-zinc-400" />}
          </button>
        </div>
        <label className="flex items-center gap-2 text-sm cursor-pointer">
          <input type="checkbox" checked={confirmed} onChange={(e) => setConfirmed(e.target.checked)} className="rounded" />
          I have copied the API key securely
        </label>
        <button
          disabled={!confirmed}
          onClick={() => onComplete(apiKey, dcName)}
          className="flex items-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
        >
          Next <ChevronRight className="h-4 w-4" />
        </button>
      </div>
    )
  }

  return (
    <form
      onSubmit={(e) => { e.preventDefault(); registerMut.mutate() }}
      className="space-y-4 max-w-md"
    >
      <div>
        <label className="block text-sm font-medium text-slate-200 mb-1">Datacenter Name</label>
        <input
          value={dcName}
          onChange={(e) => setDcName(e.target.value)}
          required
          placeholder="e.g. us-east-prod"
          className="w-full rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-blue-500"
        />
      </div>
      <div>
        <label className="block text-sm font-medium text-slate-200 mb-1">Region Label (optional)</label>
        <input
          value={region}
          onChange={(e) => setRegion(e.target.value)}
          placeholder="e.g. US East"
          className="w-full rounded-lg border border-white/10 bg-white/5 px-3 py-2 text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-blue-500"
        />
      </div>
      <button
        type="submit"
        disabled={!dcName.trim() || registerMut.isPending}
        className="flex items-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-40 transition-colors"
      >
        {registerMut.isPending ? <Loader2 className="h-4 w-4 animate-spin" /> : <Server className="h-4 w-4" />}
        Register
      </button>
      {registerMut.isError && (
        <p className="text-sm text-red-400">{(registerMut.error as Error).message}</p>
      )}
    </form>
  )
}

// ── Step 2: Install the Agent ──────────────────────────────────────

function StepInstall({ apiKey, dcName, onNext }: { apiKey: string; dcName: string; onNext: () => void }) {
  const [tab, setTab] = useState<'linux' | 'windows'>('linux')
  const [copied, setCopied] = useState(false)
  const [showSystemd, setShowSystemd] = useState(false)
  // downloadUrl: frontend origin (nginx at :5010 serves /dl/)
  const downloadUrl = window.location.origin
  // apiEndpoint: same host, direct to API port 5006
  const apiEndpoint = `${window.location.protocol}//${window.location.hostname}:5006`

  const linuxCmd = `curl -fsSL ${downloadUrl}/dl/zentra-agent-linux-x86_64 -o zentra-agent && \\
chmod +x zentra-agent && \\
ZENTRA_AGENT_MODE=remote \\
ZENTRA_API_ENDPOINT=${apiEndpoint} \\
ZENTRA_AGENT_API_KEY=${apiKey} \\
ZENTRA_DC_NAME=${dcName} \\
./zentra-agent`

  const windowsCmd = `Invoke-WebRequest -Uri "${downloadUrl}/dl/zentra-agent-windows-x86_64.exe" -OutFile zentra-agent.exe
$env:ZENTRA_AGENT_MODE="remote"
$env:ZENTRA_API_ENDPOINT="${apiEndpoint}"
$env:ZENTRA_AGENT_API_KEY="${apiKey}"
$env:ZENTRA_DC_NAME="${dcName}"
.\\zentra-agent.exe`

  const systemdUnit = `[Unit]
Description=Zentra Monitoring Agent
After=network.target

[Service]
Type=simple
Environment=ZENTRA_AGENT_MODE=remote
Environment=ZENTRA_API_ENDPOINT=${apiEndpoint}
Environment=ZENTRA_AGENT_API_KEY=${apiKey}
Environment=ZENTRA_DC_NAME=${dcName}
ExecStart=/usr/local/bin/zentra-agent
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target`

  const cmd = tab === 'linux' ? linuxCmd : windowsCmd
  const handleCopy = () => {
    navigator.clipboard.writeText(cmd)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  return (
    <div className="space-y-6">
      <div className="flex gap-1 rounded-lg bg-muted p-1 w-fit">
        <button
          onClick={() => setTab('linux')}
          className={`rounded-md px-4 py-1.5 text-sm font-medium transition-colors ${tab === 'linux' ? 'bg-background shadow-sm' : 'text-muted-foreground hover:text-foreground'}`}
        >
          Linux
        </button>
        <button
          onClick={() => setTab('windows')}
          className={`rounded-md px-4 py-1.5 text-sm font-medium transition-colors ${tab === 'windows' ? 'bg-background shadow-sm' : 'text-muted-foreground hover:text-foreground'}`}
        >
          Windows
        </button>
      </div>

      <div className="relative">
        <pre className="rounded-lg bg-zinc-900 p-4 pr-12 text-sm font-mono text-green-400 overflow-x-auto whitespace-pre-wrap">
          {cmd}
        </pre>
        <button onClick={handleCopy} className="absolute top-3 right-3 rounded p-1.5 hover:bg-zinc-700 transition-colors" title="Copy">
          {copied ? <Check className="h-4 w-4 text-green-400" /> : <Copy className="h-4 w-4 text-zinc-400" />}
        </button>
      </div>

      {tab === 'linux' && (
        <div>
          <button
            onClick={() => setShowSystemd(!showSystemd)}
            className="flex items-center gap-1.5 text-sm text-blue-400 hover:text-blue-300 transition-colors"
          >
            {showSystemd ? <ChevronDown className="h-4 w-4" /> : <ChevronRight className="h-4 w-4" />}
            systemd service file (persistent background service)
          </button>
          {showSystemd && (
            <pre className="mt-2 rounded-lg bg-zinc-900 p-4 text-sm font-mono text-zinc-300 overflow-x-auto whitespace-pre-wrap">
              {systemdUnit}
            </pre>
          )}
        </div>
      )}

      <button
        onClick={onNext}
        className="flex items-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 transition-colors"
      >
        I&apos;ve run the command <ChevronRight className="h-4 w-4" />
      </button>
    </div>
  )
}

// ── Step 3: Waiting for Connection ─────────────────────────────────

function StepWaiting({ dcName, onConnected }: { dcName: string; onConnected: () => void }) {
  const startTime = useRef(Date.now())
  const [timedOut, setTimedOut] = useState(false)

  const { data: agents } = useQuery({
    queryKey: ['agents', 'poll'],
    queryFn: listAgents,
    refetchInterval: 5_000,
  })

  useEffect(() => {
    const timer = setTimeout(() => setTimedOut(true), 5 * 60 * 1000)
    return () => clearTimeout(timer)
  }, [])

  useEffect(() => {
    if (agents?.some((a) => a.datacenter_name === dcName && a.status === 'active')) {
      onConnected()
    }
  }, [agents, dcName, onConnected])

  if (timedOut) {
    return (
      <div className="space-y-4">
        <div className="rounded-lg border border-yellow-500/30 bg-yellow-500/10 p-4 space-y-3">
          <h3 className="font-medium text-yellow-400">Connection not detected after 5 minutes</h3>
          <p className="text-sm text-muted-foreground">Troubleshooting checklist:</p>
          <ul className="list-disc list-inside text-sm text-muted-foreground space-y-1">
            <li>Verify the server can reach <code className="text-xs">{window.location.origin}</code> over HTTPS</li>
            <li>Check the API key was copied correctly (starts with <code className="text-xs">isok_</code>)</li>
            <li>Ensure no firewall is blocking outbound port 443</li>
            <li>Check the agent process is running: <code className="text-xs">ps aux | grep zentra-agent</code></li>
            <li>Inspect the live agent log tail below for errors</li>
          </ul>
        </div>
        <AgentLogTail dcName={dcName} />
        <button
          onClick={() => { setTimedOut(false); startTime.current = Date.now() }}
          className="rounded-lg bg-zinc-700 px-4 py-2 text-sm font-medium text-white hover:bg-zinc-600"
        >
          Retry
        </button>
      </div>
    )
  }

  return (
    <div className="space-y-6">
      <div className="flex flex-col items-center justify-center py-8 space-y-3">
        <Loader2 className="h-10 w-10 animate-spin text-blue-400" />
        <p className="text-muted-foreground">Waiting for agent to connect…</p>
        <p className="text-xs text-muted-foreground">Polling every 5 seconds</p>
      </div>
      <AgentLogTail dcName={dcName} />
    </div>
  )
}

// ── Step 4: Targets Discovered ─────────────────────────────────────

function componentIcon(type: string) {
  switch (type) {
    case 'oracle':
      return <Database className="h-4 w-4 text-blue-400" />
    case 'nginx': case 'apache': case 'haproxy': case 'tomcat':
      return <Globe className="h-4 w-4 text-green-400" />
    case 'redis': case 'memcached': case 'kafka': case 'rabbitmq': case 'elasticsearch':
      return <Package className="h-4 w-4 text-amber-400" />
    case 'network_interface':
      return <Network className="h-4 w-4 text-cyan-400" />
    case 'log_directory':
      return <FolderOpen className="h-4 w-4 text-yellow-400" />
    case 'linux': case 'server':
      return <Server className="h-4 w-4 text-zinc-400" />
    default:
      return <Cpu className="h-4 w-4 text-zinc-400" />
  }
}

function ComponentRow({
  comp, selected, onToggle,
}: { comp: DiscoveredComponent; selected: boolean; onToggle: () => void }) {
  const isConfirmed = comp.confirmed
  return (
    <tr
      className={`border-b border-border last:border-0 transition-colors ${
        isConfirmed ? 'opacity-60' : selected ? 'bg-blue-500/5' : 'hover:bg-muted/30'
      }`}
    >
      <td className="px-4 py-3">
        {isConfirmed ? (
          <CheckCircle2 className="h-4 w-4 text-green-400" />
        ) : (
          <input
            type="checkbox"
            checked={selected}
            onChange={onToggle}
            className="rounded border-border"
          />
        )}
      </td>
      <td className="px-4 py-3">
        <div className="flex items-center gap-2">
          {componentIcon(comp.component_type)}
          <span className="font-medium">{comp.component_name}</span>
          {isConfirmed && (
            <span className="rounded-full bg-green-500/15 px-2 py-0.5 text-xs text-green-400">Added</span>
          )}
        </div>
      </td>
      <td className="px-4 py-3">
        <span className="rounded-full bg-muted px-2 py-0.5 text-xs text-muted-foreground">
          {comp.component_type}
        </span>
      </td>
      <td className="px-4 py-3 font-mono text-xs text-muted-foreground">
        {comp.address}{comp.port ? `:${comp.port}` : ''}
      </td>
      <td className="px-4 py-3 text-xs text-muted-foreground">
        {comp.version ?? '—'}
      </td>
    </tr>
  )
}

function StepTargets({ dcName }: { dcName: string }) {
  const navigate = useNavigate()
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [confirmed, setConfirmed] = useState(false)
  const [createdCount, setCreatedCount] = useState(0)

  const { data, isLoading, isError, refetch, isFetching } = useQuery({
    queryKey: ['components', dcName],
    queryFn: () => getComponentsByDc(dcName),
    refetchInterval: confirmed ? false : 15_000,
    enabled: !!dcName,
  })

  const components = data?.components ?? []
  const pending = components.filter(c => !c.confirmed)
  const alreadyAdded = components.filter(c => c.confirmed)

  // Auto-select all pending components on first load
  useEffect(() => {
    if (pending.length > 0 && selected.size === 0) {
      setSelected(new Set(pending.map(c => c.id)))
    }
  }, [pending.length]) // eslint-disable-line react-hooks/exhaustive-deps

  const confirmMut = useMutation({
    mutationFn: () => confirmComponents({
      component_ids: Array.from(selected),
      datacenter_name: dcName,
    }),
    onSuccess: (res) => {
      setCreatedCount(res.created)
      setConfirmed(true)
      refetch()
    },
  })

  const toggleAll = () => {
    if (selected.size === pending.length) {
      setSelected(new Set())
    } else {
      setSelected(new Set(pending.map(c => c.id)))
    }
  }

  const toggle = (id: string) => {
    setSelected(prev => {
      const next = new Set(prev)
      if (next.has(id)) next.delete(id)
      else next.add(id)
      return next
    })
  }

  return (
    <div className="space-y-6">
      {/* Connection success banner */}
      <div className="flex items-center gap-3 text-green-400">
        <CheckCircle2 className="h-8 w-8" />
        <h3 className="text-lg font-semibold">Agent connected successfully!</h3>
      </div>

      {/* Confirmed banner */}
      {confirmed && createdCount > 0 && (
        <div className="rounded-lg border border-green-500/30 bg-green-500/10 p-4">
          <p className="text-sm text-green-400 font-medium">
            ✓ {createdCount} monitoring target{createdCount !== 1 ? 's' : ''} created and will start collecting shortly.
          </p>
        </div>
      )}

      {/* Discovery result */}
      {isLoading ? (
        <div className="flex flex-col items-center justify-center py-10 gap-3">
          <Loader2 className="h-8 w-8 animate-spin text-blue-400" />
          <p className="text-sm text-muted-foreground">Waiting for agent to report discovered components…</p>
          <p className="text-xs text-muted-foreground">This may take up to 30 seconds after the agent starts.</p>
        </div>
      ) : isError ? (
        <div className="rounded-lg border border-red-500/30 bg-red-500/10 p-4 space-y-3">
          <p className="text-sm font-medium text-red-400">Failed to fetch discovered components</p>
          <p className="text-xs text-muted-foreground">Possible reasons: agent not yet connected, discovery still running, or network issue.</p>
          <button
            onClick={() => refetch()}
            className="flex items-center gap-2 rounded-lg bg-zinc-700 px-3 py-1.5 text-xs font-medium text-white hover:bg-zinc-600"
          >
            <RefreshCw className="h-3.5 w-3.5" /> Retry
          </button>
        </div>
      ) : components.length === 0 ? (
        <div className="rounded-lg border border-border bg-card p-6 space-y-4">
          <div className="text-center space-y-2">
            <p className="text-muted-foreground font-medium">No components discovered yet</p>
            <p className="text-xs text-muted-foreground">The agent scans for local services every 5 minutes.</p>
          </div>
          <ul className="list-disc list-inside text-xs text-muted-foreground space-y-1">
            <li>Ensure the agent process is running on the target host</li>
            <li>Agent must be able to reach <code className="text-xs">{window.location.origin}</code></li>
            <li>Check agent logs below for errors</li>
            <li>Supported: PostgreSQL, MySQL, Redis, nginx, Apache, MongoDB, Oracle, Kafka, RabbitMQ</li>
          </ul>
          <button
            onClick={() => refetch()}
            disabled={isFetching}
            className="flex items-center gap-2 rounded-lg bg-zinc-700 px-3 py-1.5 text-sm font-medium text-white hover:bg-zinc-600 disabled:opacity-50"
          >
            {isFetching ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <RefreshCw className="h-3.5 w-3.5" />}
            Retry Discovery
          </button>
        </div>
      ) : (
        <div className="space-y-4">
          {/* Already-added section */}
          {alreadyAdded.length > 0 && (
            <p className="text-xs text-muted-foreground">
              {alreadyAdded.length} component{alreadyAdded.length !== 1 ? 's' : ''} already added as targets.
            </p>
          )}

          <div className="rounded-xl border border-border bg-card overflow-hidden">
            <table className="w-full text-sm">
              <thead>
                <tr className="border-b border-border text-left text-muted-foreground bg-muted/30">
                  <th className="px-4 py-3 w-10">
                    {pending.length > 0 && (
                      <input
                        type="checkbox"
                        checked={selected.size === pending.length && pending.length > 0}
                        onChange={toggleAll}
                        className="rounded border-border"
                      />
                    )}
                  </th>
                  <th className="px-4 py-3 font-medium">Component</th>
                  <th className="px-4 py-3 font-medium">Type</th>
                  <th className="px-4 py-3 font-medium">Address</th>
                  <th className="px-4 py-3 font-medium">Version</th>
                </tr>
              </thead>
              <tbody>
                {components.map(comp => (
                  <ComponentRow
                    key={comp.id}
                    comp={comp}
                    selected={selected.has(comp.id)}
                    onToggle={() => toggle(comp.id)}
                  />
                ))}
              </tbody>
            </table>
          </div>

          {/* Confirm button */}
          {pending.length > 0 && !confirmed && (
            <div className="flex items-center gap-4">
              <button
                onClick={() => confirmMut.mutate()}
                disabled={selected.size === 0 || confirmMut.isPending}
                className="flex items-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 disabled:opacity-40 disabled:cursor-not-allowed transition-colors"
              >
                {confirmMut.isPending
                  ? <Loader2 className="h-4 w-4 animate-spin" />
                  : <CheckCircle2 className="h-4 w-4" />}
                Add {selected.size} selected component{selected.size !== 1 ? 's' : ''} to monitoring
              </button>
              {confirmMut.isError && (
                <p className="text-sm text-red-400">{(confirmMut.error as Error).message}</p>
              )}
            </div>
          )}
        </div>
      )}

      <AgentLogTail dcName={dcName} defaultOpen={false} />

      <div className="flex items-center gap-3">
        <button
          onClick={() => navigate('/dashboard')}
          className="flex items-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 transition-colors"
        >
          Go to Dashboard <ChevronRight className="h-4 w-4" />
        </button>
        <button
          onClick={() => navigate('/servers')}
          className="flex items-center gap-2 rounded-lg bg-zinc-700 px-4 py-2 text-sm font-medium text-white hover:bg-zinc-600 transition-colors"
        >
          Server Fleet
        </button>
      </div>
    </div>
  )
}

// ── Main Wizard ────────────────────────────────────────────────────

export default function AgentInstallWizard() {
  const [step, setStep] = useState<Step>(1)
  const [apiKey, setApiKey] = useState('')
  const [dcName, setDcName] = useState('')

  const handleRegistered = useCallback((key: string, dc: string) => {
    setApiKey(key)
    setDcName(dc)
    setStep(2)
  }, [])

  return (
    <div className="space-y-6">
      <PageHeader
        title="Install Agent"
        description="Connect a datacenter in four quick steps."
        icon={Server}
      />

      {/* Progress indicator */}
      <div className="flex items-center gap-2">
        {STEP_LABELS.map((label, i) => {
          const n = (i + 1) as Step
          const isActive = n === step
          const isDone = n < step
          return (
            <div key={n} className="flex items-center gap-2">
              {i > 0 && <div className={`h-px w-8 ${isDone ? 'bg-blue-500' : 'bg-border'}`} />}
              <div className="flex items-center gap-1.5">
                <div className={`flex h-7 w-7 items-center justify-center rounded-full text-xs font-bold transition-colors ${
                  isDone ? 'bg-blue-600 text-white' : isActive ? 'bg-blue-600 text-white ring-2 ring-blue-400/50' : 'bg-muted text-muted-foreground'
                }`}>
                  {isDone ? <Check className="h-3.5 w-3.5" /> : n}
                </div>
                <span className={`text-xs hidden sm:inline ${isActive ? 'font-medium text-foreground' : 'text-muted-foreground'}`}>
                  {label}
                </span>
              </div>
            </div>
          )
        })}
      </div>

      <div className="rounded-xl border border-border bg-card p-6">
        {step === 1 && <StepRegister onComplete={handleRegistered} />}
        {step === 2 && <StepInstall apiKey={apiKey} dcName={dcName} onNext={() => setStep(3)} />}
        {step === 3 && <StepWaiting dcName={dcName} onConnected={() => setStep(4)} />}
        {step === 4 && <StepTargets dcName={dcName} />}
      </div>
    </div>
  )
}
