import { useState } from 'react'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { createTarget, verifySsh, setupSsh, crawlServer, deleteTarget } from '@/lib/api'
import type { CrawledComponent, CrawlResult } from '@/lib/api'
import { listAgents, type Agent } from '@/lib/api/fleet'
import { X, Plus, Loader2, CheckCircle, XCircle, KeyRound, AlertTriangle, Search, Database, Server, HardDrive, Wifi, Package, Clock, Bot, ChevronDown } from 'lucide-react'

interface AddTargetModalProps {
  open: boolean
  onClose: () => void
  defaultType?: string
  defaultSubtype?: string
  title?: string
}

const targetTypes = [
  { value: 'server', label: 'Server', subtypes: ['linux', 'windows', 'container', 'kubernetes', 'esxi', 'vsphere', 'vcenter'] },
  { value: 'database', label: 'Database', subtypes: ['oracle'] },
  { value: 'storage', label: 'Storage', subtypes: ['localdisk', 'nfs', 'san', 's3', 'ceph', 'netapp', 'purestorage', 'minio'] },
  { value: 'network', label: 'Network', subtypes: ['switch', 'router', 'firewall', 'interface', 'bgp', 'ospf', 'vpn', 'dns', 'dhcp'] },
]

const compTypeIcon: Record<string, typeof Database> = {
  database: Database,
  middleware: Package,
  server: Server,
  storage: HardDrive,
  network: Wifi,
}

const compTypeColor: Record<string, string> = {
  database: 'text-blue-400 bg-blue-500/10 border-blue-500/20',
  middleware: 'text-purple-400 bg-purple-500/10 border-purple-500/20',
  server: 'text-emerald-400 bg-emerald-500/10 border-emerald-500/20',
  storage: 'text-amber-400 bg-amber-500/10 border-amber-500/20',
  network: 'text-cyan-400 bg-cyan-500/10 border-cyan-500/20',
}

type ModalStep = 'form' | 'crawling' | 'discovered'

