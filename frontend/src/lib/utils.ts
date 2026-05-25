import { clsx, type ClassValue } from 'clsx'
import { twMerge } from 'tailwind-merge'

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs))
}

export function formatUptime(seconds: number): string {
  const d = Math.floor(seconds / 86400)
  const h = Math.floor((seconds % 86400) / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  if (d > 0) return `${d}d ${h}h ${m}m`
  if (h > 0) return `${h}h ${m}m`
  return `${m}m`
}

export function severityColor(severity: string): string {
  switch (severity.toLowerCase()) {
    case 'critical': return 'text-red-400'
    case 'warning': return 'text-amber-400'
    case 'info': return 'text-blue-400'
    default: return 'text-gray-400'
  }
}

export function statusColor(status: string): string {
  switch (status.toLowerCase()) {
    case 'healthy':
    case 'active':
    case 'resolved': return 'text-emerald-400'
    case 'degraded':
    case 'warning':
    case 'acknowledged': return 'text-amber-400'
    case 'unhealthy':
    case 'failed':
    case 'triggered': return 'text-red-400'
    default: return 'text-gray-400'
  }
}

export function truncate(s: string, max: number): string {
  return s.length > max ? s.slice(0, max) + '...' : s
}
