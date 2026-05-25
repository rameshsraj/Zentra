import { createContext, useContext, useState, useEffect, type ReactNode } from 'react'
import { themes, type ThemeName, type ThemeDefinition } from '@/styles/themes'

interface ThemeContextValue {
  theme: ThemeDefinition
  themeName: ThemeName
  setTheme: (name: ThemeName) => void
}

const ThemeContext = createContext<ThemeContextValue | null>(null)

const STORAGE_KEY = 'Zentra-theme'

function applyThemeCSSVars(theme: ThemeDefinition) {
  const root = document.documentElement
  const c = theme.colors
  root.style.setProperty('--bg-primary', c.bgPrimary)
  root.style.setProperty('--bg-secondary', c.bgSecondary)
  root.style.setProperty('--bg-tertiary', c.bgTertiary)
  root.style.setProperty('--text-primary', c.textPrimary)
  root.style.setProperty('--text-secondary', c.textSecondary)
  root.style.setProperty('--text-muted', c.textMuted)
  root.style.setProperty('--accent', c.accent)
  root.style.setProperty('--accent-hover', c.accentHover)
  root.style.setProperty('--border', c.border)
  root.style.setProperty('--border-hover', c.borderHover)
  root.style.setProperty('--success', c.success)
  root.style.setProperty('--warning', c.warning)
  root.style.setProperty('--danger', c.danger)
  root.style.setProperty('--info', c.info)
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [themeName, setThemeName] = useState<ThemeName>(() => {
    const saved = localStorage.getItem(STORAGE_KEY)
    return (saved && saved in themes) ? saved as ThemeName : 'Zentra-dark'
  })

  const theme = themes[themeName] || themes['Zentra-dark']

  useEffect(() => {
    applyThemeCSSVars(theme)
    localStorage.setItem(STORAGE_KEY, themeName)
    document.documentElement.classList.toggle('dark', theme.isDark)
    document.documentElement.classList.toggle('light', !theme.isDark)
  }, [theme, themeName])

  const setTheme = (name: ThemeName) => {
    if (name in themes) setThemeName(name)
  }

  return (
    <ThemeContext.Provider value={{ theme, themeName, setTheme }}>
      {children}
    </ThemeContext.Provider>
  )
}

export function useTheme() {
  const ctx = useContext(ThemeContext)
  if (!ctx) throw new Error('useTheme must be used within a ThemeProvider')
  return ctx
}
