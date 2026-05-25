import { type LucideIcon, Inbox } from 'lucide-react'

interface EmptyStateProps {
  icon?: LucideIcon
  title?: string
  description?: string
  action?: {
    label: string
    onClick: () => void
  }
}

export default function EmptyState({
  icon: Icon = Inbox,
  title = 'No data found',
  description = 'There is nothing to display here yet.',
  action,
}: EmptyStateProps) {
  return (
    <div className="flex flex-col items-center justify-center py-16 px-6">
      <div className="w-16 h-16 rounded-2xl bg-white/5 border border-white/10 flex items-center justify-center mb-4">
        <Icon className="w-8 h-8 text-slate-500" />
      </div>
      <h3 className="text-sm font-bold text-white mb-1">{title}</h3>
      <p className="text-xs text-slate-400 mb-4 text-center max-w-sm">{description}</p>
      {action && (
        <button
          onClick={action.onClick}
          className="flex items-center gap-2 px-4 py-2 bg-brand-600 hover:bg-brand-700 rounded-xl text-sm font-medium transition-colors"
        >
          {action.label}
        </button>
      )}
    </div>
  )
}
