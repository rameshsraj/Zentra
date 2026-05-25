import { useState, FormEvent } from 'react'
import { Link } from 'react-router-dom'
import { Mail, ArrowLeft, Loader2, CheckCircle, Server } from 'lucide-react'

export default function ForgotPassword() {
  const [email, setEmail] = useState('')
  const [loading, setLoading] = useState(false)
  const [sent, setSent] = useState(false)
  const [error, setError] = useState('')

  const handleSubmit = async (e: FormEvent) => {
    e.preventDefault()
    setLoading(true); setError('')
    try {
      await fetch('/api/auth/forgot-password', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ email }),
      })
      setSent(true)
    } catch (err: any) {
      setError(err.message || 'Failed to send reset email')
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
          {sent ? (
            <div className="text-center space-y-4">
              <CheckCircle className="w-12 h-12 text-green-400 mx-auto" />
              <h2 className="text-xl font-semibold text-white">Check your email</h2>
              <p className="text-sm text-gray-400">If an account with that email exists, we've sent password reset instructions.</p>
              <Link to="/login" className="inline-flex items-center gap-2 text-sm text-blue-400 hover:text-blue-300">
                <ArrowLeft className="w-4 h-4" /> Back to login
              </Link>
            </div>
          ) : (
            <>
              <div className="flex items-center gap-2 mb-6">
                <Mail className="w-5 h-5 text-blue-400" />
                <h2 className="text-xl font-semibold text-white">Forgot Password</h2>
              </div>
              <p className="text-sm text-gray-400 mb-4">Enter your email address and we'll send you a link to reset your password.</p>

              {error && <div className="p-3 mb-4 rounded-lg bg-red-900/30 border border-red-700/50 text-red-300 text-sm">{error}</div>}

              <form onSubmit={handleSubmit} className="space-y-4">
                <input type="email" value={email} onChange={e => setEmail(e.target.value)} required autoFocus
                  placeholder="Enter your email"
                  className="w-full px-4 py-2.5 rounded-lg bg-gray-900/50 border border-gray-600 text-white placeholder-gray-500 focus:border-blue-500 focus:ring-1 focus:ring-blue-500/50 outline-none transition" />
                <button type="submit" disabled={loading}
                  className="w-full py-2.5 rounded-lg bg-blue-600 hover:bg-blue-700 disabled:bg-blue-800 text-white font-medium flex items-center justify-center gap-2 transition">
                  {loading && <Loader2 className="w-4 h-4 animate-spin" />} Send Reset Link
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
