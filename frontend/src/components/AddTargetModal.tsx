import { useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { createTarget, verifySsh, setupSsh, crawlServer } from '@/lib/api'
import type { CrawledComponent, CrawlResult } from '@/lib/api'
import { X, Plus, Loader2, CheckCircle, XCircle, KeyRound, AlertTriangle, Search, Database, Server, HardDrive, Wifi, Package, Clock } from 'lucide-react'
import { useAppMode } from '@/contexts/AppModeContext'

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
  const { isDemo } = useAppMode()
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

  // Crawl state
  const [step, setStep] = useState<ModalStep>('form')
  const [crawlResult, setCrawlResult] = useState<CrawlResult | null>(null)
  const [crawlError, setCrawlError] = useState('')
  const [selectedComponents, setSelectedComponents] = useState<Set<number>>(new Set())
  const [registering, setRegistering] = useState(false)
  const [registerProgress, setRegisterProgress] = useState(0)
  const [registerDone, setRegisterDone] = useState(false)

  const isServer = form.target_type === 'server'

  const resetAll = () => {
    setForm({ name: '', target_type: defaultType, target_subtype: defaultSubtype, host: '', port: 0, description: '', environment: 'production', username: '', password: '' })
    setError('')
    setVerifyState('idle')
    setVerifyMessage('')
    setShowSshSetup(false)
    setSshSetupResult(null)
    setStep('form')
    setCrawlResult(null)
    setCrawlError('')
    setSelectedComponents(new Set())
    setRegistering(false)
    setRegisterProgress(0)
    setRegisterDone(false)
  }

  const handleClose = () => {
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
    }),
    onSuccess: (target) => {
      qc.invalidateQueries({ queryKey: ['targets'] })
      if (isServer) {
        const tid = (target as any).target_id || ''
        doCrawl(tid)
      } else {
        handleClose()
      }
    },
    onError: (e: Error) => setError(e.message),
  })

  const doCrawl = async (targetId: string) => {
    setStep('crawling')
    setCrawlError('')
    try {
      const result = await crawlServer(targetId)
      setCrawlResult(result)
      setSelectedComponents(new Set(result.components.map((_, i) => i)))
      setStep('discovered')
    } catch (err) {
      setCrawlError(String(err))
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
    if (isDemo) {
      setError('Adding targets is disabled in Demo mode. Switch to Live mode to manage infrastructure.')
      return
    }
    if (!form.name || !form.host || !form.target_type) {
      setError('Name, type, and host are required')
      return
    }
    if (!form.target_subtype) {
      setError('Please select a subtype')
      return
    }
    // For servers, require SSH verification to pass first
    if (isServer && verifyState !== 'passed') {
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
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm" onClick={handleClose}>
        <div className="w-full max-w-2xl bg-slate-900 border border-white/10 rounded-2xl shadow-2xl max-h-[85vh] flex flex-col" onClick={e => e.stopPropagation()}>
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
                  <div className="font-medium">Crawl failed</div>
                  <div className="text-xs opacity-80 mt-0.5">{crawlError}</div>
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
              <button
                type="button"
                onClick={handleClose}
                className="px-4 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm text-slate-400 hover:text-white transition-colors"
              >
                Close
              </button>
            )}
          </div>
        </div>
      </div>
    )
  }

  // ── Step 1: Form ──
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm" onClick={handleClose}>
      <div className="w-full max-w-lg bg-slate-900 border border-white/10 rounded-2xl shadow-2xl" onClick={e => e.stopPropagation()}>
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

          {/* SSH Verification Status (for server targets) */}
          {isServer && verifyState !== 'idle' && (
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

          {isServer && verifyState === 'passed' && (
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
              {isServer && verifyState === 'idle' ? 'Verify & Create' :
               isServer && verifyState === 'verifying' ? 'Verifying...' :
               isServer && verifyState === 'passed' ? 'Create & Discover' :
               isServer && verifyState === 'failed' ? 'Verify & Create' :
               'Create Target'}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}