export default function AddTargetModal({ open, onClose, defaultType = '', defaultSubtype = '', title = 'Add Target' }: AddTargetModalProps) {
  const qc = useQueryClient()
  const [form, setForm] = useState({
    name: '',
    target_type: defaultType,
    target_subtype: defaultSubtype,
    host: '',
    port: 0,
    description: '',
    environment: 'production',
    username: '',
    password: '',
  })
  const [error, setError] = useState('')
  const [verifyState, setVerifyState] = useState<'idle' | 'verifying' | 'passed' | 'failed'>('idle')
  const [verifyMessage, setVerifyMessage] = useState('')
  const [showSshSetup, setShowSshSetup] = useState(false)
  const [sshPassword, setSshPassword] = useState('')
  const [sshSetupLoading, setSshSetupLoading] = useState(false)
  const [sshSetupResult, setSshSetupResult] = useState<{ success: boolean; message: string } | null>(null)

  // Agent selection (Feature 1 / 3)
  const [selectedAgentId, setSelectedAgentId] = useState<string>('')

  // Crawl state
  const [step, setStep] = useState<ModalStep>('form')
  const [crawlResult, setCrawlResult] = useState<CrawlResult | null>(null)
  const [crawlError, setCrawlError] = useState('')
  const [selectedComponents, setSelectedComponents] = useState<Set<number>>(new Set())
  const [registering, setRegistering] = useState(false)
  const [registerProgress, setRegisterProgress] = useState(0)
  const [registerDone, setRegisterDone] = useState(false)
  const [createdTargetId, setCreatedTargetId] = useState<string>('')
  const [crawlAttempt, setCrawlAttempt] = useState(0)
  const [formBanner, setFormBanner] = useState('')

  const isServer = form.target_type === 'server'

  // Fetch active agents for the dropdown
  const { data: agents = [] } = useQuery<Agent[]>({
    queryKey: ['agents'],
    queryFn: listAgents,
    staleTime: 30_000,
    enabled: open && isServer,
  })
  const activeAgents: Agent[] = agents.filter((a: Agent) => a.status === 'active' || a.status === 'stale')
  // SSH is only required when server has no agent selected
  const agentSelected = isServer && !!selectedAgentId
  const sshRequired = isServer && !agentSelected

  const resetAll = () => {
    setForm({ name: '', target_type: defaultType, target_subtype: defaultSubtype, host: '', port: 0, description: '', environment: 'production', username: '', password: '' })
    setError('')
    setVerifyState('idle')
    setVerifyMessage('')
    setShowSshSetup(false)
    setSshSetupResult(null)
    setSelectedAgentId('')
    setStep('form')
    setCrawlResult(null)
    setCrawlError('')
    setSelectedComponents(new Set())
    setRegistering(false)
    setRegisterProgress(0)
    setRegisterDone(false)
    setCreatedTargetId('')
    setCrawlAttempt(0)
    setFormBanner('')
  }

  /// Silently delete a dangling target (best-effort) — used when the user
  /// discards a failed-crawl attempt or closes the modal mid-failure.
  const cleanupTarget = async (id: string) => {
    if (!id) return
    try { await deleteTarget(id) } catch { /* ignore */ }
    qc.invalidateQueries({ queryKey: ['targets'] })
  }

  const handleClose = async () => {
    // If we're closing while a target exists but the crawl failed, remove the orphan.
    if (createdTargetId && crawlError) {
      await cleanupTarget(createdTargetId)
    }
    onClose()
    resetAll()
  }

  // "Edit & Retry" — delete the stale target and go back to the form so the
  // user can correct host / credentials / agent selection. Form values are
  // preserved on purpose so they only have to adjust what was wrong.
  const handleEditAndRetry = async () => {
    if (createdTargetId) await cleanupTarget(createdTargetId)
    setCreatedTargetId('')
    setCrawlError('')
    setCrawlResult(null)
    setStep('form')
    setFormBanner('Previous attempt failed. Adjust the details below and try again — this is your last retry.')
  }

  // "Discard" — delete the target and close.
  const handleDiscard = async () => {
    if (createdTargetId) await cleanupTarget(createdTargetId)
    onClose()
    resetAll()
  }

  const mutation = useMutation({
    mutationFn: () => createTarget({
      name: form.name,
      target_type: form.target_type,
      target_subtype: form.target_subtype,
      host: form.host,
      port: form.port,
      description: form.description || null,
      environment: form.environment,
      ssl_enabled: false,
      is_active: true,
      agent_id: selectedAgentId || undefined,
    }),
    onSuccess: (target) => {
      qc.invalidateQueries({ queryKey: ['targets'] })
      const tid = (target as any).target_id || ''
      setCreatedTargetId(tid)
      if (isServer) {
        doCrawl(tid)
      } else {
        onClose()
        resetAll()
      }
    },
    onError: (e: Error) => setError(e.message),
  })

  const doCrawl = async (targetId: string) => {
    setStep('crawling')
    setCrawlError('')
    const attempt = crawlAttempt + 1
    setCrawlAttempt(attempt)
    try {
      const result = await crawlServer(targetId)
      setCrawlResult(result)
      setSelectedComponents(new Set(result.components.map((_, i) => i)))
      setStep('discovered')
    } catch (err) {
      const msg = String(err)
      setCrawlError(msg)
      // On the 2nd consecutive failure, auto-remove the orphan target so the
      // user isn't left with a phantom 'unreachable' entry in their fleet.
      if (attempt >= 2) {
        await cleanupTarget(targetId)
        setCreatedTargetId('')
      }
      setStep('discovered')
    }
  }

  const doRegisterSelected = async () => {
    if (!crawlResult || selectedComponents.size === 0) return
    setRegistering(true)
    setRegisterProgress(0)

    const toRegister = crawlResult.components.filter((_, i) => selectedComponents.has(i))
    let done = 0

    for (const comp of toRegister) {
      try {
        await createTarget({
          name: comp.name,
          target_type: comp.component_type,
          target_subtype: comp.subtype,
          host: crawlResult.host,
          port: comp.port,
          description: `Auto-discovered on ${crawlResult.hostname} — ${comp.version}`,
          environment: form.environment,
          ssl_enabled: false,
          is_active: true,
        })
      } catch {
        // Continue on individual failures
      }
      done++
      setRegisterProgress(Math.round((done / toRegister.length) * 100))
    }

    qc.invalidateQueries({ queryKey: ['targets'] })
    setRegistering(false)
    setRegisterDone(true)
  }

  const toggleComponent = (idx: number) => {
    setSelectedComponents(prev => {
      const next = new Set(prev)
      if (next.has(idx)) next.delete(idx)
      else next.add(idx)
      return next
    })
  }

  const toggleAll = () => {
    if (!crawlResult) return
    if (selectedComponents.size === crawlResult.components.length) {
      setSelectedComponents(new Set())
    } else {
      setSelectedComponents(new Set(crawlResult.components.map((_, i) => i)))
    }
  }

  const doVerify = async () => {
    setVerifyState('verifying')
    setVerifyMessage('')
    setError('')
    setShowSshSetup(false)
    setSshSetupResult(null)
    try {
      const res = await verifySsh(form.host, form.username || undefined)
      if (res.reachable) {
        setVerifyState('passed')
        setVerifyMessage(res.message)
      } else {
        setVerifyState('failed')
        setVerifyMessage(res.message)
      }
    } catch (err) {
      setVerifyState('failed')
      setVerifyMessage(String(err))
    }
  }

  const doSshSetup = async () => {
    if (!sshPassword) return
    setSshSetupLoading(true)
    setSshSetupResult(null)
    try {
      const res = await setupSsh(form.host, form.username || 'root', sshPassword)
      setSshSetupResult(res)
      if (res.success) {
        // Re-verify after key setup
        setTimeout(() => doVerify(), 1500)
      }
    } catch (err) {
      setSshSetupResult({ success: false, message: String(err) })
    }
    setSshSetupLoading(false)
  }

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (!form.name || !form.host || !form.target_type) {
      setError('Name, type, and host are required')
      return
    }
    if (!form.target_subtype) {
      setError('Please select a subtype')
      return
    }
    // SSH verification required only if no agent selected (Feature 3)
    if (sshRequired && verifyState !== 'passed') {
      doVerify()
      return
    }
    setError('')
    mutation.mutate()
  }

  const update = (field: string, value: string | number) => setForm(f => ({ ...f, [field]: value }))
  const currentType = targetTypes.find(t => t.value === form.target_type)

  if (!open) return null

  // ── Step 2: Crawling / Discovered Components ──
  if (step === 'crawling' || step === 'discovered') {
    const components = crawlResult?.components || []
    const grouped: Record<string, { comp: CrawledComponent; idx: number }[]> = {}
    components.forEach((comp, idx) => {
      if (!grouped[comp.component_type]) grouped[comp.component_type] = []
      grouped[comp.component_type].push({ comp, idx })
    })

    return (
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
        <div className="w-full max-w-2xl bg-slate-900 border border-white/10 rounded-2xl shadow-2xl max-h-[85vh] flex flex-col">
          <div className="flex items-center justify-between px-6 py-4 border-b border-white/10 flex-shrink-0">
            <h2 className="text-lg font-bold text-white flex items-center gap-2">
              <Search className="w-5 h-5 text-brand-400" />
              {step === 'crawling' ? 'Discovering Components...' : 'Discovered Components'}
            </h2>
            <button onClick={handleClose} className="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 transition-colors">
              <X className="w-5 h-5" />
            </button>
          </div>

          <div className="p-6 space-y-4 overflow-y-auto flex-1">
            {/* Crawling spinner */}
            {step === 'crawling' && (
              <div className="flex flex-col items-center py-12 gap-4">
                <div className="relative">
                  <Loader2 className="w-10 h-10 animate-spin text-brand-400" />
                </div>
                <div className="text-center">
                  <p className="text-sm font-medium text-white">Crawling {form.host}...</p>
                  <p className="text-xs text-slate-400 mt-1">Scanning for Oracle, Nginx, Apache, Tomcat, NFS, SNMP, and Firewall</p>
                </div>
              </div>
            )}

            {/* Crawl error */}
            {step === 'discovered' && crawlError && (
              <div className="px-4 py-3 bg-red-500/10 border border-red-500/20 rounded-xl text-sm text-red-400 flex items-start gap-2">
                <AlertTriangle className="w-4 h-4 mt-0.5 flex-shrink-0" />
                <div>
                  <div className="font-medium">Crawl failed{crawlAttempt >= 2 ? ' — target removed' : ''}</div>
                  <div className="text-xs opacity-80 mt-0.5">{crawlError}</div>
                  {crawlAttempt < 2 && (
                    <div className="text-xs opacity-70 mt-2">
                      You can edit the connection details and retry. If the second attempt also fails, the target will be removed automatically.
                    </div>
                  )}
                  {crawlAttempt >= 2 && (
                    <div className="text-xs opacity-70 mt-2">
                      The server was created but couldn&apos;t be reached after 2 attempts, so it has been removed. Verify connectivity / credentials outside Zentra and try again.
                    </div>
                  )}
                </div>
              </div>
            )}

            {/* Discovered results */}
            {step === 'discovered' && crawlResult && (
              <>
                {/* Summary bar */}
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-3">
                    <div className="px-3 py-1.5 bg-emerald-500/10 border border-emerald-500/20 rounded-lg">
                      <span className="text-xs font-medium text-emerald-400">
                        {components.length} component{components.length !== 1 ? 's' : ''} found
                      </span>
                    </div>
                    <div className="flex items-center gap-1 text-xs text-slate-500">
                      <Clock className="w-3 h-3" />
                      {crawlResult.crawl_duration_ms}ms
                    </div>
                  </div>
                  {components.length > 0 && !registerDone && (
                    <button
                      type="button"
                      onClick={toggleAll}
                      className="text-xs text-brand-400 hover:text-brand-300"
                    >
                      {selectedComponents.size === components.length ? 'Deselect All' : 'Select All'}
                    </button>
                  )}
                </div>

                {components.length === 0 && !crawlError && (
                  <div className="text-center py-8">
                    <Server className="w-8 h-8 text-slate-600 mx-auto mb-2" />
                    <p className="text-sm text-slate-400">No additional components discovered on this server.</p>
                    <p className="text-xs text-slate-500 mt-1">The server target has been created successfully.</p>
                  </div>
                )}

                {/* Grouped component list */}
                {Object.entries(grouped).map(([type, items]) => {
                  const Icon = compTypeIcon[type] || Package
                  const colorClass = compTypeColor[type] || 'text-slate-400 bg-slate-500/10 border-slate-500/20'
                  return (
                    <div key={type} className="space-y-2">
                      <div className="flex items-center gap-2 text-xs font-medium text-slate-400 uppercase tracking-wider">
                        <Icon className={`w-3.5 h-3.5 ${colorClass.split(' ')[0]}`} />
                        {type} ({items.length})
                      </div>
                      {items.map(({ comp, idx }) => (
                        <label
                          key={idx}
                          className={`flex items-center gap-3 px-4 py-3 rounded-xl border cursor-pointer transition-all ${
                            registerDone
                              ? 'bg-emerald-500/5 border-emerald-500/20'
                              : selectedComponents.has(idx)
                                ? `${colorClass} bg-opacity-50`
                                : 'bg-white/[0.02] border-white/5 hover:bg-white/[0.04]'
                          }`}
                        >
                          {!registerDone && (
                            <input
                              type="checkbox"
                              checked={selectedComponents.has(idx)}
                              onChange={() => toggleComponent(idx)}
                              disabled={registering}
                              className="w-4 h-4 rounded border-white/20 bg-white/5 text-brand-500 focus:ring-brand-500/50"
                            />
                          )}
                          {registerDone && <CheckCircle className="w-4 h-4 text-emerald-400 flex-shrink-0" />}
                          <div className="flex-1 min-w-0">
                            <div className="flex items-center gap-2">
                              <span className="text-sm font-medium text-white">{comp.name}</span>
                              <span className="text-[10px] px-1.5 py-0.5 rounded bg-white/5 text-slate-400">:{comp.port}</span>
                            </div>
                            <div className="text-xs text-slate-500 mt-0.5 truncate">{comp.version}{comp.detail ? ` · ${comp.detail}` : ''}</div>
                          </div>
                          <span className={`text-[10px] px-2 py-0.5 rounded-full border ${colorClass}`}>{comp.subtype}</span>
                        </label>
                      ))}
                    </div>
                  )
                })}

                {/* Progress bar */}
                {registering && (
                  <div className="space-y-2">
                    <div className="flex items-center justify-between text-xs">
                      <span className="text-slate-400">Registering components...</span>
                      <span className="text-brand-400 font-medium">{registerProgress}%</span>
                    </div>
                    <div className="w-full bg-white/5 rounded-full h-1.5">
                      <div className="bg-brand-500 h-1.5 rounded-full transition-all duration-300" style={{ width: `${registerProgress}%` }} />
                    </div>
                  </div>
                )}

                {/* Done message */}
                {registerDone && (
                  <div className="px-4 py-3 bg-emerald-500/10 border border-emerald-500/20 rounded-xl text-sm text-emerald-400 flex items-center gap-2">
                    <CheckCircle className="w-4 h-4 flex-shrink-0" />
                    All selected components registered successfully. Monitoring will start automatically.
                  </div>
                )}
              </>
            )}
          </div>

          {/* Footer */}
          <div className="flex justify-end gap-3 px-6 py-4 border-t border-white/10 flex-shrink-0">
            {step === 'discovered' && !registerDone && (
              <>
                <button
                  type="button"
                  onClick={handleClose}
                  className="px-4 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-slate-400 hover:text-white transition-colors"
                >
                  Skip
                </button>
                {components.length > 0 && (
                  <button
                    type="button"
                    onClick={doRegisterSelected}
                    disabled={registering || selectedComponents.size === 0}
                    className="flex items-center gap-2 px-5 py-2.5 bg-brand-600 hover:bg-brand-700 rounded-xl text-sm font-medium transition-colors disabled:opacity-50"
                  >
                    {registering ? <Loader2 className="w-4 h-4 animate-spin" /> : <Plus className="w-4 h-4" />}
                    Register {selectedComponents.size} Component{selectedComponents.size !== 1 ? 's' : ''}
                  </button>
                )}
              </>
            )}
            {(step === 'discovered' && registerDone) && (
              <button
                type="button"
                onClick={handleClose}
                className="flex items-center gap-2 px-5 py-2.5 bg-emerald-600 hover:bg-emerald-700 rounded-xl text-sm font-medium transition-colors"
              >
                <CheckCircle className="w-4 h-4" /> Done
              </button>
            )}
            {step === 'discovered' && crawlError && components.length === 0 && (
              crawlAttempt < 2 ? (
                <>
                  <button
                    type="button"
                    onClick={handleDiscard}
                    className="px-4 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-slate-400 hover:text-white transition-colors"
                  >
                    Discard
                  </button>
                  <button
                    type="button"
                    onClick={handleEditAndRetry}
                    className="flex items-center gap-2 px-5 py-2.5 bg-brand-600 hover:bg-brand-700 rounded-xl text-sm font-medium text-white transition-colors"
                  >
                    Edit &amp; Retry
                  </button>
                </>
              ) : (
                <button
                  type="button"
                  onClick={() => { onClose(); resetAll() }}
                  className="px-4 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-slate-400 hover:text-white transition-colors"
                >
                  Close
                </button>
              )
            )}
          </div>
        </div>
      </div>
    )
  }

  // ── Step 1: Form ──
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
      <div className="w-full max-w-lg bg-slate-900 border border-white/10 rounded-2xl shadow-2xl">
        <div className="flex items-center justify-between px-6 py-4 border-b border-white/10">
          <h2 className="text-lg font-bold text-white flex items-center gap-2">
            <Plus className="w-5 h-5 text-brand-400" />
            {title}
          </h2>
          <button onClick={handleClose} className="p-1.5 rounded-lg hover:bg-white/10 text-slate-400 transition-colors">
            <X className="w-5 h-5" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="p-6 space-y-4">
          {formBanner && (
            <div className="px-4 py-2.5 bg-amber-500/10 border border-amber-500/20 rounded-xl text-sm text-amber-300 flex items-start gap-2">
              <AlertTriangle className="w-4 h-4 mt-0.5 flex-shrink-0" />
              <span>{formBanner}</span>
            </div>
          )}
          {error && (
            <div className="px-4 py-2.5 bg-red-500/10 border border-red-500/20 rounded-xl text-sm text-red-400">{error}</div>
          )}

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-xs font-medium text-slate-400 mb-1.5">Name *</label>
              <input
                value={form.name}
                onChange={e => update('name', e.target.value)}
                placeholder="e.g. web-server-01"
                className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-brand-500/50"
              />
            </div>
            <div>
              <label className="block text-xs font-medium text-slate-400 mb-1.5">Environment</label>
              <select
                value={form.environment}
                onChange={e => update('environment', e.target.value)}
                className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white focus:outline-none focus:ring-2 focus:ring-brand-500/50 [&>option]:bg-slate-900 [&>option]:text-white"
              >
                <option value="production">Production</option>
                <option value="staging">Staging</option>
                <option value="development">Development</option>
                <option value="testing">Testing</option>
              </select>
            </div>
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-xs font-medium text-slate-400 mb-1.5">Type *</label>
              <select
                value={form.target_type}
                onChange={e => {
                  const newType = e.target.value
                  update('target_type', newType)
                  const subs = targetTypes.find(t => t.value === newType)?.subtypes ?? []
                  update('target_subtype', subs[0] ?? '')
                  setVerifyState('idle')
                }}
                className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white focus:outline-none focus:ring-2 focus:ring-brand-500/50 [&>option]:bg-slate-900 [&>option]:text-white"
              >
                <option value="">Select type...</option>
                {targetTypes.map(t => <option key={t.value} value={t.value}>{t.label}</option>)}
              </select>
            </div>
            <div>
              <label className="block text-xs font-medium text-slate-400 mb-1.5">Subtype</label>
              <select
                value={form.target_subtype}
                onChange={e => update('target_subtype', e.target.value)}
                className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white focus:outline-none focus:ring-2 focus:ring-brand-500/50 [&>option]:bg-slate-900 [&>option]:text-white"
                disabled={!currentType}
              >
                <option value="">Select subtype...</option>
                {(currentType?.subtypes ?? []).map(s => <option key={s} value={s}>{s}</option>)}
              </select>
            </div>
          </div>

          <div className="grid grid-cols-3 gap-4">
            <div className="col-span-2">
              <label className="block text-xs font-medium text-slate-400 mb-1.5">Host / IP *</label>
              <input
                value={form.host}
                onChange={e => update('host', e.target.value)}
                placeholder="e.g. 10.0.1.10 or hostname"
                className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-brand-500/50"
              />
            </div>
            <div>
              <label className="block text-xs font-medium text-slate-400 mb-1.5">Port</label>
              <input
                type="number"
                value={form.port || ''}
                onChange={e => update('port', parseInt(e.target.value) || 0)}
                placeholder="e.g. 22"
                className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-brand-500/50"
              />
            </div>
          </div>

          <div>
            <label className="block text-xs font-medium text-slate-400 mb-1.5">Description</label>
            <textarea
              value={form.description}
              onChange={e => update('description', e.target.value)}
              rows={2}
              placeholder="Optional description..."
              className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-brand-500/50 resize-none"
            />
          </div>

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-xs font-medium text-slate-400 mb-1.5">Username (optional)</label>
              <input
                value={form.username}
                onChange={e => update('username', e.target.value)}
                placeholder="e.g. admin"
                className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-brand-500/50"
              />
            </div>
            <div>
              <label className="block text-xs font-medium text-slate-400 mb-1.5">Password (optional)</label>
              <input
                type="password"
                value={form.password}
                onChange={e => update('password', e.target.value)}
                placeholder="••••••••"
                className="w-full px-3 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-brand-500/50"
              />
            </div>
          </div>

          {/* Agent Selection (Feature 1/3) — shown only for server targets */}
          {isServer && (
            <div>
              <label className="block text-xs font-medium text-slate-400 mb-1.5 flex items-center gap-1.5">
                <Bot className="w-3.5 h-3.5 text-blue-400" />
                Monitored via Agent
                <span className="text-slate-600 font-normal">(optional — skips SSH verification)</span>
              </label>
              <div className="relative">
                <select
                  value={selectedAgentId}
                  onChange={e => {
                    setSelectedAgentId(e.target.value)
                    if (e.target.value) {
                      setVerifyState('idle')
                      setVerifyMessage('')
                    }
                  }}
                  className="w-full px-3 py-2.5 pr-9 bg-white/5 border border-white/10 rounded-xl text-sm text-white focus:outline-none focus:ring-2 focus:ring-blue-500/50 [&>option]:bg-slate-900 [&>option]:text-white appearance-none"
                >
                  <option value="">— No agent (use SSH) —</option>
                  {activeAgents.length === 0 && (
                    <option disabled value="">No active agents in this org</option>
                  )}
                  {activeAgents.map((a: Agent) => (
                    <option key={a.agent_id} value={a.agent_id}>
                      {a.datacenter_name}{a.region_label ? ` (${a.region_label})` : ''} — {a.ip_address ?? 'IP unknown'}
                    </option>
                  ))}
                </select>
                <ChevronDown className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-500" />
              </div>
              {agentSelected && (
                <p className="text-[11px] text-blue-400 mt-1 flex items-center gap-1">
                  <CheckCircle className="w-3 h-3" /> SSH validation skipped — agent will collect metrics directly.
                </p>
              )}
            </div>
          )}

          {/* SSH Verification Status (for server targets without agent) */}
          {sshRequired && verifyState !== 'idle' && (
            <div className="space-y-3">
              <div className={`px-4 py-3 rounded-xl text-sm flex items-start gap-2.5 border ${
                verifyState === 'verifying' ? 'bg-blue-500/10 border-blue-500/20 text-blue-400' :
                verifyState === 'passed' ? 'bg-emerald-500/10 border-emerald-500/20 text-emerald-400' :
                'bg-red-500/10 border-red-500/20 text-red-400'
              }`}>
                {verifyState === 'verifying' ? <Loader2 className="w-4 h-4 animate-spin mt-0.5 flex-shrink-0" /> :
                 verifyState === 'passed' ? <CheckCircle className="w-4 h-4 mt-0.5 flex-shrink-0" /> :
                 <XCircle className="w-4 h-4 mt-0.5 flex-shrink-0" />}
                <div>
                  <div className="font-medium">
                    {verifyState === 'verifying' ? 'Verifying SSH connectivity...' :
                     verifyState === 'passed' ? 'SSH connection verified' :
                     'SSH connection failed'}
                  </div>
                  {verifyMessage && <div className="text-xs opacity-80 mt-0.5">{verifyMessage}</div>}
                </div>
              </div>

              {/* SSH failure: show setup option */}
              {verifyState === 'failed' && !showSshSetup && (
                <div className="flex items-center gap-2">
                  <button
                    type="button"
                    onClick={() => setShowSshSetup(true)}
                    className="flex items-center gap-1.5 px-3 py-2 bg-blue-600/20 border border-blue-500/30 text-blue-400 rounded-xl text-xs font-medium hover:bg-blue-600/30 transition-colors"
                  >
                    <KeyRound className="w-3.5 h-3.5" /> Setup SSH Key Access
                  </button>
                  <button
                    type="button"
                    onClick={() => doVerify()}
                    className="flex items-center gap-1.5 px-3 py-2 bg-white/5 border border-white/10 text-slate-400 rounded-xl text-xs hover:text-white transition-colors"
                  >
                    Retry
                  </button>
                </div>
              )}

              {/* Inline SSH Key Setup */}
              {verifyState === 'failed' && showSshSetup && (
                <div className="px-4 py-3 bg-slate-800/50 border border-white/5 rounded-xl space-y-3">
                  <div className="flex items-center gap-2 text-xs font-medium text-blue-400">
                    <KeyRound className="w-3.5 h-3.5" /> Setup SSH Key Access to {form.host}
                  </div>
                  <div className="text-[11px] text-slate-400">
                    Enter the password once to copy the Zentra server's SSH key to the target machine.
                  </div>
                  <input
                    type="password"
                    value={sshPassword}
                    onChange={e => setSshPassword(e.target.value)}
                    placeholder="SSH password for one-time key setup"
                    className="w-full px-3 py-2 bg-white/5 border border-white/10 rounded-lg text-sm text-white placeholder-slate-500 focus:outline-none focus:ring-2 focus:ring-blue-500/50"
                  />
                  {sshSetupResult && (
                    <div className={`px-3 py-2 rounded-lg text-xs flex items-center gap-2 border ${
                      sshSetupResult.success ? 'bg-emerald-500/10 border-emerald-500/20 text-emerald-400' : 'bg-red-500/10 border-red-500/20 text-red-400'
                    }`}>
                      {sshSetupResult.success ? <CheckCircle className="w-3.5 h-3.5" /> : <AlertTriangle className="w-3.5 h-3.5" />}
                      {sshSetupResult.message}
                    </div>
                  )}
                  <div className="flex gap-2">
                    <button type="button" onClick={() => setShowSshSetup(false)} className="px-3 py-1.5 text-xs text-slate-400 hover:text-white">Cancel</button>
                    <button
                      type="button"
                      onClick={doSshSetup}
                      disabled={sshSetupLoading || !sshPassword}
                      className="flex items-center gap-1.5 px-4 py-1.5 bg-blue-600 hover:bg-blue-700 rounded-lg text-xs font-medium text-white disabled:opacity-50"
                    >
                      {sshSetupLoading ? <Loader2 className="w-3.5 h-3.5 animate-spin" /> : <KeyRound className="w-3.5 h-3.5" />}
                      {sshSetupLoading ? 'Setting up...' : 'Setup Key'}
                    </button>
                  </div>
                </div>
              )}
            </div>
          )}

          {(agentSelected || (sshRequired && verifyState === 'passed')) && (
            <div className="px-4 py-2.5 bg-blue-500/5 border border-blue-500/10 rounded-xl text-xs text-blue-300">
              After creating the server, Zentra will automatically crawl it to discover databases, middleware, and other components for monitoring.
            </div>
          )}

          <div className="flex justify-end gap-3 pt-2">
            <button
              type="button"
              onClick={handleClose}
              className="px-4 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-slate-400 hover:text-white transition-colors"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={mutation.isPending || verifyState === 'verifying'}
              className="flex items-center gap-2 px-5 py-2.5 bg-brand-600 hover:bg-brand-700 rounded-xl text-sm font-medium transition-colors disabled:opacity-50"
            >
              {mutation.isPending || verifyState === 'verifying' ? <Loader2 className="w-4 h-4 animate-spin" /> : <Plus className="w-4 h-4" />}
              {agentSelected ? 'Create & Discover' :
               sshRequired && verifyState === 'idle' ? 'Verify SSH & Create' :
               sshRequired && verifyState === 'verifying' ? 'Verifying...' :
               sshRequired && verifyState === 'passed' ? 'Create & Discover' :
               sshRequired && verifyState === 'failed' ? 'Retry SSH & Create' :
               'Create Target'}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}
