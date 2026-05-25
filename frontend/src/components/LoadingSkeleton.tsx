import { cn } from '@/lib/utils'

interface LoadingSkeletonProps {
  variant?: 'card' | 'table' | 'kpi' | 'chart' | 'list'
  count?: number
  className?: string
}

function Pulse({ className }: { className?: string }) {
  return <div className={cn('animate-pulse bg-white/5 rounded-lg', className)} />
}

function KPISkeleton() {
  return (
    <div className="grid grid-cols-2 md:grid-cols-3 lg:grid-cols-6 gap-3">
      {Array.from({ length: 6 }).map((_, i) => (
        <div key={i} className="bg-white/5 border border-white/10 p-3 rounded-xl space-y-2">
          <Pulse className="h-3 w-16" />
          <Pulse className="h-7 w-20" />
        </div>
      ))}
    </div>
  )
}

function CardSkeleton({ count = 3 }: { count?: number }) {
  return (
    <div className="grid grid-cols-1 lg:grid-cols-2 xl:grid-cols-3 gap-4">
      {Array.from({ length: count }).map((_, i) => (
        <div key={i} className="bg-white/5 border border-white/10 rounded-xl p-5 space-y-4">
          <div className="flex items-center gap-3">
            <Pulse className="w-10 h-10 rounded-lg" />
            <div className="space-y-2 flex-1">
              <Pulse className="h-4 w-32" />
              <Pulse className="h-3 w-48" />
            </div>
          </div>
          <div className="flex justify-around">
            <Pulse className="w-20 h-20 rounded-full" />
            <Pulse className="w-20 h-20 rounded-full" />
            <Pulse className="w-20 h-20 rounded-full" />
          </div>
          <div className="space-y-2">
            <Pulse className="h-4 w-full" />
            <Pulse className="h-4 w-full" />
            <Pulse className="h-4 w-full" />
          </div>
        </div>
      ))}
    </div>
  )
}

function TableSkeleton({ count = 5 }: { count?: number }) {
  return (
    <div className="bg-white/5 border border-white/10 rounded-xl overflow-hidden">
      <div className="px-5 py-4 border-b border-white/10">
        <Pulse className="h-5 w-40" />
      </div>
      <div className="divide-y divide-white/5">
        {Array.from({ length: count }).map((_, i) => (
          <div key={i} className="px-5 py-3 flex items-center gap-4">
            <Pulse className="w-8 h-8 rounded-lg" />
            <Pulse className="h-4 w-32" />
            <Pulse className="h-4 w-24 ml-auto" />
            <Pulse className="h-4 w-16" />
            <Pulse className="h-6 w-16 rounded-full" />
          </div>
        ))}
      </div>
    </div>
  )
}

function ChartSkeleton() {
  return (
    <div className="bg-white/5 border border-white/10 rounded-xl p-5 space-y-4">
      <div className="flex items-center justify-between">
        <Pulse className="h-5 w-40" />
        <div className="flex gap-1">
          <Pulse className="h-7 w-10 rounded-lg" />
          <Pulse className="h-7 w-10 rounded-lg" />
          <Pulse className="h-7 w-10 rounded-lg" />
        </div>
      </div>
      <Pulse className="h-64 w-full rounded-lg" />
    </div>
  )
}

function ListSkeleton({ count = 5 }: { count?: number }) {
  return (
    <div className="space-y-3">
      {Array.from({ length: count }).map((_, i) => (
        <div key={i} className="bg-white/5 border border-white/10 rounded-xl p-4 flex items-center gap-4">
          <Pulse className="w-10 h-10 rounded-lg flex-shrink-0" />
          <div className="flex-1 space-y-2">
            <Pulse className="h-4 w-48" />
            <Pulse className="h-3 w-32" />
          </div>
          <Pulse className="h-6 w-20 rounded-full" />
        </div>
      ))}
    </div>
  )
}

export default function LoadingSkeleton({ variant = 'card', count = 3, className }: LoadingSkeletonProps) {
  return (
    <div className={cn('space-y-6', className)} role="status" aria-label="Loading content">
      {variant === 'kpi' && <KPISkeleton />}
      {variant === 'card' && <CardSkeleton count={count} />}
      {variant === 'table' && <TableSkeleton count={count} />}
      {variant === 'chart' && <ChartSkeleton />}
      {variant === 'list' && <ListSkeleton count={count} />}
      <span className="sr-only">Loading...</span>
    </div>
  )
}

export { KPISkeleton, CardSkeleton, TableSkeleton, ChartSkeleton, ListSkeleton, Pulse }
