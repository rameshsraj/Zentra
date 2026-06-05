import { createContext, useContext, useState, useEffect, type ReactNode } from 'react'
import { themes, type ThemeName, type ThemeDefinition } from '@/styles/themes'

interface ThemeContextValue {
  theme: ThemeDefinition
  themeName: ThemeName
  setTheme: (name: ThemeName) => void
}

const ThemeContext = createContext<ThemeContextValue | null>(null)

const STORAGE_KEY = 'Zentra-theme'

function migrateThemeName(saved: string | null): ThemeName {
  if (!saved) return 'zentra-dark'
  // Migrate old capitalized keys
  const map: Record<string, string> = { 'Zentra-dark': 'zentra-dark', 'Zentra-light': 'zentra-light' }
  const migrated = map[saved] || saved
  return (migrated in themes) ? migrated as ThemeName : 'zentra-dark'
}

function applyThemeCSSVars(theme: ThemeDefinition) {
  const root = document.documentElement
  const c = theme.colors
  // Core colors
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
  // Sidebar
  root.style.setProperty('--sidebar-bg', c.sidebarBg)
  root.style.setProperty('--sidebar-bg-gradient', c.sidebarBgGradient)
  root.style.setProperty('--sidebar-text', c.sidebarText)
  root.style.setProperty('--sidebar-text-muted', c.sidebarTextMuted)
  root.style.setProperty('--sidebar-active-text', c.sidebarActiveText)
  root.style.setProperty('--sidebar-active-bg', c.sidebarActiveBg)
  root.style.setProperty('--sidebar-border', c.sidebarBorder)
  root.style.setProperty('--sidebar-hover', c.sidebarHover)
  // Main content
  root.style.setProperty('--main-bg', c.mainBg)
  root.style.setProperty('--main-bg-gradient', c.mainBgGradient)
  // Card / surface
  root.style.setProperty('--card-bg', c.cardBg)
  root.style.setProperty('--card-border', c.cardBorder)
  // Scrollbar
  root.style.setProperty('--scroll-thumb', c.scrollThumb)
  root.style.setProperty('--scroll-thumb-hover', c.scrollThumbHover)
  // Input
  root.style.setProperty('--input-bg', c.inputBg)
  root.style.setProperty('--input-border', c.inputBorder)
}

export function ThemeProvider({ children }: { children: ReactNode }) {
  const [themeName, setThemeName] = useState<ThemeName>(() => {
    return migrateThemeName(localStorage.getItem(STORAGE_KEY))
  })

  const theme = themes[themeName] || themes['zentra-dark']

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
