import { Activity, type LucideIcon } from 'lucide-react'

interface LiveModeEmptyProps {
  title: string
  message?: string
  icon?: LucideIcon
}

export default function LiveModeEmpty({ title, message, icon: Icon = Activity }: LiveModeEmptyProps) {
  return (
    <div className="bg-white/5 backdrop-blur-sm border border-white/10 rounded-xl p-12 text-center">
      <Icon className="w-12 h-12 text-slate-600 mx-auto mb-3" />
      <h3 className="text-lg font-bold text-white mb-1">{title}</h3>
      <p className="text-sm text-slate-400">
        {message || 'Configure targets to see real data.'}
      </p>
      <div className="mt-3 inline-flex items-center gap-1.5 px-3 py-1.5 bg-emerald-500/10 border border-emerald-500/20 rounded-lg">
        <div className="w-2 h-2 rounded-full bg-emerald-400 animate-pulse" />
        <span className="text-xs text-emerald-400 font-medium">Live</span>
      </div>
    </div>
  )
}

export function useLiveMode() {
  return { isDemo: false as const, isLive: true as const }
}
