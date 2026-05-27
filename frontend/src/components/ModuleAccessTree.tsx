import { useState, useCallback } from 'react'
import { ChevronRight, ChevronDown, LayoutDashboard, Cpu, HardDrive, Network as NetworkIcon, Shield, Database, Server, ScrollText, FileInput, Bell, TrendingUp, Brain, Sparkles, SearchCode, RotateCcw, Activity, Layers, Globe, Settings, Users, Lock, User } from 'lucide-react'

// ═══════════════════════════════════════════════════════════════════════════
// Module tree definition — mirrors Layout.tsx nav structure
// ═══════════════════════════════════════════════════════════════════════════

interface TreeNode {
  id: string
  label: string
  icon: any
  children?: TreeNode[]
}

const MODULE_TREE: TreeNode[] = [
  { id: 'command_center', label: 'Command Center', icon: LayoutDashboard },
  {
    id: 'infrastructure', label: 'Infrastructure', icon: Cpu,
    children: [
      { id: 'infrastructure.dashboard', label: 'Dashboard', icon: LayoutDashboard },
      { id: 'infrastructure.servers', label: 'Servers', icon: Cpu },
      { id: 'infrastructure.storage', label: 'Storage', icon: HardDrive },
      { id: 'infrastructure.network', label: 'Network', icon: NetworkIcon },
      { id: 'infrastructure.ebpf', label: 'eBPF Network', icon: Shield },
    ],
  },
  {
    id: 'database', label: 'Database', icon: Database,
    children: [
      { id: 'database.dashboard', label: 'Dashboard', icon: LayoutDashboard },
      { id: 'database.oracle', label: 'Oracle Database', icon: Database },
    ],
  },
  {
    id: 'middleware', label: 'Middleware', icon: Server,
    children: [
      { id: 'middleware.dashboard', label: 'Dashboard', icon: LayoutDashboard },
    ],
  },
  {
    id: 'security', label: 'Security', icon: Shield,
    children: [
      { id: 'security.dashboard', label: 'Dashboard', icon: LayoutDashboard },
      { id: 'security.hunt', label: 'Threat Hunting', icon: SearchCode },
      { id: 'security.firewall', label: 'Firewall', icon: Shield },
    ],
  },
  {
    id: 'logs', label: 'Logs', icon: ScrollText,
    children: [
      { id: 'logs.explorer', label: 'Explorer', icon: ScrollText },
      { id: 'logs.sources', label: 'Sources', icon: FileInput },
      { id: 'logs.alerts', label: 'Alerts', icon: Bell },
      { id: 'logs.analytics', label: 'Analytics', icon: TrendingUp },
    ],
  },
  {
    id: 'alerts_events', label: 'Alerts & Events', icon: Bell,
    children: [
      { id: 'alerts_events.active', label: 'Active Alerts', icon: Bell },
      { id: 'alerts_events.rules', label: 'Rules', icon: Settings },
      { id: 'alerts_events.channels', label: 'Channels', icon: Bell },
    ],
  },
  {
    id: 'ai_analytics', label: 'AI & Analytics', icon: Brain,
    children: [
      { id: 'ai_analytics.insights', label: 'Insights', icon: Sparkles },
      { id: 'ai_analytics.predictive', label: 'Predictive', icon: TrendingUp },
      { id: 'ai_analytics.rca', label: 'Root Cause', icon: SearchCode },
      { id: 'ai_analytics.runbooks', label: 'RunBooks', icon: RotateCcw },
      { id: 'ai_analytics.llm', label: 'LLM Observability', icon: Brain },
      { id: 'ai_analytics.profiling', label: 'Profiling', icon: Cpu },
    ],
  },
  {
    id: 'opentelemetry', label: 'OpenTelemetry', icon: Activity,
    children: [
      { id: 'opentelemetry.health', label: 'Telemetry Health', icon: Activity },
      { id: 'opentelemetry.traces', label: 'Traces', icon: Layers },
      { id: 'opentelemetry.resources', label: 'Resources', icon: Server },
      { id: 'opentelemetry.config', label: 'Collector Setup', icon: Settings },
      { id: 'opentelemetry.sdk_guide', label: 'SDK Guide', icon: Layers },
      { id: 'opentelemetry.connectors', label: 'Signal Connectors', icon: Layers },
      { id: 'opentelemetry.pipeline', label: 'Pipeline Topology', icon: Globe },
      { id: 'opentelemetry.span_metrics', label: 'Span Metrics (RED)', icon: Activity },
      { id: 'opentelemetry.alert_rules', label: 'Signal Alerts', icon: Bell },
      { id: 'opentelemetry.security', label: 'OTLP Security', icon: Shield },
    ],
  },
  {
    id: 'admin', label: 'Administration', icon: Settings,
    children: [
      { id: 'admin.users', label: 'Users', icon: Users },
      { id: 'admin.groups', label: 'Groups', icon: Users },
      { id: 'admin.roles', label: 'Roles', icon: Lock },
      { id: 'admin.profile', label: 'My Profile', icon: User },
    ],
  },
]

// ═══════════════════════════════════════════════════════════════════════════
// Component
// ═══════════════════════════════════════════════════════════════════════════

