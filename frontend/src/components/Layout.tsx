import { useState, useEffect, useCallback } from 'react'
import { Outlet, NavLink, useLocation, useNavigate } from 'react-router-dom'
import ErrorBoundary from './ErrorBoundary'
import CommandPalette from './CommandPalette'
import FloatingChatbot from './FloatingChatbot'
import { useTheme } from '@/contexts/ThemeContext'
import { themeList, type ThemeName } from '@/styles/themes'
import { Palette, Check, Sun, Moon } from 'lucide-react'
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
  Layers,
  Building2,
  type LucideIcon,
} from 'lucide-react'
import { cn } from '@/lib/utils'
import { useAuth } from '@/contexts/AuthContext'
import { useQuery } from '@tanstack/react-query'
import { getLicenseStatus } from '@/lib/api'
import { LogOut, User, Lock, AlertTriangle } from 'lucide-react'

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
      { to: '/middleware/nginx', label: 'Nginx', icon: Globe },
      { to: '/middleware/apache', label: 'Apache / httpd', icon: Globe },
      { to: '/middleware/haproxy', label: 'HAProxy', icon: Layers },
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
      { to: '/slo', label: 'SLO / SLI', icon: Activity },
    ],
  },
  {
    to: '/fleet',
    label: 'Fleet',
    icon: Server,
    children: [
      { to: '/fleet', label: 'Agents', icon: Server },
      { to: '/fleet/install', label: 'Install Wizard', icon: Cpu },
    ],
  },
  {
    to: '/billing',
    label: 'Billing',
    icon: Activity,
  },
  {
    to: '/admin/users',
    label: 'Admin',
    icon: Settings,
    children: [
      { to: '/admin/users', label: 'Users', icon: Users },
      { to: '/admin/groups', label: 'Groups', icon: Users },
      { to: '/admin/roles', label: 'Roles', icon: Lock },
      { to: '/admin/organizations', label: 'Organizations', icon: Building2 },
      { to: '/admin/profile', label: 'My Profile', icon: User },
    ],
  },
]

function UserMenu({ collapsed }: { collapsed: boolean }) {
  const { user, logout } = useAuth()

  if (!user) return null

  if (collapsed) {
    return (
      <div className="px-3 py-2" style={{ borderTop: '1px solid var(--sidebar-border)' }}>
        <button
          onClick={logout}
          className="flex justify-center w-full p-1 rounded hover:text-red-400 transition"
          style={{ color: 'var(--sidebar-text-muted)' }}
          title="Sign out"
        >
          <LogOut className="w-4 h-4" />
        </button>
      </div>
    )
  }

  return (
    <div className="px-3 py-2 space-y-1" style={{ borderTop: '1px solid var(--sidebar-border)' }}>
      <div className="flex items-center gap-2 px-2 py-1.5">
        <div className="w-7 h-7 rounded-full flex items-center justify-center flex-shrink-0" style={{ backgroundColor: 'var(--sidebar-active-bg)' }}>
          <User className="w-3.5 h-3.5" style={{ color: 'var(--accent)' }} />
        </div>
        <div className="flex-1 min-w-0">
          <p className="text-xs font-medium truncate" style={{ color: 'var(--text-primary)' }}>{user.display_name || user.username}</p>
          <p className="text-[10px] truncate" style={{ color: 'var(--sidebar-text-muted)' }}>{user.roles?.[0] || 'User'}</p>
        </div>
      </div>
      <button
        onClick={logout}
        className="w-full flex items-center gap-2 px-2 py-1.5 rounded text-xs hover:text-red-400 transition"
        style={{ color: 'var(--sidebar-text-muted)' }}
        onMouseEnter={e => (e.currentTarget.style.backgroundColor = 'var(--sidebar-hover)')}
        onMouseLeave={e => (e.currentTarget.style.backgroundColor = 'transparent')}
      >
        <LogOut className="w-3 h-3" />
        Sign out
      </button>
    </div>
  )
}

