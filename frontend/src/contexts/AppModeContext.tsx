import { createContext, useContext, type ReactNode } from 'react'

type AppMode = 'live'

interface AppModeContextValue {
  mode: AppMode
  isDemo: false
  isLive: true
  toggleMode: () => void
  setMode: (m: AppMode) => void
}

const value: AppModeContextValue = {
  mode: 'live',
  isDemo: false,
  isLive: true,
  toggleMode: () => {},
  setMode: () => {},
}

const AppModeContext = createContext<AppModeContextValue>(value)

export function AppModeProvider({ children }: { children: ReactNode }) {
  return (
    <AppModeContext.Provider value={value}>
      {children}
    </AppModeContext.Provider>
  )
}

export function useAppMode() {
  return useContext(AppModeContext)
}