interface ModuleAccessTreeProps {
  selected: Set<string>
  onChange: (selected: Set<string>) => void
  disabled?: boolean
}

export default function ModuleAccessTree({ selected, onChange, disabled }: ModuleAccessTreeProps) {
  const [expanded, setExpanded] = useState<Set<string>>(new Set(MODULE_TREE.filter(m => m.children).map(m => m.id)))

  // Ensure command_center is always selected by default if not already
  const ensureCommandCenter = useCallback((set: Set<string>) => {
    if (!set.has('command_center')) {
      const next = new Set(set)
      next.add('command_center')
      return next
    }
    return set
  }, [])

  const toggleExpand = (id: string) => {
    const next = new Set(expanded)
    next.has(id) ? next.delete(id) : next.add(id)
    setExpanded(next)
  }

  const toggleModule = useCallback((node: TreeNode) => {
    if (node.id === 'command_center') return // Force immutable
    let next = new Set(selected)
    if (next.has(node.id)) {
      // Uncheck: remove this node + all its children
      next.delete(node.id)
      if (node.children) {
        node.children.forEach(c => next.delete(c.id))
      }
    } else {
      // Check: add this node
      next.add(node.id)
      // If it's a child, also ensure parent is checked
      const dotIdx = node.id.indexOf('.')
      if (dotIdx > 0) {
        next.add(node.id.substring(0, dotIdx))
      }
    }
    onChange(ensureCommandCenter(next))
  }, [selected, onChange, ensureCommandCenter])

  const toggleAllChildren = useCallback((parent: TreeNode, check: boolean) => {
    if (parent.id === 'command_center') return // Force immutable
    let next = new Set(selected)
    if (check) {
      next.add(parent.id)
      parent.children?.forEach(c => next.add(c.id))
    } else {
      next.delete(parent.id)
      parent.children?.forEach(c => next.delete(c.id))
    }
    onChange(ensureCommandCenter(next))
  }, [selected, onChange, ensureCommandCenter])

  const getParentState = (node: TreeNode): 'none' | 'some' | 'all' => {
    if (!node.children || node.children.length === 0) return selected.has(node.id) ? 'all' : 'none'
    const checked = node.children.filter(c => selected.has(c.id)).length
    if (checked === 0) return selected.has(node.id) ? 'some' : 'none'
    if (checked === node.children.length) return 'all'
    return 'some'
  }

  return (
    <div className="space-y-0.5">
      {MODULE_TREE.map(node => {
        const hasChildren = node.children && node.children.length > 0
        const isExp = expanded.has(node.id)
        const parentState = getParentState(node)
        const isChecked = node.id === 'command_center' ? true : selected.has(node.id)
        const isCommandCenter = node.id === 'command_center'

        return (
          <div key={node.id}>
            <div className="flex items-center gap-2 px-2 py-1.5 rounded-lg hover:bg-white/5 transition-colors group">
              {hasChildren ? (
                <button onClick={() => toggleExpand(node.id)} className="p-0.5 text-gray-500 hover:text-white">
                  {isExp ? <ChevronDown className="w-3.5 h-3.5" /> : <ChevronRight className="w-3.5 h-3.5" />}
                </button>
              ) : (
                <span className="w-4.5" />
              )}
              <label className={`flex items-center gap-2 flex-1 ${isCommandCenter ? 'cursor-not-allowed opacity-60' : 'cursor-pointer'}`}>
                <input
                  type="checkbox"
                  checked={isChecked || parentState === 'all'}
                  ref={el => { if (el && !isCommandCenter) el.indeterminate = parentState === 'some' }}
                  onChange={() => hasChildren ? toggleAllChildren(node, parentState !== 'all') : toggleModule(node)}
                  disabled={disabled || isCommandCenter}
                  className="rounded border-gray-600 bg-gray-900 text-blue-500 focus:ring-blue-500/30 disabled:opacity-50"
                />
                <node.icon className="w-4 h-4 text-gray-400 group-hover:text-gray-300" />
                <span className="text-sm text-gray-300 group-hover:text-white">{node.label} {isCommandCenter && <span className="text-[10px] text-emerald-500 bg-emerald-500/10 px-1.5 py-0.5 rounded font-mono ml-2">Required</span>}</span>
              </label>
            </div>
            {hasChildren && isExp && (
              <div className="ml-6 pl-3 border-l border-white/10 space-y-0.5">
                {node.children!.map(child => (
                  <div key={child.id} className="flex items-center gap-2 px-2 py-1 rounded-lg hover:bg-white/5 transition-colors group">
                    <label className="flex items-center gap-2 flex-1 cursor-pointer">
                      <input
                        type="checkbox"
                        checked={selected.has(child.id)}
                        onChange={() => toggleModule(child)}
                        disabled={disabled}
                        className="rounded border-gray-600 bg-gray-900 text-blue-500 focus:ring-blue-500/30"
                      />
                      <child.icon className="w-3.5 h-3.5 text-gray-500 group-hover:text-gray-400" />
                      <span className="text-xs text-gray-400 group-hover:text-gray-300">{child.label}</span>
                    </label>
                  </div>
                ))}
              </div>
            )}
          </div>
        )
      })}
    </div>
  )
}

export { MODULE_TREE }
export type { TreeNode }