function ThemeSelector({ collapsed }: { collapsed: boolean }) {
  const { themeName, setTheme, theme: currentTheme } = useTheme()
  const [open, setOpen] = useState(false)

  const darkThemes = themeList.filter(t => t.isDark)
  const lightThemes = themeList.filter(t => !t.isDark)

  if (collapsed) {
    return (
      <div className="px-3 py-2" style={{ borderTop: `1px solid var(--sidebar-border)` }}>
        <button
          onClick={() => setOpen(!open)}
          className="flex justify-center w-full p-1.5 rounded-lg transition-colors"
          style={{ color: 'var(--sidebar-text)' }}
          onMouseEnter={e => (e.currentTarget.style.backgroundColor = 'var(--sidebar-hover)')}
          onMouseLeave={e => (e.currentTarget.style.backgroundColor = 'transparent')}
          title="Change theme"
        >
          <Palette className="w-4 h-4" />
        </button>
        {open && (
          <>
            <div className="fixed inset-0 z-40" onClick={() => setOpen(false)} />
            <div
              className="fixed left-16 bottom-16 z-50 w-64 rounded-xl shadow-2xl p-3 space-y-3 max-h-[70vh] overflow-y-auto custom-scrollbar"
              style={{ backgroundColor: 'var(--card-bg)', border: `1px solid var(--card-border)` }}
            >
              <ThemePanelContent darkThemes={darkThemes} lightThemes={lightThemes} themeName={themeName} setTheme={setTheme} onClose={() => setOpen(false)} />
            </div>
          </>
        )}
      </div>
    )
  }

  return (
    <div className="px-3 py-2" style={{ borderTop: `1px solid var(--sidebar-border)` }}>
      <button
        onClick={() => setOpen(!open)}
        className="w-full flex items-center gap-2 px-2 py-1.5 rounded-lg text-xs font-medium transition-colors"
        style={{ color: 'var(--sidebar-text)' }}
        onMouseEnter={e => (e.currentTarget.style.backgroundColor = 'var(--sidebar-hover)')}
        onMouseLeave={e => (e.currentTarget.style.backgroundColor = 'transparent')}
      >
        <Palette className="w-3.5 h-3.5" />
        <span className="flex-1 text-left">Theme</span>
        <span className="text-[10px] px-1.5 py-0.5 rounded" style={{ backgroundColor: 'var(--sidebar-hover)', color: 'var(--sidebar-text-muted)' }}>{currentTheme.label}</span>
      </button>
      {open && (
        <>
          <div className="fixed inset-0 z-40" onClick={() => setOpen(false)} />
          <div
            className="fixed left-64 bottom-16 z-50 w-72 rounded-xl shadow-2xl p-3 space-y-3 max-h-[70vh] overflow-y-auto custom-scrollbar"
            style={{ backgroundColor: 'var(--card-bg)', border: `1px solid var(--card-border)` }}
          >
            <ThemePanelContent darkThemes={darkThemes} lightThemes={lightThemes} themeName={themeName} setTheme={setTheme} onClose={() => setOpen(false)} />
          </div>
        </>
      )}
    </div>
  )
}

function ThemePanelContent({ darkThemes, lightThemes, themeName, setTheme, onClose }: {
  darkThemes: typeof themeList
  lightThemes: typeof themeList
  themeName: ThemeName
  setTheme: (name: ThemeName) => void
  onClose: () => void
}) {
  return (
    <>
      <div className="flex items-center justify-between">
        <h3 className="text-sm font-semibold" style={{ color: 'var(--text-primary)' }}>Choose Theme</h3>
        <button onClick={onClose} className="text-xs px-2 py-0.5 rounded" style={{ color: 'var(--text-muted)' }}>✕</button>
      </div>
      {lightThemes.length > 0 && (
        <div>
          <div className="flex items-center gap-1.5 mb-2">
            <Sun className="w-3 h-3" style={{ color: 'var(--text-muted)' }} />
            <span className="text-[10px] uppercase tracking-wider font-semibold" style={{ color: 'var(--text-muted)' }}>Light</span>
          </div>
          <div className="space-y-1">
            {lightThemes.map(t => (
              <ThemeOption key={t.name} theme={t} isActive={themeName === t.name} onSelect={() => { setTheme(t.name as ThemeName); onClose() }} />
            ))}
          </div>
        </div>
      )}
      <div>
        <div className="flex items-center gap-1.5 mb-2">
          <Moon className="w-3 h-3" style={{ color: 'var(--text-muted)' }} />
          <span className="text-[10px] uppercase tracking-wider font-semibold" style={{ color: 'var(--text-muted)' }}>Dark</span>
        </div>
        <div className="space-y-1">
          {darkThemes.map(t => (
            <ThemeOption key={t.name} theme={t} isActive={themeName === t.name} onSelect={() => { setTheme(t.name as ThemeName); onClose() }} />
          ))}
        </div>
      </div>
    </>
  )
}

