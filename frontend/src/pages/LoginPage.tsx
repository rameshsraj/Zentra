import { useState, FormEvent } from 'react'
import { useNavigate, useLocation, Link } from 'react-router-dom'
import { useAuth } from '../contexts/AuthContext'
import { Shield, Eye, EyeOff, AlertCircle, Loader2, Server } from 'lucide-react'

export default function LoginPage() {
  const { login, isLoading, error: authError } = useAuth()
  const navigate = useNavigate()
  const location = useLocation()
  const from = (location.state as any)?.from?.pathname || '/'

  const [email, setEmail] = useState('')
  const [password, setPassword] = useState('')
  const [mfaCode, setMfaCode] = useState('')
  const [showPassword, setShowPassword] = useState(false)
  const [mfaRequired, setMfaRequired] = useState(false)
  const [error, setError] = useState('')

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault()
    setError('')
    try {
      const res = await login({ email, password, mfa_code: mfaRequired ? mfaCode : undefined })
      if (res.mfa_required && !res.token) {
        setMfaRequired(true)
        return
      }
      navigate(from, { replace: true })
    } catch (err: any) {
      const msg = err?.message || 'Login failed'
      if (msg.includes('401') || msg.includes('invalid')) {
        setError('Invalid email or password')
      } else if (msg.includes('429') || msg.includes('rate')) {
        setError('Too many failed attempts. Please try again later.')
      } else if (msg.includes('403') || msg.includes('locked')) {
        setError('Account is locked. Contact your administrator.')
      } else {
        setError(msg)
      }
    }
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-gradient-to-br from-gray-950 via-gray-900 to-gray-950 p-4">
      <div className="w-full max-w-md">
        {/* Logo */}
        <div className="flex flex-col items-center mb-8">
          <div className="flex items-center gap-3 mb-3">
            <div className="p-3 rounded-xl bg-blue-600/20 border border-blue-500/30">
              <Server className="w-8 h-8 text-blue-400" />
            </div>
            <h1 className="text-3xl font-bold text-white">Zentra</h1>
          </div>
          <p className="text-gray-400 text-sm">Infrastructure Monitoring Platform</p>
        </div>

        {/* Login Card */}
        <div className="bg-gray-800/50 backdrop-blur-sm border border-gray-700/50 rounded-2xl p-8 shadow-2xl">
          <div className="flex items-center gap-2 mb-6">
            <Shield className="w-5 h-5 text-blue-400" />
            <h2 className="text-xl font-semibold text-white">
              {mfaRequired ? 'Two-Factor Authentication' : 'Sign In'}
            </h2>
          </div>

          {(error || authError) && (
            <div className="flex items-center gap-2 p-3 mb-4 rounded-lg bg-red-900/30 border border-red-700/50 text-red-300 text-sm">
              <AlertCircle className="w-4 h-4 flex-shrink-0" />
              <span>{error || authError}</span>
            </div>
          )}

          <form onSubmit={handleSubmit} className="space-y-4">
            {!mfaRequired ? (
              <>
                <div>
                  <label className="block text-sm font-medium text-gray-300 mb-1.5">Email</label>
                  <input
                    type="email"
                    value={email}
                    onChange={e => setEmail(e.target.value)}
                    required
                    autoFocus
                    placeholder="admin@Zentra.local"
                    className="w-full px-4 py-2.5 rounded-lg bg-gray-900/50 border border-gray-600 text-white placeholder-gray-500 focus:border-blue-500 focus:ring-1 focus:ring-blue-500/50 outline-none transition"
                  />
                </div>
                <div>
                  <label className="block text-sm font-medium text-gray-300 mb-1.5">Password</label>
                  <div className="relative">
                    <input
                      type={showPassword ? 'text' : 'password'}
                      value={password}
                      onChange={e => setPassword(e.target.value)}
                      required
                      placeholder="Enter password"
                      className="w-full px-4 py-2.5 rounded-lg bg-gray-900/50 border border-gray-600 text-white placeholder-gray-500 focus:border-blue-500 focus:ring-1 focus:ring-blue-500/50 outline-none transition pr-10"
                    />
                    <button
                      type="button"
                      onClick={() => setShowPassword(!showPassword)}
                      className="absolute right-3 top-1/2 -translate-y-1/2 text-gray-400 hover:text-gray-300"
                    >
                      {showPassword ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
                    </button>
                  </div>
                </div>
              </>
            ) : (
              <div>
                <label className="block text-sm font-medium text-gray-300 mb-1.5">MFA Code</label>
                <input
                  type="text"
                  value={mfaCode}
                  onChange={e => setMfaCode(e.target.value.replace(/\D/g, '').slice(0, 6))}
                  required
                  autoFocus
                  placeholder="000000"
                  maxLength={6}
                  className="w-full px-4 py-2.5 rounded-lg bg-gray-900/50 border border-gray-600 text-white placeholder-gray-500 focus:border-blue-500 focus:ring-1 focus:ring-blue-500/50 outline-none transition text-center text-2xl tracking-[0.5em] font-mono"
                />
                <p className="text-xs text-gray-500 mt-2">Enter the 6-digit code from your authenticator app</p>
              </div>
            )}

            <button
              type="submit"
              disabled={isLoading}
              className="w-full flex items-center justify-center gap-2 py-2.5 px-4 rounded-lg bg-blue-600 hover:bg-blue-700 disabled:bg-blue-800 disabled:cursor-not-allowed text-white font-medium transition"
            >
              {isLoading ? (
                <>
                  <Loader2 className="w-4 h-4 animate-spin" />
                  Signing in...
                </>
              ) : mfaRequired ? (
                'Verify'
              ) : (
                'Sign In'
              )}
            </button>

            {!mfaRequired && (
              <div className="text-right">
                <Link to="/forgot-password" className="text-xs text-gray-400 hover:text-blue-400 transition">Forgot password?</Link>
              </div>
            )}

            {mfaRequired && (
              <button
                type="button"
                onClick={() => { setMfaRequired(false); setMfaCode(''); setError('') }}
                className="w-full text-sm text-gray-400 hover:text-gray-300 transition"
              >
                Back to login
              </button>
            )}
          </form>
        </div>

        <p className="text-center text-xs text-gray-600 mt-6">
          Zentra v0.1.0 &mdash; Infrastructure Monitoring Platform
        </p>
      </div>
    </div>
  )
}
