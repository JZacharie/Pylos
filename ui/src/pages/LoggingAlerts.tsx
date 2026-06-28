import { useState, useEffect } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  ScrollText, Bell, Webhook, Settings, Save, Plus, Trash2,
  ToggleLeft, ToggleRight, Database, AlertTriangle, RotateCcw,
} from 'lucide-react'
import { configApi } from '../lib/api'

interface WebhookEntry {
  id: string
  url: string
  events: string[]
  active: boolean
}

interface AlertConfig {
  budget_threshold_pct: number
  error_rate_threshold_pct: number
  rate_limit_alerts: boolean
}

const DEFAULT_WEBHOOKS: WebhookEntry[] = []
const DEFAULT_ALERTS: AlertConfig = {
  budget_threshold_pct: 80,
  error_rate_threshold_pct: 10,
  rate_limit_alerts: true,
}

const EVENT_OPTIONS = [
  'request.success',
  'request.error',
  'budget.threshold',
  'rate_limit.exceeded',
  'guardrail.triggered',
  'key.expired',
]

export default function LoggingAlerts() {
  const queryClient = useQueryClient()

  const { data: configData, isLoading } = useQuery({
    queryKey: ['config'],
    queryFn: configApi.get,
  })

  // Logging config from server
  const [logRetentionDays, setLogRetentionDays] = useState(30)
  const [disableContentLogging, setDisableContentLogging] = useState(false)
  const [logLevel, setLogLevel] = useState('info')
  const [dbBackend, setDbBackend] = useState('SQLite')

  // Local state for webhooks and alerts
  const [webhooks, setWebhooks] = useState<WebhookEntry[]>(DEFAULT_WEBHOOKS)
  const [alerts, setAlerts] = useState<AlertConfig>(DEFAULT_ALERTS)

  // Sync from config
  useEffect(() => {
    if (configData) {
      const logging = configData.logging || {}
      setLogRetentionDays(logging.retention_days ?? 30)
      setDisableContentLogging(logging.disable_content ?? false)
      setLogLevel(logging.level ?? 'info')
      setDbBackend(logging.database ?? 'SQLite')

      if (configData.webhooks) setWebhooks(configData.webhooks)
      if (configData.alerts) setAlerts({ ...DEFAULT_ALERTS, ...configData.alerts })
    }
  }, [configData])

  const saveMutation = useMutation({
    mutationFn: () => configApi.reload(),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['config'] })
    },
  })

  // ─── Webhook helpers ──────────────────────────────────────────────────────

  const addWebhook = () => {
    setWebhooks(prev => [
      ...prev,
      { id: crypto.randomUUID(), url: '', events: [], active: true },
    ])
  }

  const removeWebhook = (id: string) => {
    setWebhooks(prev => prev.filter(w => w.id !== id))
  }

  const updateWebhook = (id: string, patch: Partial<WebhookEntry>) => {
    setWebhooks(prev => prev.map(w => (w.id === id ? { ...w, ...patch } : w)))
  }

  const toggleWebhookEvent = (id: string, event: string) => {
    setWebhooks(prev =>
      prev.map(w => {
        if (w.id !== id) return w
        const has = w.events.includes(event)
        return { ...w, events: has ? w.events.filter(e => e !== event) : [...w.events, event] }
      })
    )
  }

  // ─── Render ───────────────────────────────────────────────────────────────

  if (isLoading) {
    return (
      <div className="flex-1 flex items-center justify-center p-8">
        <div className="text-zinc-500 animate-pulse">Loading configuration…</div>
      </div>
    )
  }

  return (
    <div className="flex-1 flex flex-col h-full bg-zinc-950 text-zinc-100 overflow-y-auto">
      {/* Header */}
      <header className="border-b border-zinc-800 bg-zinc-900/30 shrink-0">
        <div className="px-6 py-4 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="p-2 bg-amber-500/10 rounded-lg">
              <ScrollText className="text-amber-400 w-5 h-5" />
            </div>
            <h1 className="text-lg font-semibold text-white">Logging & Alerts</h1>
          </div>

          <button
            onClick={() => saveMutation.mutate()}
            disabled={saveMutation.isPending}
            className="flex items-center gap-2 px-4 py-2 bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-white text-xs font-semibold rounded-lg transition-all"
          >
            {saveMutation.isPending ? <RotateCcw size={14} className="animate-spin" /> : <Save size={14} />}
            Save Config
          </button>
        </div>
      </header>

      <div className="px-6 py-8 max-w-4xl mx-auto w-full space-y-8">
        {/* ─── Logging Configuration ────────────────────────────────────── */}
        <Section icon={<ScrollText className="text-amber-400 w-4 h-4" />} title="Logging Configuration" subtitle="Control how logs are stored and what is captured">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
            {/* Retention */}
            <Card>
              <h3 className="text-sm font-medium text-white mb-1.5">Log Retention Period</h3>
              <p className="text-zinc-500 text-xs mb-4">Number of days to keep log entries before automatic cleanup.</p>
              <div className="flex items-center gap-3">
                <input
                  type="number"
                  min={1}
                  max={365}
                  value={logRetentionDays}
                  onChange={e => setLogRetentionDays(Math.max(1, Number(e.target.value)))}
                  className="w-24 bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50"
                />
                <span className="text-xs text-zinc-500">days</span>
              </div>
            </Card>

            {/* Log Level */}
            <Card>
              <h3 className="text-sm font-medium text-white mb-1.5">Log Level</h3>
              <p className="text-zinc-500 text-xs mb-4">Minimum severity level to record in the logging backend.</p>
              <select
                value={logLevel}
                onChange={e => setLogLevel(e.target.value)}
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50"
              >
                {['error', 'warn', 'info', 'debug', 'trace'].map(lvl => (
                  <option key={lvl} value={lvl}>{lvl.charAt(0).toUpperCase() + lvl.slice(1)}</option>
                ))}
              </select>
            </Card>

            {/* Disable content logging */}
            <Card>
              <h3 className="text-sm font-medium text-white mb-1.5">Disable Content Logging</h3>
              <p className="text-zinc-500 text-xs mb-4">When enabled, prompt and completion text will not be stored in logs.</p>
              <label className="flex items-center cursor-pointer gap-3">
                <button
                  type="button"
                  onClick={() => setDisableContentLogging(!disableContentLogging)}
                  className="text-zinc-400"
                >
                  {disableContentLogging
                    ? <ToggleRight className="w-8 h-8 text-emerald-400" />
                    : <ToggleLeft className="w-8 h-8 text-zinc-600" />}
                </button>
                <span className="text-zinc-300 text-sm">{disableContentLogging ? 'Content logging disabled' : 'Content logging enabled'}</span>
              </label>
            </Card>

            {/* Database backend */}
            <Card>
              <h3 className="text-sm font-medium text-white mb-1.5">Database Backend</h3>
              <p className="text-zinc-500 text-xs mb-4">Storage engine used for persisting log data.</p>
              <div className="flex items-center gap-3">
                <Database className="w-4 h-4 text-zinc-500" />
                <select
                  value={dbBackend}
                  onChange={e => setDbBackend(e.target.value)}
                  className="flex-1 bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50"
                >
                  <option value="SQLite">SQLite</option>
                  <option value="PostgreSQL">PostgreSQL</option>
                </select>
              </div>
            </Card>
          </div>
        </Section>

        {/* ─── Current Log Statistics ───────────────────────────────────── */}
        <Section icon={<Settings className="text-blue-400 w-4 h-4" />} title="Log Statistics" subtitle="Current state of the logging system">
          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
            <StatCard label="Total Log Entries" value={configData?.log_stats?.total_entries?.toLocaleString() ?? '—'} />
            <StatCard label="Storage Usage" value={configData?.log_stats?.storage_usage ?? '—'} />
            <StatCard label="Oldest Entry" value={configData?.log_stats?.oldest_entry ?? '—'} />
            <StatCard label="Database" value={dbBackend} />
          </div>
        </Section>

        {/* ─── Webhook Configuration ────────────────────────────────────── */}
        <Section icon={<Webhook className="text-purple-400 w-4 h-4" />} title="Webhooks" subtitle="Send event notifications to external endpoints">
          <div className="space-y-4">
            {webhooks.map(wh => (
              <div key={wh.id} className="bg-zinc-950 border border-zinc-800/80 rounded-xl p-4 space-y-3">
                <div className="flex items-center gap-3">
                  <input
                    type="text"
                    value={wh.url}
                    onChange={e => updateWebhook(wh.id, { url: e.target.value })}
                    placeholder="https://example.com/webhook"
                    className="flex-1 bg-zinc-900 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 placeholder-zinc-700 focus:outline-none focus:border-emerald-500/50"
                  />
                  <button
                    type="button"
                    onClick={() => updateWebhook(wh.id, { active: !wh.active })}
                    className={`p-2 rounded-lg transition-colors ${wh.active ? 'text-emerald-400 bg-emerald-500/10' : 'text-zinc-600 bg-zinc-900'}`}
                  >
                    {wh.active ? <ToggleRight className="w-5 h-5" /> : <ToggleLeft className="w-5 h-5" />}
                  </button>
                  <button
                    type="button"
                    onClick={() => removeWebhook(wh.id)}
                    className="p-2 text-zinc-600 hover:text-red-400 hover:bg-red-500/10 rounded-lg transition-colors"
                  >
                    <Trash2 className="w-4 h-4" />
                  </button>
                </div>
                <div className="flex flex-wrap gap-2">
                  {EVENT_OPTIONS.map(evt => {
                    const selected = wh.events.includes(evt)
                    return (
                      <button
                        key={evt}
                        type="button"
                        onClick={() => toggleWebhookEvent(wh.id, evt)}
                        className={`px-2.5 py-1 rounded-md text-[11px] font-medium border transition-colors ${
                          selected
                            ? 'bg-purple-500/15 border-purple-500/30 text-purple-300'
                            : 'bg-zinc-900 border-zinc-800 text-zinc-500 hover:text-zinc-300'
                        }`}
                      >
                        {evt}
                      </button>
                    )
                  })}
                </div>
              </div>
            ))}

            <button
              type="button"
              onClick={addWebhook}
              className="flex items-center gap-2 px-4 py-2 border border-dashed border-zinc-700 hover:border-zinc-500 text-zinc-400 hover:text-zinc-200 text-xs rounded-xl transition-colors w-full justify-center"
            >
              <Plus className="w-4 h-4" />
              Add Webhook
            </button>
          </div>
        </Section>

        {/* ─── Alert Configuration ──────────────────────────────────────── */}
        <Section icon={<Bell className="text-rose-400 w-4 h-4" />} title="Alerts" subtitle="Configure thresholds for automatic notifications">
          <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
            {/* Budget alerts */}
            <Card>
              <div className="flex items-center gap-2 mb-1.5">
                <AlertTriangle className="w-4 h-4 text-amber-400" />
                <h3 className="text-sm font-medium text-white">Budget Alert Threshold</h3>
              </div>
              <p className="text-zinc-500 text-xs mb-4">Trigger an alert when a key's spending reaches this percentage of its budget.</p>
              <div className="flex items-center gap-3">
                <input
                  type="range"
                  min={10}
                  max={100}
                  step={5}
                  value={alerts.budget_threshold_pct}
                  onChange={e => setAlerts(a => ({ ...a, budget_threshold_pct: Number(e.target.value) }))}
                  className="flex-1 accent-amber-500"
                />
                <span className="text-sm font-mono text-zinc-300 w-12 text-right">{alerts.budget_threshold_pct}%</span>
              </div>
            </Card>

            {/* Error rate alerts */}
            <Card>
              <div className="flex items-center gap-2 mb-1.5">
                <AlertTriangle className="w-4 h-4 text-red-400" />
                <h3 className="text-sm font-medium text-white">Error Rate Alert Threshold</h3>
              </div>
              <p className="text-zinc-500 text-xs mb-4">Trigger an alert when the error rate exceeds this percentage.</p>
              <div className="flex items-center gap-3">
                <input
                  type="range"
                  min={1}
                  max={50}
                  step={1}
                  value={alerts.error_rate_threshold_pct}
                  onChange={e => setAlerts(a => ({ ...a, error_rate_threshold_pct: Number(e.target.value) }))}
                  className="flex-1 accent-red-500"
                />
                <span className="text-sm font-mono text-zinc-300 w-12 text-right">{alerts.error_rate_threshold_pct}%</span>
              </div>
            </Card>

            {/* Rate limit alerts */}
            <Card className="md:col-span-2">
              <h3 className="text-sm font-medium text-white mb-1.5">Rate Limit Alerts</h3>
              <p className="text-zinc-500 text-xs mb-4">Notify when a virtual key approaches or hits its rate limit.</p>
              <label className="flex items-center cursor-pointer gap-3">
                <button
                  type="button"
                  onClick={() => setAlerts(a => ({ ...a, rate_limit_alerts: !a.rate_limit_alerts }))}
                  className="text-zinc-400"
                >
                  {alerts.rate_limit_alerts
                    ? <ToggleRight className="w-8 h-8 text-emerald-400" />
                    : <ToggleLeft className="w-8 h-8 text-zinc-600" />}
                </button>
                <span className="text-zinc-300 text-sm">{alerts.rate_limit_alerts ? 'Rate limit alerts enabled' : 'Rate limit alerts disabled'}</span>
              </label>
            </Card>
          </div>
        </Section>

        {/* Bottom save */}
        <div className="flex justify-end pt-2 pb-8">
          <button
            onClick={() => saveMutation.mutate()}
            disabled={saveMutation.isPending}
            className="flex items-center gap-2 px-6 py-3 bg-white hover:bg-zinc-200 text-zinc-950 font-semibold text-xs rounded-xl transition-colors disabled:opacity-50"
          >
            {saveMutation.isPending ? <RotateCcw size={16} className="animate-spin" /> : <Save size={16} />}
            Save Configuration
          </button>
        </div>
      </div>
    </div>
  )
}

// ─── Small helpers ──────────────────────────────────────────────────────────

function Section({ icon, title, subtitle, children }: {
  icon: React.ReactNode
  title: string
  subtitle: string
  children: React.ReactNode
}) {
  return (
    <div className="space-y-4">
      <div className="flex items-center gap-2">
        {icon}
        <div>
          <h2 className="text-base font-semibold text-white">{title}</h2>
          <p className="text-xs text-zinc-500">{subtitle}</p>
        </div>
      </div>
      {children}
    </div>
  )
}

function Card({ children, className = '' }: { children: React.ReactNode; className?: string }) {
  return (
    <div className={`bg-zinc-900 border border-zinc-800/50 rounded-2xl p-6 ${className}`}>
      {children}
    </div>
  )
}

function StatCard({ label, value }: { label: string; value: string }) {
  return (
    <div className="bg-zinc-900 border border-zinc-800/50 rounded-xl p-4">
      <p className="text-xs text-zinc-500 mb-1">{label}</p>
      <p className="text-lg font-semibold text-white">{value}</p>
    </div>
  )
}
