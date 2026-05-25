import { createContext, useContext, useState, useEffect, useCallback, ReactNode } from 'react'
import { authLogin, authLogout, authMe, LoginRequest, LoginResponse, UserProfile } from '../lib/api'

interface AuthState {
  user: UserProfile | null
  isAuthenticated: boolean
  isLoading: boolean
  error: string | null
}

interface AuthContextType extends AuthState {
  login: (data: LoginRequest) => Promise<LoginResponse>
  logout: () => Promise<void>
  refreshProfile: () => Promise<void>
  hasPermission: (resource: string, action: string) => boolean
  hasRole: (role: string) => boolean
  isAdmin: boolean
}

const AuthContext = createContext<AuthContextType | undefined>(undefined)

export function AuthProvider({ children }: { children: ReactNode }) {
  const [state, setState] = useState<AuthState>({
    user: null,
    isAuthenticated: !!localStorage.getItem('Zentra_token'),
    isLoading: true,
    error: null,
  })

  const refreshProfile = useCallback(async () => {
    const token = localStorage.getItem('Zentra_token')
    if (!token) {
      setState({ user: null, isAuthenticated: false, isLoading: false, error: null })
      return
    }
    try {
      const profile = await authMe()
      setState({ user: profile, isAuthenticated: true, isLoading: false, error: null })
    } catch {
      setState({ user: null, isAuthenticated: false, isLoading: false, error: null })
      localStorage.removeItem('Zentra_token')
      localStorage.removeItem('Zentra_refresh_token')
    }
  }, [])

  useEffect(() => {
    refreshProfile()
  }, [refreshProfile])

  // Listen for auth:logout events from fetchJSON
  useEffect(() => {
    const handler = () => {
      setState({ user: null, isAuthenticated: false, isLoading: false, error: null })
    }
    window.addEventListener('auth:logout', handler)
    return () => window.removeEventListener('auth:logout', handler)
  }, [])

  const login = useCallback(async (data: LoginRequest): Promise<LoginResponse> => {
    setState(s => ({ ...s, isLoading: true, error: null }))
    try {
      const res = await authLogin(data)
      if (res.mfa_required && !res.token) {
        setState(s => ({ ...s, isLoading: false }))
        return res
      }
      localStorage.setItem('Zentra_token', res.token)
      localStorage.setItem('Zentra_refresh_token', res.refresh_token)
      // Fetch full profile
      const profile = await authMe()
      setState({ user: profile, isAuthenticated: true, isLoading: false, error: null })
      return res
    } catch (e: any) {
      const msg = e?.message || 'Login failed'
      setState(s => ({ ...s, isLoading: false, error: msg }))
      throw e
    }
  }, [])

  const logout = useCallback(async () => {
    try {
      await authLogout()
    } catch { /* ignore */ }
    localStorage.removeItem('Zentra_token')
    localStorage.removeItem('Zentra_refresh_token')
    setState({ user: null, isAuthenticated: false, isLoading: false, error: null })
  }, [])

  const hasPermission = useCallback((resource: string, action: string): boolean => {
    if (!state.user) return false
    if (state.user.roles.includes('Admin') || state.user.roles.includes('Super Admin')) return true
    return state.user.permissions?.some(p => p.resource === resource && p.action === action) ?? false
  }, [state.user])

  const hasRole = useCallback((role: string): boolean => {
    return state.user?.roles?.includes(role) ?? false
  }, [state.user])

  const isAdmin = state.user?.roles?.some(r => r === 'Admin' || r === 'Super Admin') ?? false

  return (
    <AuthContext.Provider value={{
      ...state,
      login,
      logout,
      refreshProfile,
      hasPermission,
      hasRole,
      isAdmin,
    }}>
      {children}
    </AuthContext.Provider>
  )
}

export function useAuth(): AuthContextType {
  const ctx = useContext(AuthContext)
  if (!ctx) throw new Error('useAuth must be used within AuthProvider')
  return ctx
}

export default AuthContext
