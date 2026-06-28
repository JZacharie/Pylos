import { useState, useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import { setupApi } from '../lib/api'
import { KeyRound, ShieldAlert, ShieldCheck } from 'lucide-react'

export default function Setup() {
  const navigate = useNavigate()
  const [password, setPassword] = useState('')
  const [confirmPassword, setConfirmPassword] = useState('')
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const [checking, setChecking] = useState(true)

  useEffect(() => {
    // If setup is already done, redirect to login
    setupApi.getStatus()
      .then(res => {
        if (!res.setup_required) {
          navigate('/login')
        }
        setChecking(false)
      })
      .catch(err => {
        console.error("Failed to check setup status:", err)
        setChecking(false)
      })
  }, [navigate])

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault()
    if (password.length < 10) {
      setError("Password must be at least 10 characters long.")
      return
    }
    if (password !== confirmPassword) {
      setError("Passwords do not match.")
      return
    }

    setError(null)
    setLoading(true)

    try {
      await setupApi.setup(password, confirmPassword)
      // Save it locally to log the user in immediately
      localStorage.setItem('pylos_admin_key', password)
      navigate('/dashboard')
    } catch (err: any) {
      setError(err.response?.data?.message || "Failed to configure the admin password.")
    } finally {
      setLoading(false)
    }
  }

  if (checking) {
    return (
      <div className="min-h-screen flex items-center justify-center bg-zinc-950 text-zinc-500">
        Checking setup status...
      </div>
    )
  }

  const isLengthValid = password.length >= 10
  const doPasswordsMatch = password && password === confirmPassword

  return (
    <div className="min-h-screen flex items-center justify-center bg-zinc-950 px-4">
      <div className="absolute inset-0 bg-[radial-gradient(ellipse_at_top,_var(--tw-gradient-stops))] from-emerald-900/10 via-zinc-950 to-zinc-950 pointer-events-none" />

      <div className="w-full max-w-md bg-zinc-900/40 backdrop-blur-xl border border-zinc-800/80 rounded-2xl p-8 shadow-2xl relative z-10">
        <div className="flex flex-col items-center mb-8">
          <div className="w-14 h-14 overflow-hidden rounded-2xl bg-zinc-800 border border-zinc-700/50 flex items-center justify-center p-2 mb-4 shadow-lg">
            <img src="/logo.png" alt="Pylos" className="w-full h-full object-contain" />
          </div>
          <h1 className="text-2xl font-bold text-white tracking-wide">First-time Setup</h1>
          <p className="text-zinc-400 text-sm mt-1.5 text-center">
            Set your secure Pylos administrator password to initialize the gateway
          </p>
        </div>

        {error && (
          <div className="mb-6 p-3 rounded-lg border border-red-500/20 bg-red-500/10 text-red-400 text-xs font-medium flex items-start gap-2">
            <ShieldAlert size={14} className="mt-0.5 shrink-0" />
            <span>{error}</span>
          </div>
        )}

        <form onSubmit={handleSubmit} className="space-y-4">
          <div>
            <label className="block text-xs font-semibold text-zinc-400 uppercase tracking-wider mb-2">
              Administrator Password
            </label>
            <div className="relative">
              <input
                type="password"
                placeholder="Enter secure password"
                value={password}
                onChange={(e) => setPassword(e.target.value)}
                className="w-full pl-10 pr-4 py-3 rounded-xl border border-zinc-800 bg-zinc-950/50 text-white placeholder-zinc-600 focus:outline-none focus:border-emerald-500 transition-colors text-sm"
              />
              <KeyRound size={16} className="absolute left-3.5 top-3.5 text-zinc-500" />
            </div>
          </div>

          <div>
            <label className="block text-xs font-semibold text-zinc-400 uppercase tracking-wider mb-2">
              Confirm Password
            </label>
            <div className="relative">
              <input
                type="password"
                placeholder="Re-enter password"
                value={confirmPassword}
                onChange={(e) => setConfirmPassword(e.target.value)}
                className="w-full pl-10 pr-4 py-3 rounded-xl border border-zinc-800 bg-zinc-950/50 text-white placeholder-zinc-600 focus:outline-none focus:border-emerald-500 transition-colors text-sm"
              />
              <KeyRound size={16} className="absolute left-3.5 top-3.5 text-zinc-500" />
            </div>
          </div>

          {/* Validation indicators */}
          <div className="py-2 space-y-2">
            <div className="flex items-center gap-2 text-xs font-medium">
              {isLengthValid ? (
                <ShieldCheck size={14} className="text-emerald-500" />
              ) : (
                <ShieldAlert size={14} className="text-zinc-500" />
              )}
              <span className={isLengthValid ? "text-emerald-400" : "text-zinc-500"}>
                At least 10 characters long
              </span>
            </div>
            <div className="flex items-center gap-2 text-xs font-medium">
              {doPasswordsMatch ? (
                <ShieldCheck size={14} className="text-emerald-500" />
              ) : (
                <ShieldAlert size={14} className="text-zinc-500" />
              )}
              <span className={doPasswordsMatch ? "text-emerald-400" : "text-zinc-500"}>
                Passwords match
              </span>
            </div>
          </div>

          <button
            type="submit"
            disabled={loading || !isLengthValid || !doPasswordsMatch}
            className="w-full py-3 px-4 rounded-xl bg-emerald-600 hover:bg-emerald-500 disabled:bg-zinc-800 disabled:text-zinc-600 text-white font-semibold text-sm transition-colors shadow-lg shadow-emerald-600/10 hover:shadow-emerald-500/20 active:scale-98"
          >
            {loading ? "Configuring..." : "Complete Setup"}
          </button>
        </form>
      </div>
    </div>
  )
}
