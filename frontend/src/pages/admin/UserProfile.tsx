import { useState, useEffect } from 'react'
import { useAuth } from '../../contexts/AuthContext'
import { authUpdateProfile, authChangePassword, authMfaSetup, authMfaVerify, authSessions, authRevokeSession, UserSession, MfaSetupResponse } from '../../lib/api'
import { User, Shield, Key, Monitor, Save, Loader2, QrCode, Trash2 } from 'lucide-react'

export default function UserProfile() {
  const { user, refreshProfile } = useAuth()
  const [tab, setTab] = useState<'profile' | 'security' | 'sessions'>('profile')

  if (!user) return null

  return (
    <div className="space-y-6">
      <div className="flex items-center gap-3">
        <div className="p-2 rounded-lg bg-blue-600/20"><User className="w-5 h-5 text-blue-400" /></div>
        <div>
          <h1 className="text-2xl font-bold text-white">My Profile</h1>
          <p className="text-sm text-gray-400">{user.email}</p>
        </div>
      </div>

      <div className="flex gap-2 border-b border-gray-700/50 pb-0">
        {(['profile', 'security', 'sessions'] as const).map(t => (
          <button key={t} onClick={() => setTab(t)}
            className={`px-4 py-2 text-sm font-medium border-b-2 transition ${tab === t ? 'border-blue-500 text-blue-400' : 'border-transparent text-gray-400 hover:text-gray-300'}`}>
            {t === 'profile' ? 'Profile' : t === 'security' ? 'Security' : 'Sessions'}
          </button>
        ))}
      </div>

      {tab === 'profile' && <ProfileTab user={user} onSaved={refreshProfile} />}
      {tab === 'security' && <SecurityTab />}
      {tab === 'sessions' && <SessionsTab />}
    </div>
  )
}

function ProfileTab({ user, onSaved }: { user: any; onSaved: () => void }) {
  const [form, setForm] = useState({
    display_name: user.display_name || '',
    email: user.email || '',
    timezone: user.timezone || 'UTC',
    theme: user.theme || 'dark',
  })
  const [saving, setSaving] = useState(false)
  const [msg, setMsg] = useState('')

  const save = async () => {
    setSaving(true); setMsg('')
    try {
      await authUpdateProfile(form)
      onSaved()
      setMsg('Profile updated!')
    } catch (e: any) { setMsg(e.message) }
    setSaving(false)
  }

  return (
    <div className="bg-gray-800/50 border border-gray-700/50 rounded-xl p-6 max-w-xl space-y-4">
      <div>
        <label className="block text-sm text-gray-400 mb-1">Display Name</label>
        <input value={form.display_name} onChange={e => setForm({ ...form, display_name: e.target.value })}
          className="w-full px-3 py-2 rounded-lg bg-gray-900 border border-gray-600 text-white text-sm outline-none focus:border-blue-500" />
      </div>
      <div>
        <label className="block text-sm text-gray-400 mb-1">Email</label>
        <input value={form.email} onChange={e => setForm({ ...form, email: e.target.value })}
          className="w-full px-3 py-2 rounded-lg bg-gray-900 border border-gray-600 text-white text-sm outline-none focus:border-blue-500" />
      </div>
      <div>
        <label className="block text-sm text-gray-400 mb-1">Timezone</label>
        <select value={form.timezone} onChange={e => setForm({ ...form, timezone: e.target.value })}
          className="w-full px-3 py-2 rounded-lg bg-gray-900 border border-gray-600 text-white text-sm">
          {['UTC', 'US/Eastern', 'US/Central', 'US/Pacific', 'Europe/London', 'Europe/Berlin', 'Asia/Kolkata', 'Asia/Tokyo', 'Australia/Sydney'].map(tz => (
            <option key={tz} value={tz}>{tz}</option>
          ))}
        </select>
      </div>
      {msg && <p className={`text-sm ${msg.includes('updated') ? 'text-green-400' : 'text-red-400'}`}>{msg}</p>}
      <button onClick={save} disabled={saving}
        className="flex items-center gap-2 px-4 py-2 rounded-lg bg-blue-600 hover:bg-blue-700 disabled:bg-blue-800 text-white text-sm font-medium">
        {saving ? <Loader2 className="w-4 h-4 animate-spin" /> : <Save className="w-4 h-4" />} Save Changes
      </button>
    </div>
  )
}

