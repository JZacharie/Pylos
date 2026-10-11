import { useEffect, useState } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import { authApi } from '../lib/api'
import { ShieldAlert, Loader2 } from 'lucide-react'

export default function Callback() {
  const navigate = useNavigate()
  const [searchParams] = useSearchParams()
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    const code = searchParams.get('code')
    const state = searchParams.get('state')
    if (!code) {
      setError("No authorization code found in URL.")
      return
    }
    if (!state) {
      setError("Missing OAuth state (CSRF protection).")
      return
    }

    // Call backend callback endpoint to exchange code for session token
    const redirectUri = `${window.location.origin}/callback`
    authApi.googleCallback(code, redirectUri, state)
      .then(res => {
        localStorage.setItem('pylos_admin_key', res.token)
        localStorage.setItem('pylos_user', JSON.stringify(res.user))
        navigate('/dashboard')
      })
      .catch(err => {
        const errMsg = err.response?.data?.error || err.message || "Failed to authenticate with Google."
        setError(errMsg)
      })
  }, [searchParams, navigate])

  return (
    <div className="min-h-screen flex items-center justify-center bg-zinc-950 px-4">
      <div className="absolute inset-0 bg-[radial-gradient(ellipse_at_top,_var(--tw-gradient-stops))] from-indigo-900/10 via-zinc-950 to-zinc-950 pointer-events-none" />
      
      <div className="w-full max-w-md bg-zinc-900/40 backdrop-blur-xl border border-zinc-800/80 rounded-2xl p-8 shadow-2xl relative z-10 text-center">
        {error ? (
          <div className="space-y-6">
            <div className="w-12 h-12 rounded-full bg-red-500/10 border border-red-500/20 flex items-center justify-center mx-auto text-red-500">
              <ShieldAlert size={24} />
            </div>
            <div className="space-y-2">
              <h2 className="text-lg font-semibold text-white">Authentication Failed</h2>
              <p className="text-zinc-400 text-sm max-w-xs mx-auto break-words">
                {error}
              </p>
            </div>
            <button
              onClick={() => navigate('/login')}
              className="px-6 py-2.5 rounded-xl bg-indigo-600 hover:bg-indigo-500 text-white font-semibold text-sm transition-colors shadow-lg shadow-indigo-600/10 hover:shadow-indigo-500/20"
            >
              Back to Login
            </button>
          </div>
        ) : (
          <div className="space-y-6">
            <div className="relative w-12 h-12 mx-auto">
              <Loader2 size={40} className="text-indigo-500 animate-spin" />
            </div>
            <div className="space-y-1">
              <h2 className="text-lg font-semibold text-white">Completing Sign-In</h2>
              <p className="text-zinc-400 text-sm">
                Exchanging code with the Pylos gateway...
              </p>
            </div>
          </div>
        )}
      </div>
    </div>
  )
}
