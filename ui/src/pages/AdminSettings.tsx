import { useState } from 'react'
import { useQuery, useMutation } from '@tanstack/react-query'
import { configApi } from '../lib/api'
import {
  Shield, Server, RotateCcw, AlertTriangle,
  Check, X, Rocket, FileText, Clock,
  Eye, EyeOff,
} from 'lucide-react'

// ─── Types ────────────────────────────────────────────────────────────────────

interface ServerConfig {
  port: number
  log_level: string
  enable_logging: boolean
  disable_content_logging: boolean
  max_request_body_size_mb: number
  cors_allowed_origins: string[]
}

interface AuthConfig {
  google_oauth_enabled: boolean
  google_client_id: string | null
  admin_key_configured: boolean
  jwt_secret_configured: boolean
}

interface QueueConfig {
  max_concurrency: number
  max_queue_size: number
  queue_timeout_ms: number
}

interface SystemInfo {
  version: string
  config_file_path: string
  last_config_reload: number | null
}

interface AppConfig {
  server: ServerConfig
  auth: AuthConfig
  queue: QueueConfig
  system: SystemInfo
  environment?: string
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

function formatTimestamp(ms: number | null): string {
  if (!ms) return 'Never'
  const d = new Date(ms)
  return d.toLocaleString()
}

// ─── Section Card ─────────────────────────────────────────────────────────────

function SectionCard({
  title,
  icon,
  children,
}: {
  title: string
  icon: React.ReactNode
  children: React.ReactNode
}) {
  return (
    <div className="rounded-xl border border-zinc-800/50 bg-zinc-900/30 p-5">
      <div className="flex items-center gap-2 mb-4">
        {icon}
        <h2 className="text-sm font-semibold text-zinc-300">{title}</h2>
      </div>
      <div className="space-y-3">{children}</div>
    </div>
  )
}

// ─── Status Badge ─────────────────────────────────────────────────────────────

function StatusBadge({ enabled, label }: { enabled: boolean; label: string }) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-xs text-zinc-400">{label}</span>
      <span
        className={`inline-flex items-center gap-1.5 text-xs px-2 py-0.5 rounded-full font-medium ${
          enabled
            ? 'bg-emerald-500/15 text-emerald-400'
            : 'bg-zinc-800 text-zinc-500'
        }`}
      >
        {enabled ? <Check size={11} /> : <X size={11} />}
        {enabled ? 'Enabled' : 'Disabled'}
      </span>
    </div>
  )
}

// ─── ConfigRow ────────────────────────────────────────────────────────────────

function ConfigRow({ label, value }: { label: string; value: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-xs text-zinc-400">{label}</span>
      <span className="text-xs text-zinc-200 font-mono">{value}</span>
    </div>
  )
}

// ─── Main Page ────────────────────────────────────────────────────────────────

