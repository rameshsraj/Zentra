const BASE = '/api'

function getAuthToken(): string | null {
  return localStorage.getItem('Zentra_token')
}

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
  if (res.status === 401) {
    // Try refresh token
    const refreshToken = localStorage.getItem('Zentra_refresh_token')
    if (refreshToken && !path.includes('/auth/')) {
      try {
        const refreshRes = await fetch(`${BASE}/auth/refresh`, {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ refresh_token: refreshToken }),
        })
        if (refreshRes.ok) {
          const data = await refreshRes.json()
          localStorage.setItem('Zentra_token', data.token)
          // Retry original request with new token
          headers['Authorization'] = `Bearer ${data.token}`
          const retryRes = await fetch(`${BASE}${path}`, { ...init, headers })
          if (!retryRes.ok) throw new Error(`API ${retryRes.status}: ${await retryRes.text()}`)
          return retryRes.json()
        }
      } catch { /* fall through to error */ }
    }
    localStorage.removeItem('Zentra_token')
    localStorage.removeItem('Zentra_refresh_token')
    window.dispatchEvent(new CustomEvent('auth:logout'))
    throw new Error('Session expired')
  }
  if (!res.ok) {
    throw new Error(`API ${res.status}: ${await res.text()}`)
  }
  return res.json()
}
