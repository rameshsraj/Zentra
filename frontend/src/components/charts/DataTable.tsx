import { useState, useMemo } from 'react'
import { cn } from '@/lib/utils'
import { ChevronUp, ChevronDown, ChevronsUpDown } from 'lucide-react'

export interface Column<T> {
  key: string
  header: string
  render?: (row: T, idx: number) => React.ReactNode
  sortable?: boolean
  width?: string
  align?: 'left' | 'center' | 'right'
}

interface DataTableProps<T> {
  columns: Column<T>[]
  data: T[]
  keyField?: string
  onRowClick?: (row: T) => void
  emptyMessage?: string
  className?: string
  maxHeight?: string
  striped?: boolean
  compact?: boolean
  pageSize?: number
}

export default function DataTable<T extends Record<string, unknown>>({
  columns,
  data,
  keyField = 'id',
  onRowClick,
  emptyMessage = 'No data available',
  className,
  maxHeight,
  striped = false,
  compact = false,
  pageSize = 25,
}: DataTableProps<T>) {
  const [sortKey, setSortKey] = useState<string | null>(null)
  const [sortDir, setSortDir] = useState<'asc' | 'desc'>('asc')
  const [page, setPage] = useState(0)

  const sorted = useMemo(() => {
    if (!sortKey) return data
    return [...data].sort((a, b) => {
      const av = a[sortKey]
      const bv = b[sortKey]
      if (typeof av === 'number' && typeof bv === 'number') return sortDir === 'asc' ? av - bv : bv - av
      return sortDir === 'asc' ? String(av ?? '').localeCompare(String(bv ?? '')) : String(bv ?? '').localeCompare(String(av ?? ''))
    })
  }, [data, sortKey, sortDir])

  const totalPages = Math.ceil(sorted.length / pageSize)
  const paginated = sorted.slice(page * pageSize, (page + 1) * pageSize)

  const handleSort = (key: string) => {
    if (sortKey === key) setSortDir(d => d === 'asc' ? 'desc' : 'asc')
    else { setSortKey(key); setSortDir('asc') }
  }

  return (
    <div className={cn('bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl overflow-hidden', className)}>
      <div className={cn('overflow-x-auto', maxHeight && 'overflow-y-auto')} style={maxHeight ? { maxHeight } : undefined}>
        <table className="w-full text-sm">
          <thead className="sticky top-0 z-10">
            <tr className="bg-white/5 border-b border-white/10">
              {columns.map(col => (
                <th
                  key={col.key}
                  onClick={() => col.sortable !== false && handleSort(col.key)}
                  className={cn(
                    'text-xs font-semibold text-slate-400 uppercase tracking-wider',
                    compact ? 'px-3 py-2' : 'px-5 py-3',
                    col.align === 'right' ? 'text-right' : col.align === 'center' ? 'text-center' : 'text-left',
                    col.sortable !== false && 'cursor-pointer hover:text-white transition-colors select-none',
                    col.width
                  )}
                >
                  <span className="inline-flex items-center gap-1">
                    {col.header}
                    {col.sortable !== false && (
                      sortKey === col.key
                        ? (sortDir === 'asc' ? <ChevronUp className="w-3 h-3" /> : <ChevronDown className="w-3 h-3" />)
                        : <ChevronsUpDown className="w-3 h-3 opacity-30" />
                    )}
                  </span>
                </th>
              ))}
            </tr>
          </thead>
          <tbody className="divide-y divide-white/5">
            {paginated.length === 0 ? (
              <tr>
                <td colSpan={columns.length} className="px-5 py-12 text-center text-slate-500">
                  {emptyMessage}
                </td>
              </tr>
            ) : paginated.map((row, idx) => (
              <tr
                key={String((row as Record<string, unknown>)[keyField] ?? idx)}
                onClick={() => onRowClick?.(row)}
                className={cn(
                  'transition-colors',
                  onRowClick && 'cursor-pointer',
                  striped && idx % 2 === 1 ? 'bg-white/[0.02]' : '',
                  'hover:bg-white/5'
                )}
              >
                {columns.map(col => (
                  <td
                    key={col.key}
                    className={cn(
                      compact ? 'px-3 py-1.5' : 'px-5 py-2.5',
                      col.align === 'right' ? 'text-right' : col.align === 'center' ? 'text-center' : 'text-left',
                    )}
                  >
                    {col.render ? col.render(row, idx) : String((row as Record<string, unknown>)[col.key] ?? '')}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {totalPages > 1 && (
        <div className="flex items-center justify-between px-5 py-3 border-t border-white/10 text-xs text-slate-500">
          <span>Showing {page * pageSize + 1}–{Math.min((page + 1) * pageSize, sorted.length)} of {sorted.length}</span>
          <div className="flex gap-1">
            <button disabled={page === 0} onClick={() => setPage(0)} className="px-2 py-1 rounded bg-white/5 hover:bg-white/10 disabled:opacity-30">First</button>
            <button disabled={page === 0} onClick={() => setPage(p => p - 1)} className="px-2 py-1 rounded bg-white/5 hover:bg-white/10 disabled:opacity-30">Prev</button>
            <span className="px-3 py-1 text-white font-medium">{page + 1} / {totalPages}</span>
            <button disabled={page >= totalPages - 1} onClick={() => setPage(p => p + 1)} className="px-2 py-1 rounded bg-white/5 hover:bg-white/10 disabled:opacity-30">Next</button>
            <button disabled={page >= totalPages - 1} onClick={() => setPage(totalPages - 1)} className="px-2 py-1 rounded bg-white/5 hover:bg-white/10 disabled:opacity-30">Last</button>
          </div>
        </div>
      )}
    </div>
  )
}
