import { type ReactNode } from 'react'
import { useAppMode } from '@/contexts/AppModeContext'

interface LiveModeButtonProps {
  children: ReactNode
  showDisabled?: boolean
}

export default function LiveModeButton({ children }: LiveModeButtonProps) {
  return <>{children}</>
}

export { useAppMode }
