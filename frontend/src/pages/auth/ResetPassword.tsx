import { useState, FormEvent } from 'react'
import { useSearchParams, Link, useNavigate } from 'react-router-dom'
import { Lock, ArrowLeft, Loader2, CheckCircle, Server } from 'lucide-react'

export default function ResetPassword() {
  const [searchParams] = useSearchParams()
  const navigate = useNavigate()
  const token = searchParams.get('token') || ''
  const [password, setPassword] = useState('')
  const [confirm, setConfirm] = useState('')
  const [loading, setLoading] = useState(false)
  const [done, setDone] = useState(false)
  const [error, setError] = useState('')

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault()
    if (password !== confirm) { setError('Passwords do not match'); return }
    if (password.length < 8) { setError('Password must be at least 8 characters'); return }
    setLoading(true); setError('')
    try {
      const res = await fetch('/api/auth/reset-password', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ token, new_password: password }),
      })
      if (!res.ok) throw new Error('Reset failed')
      setDone(true)
      setTimeout(() => navigate('/login'), 3000)
    } catch (err: any) {
      setError(err.message || 'Failed to reset password')
    }
    setLoading(false)
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-gradient-to-br from-gray-950 via-gray-900 to-gray-950 p-4">
      <div className="w-full max-w-md">
        <div className="flex flex-col items-center mb-8">
          <div className="flex items-center gap-3 mb-3">
            <div className="p-3 rounded-xl bg-blue-600/20 border border-blue-500/30"><Server className="w-8 h-8 text-blue-400" /></div>
            <h1 className="text-3xl font-bold text-white">Zentra</h1>
          </div>
        </div>

        <div className="bg-gray-800/50 backdrop-blur-sm border border-gray-700/50 rounded-2xl p-8 shadow-2xl">
          {done ? (
            <div className="text-center space-y-4">
              <CheckCircle className="w-12 h-12 text-green-400 mx-auto" />
              <h2 className="text-xl font-semibold text-white">Password Reset!</h2>
              <p className="text-sm text-gray-400">Redirecting to login...</p>
            </div>
          ) : (
            <>
              <div className="flex items-center gap-2 mb-6">
                <Lock className="w-5 h-5 text-blue-400" />
                <h2 className="text-xl font-semibold text-white">Reset Password</h2>
              </div>

              {!token && <div className="p-3 mb-4 rounded-lg bg-yellow-900/30 border border-yellow-700/50 text-yellow-300 text-sm">Invalid or missing reset token.</div>}
              {error && <div className="p-3 mb-4 rounded-lg bg-red-900/30 border border-red-700/50 text-red-300 text-sm">{error}</div>}

              <form onSubmit={handleSubmit} className="space-y-4">
                <input type="password" value={password} onChange={e => setPassword(e.target.value)} required
                  placeholder="New password"
                  className="w-full px-4 py-2.5 rounded-lg bg-gray-900/50 border border-gray-600 text-white placeholder-gray-500 focus:border-blue-500 outline-none transition" />
                <input type="password" value={confirm} onChange={e => setConfirm(e.target.value)} required
                  placeholder="Confirm password"
                  className="w-full px-4 py-2.5 rounded-lg bg-gray-900/50 border border-gray-600 text-white placeholder-gray-500 focus:border-blue-500 outline-none transition" />
                <button type="submit" disabled={loading || !token}
                  className="w-full py-2.5 rounded-lg bg-blue-600 hover:bg-blue-700 disabled:bg-blue-800 text-white font-medium flex items-center justify-center gap-2 transition">
                  {loading && <Loader2 className="w-4 h-4 animate-spin" />} Reset Password
                </button>
              </form>

              <Link to="/login" className="flex items-center gap-2 text-sm text-gray-400 hover:text-gray-300 mt-4">
                <ArrowLeft className="w-4 h-4" /> Back to login
              </Link>
            </>
          )}
        </div>
      </div>
    </div>
  )
}
