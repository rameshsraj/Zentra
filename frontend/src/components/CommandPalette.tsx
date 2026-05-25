import { useState, useEffect, useRef, useCallback, useMemo } from 'react'
import { useNavigate } from 'react-router-dom'
import {
  Search, LayoutDashboard, Server, Database, HardDrive, Bell, Network,
  ScrollText, Shield, Brain, Bot, FileSearch, Key, Radio, BarChart3,
  Settings, Bookmark, ArrowRight, Command, Zap,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { useAuth } from '@/contexts/AuthContext'

interface PaletteItem {
  id: string
  label: string
  description?: string
  icon: React.ComponentType<{ className?: string }>
  action: () => void
  category: string
  keywords?: string[]
}

interface CommandPaletteProps {
  open: boolean
  onClose: () => void
}

export default function CommandPalette({ open, onClose }: CommandPaletteProps) {
  const navigate = useNavigate()
  const { isAdmin } = useAuth()
  const [query, setQuery] = useState('')
  const inputRef = useRef<HTMLInputElement>(null)
  const listRef = useRef<HTMLDivElement>(null)
  const [selectedIdx, setSelectedIdx] = useState(0)

  const go = useCallback((path: string) => { navigate(path); onClose() }, [navigate, onClose])

  const items: PaletteItem[] = useMemo(() => {
    const all: PaletteItem[] = [
    // Navigation
    { id: 'dashboard', label: 'Dashboard', description: 'Command Center overview', icon: LayoutDashboard, action: () => go('/dashboard'), category: 'Navigation', keywords: ['home', 'overview'] },
    { id: 'servers', label: 'Servers', description: 'Fleet monitoring', icon: Server, action: () => go('/servers'), category: 'Navigation' },
    { id: 'targets', label: 'Targets', description: 'Infrastructure targets', icon: Database, action: () => go('/targets'), category: 'Navigation' },
    { id: 'storage', label: 'Storage', description: 'Storage arrays', icon: HardDrive, action: () => go('/storage'), category: 'Navigation' },
    { id: 'alerts', label: 'Alerts', description: 'Active alerts & rules', icon: Bell, action: () => go('/alerts'), category: 'Navigation' },
    { id: 'network', label: 'Network', description: 'Network monitoring', icon: Network, action: () => go('/network'), category: 'Navigation' },
    { id: 'logs', label: 'Logs', description: 'Log search & analysis', icon: ScrollText, action: () => go('/logs'), category: 'Navigation' },
    { id: 'threats', label: 'Threats', description: 'Threat intelligence', icon: Shield, action: () => go('/threats'), category: 'Navigation' },
    { id: 'ai-insights', label: 'AI Insights', description: 'ML-powered insights', icon: Brain, action: () => go('/ai-insights'), category: 'Navigation' },
    { id: 'chatbot', label: 'AI Chatbot', description: 'Chat with Zentra', icon: Bot, action: () => go('/chatbot'), category: 'Navigation' },
    { id: 'rca', label: 'Root Cause Analysis', description: 'Log-based RCA engine', icon: FileSearch, action: () => go('/rca'), category: 'Navigation' },
    { id: 'query', label: 'Query Explorer', description: 'Metric query browser', icon: BarChart3, action: () => go('/query'), category: 'Navigation' },
    { id: 'predictive', label: 'Predictive Alerts', description: 'Anomaly & forecast alerts', icon: Zap, action: () => go('/predictive-alerts'), category: 'Navigation' },
    { id: 'annotations', label: 'Annotations', description: 'Event annotations', icon: Bookmark, action: () => go('/annotations'), category: 'Navigation' },
    { id: 'dashboards', label: 'Dashboard Manager', description: 'Manage dashboards', icon: Settings, action: () => go('/dashboards'), category: 'Navigation' },
    { id: 'licenses', label: 'License Management', description: 'Software licenses', icon: Key, action: () => go('/admin/licenses'), category: 'Admin' },
    { id: 'scrape', label: 'Scrape Targets', description: 'Prometheus scrape config', icon: Radio, action: () => go('/scrape-targets'), category: 'Admin' },
    { id: 'log-sources', label: 'Log Sources', description: 'Configure log ingestion', icon: ScrollText, action: () => go('/log-sources'), category: 'Admin' },
    ];
    // Filter admin-only items for non-admin users
    if (!isAdmin) {
      return all.filter(item => item.category !== 'Admin');
    }
    return all;
  }, [go, isAdmin])

  const filtered = useMemo(() => {
    if (!query.trim()) return items
    const q = query.toLowerCase()
    return items.filter(it =>
      it.label.toLowerCase().includes(q) ||
      it.description?.toLowerCase().includes(q) ||
      it.category.toLowerCase().includes(q) ||
      it.keywords?.some(k => k.includes(q))
    )
  }, [query, items])

  const grouped = useMemo(() => {
    const map = new Map<string, PaletteItem[]>()
    for (const item of filtered) {
      const list = map.get(item.category) ?? []
      list.push(item)
      map.set(item.category, list)
    }
    return map
  }, [filtered])

  useEffect(() => { if (open) { setQuery(''); setSelectedIdx(0); setTimeout(() => inputRef.current?.focus(), 50) } }, [open])
  useEffect(() => { setSelectedIdx(0) }, [query])

  const handleKeyDown = useCallback((e: React.KeyboardEvent) => {
    if (e.key === 'Escape') { onClose(); return }
    if (e.key === 'ArrowDown') { e.preventDefault(); setSelectedIdx(i => Math.min(i + 1, filtered.length - 1)) }
    if (e.key === 'ArrowUp') { e.preventDefault(); setSelectedIdx(i => Math.max(i - 1, 0)) }
    if (e.key === 'Enter' && filtered[selectedIdx]) { filtered[selectedIdx].action() }
  }, [filtered, selectedIdx, onClose])

  useEffect(() => {
    if (!open) return
    const flat = listRef.current?.querySelectorAll('[data-idx]')
    flat?.[selectedIdx]?.scrollIntoView({ block: 'nearest' })
  }, [selectedIdx, open])

  if (!open) return null

  return (
    <div className="fixed inset-0 z-[100] flex items-start justify-center pt-[15vh]" onClick={onClose}>
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" />
      <div
        role="dialog"
        aria-label="Command palette"
        aria-modal="true"
        className="relative w-full max-w-xl bg-slate-900 border border-white/10 rounded-2xl shadow-2xl overflow-hidden"
        onClick={e => e.stopPropagation()}
        onKeyDown={handleKeyDown}
      >
        {/* Search input */}
        <div className="flex items-center gap-3 px-4 py-3 border-b border-white/10">
          <Search className="w-5 h-5 text-slate-400" />
          <input
            ref={inputRef}
            value={query}
            onChange={e => setQuery(e.target.value)}
            className="flex-1 bg-transparent text-white text-sm outline-none placeholder-slate-500"
            placeholder="Type a command or search..."
            aria-label="Search commands"
            aria-activedescendant={filtered[selectedIdx] ? `palette-item-${filtered[selectedIdx].id}` : undefined}
            autoFocus
          />
          <kbd className="hidden sm:inline-flex px-2 py-0.5 rounded bg-white/5 border border-white/10 text-[10px] text-slate-400 font-mono">ESC</kbd>
        </div>

        {/* Results */}
        <div ref={listRef} role="listbox" aria-label="Command results" className="max-h-[400px] overflow-y-auto p-2">
          {filtered.length === 0 && (
            <div className="px-4 py-8 text-center text-sm text-slate-500">No results for "{query}"</div>
          )}

          {[...grouped.entries()].map(([cat, list]) => (
            <div key={cat} className="mb-2">
              <div className="px-3 py-1 text-[10px] font-bold text-slate-500 uppercase tracking-wider">{cat}</div>
              {list.map((item) => {
                const globalIdx = filtered.indexOf(item)
                return (
                  <button
                    key={item.id}
                    id={`palette-item-${item.id}`}
                    role="option"
                    aria-selected={globalIdx === selectedIdx}
                    data-idx={globalIdx}
                    onClick={item.action}
                    className={cn(
                      'w-full flex items-center gap-3 px-3 py-2 rounded-lg text-left transition-colors',
                      globalIdx === selectedIdx ? 'bg-brand-600/20 text-white' : 'text-slate-300 hover:bg-white/5'
                    )}
                  >
                    <item.icon className={cn('w-4 h-4', globalIdx === selectedIdx ? 'text-brand-400' : 'text-slate-500')} />
                    <div className="flex-1 min-w-0">
                      <div className="text-sm font-medium truncate">{item.label}</div>
                      {item.description && <div className="text-[11px] text-slate-500 truncate">{item.description}</div>}
                    </div>
                    <ArrowRight className={cn('w-3.5 h-3.5 opacity-0 transition-opacity', globalIdx === selectedIdx && 'opacity-100 text-brand-400')} />
                  </button>
                )
              })}
            </div>
          ))}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between px-4 py-2 border-t border-white/10 text-[10px] text-slate-500">
          <span className="flex items-center gap-1">
            <Command className="w-3 h-3" /> + K to toggle
          </span>
          <span>{filtered.length} commands</span>
        </div>
      </div>
    </div>
  )
}
