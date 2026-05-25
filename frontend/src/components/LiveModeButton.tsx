/**
 * LiveModeButton — renders children only when the app is in Live Mode.
 * In Demo Mode, renders nothing. Use this to wrap all Add/Create/Configure
 * action buttons so they don't appear in demo mode.
 */
import { useAppMode } from '@/contexts/AppModeContext'
import { type ReactNode } from 'react'

interface LiveModeButtonProps {
  children: ReactNode
  /** Optional: show a disabled/greyed version in demo mode instead of hiding */
  showDisabled?: boolean
}

export default function LiveModeButton({ children, showDisabled = false }: LiveModeButtonProps) {
  const { isLive, isDemo } = useAppMode()

  if (isLive) return <>{children}</>
  if (isDemo && showDisabled) {
    return (
      <div className="opacity-40 pointer-events-none select-none" title="Switch to Live Mode to use this feature">
        {children}
      </div>
    )
  }
  return null
}

/** Hook shorthand */
export { useAppMode }
