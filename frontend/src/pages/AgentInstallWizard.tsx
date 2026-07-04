import { useState, useEffect, useCallback, useRef } from 'react'
import { useNavigate } from 'react-router-dom'
import { useMutation, useQuery } from '@tanstack/react-query'
import { registerAgent, listAgents, getAgentLogsByDc, type AgentLogLine } from '@/lib/api/fleet'
import { fetchJSON } from '@/lib/api/_client'
import PageHeader from '@/components/PageHeader'
import {
  Server, Copy, Check, Loader2, CheckCircle2,
  AlertTriangle, ChevronRight, ChevronDown, Terminal,
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
  const serverUrl = window.location.origin

  const linuxCmd = `curl -fsSL ${serverUrl}/dl/zentra-agent-linux-x86_64 -o zentra-agent && \\
chmod +x zentra-agent && \\
ZENTRA_AGENT_MODE=remote \\
ZENTRA_API_ENDPOINT=${serverUrl} \\
ZENTRA_AGENT_API_KEY=${apiKey} \\
ZENTRA_DC_NAME=${dcName} \\
./zentra-agent`

  const windowsCmd = `Invoke-WebRequest -Uri "${serverUrl}/dl/zentra-agent-windows-x86_64.exe" -OutFile zentra-agent.exe
$env:ZENTRA_AGENT_MODE="remote"
$env:ZENTRA_API_ENDPOINT="${serverUrl}"
$env:ZENTRA_AGENT_API_KEY="${apiKey}"
$env:ZENTRA_DC_NAME="${dcName}"
.\\zentra-agent.exe`

  const systemdUnit = `[Unit]
Description=Zentra Monitoring Agent
After=network.target

[Service]
Type=simple
Environment=ZENTRA_AGENT_MODE=remote
Environment=ZENTRA_API_ENDPOINT=${serverUrl}
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

interface Target { target_id: string; name: string; host: string; target_subtype: string }

function StepTargets({ dcName }: { dcName: string }) {
  const navigate = useNavigate()
  const { data: targets = [] } = useQuery({
    queryKey: ['targets', 'recent'],
    queryFn: () => fetchJSON<Target[]>('/targets'),
  })

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-3 text-green-400">
        <CheckCircle2 className="h-8 w-8" />
        <h3 className="text-lg font-semibold">Agent connected successfully!</h3>
      </div>

      {targets.length > 0 ? (
        <div className="rounded-xl border border-border bg-card overflow-hidden">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-border text-left text-muted-foreground">
                <th className="px-4 py-3 font-medium">Name</th>
                <th className="px-4 py-3 font-medium">Host</th>
                <th className="px-4 py-3 font-medium">Type</th>
              </tr>
            </thead>
            <tbody>
              {targets.map((t) => (
                <tr key={t.target_id} className="border-b border-border last:border-0">
                  <td className="px-4 py-3 font-medium">{t.name}</td>
                  <td className="px-4 py-3 font-mono text-xs text-muted-foreground">{t.host}</td>
                  <td className="px-4 py-3 text-muted-foreground">{t.target_subtype}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      ) : (
        <div className="rounded-lg border border-border bg-card p-6 text-center space-y-3">
          <p className="text-muted-foreground">No targets discovered yet.</p>
          <button
            onClick={() => navigate('/servers')}
            className="rounded-lg bg-zinc-700 px-4 py-2 text-sm font-medium text-white hover:bg-zinc-600"
          >
            Add target manually
          </button>
        </div>
      )}

      <AgentLogTail dcName={dcName} defaultOpen={false} />

      <button
        onClick={() => navigate('/dashboard')}
        className="flex items-center gap-2 rounded-lg bg-blue-600 px-4 py-2 text-sm font-medium text-white hover:bg-blue-700 transition-colors"
      >
        Go to Dashboard <ChevronRight className="h-4 w-4" />
      </button>
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