function ThemeOption({ theme: t, isActive, onSelect }: { theme: typeof themeList[0]; isActive: boolean; onSelect: () => void }) {
  return (
    <button
      onClick={onSelect}
      className={cn(
        'w-full flex items-center gap-2.5 px-2.5 py-2 rounded-lg text-xs font-medium transition-all',
        isActive ? 'ring-1' : 'opacity-80 hover:opacity-100'
      )}
      style={{
        backgroundColor: isActive ? t.colors.sidebarActiveBg : 'transparent',
        color: isActive ? t.colors.accent : 'var(--text-secondary)',
        outlineColor: isActive ? t.colors.accent : undefined,
      }}
      onMouseEnter={e => { if (!isActive) e.currentTarget.style.backgroundColor = 'var(--sidebar-hover)' }}
      onMouseLeave={e => { if (!isActive) e.currentTarget.style.backgroundColor = 'transparent' }}
    >
      {/* Color preview dots */}
      <div className="flex gap-0.5 flex-shrink-0">
        <div className="w-3 h-3 rounded-full" style={{ backgroundColor: t.colors.bgPrimary, border: `1px solid ${t.colors.border}` }} />
        <div className="w-3 h-3 rounded-full" style={{ backgroundColor: t.colors.accent }} />
        <div className="w-3 h-3 rounded-full" style={{ backgroundColor: t.colors.success }} />
        <div className="w-3 h-3 rounded-full" style={{ backgroundColor: t.colors.danger }} />
      </div>
      <span className="flex-1 text-left">{t.label}</span>
      {isActive && <Check className="w-3.5 h-3.5" style={{ color: t.colors.accent }} />}
    </button>
  )
}

function ModeToggle({ collapsed }: { collapsed: boolean }) {
  if (collapsed) {
    return (
      <div className="p-3" style={{ borderTop: '1px solid var(--sidebar-border)' }}>
        <div className="flex justify-center w-full">
          <div className="w-2.5 h-2.5 rounded-full animate-pulse" style={{ backgroundColor: 'var(--success)' }} />
        </div>
      </div>
    )
  }

  return (
    <div className="p-3 space-y-2" style={{ borderTop: '1px solid var(--sidebar-border)' }}>
      <div className="flex items-center gap-2 text-xs" style={{ color: 'var(--sidebar-text-muted)' }}>
        <div className="w-2 h-2 rounded-full animate-pulse" style={{ backgroundColor: 'var(--success)' }} />
        <span>Zentra v0.1.0</span>
      </div>
    </div>
  )
}

