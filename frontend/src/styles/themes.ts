export interface ThemeDefinition {
  name: string
  label: string
  isDark: boolean
  colors: {
    bgPrimary: string
    bgSecondary: string
    bgTertiary: string
    textPrimary: string
    textSecondary: string
    textMuted: string
    accent: string
    accentHover: string
    border: string
    borderHover: string
    success: string
    warning: string
    danger: string
    info: string
    chartPrimary: string
    chartSecondary: string
    chartTertiary: string
    chartQuaternary: string
  }
}

export const themes: Record<string, ThemeDefinition> = {
  'Zentra-dark': {
    name: 'Zentra-dark',
    label: 'Zentra Dark',
    isDark: true,
    colors: {
      bgPrimary: '#0a0e1a',
      bgSecondary: '#111827',
      bgTertiary: '#1e293b',
      textPrimary: '#f8fafc',
      textSecondary: '#cbd5e1',
      textMuted: '#64748b',
      accent: '#6366f1',
      accentHover: '#818cf8',
      border: 'rgba(255,255,255,0.1)',
      borderHover: 'rgba(255,255,255,0.2)',
      success: '#10b981',
      warning: '#f59e0b',
      danger: '#ef4444',
      info: '#3b82f6',
      chartPrimary: '#3b82f6',
      chartSecondary: '#f59e0b',
      chartTertiary: '#8b5cf6',
      chartQuaternary: '#10b981',
    },
  },
  'Zentra-light': {
    name: 'Zentra-light',
    label: 'Zentra Light',
    isDark: false,
    colors: {
      bgPrimary: '#ffffff',
      bgSecondary: '#f8fafc',
      bgTertiary: '#f1f5f9',
      textPrimary: '#0f172a',
      textSecondary: '#475569',
      textMuted: '#94a3b8',
      accent: '#6366f1',
      accentHover: '#4f46e5',
      border: 'rgba(0,0,0,0.1)',
      borderHover: 'rgba(0,0,0,0.2)',
      success: '#059669',
      warning: '#d97706',
      danger: '#dc2626',
      info: '#2563eb',
      chartPrimary: '#2563eb',
      chartSecondary: '#d97706',
      chartTertiary: '#7c3aed',
      chartQuaternary: '#059669',
    },
  },
  'nord-dark': {
    name: 'nord-dark',
    label: 'Nord Dark',
    isDark: true,
    colors: {
      bgPrimary: '#2e3440',
      bgSecondary: '#3b4252',
      bgTertiary: '#434c5e',
      textPrimary: '#eceff4',
      textSecondary: '#d8dee9',
      textMuted: '#7b88a1',
      accent: '#88c0d0',
      accentHover: '#8fbcbb',
      border: 'rgba(216,222,233,0.1)',
      borderHover: 'rgba(216,222,233,0.2)',
      success: '#a3be8c',
      warning: '#ebcb8b',
      danger: '#bf616a',
      info: '#81a1c1',
      chartPrimary: '#81a1c1',
      chartSecondary: '#ebcb8b',
      chartTertiary: '#b48ead',
      chartQuaternary: '#a3be8c',
    },
  },
  'dracula': {
    name: 'dracula',
    label: 'Dracula',
    isDark: true,
    colors: {
      bgPrimary: '#282a36',
      bgSecondary: '#343746',
      bgTertiary: '#44475a',
      textPrimary: '#f8f8f2',
      textSecondary: '#bd93f9',
      textMuted: '#6272a4',
      accent: '#ff79c6',
      accentHover: '#ff92d0',
      border: 'rgba(248,248,242,0.1)',
      borderHover: 'rgba(248,248,242,0.2)',
      success: '#50fa7b',
      warning: '#f1fa8c',
      danger: '#ff5555',
      info: '#8be9fd',
      chartPrimary: '#8be9fd',
      chartSecondary: '#f1fa8c',
      chartTertiary: '#bd93f9',
      chartQuaternary: '#50fa7b',
    },
  },
  'solarized-dark': {
    name: 'solarized-dark',
    label: 'Solarized Dark',
    isDark: true,
    colors: {
      bgPrimary: '#002b36',
      bgSecondary: '#073642',
      bgTertiary: '#0a4a5c',
      textPrimary: '#fdf6e3',
      textSecondary: '#93a1a1',
      textMuted: '#586e75',
      accent: '#268bd2',
      accentHover: '#2aa198',
      border: 'rgba(147,161,161,0.15)',
      borderHover: 'rgba(147,161,161,0.3)',
      success: '#859900',
      warning: '#b58900',
      danger: '#dc322f',
      info: '#268bd2',
      chartPrimary: '#268bd2',
      chartSecondary: '#b58900',
      chartTertiary: '#6c71c4',
      chartQuaternary: '#859900',
    },
  },
  'high-contrast': {
    name: 'high-contrast',
    label: 'High Contrast',
    isDark: true,
    colors: {
      bgPrimary: '#000000',
      bgSecondary: '#0a0a0a',
      bgTertiary: '#1a1a1a',
      textPrimary: '#ffffff',
      textSecondary: '#e0e0e0',
      textMuted: '#808080',
      accent: '#00ff00',
      accentHover: '#33ff33',
      border: 'rgba(255,255,255,0.3)',
      borderHover: 'rgba(255,255,255,0.5)',
      success: '#00ff00',
      warning: '#ffff00',
      danger: '#ff0000',
      info: '#00ffff',
      chartPrimary: '#00ffff',
      chartSecondary: '#ffff00',
      chartTertiary: '#ff00ff',
      chartQuaternary: '#00ff00',
    },
  },
}

export const themeList = Object.values(themes)
export type ThemeName = keyof typeof themes