function SecurityTab() {
  const [mfaSetup, setMfaSetup] = useState<MfaSetupResponse | null>(null)
  const [mfaCode, setMfaCode] = useState('')
  const [mfaMsg, setMfaMsg] = useState('')
  const [pwForm, setPwForm] = useState({ current_password: '', new_password: '', confirm: '' })
  const [pwMsg, setPwMsg] = useState('')
  const { user } = useAuth()

  const setupMfa = async () => {
    try {
      const res = await authMfaSetup()
      setMfaSetup(res)
    } catch (e: any) { setMfaMsg(e.message) }
  }

  const verifyMfa = async () => {
    try {
      await authMfaVerify(mfaCode)
      setMfaMsg('MFA enabled successfully!')
      setMfaSetup(null)
    } catch (e: any) { setMfaMsg(e.message) }
  }

  const changePw = async () => {
    if (pwForm.new_password !== pwForm.confirm) { setPwMsg('Passwords do not match'); return }
    try {
      await authChangePassword(pwForm.current_password, pwForm.new_password)
      setPwMsg('Password changed!')
      setPwForm({ current_password: '', new_password: '', confirm: '' })
    } catch (e: any) { setPwMsg(e.message) }
  }

  return (
    <div className="space-y-6 max-w-xl">
      {/* MFA Section */}
      <div className="bg-gray-800/50 border border-gray-700/50 rounded-xl p-6">
        <div className="flex items-center gap-2 mb-4">
          <Shield className="w-5 h-5 text-blue-400" />
          <h3 className="font-semibold text-white">Two-Factor Authentication</h3>
          {user?.mfa_enabled && <span className="px-2 py-0.5 text-xs rounded bg-green-600/20 text-green-400">Enabled</span>}
        </div>

        {!mfaSetup ? (
          <button onClick={setupMfa} className="px-4 py-2 rounded-lg bg-blue-600/20 text-blue-400 hover:bg-blue-600/30 text-sm">
            {user?.mfa_enabled ? 'Re-configure MFA' : 'Enable MFA'}
          </button>
        ) : (
          <div className="space-y-3">
            <p className="text-sm text-gray-400">Scan this QR code with your authenticator app:</p>
            <div className="p-3 rounded-lg bg-white inline-block">
              <QrCode className="w-32 h-32 text-black" />
            </div>
            <p className="text-xs text-gray-500 break-all">Manual entry: <code className="text-blue-400">{mfaSetup.secret}</code></p>
            <div className="flex gap-2">
              <input value={mfaCode} onChange={e => setMfaCode(e.target.value.replace(/\D/g, '').slice(0, 6))}
                placeholder="000000" maxLength={6}
                className="px-3 py-2 rounded-lg bg-gray-900 border border-gray-600 text-white text-sm text-center tracking-widest font-mono w-32 outline-none focus:border-blue-500" />
              <button onClick={verifyMfa} disabled={mfaCode.length !== 6}
                className="px-4 py-2 rounded-lg bg-blue-600 hover:bg-blue-700 disabled:bg-blue-800 text-white text-sm font-medium">Verify</button>
            </div>
          </div>
        )}
        {mfaMsg && <p className={`text-sm mt-2 ${mfaMsg.includes('success') ? 'text-green-400' : 'text-red-400'}`}>{mfaMsg}</p>}
      </div>

      {/* Change Password */}
      <div className="bg-gray-800/50 border border-gray-700/50 rounded-xl p-6">
        <div className="flex items-center gap-2 mb-4">
          <Key className="w-5 h-5 text-yellow-400" />
          <h3 className="font-semibold text-white">Change Password</h3>
        </div>
        <div className="space-y-3">
          <input type="password" placeholder="Current Password" value={pwForm.current_password}
            onChange={e => setPwForm({ ...pwForm, current_password: e.target.value })}
            className="w-full px-3 py-2 rounded-lg bg-gray-900 border border-gray-600 text-white text-sm outline-none focus:border-blue-500" />
          <input type="password" placeholder="New Password" value={pwForm.new_password}
            onChange={e => setPwForm({ ...pwForm, new_password: e.target.value })}
            className="w-full px-3 py-2 rounded-lg bg-gray-900 border border-gray-600 text-white text-sm outline-none focus:border-blue-500" />
          <input type="password" placeholder="Confirm Password" value={pwForm.confirm}
            onChange={e => setPwForm({ ...pwForm, confirm: e.target.value })}
            className="w-full px-3 py-2 rounded-lg bg-gray-900 border border-gray-600 text-white text-sm outline-none focus:border-blue-500" />
          {pwMsg && <p className={`text-sm ${pwMsg.includes('changed') ? 'text-green-400' : 'text-red-400'}`}>{pwMsg}</p>}
          <button onClick={changePw} disabled={!pwForm.current_password || !pwForm.new_password || !pwForm.confirm}
            className="px-4 py-2 rounded-lg bg-yellow-600 hover:bg-yellow-700 disabled:bg-yellow-800 text-white text-sm font-medium">Change Password</button>
        </div>
      </div>
    </div>
  )
}

function SessionsTab() {
  const [sessions, setSessions] = useState<UserSession[]>([])
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    authSessions().then(setSessions).finally(() => setLoading(false))
  }, [])

  const revoke = async (id: string) => {
    await authRevokeSession(id)
    setSessions(s => s.filter(x => x.id !== id))
  }

  return (
    <div className="bg-gray-800/50 border border-gray-700/50 rounded-xl overflow-hidden max-w-2xl">
      <div className="flex items-center gap-2 p-4 border-b border-gray-700/50">
        <Monitor className="w-5 h-5 text-green-400" />
        <h3 className="font-semibold text-white">Active Sessions</h3>
      </div>
      {loading ? (
        <div className="p-8 text-center text-gray-500"><Loader2 className="w-5 h-5 animate-spin inline" /></div>
      ) : sessions.length === 0 ? (
        <div className="p-8 text-center text-gray-500">No active sessions</div>
      ) : (
        <div className="divide-y divide-gray-700/30">
          {sessions.map(s => (
            <div key={s.id} className="flex items-center justify-between p-4">
              <div>
                <p className="text-sm text-white">{s.user_agent?.substring(0, 60) || 'Unknown device'}</p>
                <p className="text-xs text-gray-500">{s.ip_address || 'Unknown IP'} &middot; Expires {new Date(s.expires_at).toLocaleString()}</p>
              </div>
              <button onClick={() => revoke(s.id)} className="p-1.5 rounded hover:bg-gray-700 text-red-400" title="Revoke">
                <Trash2 className="w-4 h-4" />
              </button>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}
