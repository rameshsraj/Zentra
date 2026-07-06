import { clearAuthTokens, getAccessToken, getRefreshToken, setAuthTokens } from '../authStorage'

const BASE = '/api'

function getAuthToken(): string | null {
  return getAccessToken()
}

// ── Token refresh mutex ─────────────────────────────────────────────
// Prevents multiple concurrent 401s from each trying to refresh.
let refreshPromise: Promise<string | null> | null = null

async function doTokenRefresh(): Promise<string | null> {
  const refreshToken = getRefreshToken()
  if (!refreshToken) return null
  try {
    const res = await fetch(`${BASE}/auth/refresh`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ refresh_token: refreshToken }),
    })
    if (res.ok) {
      const data = await res.json()
      setAuthTokens(data.token, data.refresh_token)
      scheduleProactiveRefresh(data.expires_at)
      return data.token
    }
  } catch { /* fall through */ }
  return null
}

function refreshTokenOnce(): Promise<string | null> {
  if (!refreshPromise) {
    refreshPromise = doTokenRefresh().finally(() => { refreshPromise = null })
  }
  return refreshPromise
}

// ── Proactive refresh ───────────────────────────────────────────────
// Refresh the token 5 minutes before expiry to avoid 401s entirely.
let proactiveTimer: ReturnType<typeof setTimeout> | null = null

function scheduleProactiveRefresh(expiresAt?: string) {
  if (proactiveTimer) clearTimeout(proactiveTimer)
  if (!expiresAt) return
  const expiresMs = new Date(expiresAt).getTime()
  const refreshAt = expiresMs - 5 * 60 * 1000 // 5 min before expiry
  const delay = refreshAt - Date.now()
  if (delay > 0) {
    proactiveTimer = setTimeout(() => {
      refreshTokenOnce().catch(() => {})
    }, delay)
  }
}

// Parse JWT exp claim to schedule initial proactive refresh on page load
function initProactiveRefresh() {
  const token = getAuthToken()
  if (!token) return
  try {
    const payload = JSON.parse(atob(token.split('.')[1]))
    if (payload.exp) {
      scheduleProactiveRefresh(new Date(payload.exp * 1000).toISOString())
    }
  } catch { /* ignore */ }
}
initProactiveRefresh()

// ── Main fetch wrapper ──────────────────────────────────────────────

export async function fetchJSON<T>(path: string, init?: RequestInit): Promise<T> {
  const token = getAuthToken()
  const headers: Record<string, string> = {
    'Content-Type': 'application/json',
    ...(init?.headers as Record<string, string> || {}),
  }
  if (token) {
    headers['Authorization'] = `Bearer ${token}`
  }
  const res = await fetch(`${BASE}${path}`, {
    ...init,
    headers,
  })
  if (res.status === 401 && !path.includes('/auth/')) {
    // Try refresh (mutex ensures only one concurrent refresh)
    const newToken = await refreshTokenOnce()
    if (newToken) {
      headers['Authorization'] = `Bearer ${newToken}`
      const retryRes = await fetch(`${BASE}${path}`, { ...init, headers })
      if (!retryRes.ok) throw new Error(`API ${retryRes.status}: ${await retryRes.text()}`)
      return retryRes.json()
    }
    // Refresh failed — force logout
    clearAuthTokens()
    window.dispatchEvent(new CustomEvent('auth:logout'))
    throw new Error('Session expired')
  }
  if (!res.ok) {
    throw new Error(`API ${res.status}: ${await res.text()}`)
  }
  return res.json()
}
