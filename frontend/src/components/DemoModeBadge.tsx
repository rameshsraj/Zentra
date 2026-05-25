import { useAppMode } from '@/contexts/AppModeContext'

interface DemoModeBadgeProps {
  className?: string
}

export default function DemoModeBadge({ className = '' }: DemoModeBadgeProps) {
  const { isDemo } = useAppMode()
  if (!isDemo) return null

  return (
    <span className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-bold uppercase tracking-wider bg-amber-500/20 text-amber-400 border border-amber-500/30 ${className}`}>
      <span className="w-1.5 h-1.5 rounded-full bg-amber-400 animate-pulse" />
      Demo
    </span>
  )
}