export default function AdminSettings() {
  const [actionMessage, setActionMessage] = useState<string | null>(null)
  const [isPromoting, setIsPromoting] = useState(false)
  const [showOrigins, setShowOrigins] = useState(false)

  const { data: config, isLoading, refetch } = useQuery<AppConfig>({
    queryKey: ['admin-config'],
    queryFn: configApi.get,
    refetchInterval: 30_000,
  })

  const reloadMutation = useMutation({
    mutationFn: configApi.reload,
    onSuccess: (data) => {
      setActionMessage(data?.message || 'Config reloaded successfully')
      refetch()
      setTimeout(() => setActionMessage(null), 4000)
    },
    onError: (err: any) => {
      setActionMessage(`Error: ${err.response?.data?.error || err.message}`)
      setTimeout(() => setActionMessage(null), 5000)
    },
  })

  const handlePromote = async () => {
    if (!window.confirm('Are you sure you want to promote the current DEV version to PRODUCTION?')) {
      return
    }
    setIsPromoting(true)
    setActionMessage('Triggering promotion...')
    try {
      const res = await configApi.promote()
      setActionMessage(res.message || 'Promotion started successfully!')
      setTimeout(() => setActionMessage(null), 5000)
    } catch (err: any) {
      setActionMessage(`Error: ${err.response?.data?.error || err.message}`)
      setTimeout(() => setActionMessage(null), 7000)
    } finally {
      setIsPromoting(false)
    }
  }

  if (isLoading) {
    return (
      <div className="p-6 space-y-6">
        <div>
          <h1 className="text-2xl font-bold text-white">Admin Settings</h1>
          <p className="text-sm text-zinc-400 mt-1">System configuration</p>
        </div>
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          {[1, 2, 3, 4].map(i => (
            <div key={i} className="rounded-xl border border-zinc-800/50 bg-zinc-900/30 p-5 animate-pulse h-48" />
          ))}
        </div>
      </div>
    )
  }

  const server = config?.server
  const auth = config?.auth
  const queue = config?.queue
  const system = config?.system

  return (
    <div className="p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white">Admin Settings</h1>
          <p className="text-sm text-zinc-400 mt-1">System configuration & administration</p>
        </div>
      </div>

      {/* Action message */}
      {actionMessage && (
        <div
          className={`p-3 rounded-lg border text-xs font-medium flex items-center gap-2 ${
            actionMessage.startsWith('Error')
              ? 'bg-red-500/10 border-red-500/20 text-red-400'
              : 'bg-indigo-500/10 border-indigo-500/20 text-indigo-400'
          }`}
        >
          {actionMessage.startsWith('Error') ? (
            <AlertTriangle size={13} />
          ) : (
            <Check size={13} />
          )}
          {actionMessage}
        </div>
      )}

      {/* Config grid */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        {/* Server Configuration */}
        <SectionCard title="Server Configuration" icon={<Server size={15} className="text-blue-400" />}>
          <ConfigRow label="Port" value={server?.port ?? '—'} />
          <div className="flex items-center justify-between">
            <span className="text-xs text-zinc-400">Log Level</span>
            <span className="text-xs text-zinc-200 font-mono px-2 py-0.5 bg-zinc-800 rounded">
              {server?.log_level?.toUpperCase() ?? '—'}
            </span>
          </div>
          <StatusBadge enabled={server?.enable_logging ?? false} label="Logging" />
          <StatusBadge enabled={server?.disable_content_logging ?? false} label="Content Logging Disabled" />
          <ConfigRow
            label="Max Request Body"
            value={server?.max_request_body_size_mb != null ? `${server.max_request_body_size_mb} MB` : '—'}
          />
          <div>
            <div className="flex items-center justify-between mb-1">
              <span className="text-xs text-zinc-400">CORS Origins</span>
              <button
                onClick={() => setShowOrigins(!showOrigins)}
                className="text-zinc-500 hover:text-zinc-300 transition-colors"
              >
                {showOrigins ? <EyeOff size={12} /> : <Eye size={12} />}
              </button>
            </div>
            {showOrigins && (
              <div className="bg-zinc-950 border border-zinc-800/50 rounded-lg p-2 mt-1">
                {server?.cors_allowed_origins?.length ? (
                  server.cors_allowed_origins.map((origin, i) => (
                    <div key={i} className="text-xs text-zinc-300 font-mono py-0.5">{origin}</div>
                  ))
                ) : (
                  <div className="text-xs text-zinc-600">No origins configured</div>
                )}
              </div>
            )}
            {!showOrigins && (
              <div className="text-xs text-zinc-500 text-right">
                {server?.cors_allowed_origins?.length ?? 0} origin{(server?.cors_allowed_origins?.length ?? 0) !== 1 ? 's' : ''}
              </div>
            )}
          </div>
        </SectionCard>

        {/* Authentication */}
        <SectionCard title="Authentication" icon={<Shield size={15} className="text-emerald-400" />}>
          <StatusBadge enabled={auth?.google_oauth_enabled ?? false} label="Google OAuth" />
          {auth?.google_client_id && (
            <ConfigRow label="Google Client ID" value={`${auth.google_client_id.substring(0, 12)}…`} />
          )}
          <StatusBadge enabled={auth?.admin_key_configured ?? false} label="Admin Key" />
          <StatusBadge enabled={auth?.jwt_secret_configured ?? false} label="JWT Secret" />
        </SectionCard>

        {/* Queuing Configuration */}
        <SectionCard title="Queuing Configuration" icon={<Clock size={15} className="text-yellow-400" />}>
          <ConfigRow label="Max Concurrency" value={queue?.max_concurrency ?? '—'} />
          <ConfigRow label="Max Queue Size" value={queue?.max_queue_size ?? '—'} />
          <ConfigRow
            label="Queue Timeout"
            value={queue?.queue_timeout_ms != null ? `${queue.queue_timeout_ms} ms` : '—'}
          />
        </SectionCard>

        {/* System Information */}
        <SectionCard title="System Information" icon={<FileText size={15} className="text-purple-400" />}>
          <ConfigRow label="Version" value={system?.version ?? '—'} />
          <ConfigRow label="Config File" value={system?.config_file_path ?? '—'} />
          <ConfigRow label="Last Reload" value={formatTimestamp(system?.last_config_reload ?? null)} />
        </SectionCard>
      </div>

      {/* Actions */}
      <div className="flex items-center gap-3">
        <button
          onClick={() => reloadMutation.mutate()}
          disabled={reloadMutation.isPending}
          className={`flex items-center gap-2 px-4 py-2 text-sm rounded-lg border transition-all ${
            reloadMutation.isPending
              ? 'bg-zinc-800 border-zinc-700 text-zinc-500 cursor-not-allowed'
              : 'bg-zinc-900 border-zinc-800 text-zinc-300 hover:border-zinc-600 hover:text-white active:scale-95'
          }`}
        >
          <RotateCcw size={14} className={reloadMutation.isPending ? 'animate-spin' : ''} />
          {reloadMutation.isPending ? 'Reloading...' : 'Reload Config'}
        </button>

        {config?.environment === 'dev' && (
          <button
            onClick={handlePromote}
            disabled={isPromoting}
            className={`flex items-center gap-2 px-4 py-2 text-sm rounded-lg border font-semibold transition-all ${
              isPromoting
                ? 'bg-zinc-800 border-zinc-700 text-zinc-500 cursor-not-allowed'
                : 'bg-gradient-to-r from-purple-600 to-indigo-600 border-indigo-500 hover:border-indigo-400 text-white shadow-lg hover:shadow-indigo-500/20 active:scale-95'
            }`}
          >
            <Rocket size={14} className={isPromoting ? 'animate-bounce' : ''} />
            {isPromoting ? 'Promoting...' : 'Promote to Production'}
          </button>
        )}
      </div>
    </div>
  )
}
