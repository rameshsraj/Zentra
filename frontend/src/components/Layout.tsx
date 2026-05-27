import { useState, useEffect, useCallback, useMemo } from 'react'
import { Outlet, NavLink, useLocation } from 'react-router-dom'
import ErrorBoundary from './ErrorBoundary'
import CommandPalette from './CommandPalette'
import FloatingChatbot from './FloatingChatbot'
import { useTheme } from '@/contexts/ThemeContext'
import { themeList, type ThemeName } from '@/styles/themes'
import {
  LayoutDashboard,
  Bell,
  Network,
  ScrollText,
  Shield,
  Activity,
  HardDrive,
  Settings,
  ChevronRight,
  ChevronLeft,
  Database,
  Cpu,
  Menu,
  Search,
  Brain,
  SearchCode,
  FileInput,
  Sparkles,
  TrendingUp,
  Globe,
  Server,
  RotateCcw,
  Users,
  Container,
  Layers,
  MessageSquare,
  type LucideIcon,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { useAuth } from '@/contexts/AuthContext'
import { useQuery } from '@tanstack/react-query'
import { getMiddlewareDashboard, type MiddlewareServiceStatus } from '@/lib/api'
import { LogOut, User, Lock } from 'lucide-react'

const MIDDLEWARE_META: Record<string, { label: string; icon: LucideIcon }> = {
  nginx: { label: 'Nginx', icon: Globe },
  apache: { label: 'Apache', icon: Globe },
  tomcat: { label: 'Tomcat', icon: Cpu },
  haproxy: { label: 'HAProxy', icon: Layers },
  elasticsearch: { label: 'Elasticsearch', icon: Database },
  redis: { label: 'Redis', icon: HardDrive },
  memcached: { label: 'Memcached', icon: HardDrive },
  rabbitmq: { label: 'RabbitMQ', icon: MessageSquare },
  kafka: { label: 'Kafka', icon: MessageSquare },
  docker: { label: 'Docker', icon: Container },
  kubernetes: { label: 'Kubernetes', icon: Container },
  phpfpm: { label: 'PHP-FPM', icon: Cpu },
}

interface NavChild {
  to: string
  label: string
  icon: LucideIcon
  children?: { to: string; label: string; icon: LucideIcon }[]
}

interface NavItem {
  to: string
  label: string
  icon: LucideIcon
  children?: NavChild[]
}

const navItems: NavItem[] = [
  { to: '/dashboard', label: 'Command Center', icon: LayoutDashboard },
  {
    to: '/infrastructure',
    label: 'Infrastructure',
    icon: Cpu,
    children: [
      { to: '/infrastructure', label: 'Dashboard', icon: LayoutDashboard },
      { to: '/servers', label: 'Servers', icon: Cpu },
      { to: '/storage-arrays', label: 'Storage', icon: HardDrive },
      { to: '/network', label: 'Network', icon: Network },
      { to: '/cilium', label: 'eBPF Network', icon: Shield },
    ],
  },
  {
    to: '/databases',
    label: 'Database',
    icon: Database,
    children: [
      { to: '/databases', label: 'Dashboard', icon: LayoutDashboard },
      {
        to: '/oracle',
        label: 'Oracle Database',
        icon: Database,
        children: [
          { to: '/oracle', label: 'Oracle Monitoring', icon: Activity },
          { to: '/oracle/metrics', label: 'Metric Editor', icon: Settings },
        ],
      },
    ],
  },
  {
    to: '/middleware',
    label: 'Middleware',
    icon: Server,
    children: [
      { to: '/middleware', label: 'Dashboard', icon: LayoutDashboard },
    ],
  },
  {
    to: '/security/dashboard',
    label: 'Security',
    icon: Shield,
    children: [
      { to: '/security/dashboard', label: 'Dashboard', icon: LayoutDashboard },
      { to: '/security/hunt', label: 'Threat Hunting', icon: SearchCode },
      { to: '/security/firewall', label: 'Firewall', icon: Shield },
    ],
  },
  {
    to: '/log-explorer',
    label: 'Logs',
    icon: ScrollText,
    children: [
      { to: '/log-explorer', label: 'Explorer', icon: ScrollText },
      { to: '/log-sources', label: 'Sources', icon: FileInput },
      { to: '/log-alerts', label: 'Alerts', icon: Bell },
      { to: '/log-analytics', label: 'Analytics', icon: TrendingUp },
    ],
  },
  {
    to: '/alerts',
    label: 'Alerts & Events',
    icon: Bell,
    children: [
      { to: '/alerts', label: 'Active Alerts', icon: Bell },
      { to: '/alerts/rules', label: 'Rules', icon: Settings },
      { to: '/notifications', label: 'Channels', icon: Bell },
    ],
  },
  {
    to: '/ai-insights',
    label: 'AI & Analytics',
    icon: Brain,
    children: [
      { to: '/ai-insights', label: 'Insights', icon: Sparkles },
      { to: '/predictive-alerts', label: 'Predictive', icon: TrendingUp },
      { to: '/rca', label: 'Root Cause', icon: SearchCode },
      { to: '/runbooks', label: 'RunBooks', icon: RotateCcw },
      { to: '/llm-observability', label: 'LLM Observability', icon: Brain },
      { to: '/profiling', label: 'Profiling', icon: Cpu },
    ],
  },
  {
    to: '/otel-dashboard',
    label: 'OpenTelemetry',
    icon: Activity,
    children: [
      { to: '/otel-dashboard', label: 'Telemetry Health', icon: Activity },
      { to: '/traces', label: 'Traces', icon: Layers },
      { to: '/otel-resources', label: 'Resources', icon: Server },
      { to: '/otel-config', label: 'Collector Setup', icon: Settings },
      { to: '/otel-sdk-guide', label: 'SDK Guide', icon: Layers },
      { to: '/otel-connectors', label: 'Signal Connectors', icon: Layers },
      { to: '/otel-pipeline', label: 'Pipeline Topology', icon: Globe },
      { to: '/otel-span-metrics', label: 'Span Metrics (RED)', icon: Activity },
      { to: '/otel-alert-rules', label: 'Signal Alerts', icon: Bell },
      { to: '/otel-security', label: 'OTLP Security', icon: Shield },
    ],
  },
  {
    to: '/admin/users',
    label: 'Admin',
    icon: Settings,
    children: [
      { to: '/admin/users', label: 'Users', icon: Users },
      { to: '/admin/groups', label: 'Groups', icon: Users },
      { to: '/admin/roles', label: 'Roles', icon: Lock },
      { to: '/admin/profile', label: 'My Profile', icon: User },
    ],
  },
]

function UserMenu({ collapsed }: { collapsed: boolean }) {
  const { user, logout } = useAuth()

  if (!user) return null

  if (collapsed) {
    return (
      <div className="px-3 py-2 border-t border-white/10">
        <button
          onClick={logout}
          className="flex justify-center w-full p-1 rounded hover:bg-white/10 text-gray-400 hover:text-red-400 transition"
          title="Sign out"
        >
          <LogOut className="w-4 h-4" />
        </button>
      </div>
    )
  }

  return (
    <div className="px-3 py-2 border-t border-white/10 space-y-1">
      <div className="flex items-center gap-2 px-2 py-1.5">
        <div className="w-7 h-7 rounded-full bg-brand-600/30 flex items-center justify-center flex-shrink-0">
          <User className="w-3.5 h-3.5 text-brand-400" />
        </div>
        <div className="flex-1 min-w-0">
          <p className="text-xs font-medium text-white truncate">{user.display_name || user.username}</p>
          <p className="text-[10px] text-gray-500 truncate">{user.roles?.[0] || 'User'}</p>
        </div>
      </div>
      <button
        onClick={logout}
        className="w-full flex items-center gap-2 px-2 py-1.5 rounded text-xs text-gray-400 hover:text-red-400 hover:bg-white/5 transition"
      >
        <LogOut className="w-3 h-3" />
        Sign out
      </button>
    </div>
  )
}

function ThemeSelector() {
  const { themeName, setTheme } = useTheme()
  return (
    <div className="px-3 py-2 border-t border-white/10">
      <label className="text-[9px] text-slate-500 uppercase tracking-wider font-semibold">Theme</label>
      <div className="flex flex-wrap gap-1 mt-1">
        {themeList.map(t => (
          <button
            key={t.name}
            onClick={() => setTheme(t.name as ThemeName)}
            className={`w-5 h-5 rounded-full border-2 transition-all ${themeName === t.name ? 'border-white scale-110' : 'border-transparent opacity-60 hover:opacity-100'}`}
            style={{ backgroundColor: t.colors.accent }}
            title={t.label}
          />
        ))}
      </div>
    </div>
  )
}

function ModeToggle({ collapsed }: { collapsed: boolean }) {
  if (collapsed) {
    return (
      <div className="p-3 border-t border-white/10">
        <div className="flex justify-center w-full">
          <div className="w-2.5 h-2.5 rounded-full animate-pulse bg-emerald-500" />
        </div>
      </div>
    )
  }

  return (
    <div className="p-3 border-t border-white/10 space-y-2">
      <div className="flex items-center gap-2 text-xs text-gray-500">
        <div className="w-2 h-2 rounded-full animate-pulse bg-emerald-500" />
        <span>Zentra v0.1.0</span>
      </div>
    </div>
  )
}

export default function Layout() {
  const [collapsed, setCollapsed] = useState(false)
  const [expanded, setExpanded] = useState<string[]>([])
  const [paletteOpen, setPaletteOpen] = useState(false)
  const location = useLocation()
  const { isAdmin: userIsAdmin, hasModule, hasMenuItem } = useAuth()

  const { data: mwDashboard = [] } = useQuery({
    queryKey: ['middleware-dashboard-nav'],
    queryFn: getMiddlewareDashboard,
    staleTime: 60_000,
    refetchInterval: 120_000,
  })

  // Map nav labels to module IDs for access control
  const NAV_MODULE_MAP: Record<string, string> = {
    'Command Center': 'command_center',
    'Infrastructure': 'infrastructure',
    'Database': 'database',
    'Middleware': 'middleware',
    'Security': 'security',
    'Logs': 'logs',
    'Alerts & Events': 'alerts_events',
    'AI & Analytics': 'ai_analytics',
    'OpenTelemetry': 'opentelemetry',
    'Admin': 'admin',
  }

  // Map child routes to sub-module menu_item IDs for fine-grained access
  const NAV_CHILD_MAP: Record<string, string> = {
    // Infrastructure
    '/infrastructure': 'infrastructure.dashboard',
    '/servers': 'infrastructure.servers',
    '/storage-arrays': 'infrastructure.storage',
    '/network': 'infrastructure.network',
    '/cilium': 'infrastructure.ebpf',
    // Database
    '/databases': 'database.dashboard',
    '/oracle': 'database.oracle',
    // Security
    '/security/dashboard': 'security.dashboard',
    '/security/hunt': 'security.hunt',
    '/security/firewall': 'security.firewall',
    // Logs
    '/log-explorer': 'logs.explorer',
    '/log-sources': 'logs.sources',
    '/log-alerts': 'logs.alerts',
    '/log-analytics': 'logs.analytics',
    // Alerts & Events
    '/alerts': 'alerts_events.active',
    '/alerts/rules': 'alerts_events.rules',
    '/notifications': 'alerts_events.channels',
    // AI & Analytics
    '/ai-insights': 'ai_analytics.insights',
    '/predictive-alerts': 'ai_analytics.predictive',
    '/rca': 'ai_analytics.rca',
    '/runbooks': 'ai_analytics.runbooks',
    '/llm-observability': 'ai_analytics.llm',
    '/profiling': 'ai_analytics.profiling',
    // OpenTelemetry
    '/otel-dashboard': 'opentelemetry.health',
    '/traces': 'opentelemetry.traces',
    '/otel-resources': 'opentelemetry.resources',
    '/otel-config': 'opentelemetry.config',
    '/otel-sdk-guide': 'opentelemetry.sdk_guide',
    '/otel-connectors': 'opentelemetry.connectors',
    '/otel-pipeline': 'opentelemetry.pipeline',
    '/otel-span-metrics': 'opentelemetry.span_metrics',
    '/otel-alert-rules': 'opentelemetry.alert_rules',
    '/otel-security': 'opentelemetry.security',
    // Admin
    '/admin/users': 'admin.users',
    '/admin/groups': 'admin.groups',
    '/admin/roles': 'admin.roles',
    '/admin/profile': 'admin.profile',
  }

  // Filter children based on sub-module access
  const filterChildren = (children: NavChild[]): NavChild[] => {
    return children.filter(child => {
      const menuId = NAV_CHILD_MAP[child.to]
      if (!menuId) return true // not mapped = always visible
      return hasMenuItem(menuId)
    }).map(child => {
      // Also filter nested grandchildren (e.g. Oracle sub-items)
      if (child.children) {
        const filtered = child.children.filter(sub => {
          const menuId = NAV_CHILD_MAP[sub.to]
          if (!menuId) return true
          return hasMenuItem(menuId)
        })
        return { ...child, children: filtered.length > 0 ? filtered : undefined }
      }
      return child
    }).filter(child => {
      // Remove parents whose all children were filtered out
      if (child.children !== undefined && child.children.length === 0) return false
      return true
    })
  }

  const dynamicNav = useMemo(() => {
    const activeTypes = [...new Set(mwDashboard.map((s: MiddlewareServiceStatus) => s.service_type))]
    const middlewareChildren: NavChild[] = [
      { to: '/middleware', label: 'Dashboard', icon: LayoutDashboard },
    ]
    for (const t of activeTypes) {
      const meta = MIDDLEWARE_META[t]
      if (meta) {
        middlewareChildren.push({ to: `/middleware/${t}`, label: meta.label, icon: meta.icon })
      }
    }
    return navItems.map(item =>
      item.label === 'Middleware' ? { ...item, children: middlewareChildren } : item
    )
  }, [mwDashboard])

  const openPalette = useCallback(() => setPaletteOpen(true), [])
  const closePalette = useCallback(() => setPaletteOpen(false), [])

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
        e.preventDefault()
        setPaletteOpen(prev => !prev)
      }
    }
    window.addEventListener('keydown', handler)
    return () => window.removeEventListener('keydown', handler)
  }, [])

  const toggleExpand = (label: string) => {
    setExpanded((prev) =>
      prev.includes(label) ? prev.filter((l) => l !== label) : [...prev, label]
    )
  }

  const isActive = (to: string) => location.pathname === to || location.pathname.startsWith(to + '/')

  return (
    <div className="flex h-screen overflow-hidden">
      <a href="#main-content" className="sr-only focus:not-sr-only focus:absolute focus:z-50 focus:p-2 focus:bg-blue-600 focus:text-white">
        Skip to content
      </a>
      {/* Sidebar */}
      <aside
        className={cn(
          'flex-shrink-0 flex flex-col transition-all duration-300 bg-gradient-to-b from-slate-900 via-blue-950/30 to-slate-900 border-r border-white/10 shadow-2xl',
          collapsed ? 'w-16' : 'w-64'
        )}
      >
        {/* Logo */}
        <div className="h-14 flex items-center justify-between px-3 border-b border-white/10">
          <div className="flex items-center gap-2 overflow-hidden">
            <div className="w-8 h-8 rounded-lg bg-brand-600/30 flex items-center justify-center flex-shrink-0">
              <Activity className="w-5 h-5 text-brand-400" />
            </div>
            {!collapsed && (
              <span className="text-lg font-bold tracking-tight text-white whitespace-nowrap">
                Zentra
              </span>
            )}
          </div>
          <button
            onClick={() => setCollapsed(!collapsed)}
            className="p-1 rounded hover:bg-white/10 text-gray-400 hover:text-white transition-colors"
          >
            {collapsed ? <Menu className="w-4 h-4" /> : <ChevronLeft className="w-4 h-4" />}
          </button>
        </div>

        {/* Search */}
        {!collapsed && (
          <div className="px-3 pt-3">
            <button
              onClick={openPalette}
              className="w-full flex items-center gap-2 px-3 py-2 rounded-lg bg-white/5 border border-white/10 text-gray-500 text-sm hover:bg-white/10 transition-colors"
            >
              <Search className="w-3.5 h-3.5" />
              <span>Search...</span>
              <kbd className="ml-auto text-[10px] bg-white/10 px-1.5 py-0.5 rounded">⌘K</kbd>
            </button>
          </div>
        )}

        {/* Navigation */}
        <nav role="navigation" aria-label="Main navigation" className="flex-1 py-3 px-2 space-y-0.5 overflow-y-auto custom-scrollbar">
          {dynamicNav.filter(item => {
            if (item.label === 'Administration') return userIsAdmin
            const moduleId = NAV_MODULE_MAP[item.label]
            if (moduleId && !hasModule(moduleId)) return false
            return true
          }).map((item) => {
            const hasChildren = item.children && item.children.length > 0
            const isExp = expanded.includes(item.label)
            const active = isActive(item.to)

            if (hasChildren) {
              return (
                <div key={item.label}>
                  <button
                    onClick={() => toggleExpand(item.label)}
                    className={cn(
                      'w-full flex items-center gap-3 px-3 py-2 rounded-lg text-sm font-medium transition-all',
                      active
                        ? 'bg-brand-600/20 text-brand-400'
                        : 'text-gray-400 hover:bg-white/10 hover:text-gray-200'
                    )}
                  >
                    <item.icon className="w-4 h-4 flex-shrink-0" />
                    {!collapsed && (
                      <>
                        <span className="flex-1 text-left">{item.label}</span>
                        <ChevronRight
                          className={cn(
                            'w-3.5 h-3.5 transition-transform',
                            isExp && 'rotate-90'
                          )}
                        />
                      </>
                    )}
                  </button>
                  {isExp && !collapsed && (
                    <div className="ml-4 mt-0.5 space-y-0.5 border-l border-white/10 pl-2">
                      {filterChildren(item.children!).map((child) => {
                        const hasSubChildren = child.children && child.children.length > 0
                        const isSubExp = expanded.includes(child.label)
                        if (hasSubChildren) {
                          return (
                            <div key={child.label}>
                              <button
                                onClick={() => toggleExpand(child.label)}
                                className={cn(
                                  'w-full flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors',
                                  isActive(child.to)
                                    ? 'bg-brand-600/20 text-brand-400'
                                    : 'text-gray-500 hover:bg-white/10 hover:text-gray-300'
                                )}
                              >
                                <child.icon className="w-3.5 h-3.5" />
                                <span className="flex-1 text-left">{child.label}</span>
                                <ChevronRight
                                  className={cn(
                                    'w-3 h-3 transition-transform',
                                    isSubExp && 'rotate-90'
                                  )}
                                />
                              </button>
                              {isSubExp && (
                                <div className="ml-4 mt-0.5 space-y-0.5 border-l border-white/10 pl-2">
                                  {child.children!.map((sub) => (
                                    <NavLink
                                      key={sub.to}
                                      to={sub.to}
                                      end
                                      className={({ isActive: isSubActive }) =>
                                        cn(
                                          'flex items-center gap-2 px-3 py-1.5 rounded-lg text-[11px] font-medium transition-colors',
                                          isSubActive
                                            ? 'bg-brand-600/20 text-brand-400'
                                            : 'text-gray-500 hover:bg-white/10 hover:text-gray-300'
                                        )
                                      }
                                    >
                                      <sub.icon className="w-3 h-3" />
                                      {sub.label}
                                    </NavLink>
                                  ))}
                                </div>
                              )}
                            </div>
                          )
                        }
                        return (
                          <NavLink
                            key={child.to}
                            to={child.to}
                            end
                            className={({ isActive: isChildActive }) =>
                              cn(
                                'flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors',
                                isChildActive
                                  ? 'bg-brand-600/20 text-brand-400'
                                  : 'text-gray-500 hover:bg-white/10 hover:text-gray-300'
                              )
                            }
                          >
                            <child.icon className="w-3.5 h-3.5" />
                            {child.label}
                          </NavLink>
                        )
                      })}
                    </div>
                  )}
                </div>
              )
            }

            return (
              <NavLink
                key={item.to}
                to={item.to}
                className={({ isActive: isNavActive }) =>
                  cn(
                    'flex items-center gap-3 px-3 py-2 rounded-lg text-sm font-medium transition-all',
                    isNavActive
                      ? 'bg-brand-600/20 text-brand-400 shadow-lg shadow-brand-600/5'
                      : 'text-gray-400 hover:bg-white/10 hover:text-gray-200'
                  )
                }
              >
                <item.icon className="w-4 h-4 flex-shrink-0" />
                {!collapsed && item.label}
              </NavLink>
            )
          })}
        </nav>

        {/* Theme Selector */}
        {!collapsed && (
          <ThemeSelector />
        )}

        {/* User Menu */}
        <UserMenu collapsed={collapsed} />

        {/* Demo/Live Toggle + Footer */}
        <ModeToggle collapsed={collapsed} />
      </aside>

      {/* Main content */}
      <main id="main-content" className="flex-1 overflow-y-auto bg-gradient-to-br from-slate-900 via-slate-800 to-slate-900">
        <div className="p-6 max-w-[1600px] mx-auto">
          <ErrorBoundary>
            <Outlet />
          </ErrorBoundary>
        </div>
      </main>

      {/* Command Palette */}
      <CommandPalette open={paletteOpen} onClose={closePalette} />

      {/* Floating MCP Chatbot */}
      <FloatingChatbot />
    </div>
  )
}
