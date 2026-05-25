import { format, formatDistanceToNow } from 'date-fns'

export function formatDate(date: string | Date | null | undefined): string {
  if (!date) return '—'
  const d = typeof date === 'string' ? new Date(date) : date
  if (isNaN(d.getTime())) return '—'
  return format(d, 'MMM d, yyyy HH:mm')
}

export function formatRelative(date: string | Date | null | undefined): string {
  if (!date) return '—'
  const d = typeof date === 'string' ? new Date(date) : date
  if (isNaN(d.getTime())) return '—'
  return formatDistanceToNow(d, { addSuffix: true })
}

export function formatNumber(value: number | null | undefined): string {
  if (value == null) return '—'
  return new Intl.NumberFormat('en-US').format(value)
}

export function formatBytes(bytes: number | null | undefined): string {
  if (bytes == null) return '—'
  if (bytes === 0) return '0 B'
  const units = ['B', 'KB', 'MB', 'GB', 'TB', 'PB']
  const i = Math.floor(Math.log(Math.abs(bytes)) / Math.log(1024))
  return `${(bytes / Math.pow(1024, i)).toFixed(1)} ${units[i]}`
}

export function formatPercent(value: number | null | undefined): string {
  if (value == null) return '—'
  return `${value.toFixed(1)}%`
}