export default function Layout() {
  const [collapsed, setCollapsed] = useState(() => {
    try { return localStorage.getItem('zentra-sidebar-collapsed') === 'true' } catch { return false }
  })
  const [expanded, setExpanded] = useState<string[]>(() => {
    try { return JSON.parse(localStorage.getItem('zentra-sidebar-expanded') || '[]') } catch { return [] }
  })
  const [paletteOpen, setPaletteOpen] = useState(false)
  const location = useLocation()
  const { isAdmin: userIsAdmin, hasModule, hasMenuItem } = useAuth()

  const navigate = useNavigate()

  const { data: licenseData } = useQuery({
    queryKey: ['license-status'],
    queryFn: getLicenseStatus,
    staleTime: 300_000,
    refetchInterval: 300_000,
    retry: false,
  })
  const trialExpired = licenseData?.trial_expired ?? false

  // When trial is expired, force-redirect any non-dashboard route to /dashboard
  useEffect(() => {
    if (trialExpired && location.pathname !== '/dashboard') {
      navigate('/dashboard', { replace: true })
    }
  }, [trialExpired, location.pathname, navigate])

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
    '/admin/organizations': 'admin.organizations',
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

  const dynamicNav = navItems

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

  // Auto-expand only the parent menu(s) that contain the current route.
  // On navigation, we set expanded to ONLY the relevant parents (no accumulation).
  // User manual toggles (via toggleExpand) still persist to localStorage normally.
  useEffect(() => {
    const toExpand: string[] = []
    for (const item of dynamicNav) {
      if (item.children) {
        for (const child of item.children) {
          if (location.pathname === child.to || location.pathname.startsWith(child.to + '/')) {
            if (!toExpand.includes(item.label)) toExpand.push(item.label)
          }
          if (child.children) {
            for (const sub of child.children) {
              if (location.pathname === sub.to || location.pathname.startsWith(sub.to + '/')) {
                if (!toExpand.includes(item.label)) toExpand.push(item.label)
                if (!toExpand.includes(child.label)) toExpand.push(child.label)
              }
            }
          }
        }
      }
    }
    if (toExpand.length > 0) {
      setExpanded(toExpand)
      try { localStorage.setItem('zentra-sidebar-expanded', JSON.stringify(toExpand)) } catch {}
    }
  }, [location.pathname, dynamicNav])

  const toggleExpand = (label: string) => {
    setExpanded((prev) => {
      const next = prev.includes(label) ? prev.filter((l) => l !== label) : [...prev, label]
      try { localStorage.setItem('zentra-sidebar-expanded', JSON.stringify(next)) } catch {}
      return next
    })
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
          'flex-shrink-0 flex flex-col transition-all duration-300 shadow-2xl',
          collapsed ? 'w-16' : 'w-64'
        )}
        style={{
          background: 'var(--sidebar-bg-gradient, var(--sidebar-bg))',
          borderRight: '1px solid var(--sidebar-border)',
        }}
      >
        {/* Logo */}
        <div className="h-14 flex items-center justify-between px-3" style={{ borderBottom: '1px solid var(--sidebar-border)' }}>
          <div className="flex items-center gap-2 overflow-hidden">
            <div className="w-8 h-8 rounded-lg flex items-center justify-center flex-shrink-0" style={{ backgroundColor: 'var(--sidebar-active-bg)' }}>
              <Activity className="w-5 h-5" style={{ color: 'var(--accent)' }} />
            </div>
            {!collapsed && (
              <span className="text-lg font-bold tracking-tight whitespace-nowrap" style={{ color: 'var(--text-primary)' }}>
                Zentra
              </span>
            )}
          </div>
          <button
            onClick={() => { setCollapsed(c => { const next = !c; try { localStorage.setItem('zentra-sidebar-collapsed', String(next)) } catch {} return next }) }}
            className="p-1 rounded transition-colors"
            style={{ color: 'var(--sidebar-text-muted)' }}
            onMouseEnter={e => { e.currentTarget.style.backgroundColor = 'var(--sidebar-hover)'; e.currentTarget.style.color = 'var(--text-primary)' }}
            onMouseLeave={e => { e.currentTarget.style.backgroundColor = 'transparent'; e.currentTarget.style.color = 'var(--sidebar-text-muted)' }}
          >
            {collapsed ? <Menu className="w-4 h-4" /> : <ChevronLeft className="w-4 h-4" />}
          </button>
        </div>

        {/* Search */}
        {!collapsed && (
          <div className="px-3 pt-3">
            <button
              onClick={openPalette}
              className="w-full flex items-center gap-2 px-3 py-2 rounded-lg text-sm transition-colors"
              style={{
                backgroundColor: 'var(--input-bg)',
                border: '1px solid var(--input-border)',
                color: 'var(--sidebar-text-muted)',
              }}
              onMouseEnter={e => (e.currentTarget.style.backgroundColor = 'var(--sidebar-hover)')}
              onMouseLeave={e => (e.currentTarget.style.backgroundColor = 'var(--input-bg)')}
            >
              <Search className="w-3.5 h-3.5" />
              <span>Search...</span>
              <kbd className="ml-auto text-[10px] px-1.5 py-0.5 rounded" style={{ backgroundColor: 'var(--sidebar-hover)' }}>⌘K</kbd>
            </button>
          </div>
        )}

        {/* Navigation */}
        <nav role="navigation" aria-label="Main navigation" className="flex-1 py-3 px-2 space-y-0.5 overflow-y-auto custom-scrollbar">
          {dynamicNav.filter(item => {
            if (trialExpired) return item.label === 'Command Center'
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
                    className="w-full flex items-center gap-3 px-3 py-2 rounded-lg text-sm font-medium transition-all"
                    style={{
                      backgroundColor: active ? 'var(--sidebar-active-bg)' : 'transparent',
                      color: active ? 'var(--sidebar-active-text)' : 'var(--sidebar-text)',
                    }}
                    onMouseEnter={e => { if (!active) { e.currentTarget.style.backgroundColor = 'var(--sidebar-hover)' } }}
                    onMouseLeave={e => { if (!active) { e.currentTarget.style.backgroundColor = 'transparent' } }}
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
                    <div className="ml-4 mt-0.5 space-y-0.5 pl-2" style={{ borderLeft: '1px solid var(--sidebar-border)' }}>
                      {filterChildren(item.children!).map((child) => {
                        const hasSubChildren = child.children && child.children.length > 0
                        const isSubExp = expanded.includes(child.label)
                        const childActive = isActive(child.to)
                        if (hasSubChildren) {
                          return (
                            <div key={child.label}>
                              <button
                                onClick={() => toggleExpand(child.label)}
                                className="w-full flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors"
                                style={{
                                  backgroundColor: childActive ? 'var(--sidebar-active-bg)' : 'transparent',
                                  color: childActive ? 'var(--sidebar-active-text)' : 'var(--sidebar-text-muted)',
                                }}
                                onMouseEnter={e => { if (!childActive) { e.currentTarget.style.backgroundColor = 'var(--sidebar-hover)'; e.currentTarget.style.color = 'var(--sidebar-text)' } }}
                                onMouseLeave={e => { if (!childActive) { e.currentTarget.style.backgroundColor = 'transparent'; e.currentTarget.style.color = 'var(--sidebar-text-muted)' } }}
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
                                <div className="ml-4 mt-0.5 space-y-0.5 pl-2" style={{ borderLeft: '1px solid var(--sidebar-border)' }}>
                                  {child.children!.map((sub) => {
                                    const subActive = location.pathname === sub.to || location.pathname.startsWith(sub.to + '/')
                                    return (
                                      <NavLink
                                        key={sub.to}
                                        to={sub.to}
                                        end
                                        className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-[11px] font-medium transition-colors"
                                        style={{
                                          backgroundColor: subActive ? 'var(--sidebar-active-bg)' : 'transparent',
                                          color: subActive ? 'var(--sidebar-active-text)' : 'var(--sidebar-text-muted)',
                                        }}
                                        onMouseEnter={e => { if (!subActive) { e.currentTarget.style.backgroundColor = 'var(--sidebar-hover)'; e.currentTarget.style.color = 'var(--sidebar-text)' } }}
                                        onMouseLeave={e => { if (!subActive) { e.currentTarget.style.backgroundColor = 'transparent'; e.currentTarget.style.color = 'var(--sidebar-text-muted)' } }}
                                      >
                                        <sub.icon className="w-3 h-3" />
                                        {sub.label}
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
                            key={child.to}
                            to={child.to}
                            end
                            className="flex items-center gap-2 px-3 py-1.5 rounded-lg text-xs font-medium transition-colors"
                            style={{
                              backgroundColor: childActive ? 'var(--sidebar-active-bg)' : 'transparent',
                              color: childActive ? 'var(--sidebar-active-text)' : 'var(--sidebar-text-muted)',
                            }}
                            onMouseEnter={e => { if (!childActive) { e.currentTarget.style.backgroundColor = 'var(--sidebar-hover)'; e.currentTarget.style.color = 'var(--sidebar-text)' } }}
                            onMouseLeave={e => { if (!childActive) { e.currentTarget.style.backgroundColor = 'transparent'; e.currentTarget.style.color = 'var(--sidebar-text-muted)' } }}
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
                className="flex items-center gap-3 px-3 py-2 rounded-lg text-sm font-medium transition-all"
                style={({ isActive: isNavActive }) => ({
                  backgroundColor: isNavActive ? 'var(--sidebar-active-bg)' : 'transparent',
                  color: isNavActive ? 'var(--sidebar-active-text)' : 'var(--sidebar-text)',
                })}
                onMouseEnter={e => { if (!isActive(item.to)) { e.currentTarget.style.backgroundColor = 'var(--sidebar-hover)' } }}
                onMouseLeave={e => { if (!isActive(item.to)) { e.currentTarget.style.backgroundColor = 'transparent' } }}
              >
                <item.icon className="w-4 h-4 flex-shrink-0" />
                {!collapsed && item.label}
              </NavLink>
            )
          })}
        </nav>

        {/* Theme Selector */}
        <ThemeSelector collapsed={collapsed} />

        {/* User Menu */}
        <UserMenu collapsed={collapsed} />

        {/* Demo/Live Toggle + Footer */}
        <ModeToggle collapsed={collapsed} />
      </aside>

      {/* Main content */}
      <main id="main-content" className="flex-1 overflow-y-auto" style={{ background: 'var(--main-bg-gradient, var(--main-bg))' }}>
        {trialExpired && (
          <div className="bg-amber-900/80 border-b border-amber-600/50 px-6 py-3 flex items-center gap-3">
            <AlertTriangle className="w-5 h-5 text-amber-400 flex-shrink-0" />
            <div className="flex-1">
              <p className="text-amber-200 text-sm font-medium">
                Trial period has expired. Only the Dashboard is available.
              </p>
              <p className="text-amber-300/70 text-xs mt-0.5">
                Contact the Zentra team for a production license to unlock all features.
              </p>
            </div>
          </div>
        )}
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
