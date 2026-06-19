import { useQuery } from '@tanstack/react-query'
import { useNavigate } from 'react-router-dom'
import { getOnboardingChecklist, type OnboardingChecklist } from '@/lib/api/fleet'
import { Check, Circle, ArrowRight } from 'lucide-react'

const STEPS: { key: keyof OnboardingChecklist; label: string; link: string }[] = [
  { key: 'email_verified', label: 'Verify your email', link: '/admin/profile' },
  { key: 'agent_registered', label: 'Register an agent', link: '/fleet/install' },
  { key: 'target_added', label: 'Add a monitoring target', link: '/servers' },
  { key: 'metric_received', label: 'Receive first metrics', link: '/dashboard' },
  { key: 'dashboard_viewed', label: 'View a dashboard', link: '/dashboard' },
]

export default function OnboardingChecklistWidget() {
  const navigate = useNavigate()
  const { data, isLoading } = useQuery({
    queryKey: ['onboarding-checklist'],
    queryFn: getOnboardingChecklist,
    refetchInterval: 30_000,
    staleTime: 10_000,
  })

  if (isLoading || !data || data.all_complete) return null

  const completed = STEPS.filter((s) => data[s.key]).length

  return (
    <div className="rounded-xl border border-blue-500/30 bg-blue-500/5 p-5 space-y-4">
      <div className="flex items-center justify-between">
        <h3 className="font-semibold text-sm">Getting Started</h3>
        <span className="text-xs text-muted-foreground">{completed}/{STEPS.length} complete</span>
      </div>

      {/* Progress bar */}
      <div className="h-1.5 w-full rounded-full bg-muted overflow-hidden">
        <div
          className="h-full rounded-full bg-blue-500 transition-all duration-500"
          style={{ width: `${(completed / STEPS.length) * 100}%` }}
        />
      </div>

      <div className="space-y-2">
        {STEPS.map((s) => {
          const done = data[s.key]
          return (
            <button
              key={s.key}
              onClick={() => !done && navigate(s.link)}
              disabled={done}
              className={`flex items-center gap-3 w-full rounded-lg px-3 py-2 text-sm text-left transition-colors ${
                done
                  ? 'text-muted-foreground cursor-default'
                  : 'hover:bg-blue-500/10 cursor-pointer'
              }`}
            >
              {done ? (
                <Check className="h-4 w-4 text-green-400 shrink-0" />
              ) : (
                <Circle className="h-4 w-4 text-muted-foreground shrink-0" />
              )}
              <span className={done ? 'line-through' : ''}>{s.label}</span>
              {!done && <ArrowRight className="h-3.5 w-3.5 text-muted-foreground ml-auto shrink-0" />}
            </button>
          )
        })}
      </div>
    </div>
  )
}
