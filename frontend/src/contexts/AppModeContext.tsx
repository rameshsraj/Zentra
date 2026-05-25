import { createContext, useContext, useState, useEffect, useCallback, type ReactNode } from 'react'
import { fetchJSON } from '@/lib/api'

type AppMode = 'demo' | 'live'

interface AppModeContextValue {
  mode: AppMode
  isDemo: boolean
  isLive: boolean
  toggleMode: () => void
  setMode: (m: AppMode) => void
}

const AppModeContext = createContext<AppModeContextValue>({
  mode: 'live',
  isDemo: false,
  isLive: true,
  toggleMode: () => {},
  setMode: () => {},
})

export function AppModeProvider({ children }: { children: ReactNode }) {
  const [mode, setModeState] = useState<AppMode>('live')

  useEffect(() => {
    fetchJSON('/api/config/mode')
      .then((res: any) => {
        if (res?.mode === 'live' || res?.mode === 'demo') {
          setModeState(res.mode)
        }
      })
      .catch(() => {})
  }, [])

  const setMode = useCallback((m: AppMode) => {
    setModeState(m)
    fetch('/api/config/mode', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ mode: m }),
    }).catch(() => {})
  }, [])

  const toggleMode = useCallback(() => {
    setMode(mode === 'demo' ? 'live' : 'demo')
  }, [mode, setMode])

  return (
    <AppModeContext.Provider value={{
      mode,
      isDemo: mode === 'demo',
      isLive: mode === 'live',
      toggleMode,
      setMode,
    }}>
      {children}
    </AppModeContext.Provider>
  )
}

export function useAppMode() {
  return useContext(AppModeContext)
}
