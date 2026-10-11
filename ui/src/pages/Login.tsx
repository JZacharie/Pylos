import { useState, useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import { authApi } from '../lib/api'
import { KeyRound, ShieldAlert } from 'lucide-react'

export default function Login() {
  const navigate = useNavigate()
  const [adminKey, setAdminKey] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [googleEnabled, setGoogleEnabled] = useState(false)
  const [googleClientId, setGoogleClientId] = useState<string | null>(null)
  const [googleRedirectUri, setGoogleRedirectUri] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)

  useEffect(() => {
    // Check if user is already logged in
    const token = localStorage.getItem('pylos_admin_key')
    if (token) {
      navigate('/dashboard')
    }

    // Fetch auth config from backend
    authApi.getConfig()
      .then(cfg => {
        setGoogleEnabled(cfg.google_auth_enabled)
        setGoogleClientId(cfg.google_client_id)
        setGoogleRedirectUri(cfg.google_redirect_uri)
      })
      .catch(err => {
        console.error("Failed to load auth config:", err)
      })
  }, [navigate])

  const handleAdminKeyLogin = (e: React.FormEvent) => {
    e.preventDefault()
    if (!adminKey.trim()) {
      setError("Please enter a valid PYLOS_ADMIN_KEY.")
      return
    }
    setError(null)
    localStorage.setItem('pylos_admin_key', adminKey)
    navigate('/dashboard')
  }

  const handleGoogleLogin = async () => {
    if (!googleClientId) return
    setError(null)
    setLoading(true)

    try {
      // L'état anti-CSRF est émis par le serveur et posé en cookie HttpOnly.
      const { state } = await authApi.googleStart()
      const redirectUri = googleRedirectUri || `${window.location.origin}/callback`
      const targetUrl = `https://accounts.google.com/o/oauth2/v2/auth?` +
        `client_id=${encodeURIComponent(googleClientId)}&` +
        `redirect_uri=${encodeURIComponent(redirectUri)}&` +
        `response_type=code&` +
        `scope=${encodeURIComponent('openid email profile')}&` +
        `state=${encodeURIComponent(state)}`

      window.location.href = targetUrl
    } catch (err: any) {
      setLoading(false)
      setError(err.response?.data?.error || 'Failed to start Google sign-in.')
    }
  }

  return (
    <div className="min-h-screen flex items-center justify-center bg-zinc-950 px-4">
      <div className="absolute inset-0 bg-[radial-gradient(ellipse_at_top,_var(--tw-gradient-stops))] from-indigo-900/10 via-zinc-950 to-zinc-950 pointer-events-none" />
      
      <div className="w-full max-w-md bg-zinc-900/40 backdrop-blur-xl border border-zinc-800/80 rounded-2xl p-8 shadow-2xl relative z-10">
        <div className="flex flex-col items-center mb-8">
          <div className="w-14 h-14 overflow-hidden rounded-2xl bg-zinc-800 border border-zinc-700/50 flex items-center justify-center p-2 mb-4 shadow-lg">
            <img src="/logo.png" alt="Pylos" className="w-full h-full object-contain" />
          </div>
          <h1 className="text-2xl font-bold text-white tracking-wide">Welcome to Pylos</h1>
          <p className="text-zinc-400 text-sm mt-1.5 text-center">
            Sign in to access your Pylos AI Gateway Administration Console
          </p>
        </div>

        {error && (
          <div className="mb-6 p-3 rounded-lg border border-red-500/20 bg-red-500/10 text-red-400 text-xs font-medium flex items-start gap-2">
            <ShieldAlert size={14} className="mt-0.5 shrink-0" />
            <span>{error}</span>
          </div>
        )}

        {googleEnabled && googleClientId && (
          <div className="space-y-4 mb-6">
            <button
              onClick={handleGoogleLogin}
              disabled={loading}
              className="w-full flex items-center justify-center gap-3 py-3 px-4 rounded-xl border border-zinc-800 bg-zinc-900/60 hover:bg-zinc-800/60 text-zinc-100 hover:text-white font-medium text-sm transition-all shadow-md active:scale-98"
            >
              <svg className="w-4 h-4" viewBox="0 0 24 24">
                <path
                  fill="currentColor"
                  d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"
                />
                <path
                  fill="currentColor"
                  d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"
                />
                <path
                  fill="currentColor"
                  d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.06H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.94l2.85-2.22.81-.63z"
                />
                <path
                  fill="currentColor"
                  d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.06l3.66 2.84c.87-2.6 3.3-4.52 6.16-4.52z"
                />
              </svg>
              <span>{loading ? 'Redirecting...' : 'Sign in with Google'}</span>
            </button>

            <div className="relative flex py-2 items-center">
              <div className="flex-grow border-t border-zinc-800/60"></div>
              <span className="flex-shrink mx-4 text-zinc-500 text-xs uppercase tracking-wider">Or</span>
              <div className="flex-grow border-t border-zinc-800/60"></div>
            </div>
          </div>
        )}

        <form onSubmit={handleAdminKeyLogin} className="space-y-4">
          <div>
            <label htmlFor="adminKey" className="block text-xs font-semibold text-zinc-400 uppercase tracking-wider mb-2">
              Admin Key Fallback
            </label>
            <div className="relative">
              <input
                id="adminKey"
                type="password"
                placeholder="Enter PYLOS_ADMIN_KEY"
                value={adminKey}
                onChange={(e) => setAdminKey(e.target.value)}
                className="w-full pl-10 pr-4 py-3 rounded-xl border border-zinc-800 bg-zinc-950/50 text-white placeholder-zinc-600 focus:outline-none focus:border-indigo-500 transition-colors text-sm"
              />
              <KeyRound size={16} className="absolute left-3.5 top-3.5 text-zinc-500" />
            </div>
          </div>

          <button
            type="submit"
            className="w-full py-3 px-4 rounded-xl bg-indigo-600 hover:bg-indigo-500 text-white font-semibold text-sm transition-colors shadow-lg shadow-indigo-600/10 hover:shadow-indigo-500/20 active:scale-98"
          >
            Access with Admin Key
          </button>
        </form>
      </div>
    </div>
  )
}
