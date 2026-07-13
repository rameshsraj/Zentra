import { useState, useEffect, useCallback } from 'react'
import { useLocation, Link } from 'react-router-dom'
import {
  Search,
  RefreshCw,
  ChevronRight,
  Home,
  Timer,
  TimerOff,
  Plus,
  type LucideIcon,
} from 'lucide-react'
import { cn } from '@/lib/utils'

const breadcrumbMap: Record<string, string> = {
  dashboard: 'Command Center',
  targets: 'All Targets',
  servers: 'Servers',
  databases: 'Databases',
  storage: 'Storage',
  network: 'Network',
  topology: 'Topology',
  threats: 'Threat Detection',
  ids: 'IDS / IPS',
  alerts: 'Alerts',
  rules: 'Alert Rules',
  logs: 'Log Analysis',
  'log-sources': 'Log Sources',
  'ai-insights': 'AI Insights',
  chatbot: 'AI Assistant',
  rca: 'Root Cause Analysis',
  'predictive-alerts': 'Predictive Alerts',
  dashboards: 'Dashboards',
  annotations: 'Annotations',
  'scrape-targets': 'Scrape Targets',
  admin: 'Product License',
  licenses: 'License Management',
  configuration: 'Configuration',
  explore: 'Explore',
  settings: 'Settings',
  reports: 'Reports',
}

interface PageHeaderProps {
  title: string
  subtitle?: string
  description?: string
  icon?: LucideIcon
  onRefresh?: () => void
  onSearch?: (query: string) => void
  searchPlaceholder?: string
  showAddTarget?: boolean
  onAddTarget?: () => void
  addTargetLabel?: string
  children?: React.ReactNode
}

export default function PageHeader({
  title,
  subtitle,
  description,
  icon: Icon,
  onRefresh,
  onSearch,
  searchPlaceholder = 'Search...',
  showAddTarget = false,
  onAddTarget,
  addTargetLabel = 'Add Target',
  children,
}: PageHeaderProps) {
  const location = useLocation()
  const [search, setSearch] = useState('')
  const [autoRefresh, setAutoRefresh] = useState(0) // 0 = off, seconds otherwise
  const [isRefreshing, setIsRefreshing] = useState(false)

  const segments = location.pathname.split('/').filter(Boolean)

  const handleRefresh = useCallback(() => {
    if (!onRefresh) return
    setIsRefreshing(true)
    onRefresh()
    setTimeout(() => setIsRefreshing(false), 600)
  }, [onRefresh])

  useEffect(() => {
    if (autoRefresh <= 0 || !onRefresh) return
    const interval = setInterval(() => {
      onRefresh()
    }, autoRefresh * 1000)
    return () => clearInterval(interval)
  }, [autoRefresh, onRefresh])

  const handleSearchChange = (val: string) => {
    setSearch(val)
    onSearch?.(val)
  }

  return (
    <div className="space-y-4 mb-6">
      {/* Breadcrumb */}
      <nav aria-label="Breadcrumb" className="flex items-center gap-1.5 text-xs text-slate-500">
        <Link to="/dashboard" aria-label="Home" className="hover:text-white transition-colors">
          <Home className="w-3.5 h-3.5" />
        </Link>
        {segments.map((seg, i) => {
          const path = '/' + segments.slice(0, i + 1).join('/')
          const isLast = i === segments.length - 1
          const label = breadcrumbMap[seg] || seg.charAt(0).toUpperCase() + seg.slice(1)
          return (
            <span key={path} className="flex items-center gap-1.5">
              <ChevronRight className="w-3 h-3 text-slate-600" aria-hidden="true" />
              {isLast ? (
                <span className="text-slate-300 font-medium" aria-current="page">{label}</span>
              ) : (
                <Link to={path} className="hover:text-white transition-colors">
                  {label}
                </Link>
              )}
            </span>
          )
        })}
      </nav>

      {/* Title Row */}
      <div className="flex items-start justify-between gap-4">
        <div className="flex items-start gap-3">
          {Icon && (
            <div className="mt-1 rounded-lg border border-white/10 bg-white/5 p-2 text-brand-400">
              <Icon className="h-5 w-5" aria-hidden="true" />
            </div>
          )}
          <div>
            <h1 className="text-3xl font-bold text-white">{title}</h1>
            {(subtitle || description) && <p className="text-slate-400 text-sm mt-1">{subtitle ?? description}</p>}
          </div>
        </div>
        <div className="flex items-center gap-2 flex-shrink-0">
          {children}
          {showAddTarget && (
            <button
              onClick={onAddTarget}
              className="flex items-center gap-2 px-4 py-2.5 bg-brand-600 hover:bg-brand-700 rounded-xl text-sm font-medium transition-colors shadow-lg shadow-brand-600/20"
            >
              <Plus className="w-4 h-4" />
              {addTargetLabel}
            </button>
          )}
        </div>
      </div>

      {/* Controls Row */}
      <div className="flex items-center gap-3 flex-wrap">
        {/* Search */}
        {onSearch && (
          <div className="relative flex-1 max-w-md">
            <Search className="absolute left-3 top-1/2 -translate-y-1/2 w-4 h-4 text-slate-500" />
            <input
              type="text"
              placeholder={searchPlaceholder}
              aria-label={searchPlaceholder}
              value={search}
              onChange={(e) => handleSearchChange(e.target.value)}
              className="w-full pl-10 pr-4 py-2.5 bg-white/5 border border-white/10 rounded-xl text-sm focus:outline-none focus:ring-2 focus:ring-brand-500/50 text-white placeholder-slate-500"
            />
          </div>
        )}

        <div className="flex items-center gap-1.5 ml-auto">
          {/* Refresh */}
          {onRefresh && (
            <button
              onClick={handleRefresh}
              className="p-2.5 rounded-xl bg-white/5 border border-white/10 text-slate-400 hover:text-white hover:bg-white/10 transition-colors"
              title="Refresh"
              aria-label="Refresh data"
            >
              <RefreshCw className={cn('w-4 h-4', isRefreshing && 'animate-spin')} aria-hidden="true" />
            </button>
          )}

          {/* Auto-refresh */}
          {onRefresh && (
            <div className="relative">
              <button
                onClick={() =>
                  setAutoRefresh((prev) => {
                    const opts = [0, 5, 10, 30, 60]
                    const idx = opts.indexOf(prev)
                    return opts[(idx + 1) % opts.length]
                  })
                }
                className={cn(
                  'flex items-center gap-1.5 px-3 py-2.5 rounded-xl border text-xs font-medium transition-colors',
                  autoRefresh > 0
                    ? 'bg-emerald-500/20 border-emerald-500/30 text-emerald-400'
                    : 'bg-white/5 border-white/10 text-slate-500 hover:text-white'
                )}
                title={autoRefresh > 0 ? `Auto-refresh: ${autoRefresh}s` : 'Auto-refresh: Off'}
                aria-label={autoRefresh > 0 ? `Auto-refresh every ${autoRefresh} seconds` : 'Auto-refresh off'}
              >
                {autoRefresh > 0 ? (
                  <>
                    <Timer className="w-3.5 h-3.5" />
                    {autoRefresh}s
                  </>
                ) : (
                  <>
                    <TimerOff className="w-3.5 h-3.5" />
                    Off
                  </>
                )}
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  )
}



