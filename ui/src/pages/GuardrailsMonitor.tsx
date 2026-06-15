import { useState, useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import { guardrailsMonitorApi, configApi, type LogEntry } from '../lib/api'
import { formatTimestamp, formatLatency } from '../lib/utils'
import {
  ShieldAlert, Shield, AlertTriangle, Ban, Search, Filter,
  ChevronDown, ChevronUp, RefreshCw,
} from 'lucide-react'
import {
  BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer, Cell,
  PieChart, Pie, Legend,
} from 'recharts'

const PERIODS = ['1h', '6h', '24h', '7d', '30d'] as const
type Period = typeof PERIODS[number]

const VIOLATION_COLORS: Record<string, string> = {
  content_filter: '#ef4444',
  jailbreak: '#f59e0b',
  keyword: '#8b5cf6',
  pii: '#3b82f6',
  prompt_injection: '#ec4899',
  unknown: '#6b7280',
}

function classifyViolation(log: LogEntry): string {
  if (log.finish_reason === 'content_filter') {
    const input = (log.input_preview ?? '').toLowerCase()
    const output = (log.output_preview ?? '').toLowerCase()
    if (input.includes('jailbreak') || output.includes('jailbreak')) return 'jailbreak'
    if (input.includes('injection') || output.includes('injection')) return 'prompt_injection'
    if (input.includes('pii') || output.includes('pii') || input.includes('email') || input.includes('credit')) return 'pii'
    if (input.includes('block') || output.includes('block') || input.includes('keyword')) return 'keyword'
    return 'content_filter'
  }
  return 'unknown'
}

function formatCost(v: number): string {
  if (v === 0) return '$0.00'
  if (v < 0.01) return `$${v.toFixed(5)}`
  return `$${v.toFixed(4)}`
}

function formatNumber(v: number): string {
  if (v >= 1_000_000) return `${(v / 1_000_000).toFixed(1)}M`
  if (v >= 1_000) return `${(v / 1_000).toFixed(1)}k`
  return v.toFixed(0)
}

function Skeleton({ className }: { className?: string }) {
  return <div className={`animate-pulse bg-gray-800 rounded ${className ?? ''}`} />
}

function StatCard({
  label, value, sub, icon, accent = 'blue',
}: {
  label: string
  value: string
  sub?: string
  icon: React.ReactNode
  accent?: 'blue' | 'green' | 'yellow' | 'red' | 'indigo' | 'gray' | 'emerald'
}) {
  const colors = {
    blue:    'text-blue-400 bg-blue-900/30',
    green:   'text-green-400 bg-green-900/30',
    yellow:  'text-yellow-400 bg-yellow-900/30',
    red:     'text-red-400 bg-red-900/30',
    indigo:  'text-indigo-400 bg-indigo-900/30',
    gray:    'text-gray-400 bg-gray-800',
    emerald: 'text-emerald-400 bg-emerald-900/30',
  }
  return (
    <div className="rounded-xl border border-gray-800 bg-gray-900 p-5 hover:border-gray-700 hover:shadow-lg transition-all duration-200">
      <div className="flex items-start justify-between mb-3">
        <span className="text-xs text-gray-500 uppercase tracking-wider">{label}</span>
        <span className={`p-1.5 rounded-lg ${colors[accent]}`}>{icon}</span>
      </div>
      <div className="text-2xl font-bold text-white tabular-nums">{value}</div>
      {sub && <div className="text-xs text-gray-500 mt-1">{sub}</div>}
    </div>
  )
}

export default function GuardrailsMonitor() {
  const [period, setPeriod] = useState<Period>('24h')
  const [searchQuery, setSearchQuery] = useState('')
  const [expandedId, setExpandedId] = useState<string | null>(null)
  const [autoRefresh, setAutoRefresh] = useState(true)

  const interventionsQ = useQuery({
    queryKey: ['guardrails-interventions', period],
    queryFn: () => guardrailsMonitorApi.getInterventions({
      period,
      limit: 500,
      finish_reason: 'content_filter',
    }),
    refetchInterval: autoRefresh ? 15_000 : undefined,
  })

  const histogramQ = useQuery({
    queryKey: ['guardrails-histogram', period],
    queryFn: () => guardrailsMonitorApi.getHistogram({
      period,
      finish_reason: 'content_filter',
    }),
    refetchInterval: autoRefresh ? 15_000 : undefined,
  })

  const configQ = useQuery({
    queryKey: ['config'],
    queryFn: configApi.get,
    staleTime: 60_000,
  })

  const logs = interventionsQ.data?.logs ?? []
  const totalCount = interventionsQ.data?.pagination.total_count ?? 0
  const histogramBuckets = histogramQ.data?.buckets ?? []
  const isLoading = interventionsQ.isLoading

  const guardrailsPlugin = configQ.data?.plugins?.find((p: any) => p.name === 'guardrails')
  const guardrailsEnabled = guardrailsPlugin?.enabled ?? false
  const guardrailsConfig = guardrailsPlugin?.config ?? {}

  const filteredLogs = useMemo(() => {
    if (!searchQuery) return logs
    const q = searchQuery.toLowerCase()
    return logs.filter(l =>
      (l.virtual_key?.toLowerCase().includes(q)) ||
      (l.model?.toLowerCase().includes(q)) ||
      (l.provider?.toLowerCase().includes(q)) ||
      (l.input_preview?.toLowerCase().includes(q)) ||
      (l.output_preview?.toLowerCase().includes(q))
    )
  }, [logs, searchQuery])

  const { uniqueVks, violationTypeBreakdown } = useMemo(() => {
    const vkSet = new Set<string>()
    const typeMap = new Map<string, number>()
    for (const l of logs) {
      if (l.virtual_key) vkSet.add(l.virtual_key)
      const type = classifyViolation(l)
      typeMap.set(type, (typeMap.get(type) ?? 0) + 1)
    }
    return {
      uniqueVks: vkSet.size,
      violationTypeBreakdown: Array.from(typeMap.entries()).map(([name, value]) => ({ name, value })),
    }
  }, [logs])

  const activeFilters = useMemo(() => {
    const cfg = guardrailsConfig
    const filters: string[] = []
    if (cfg.mask_pii) filters.push('PII Masking')
    if (cfg.mask_secrets) filters.push('Secrets Masking')
    if (cfg.prevent_prompt_injection) filters.push('Prompt Injection Prevention')
    if (cfg.blocked_keywords?.length > 0) filters.push(`${cfg.blocked_keywords.length} Blocked Keywords`)
    return filters
  }, [guardrailsConfig])

  const blockRate = interventionsQ.data?.stats
    ? ((totalCount / Math.max(1, interventionsQ.data.stats.total_requests)) * 100).toFixed(2)
    : '0.00'

  return (
    <div className="p-6 space-y-8 overflow-y-auto h-full">
      {/* Header */}
      <div className="flex items-center justify-between flex-wrap gap-3">
        <div>
          <h1 className="text-2xl font-bold text-white flex items-center gap-2">
            <ShieldAlert size={22} className="text-red-400" />
            Guardrails Monitor
          </h1>
          <p className="text-sm text-gray-400 mt-1">
            Real-time monitoring and alerting for guardrail interventions, safety violations, and content filtering.
          </p>
        </div>
        <div className="flex items-center gap-3">
          <button
            onClick={() => setAutoRefresh(!autoRefresh)}
            className={`flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg border transition-colors ${
              autoRefresh
                ? 'bg-emerald-900/30 border-emerald-800 text-emerald-400'
                : 'bg-gray-900 border-gray-800 text-gray-500 hover:text-gray-300'
            }`}
          >
            <div className={`w-1.5 h-1.5 rounded-full ${autoRefresh ? 'bg-emerald-400 animate-pulse' : 'bg-gray-600'}`} />
            Auto-refresh
          </button>
          <div className="flex rounded-lg border border-gray-700 overflow-hidden">
            {PERIODS.map(p => (
              <button
                key={p}
                onClick={() => setPeriod(p)}
                className={`px-3 py-1.5 text-xs transition-colors ${
                  period === p
                    ? 'bg-red-600 text-white'
                    : 'text-gray-400 hover:text-white hover:bg-gray-800'
                }`}
              >
                {p}
              </button>
            ))}
          </div>
          <button
            onClick={() => interventionsQ.refetch()}
            className="p-1.5 rounded-lg bg-gray-900 border border-gray-800 text-gray-400 hover:text-white hover:bg-gray-800 transition-colors"
          >
            <RefreshCw size={14} className={interventionsQ.isFetching ? 'animate-spin' : ''} />
          </button>
        </div>
      </div>

      {/* Stats Cards */}
      <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
        {isLoading ? (
          Array.from({ length: 4 }).map((_, i) => <Skeleton key={i} className="h-28" />)
        ) : (
          <>
            <StatCard
              label="Total Interventions"
              value={formatNumber(totalCount)}
              sub={`Over ${period}`}
              icon={<ShieldAlert size={14} />}
              accent="red"
            />
            <StatCard
              label="Block Rate"
              value={`${blockRate}%`}
              sub="Of total requests"
              icon={<Ban size={14} />}
              accent={Number(blockRate) > 5 ? 'yellow' : 'green'}
            />
            <StatCard
              label="Unique Virtual Keys"
              value={formatNumber(uniqueVks)}
              sub="Affected keys"
              icon={<Filter size={14} />}
              accent="indigo"
            />
            <StatCard
              label="Guardrails Status"
              value={guardrailsEnabled ? 'Active' : 'Disabled'}
              sub={activeFilters.length > 0 ? activeFilters.join(', ') : 'No filters configured'}
              icon={<Shield size={14} />}
              accent={guardrailsEnabled ? 'emerald' : 'gray'}
            />
          </>
        )}
      </div>

      {/* Charts Row */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Interventions over time */}
        <div className="rounded-xl border border-gray-800 bg-gray-900 p-5">
          <h2 className="text-sm font-semibold text-gray-300 mb-5 flex items-center gap-2">
            <AlertTriangle size={14} className="text-red-400" />
            Interventions Over Time
          </h2>
          {isLoading ? (
            <Skeleton className="h-52" />
          ) : histogramBuckets.length > 0 ? (
            <ResponsiveContainer width="100%" height={200}>
              <BarChart data={histogramBuckets} barSize={24}>
                <XAxis
                  dataKey="timestamp"
                  tickFormatter={(ts) => {
                    const d = new Date(ts)
                    return period === '1h' || period === '6h'
                      ? `${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')}`
                      : `${d.getMonth() + 1}/${d.getDate()}`
                  }}
                  tick={{ fill: '#9ca3af', fontSize: 11 }}
                  axisLine={false}
                  tickLine={false}
                />
                <YAxis
                  tick={{ fill: '#6b7280', fontSize: 10 }}
                  axisLine={false}
                  tickLine={false}
                  allowDecimals={false}
                />
                <Tooltip
                  contentStyle={{ background: '#111827', border: '1px solid #374151', borderRadius: 8 }}
                  labelStyle={{ color: '#e5e7eb' }}
                  labelFormatter={(ts) => new Date(Number(ts)).toLocaleString()}
                />
                <Bar dataKey="count" radius={[4, 4, 0, 0]} fill="#ef4444" />
              </BarChart>
            </ResponsiveContainer>
          ) : (
            <div className="h-52 flex items-center justify-center text-gray-600 text-sm">
              No intervention data in this period
            </div>
          )}
        </div>

        {/* Violation Types Breakdown */}
        <div className="rounded-xl border border-gray-800 bg-gray-900 p-5">
          <h2 className="text-sm font-semibold text-gray-300 mb-5 flex items-center gap-2">
            <Ban size={14} className="text-yellow-400" />
            Violation Types
          </h2>
          {isLoading ? (
            <Skeleton className="h-52" />
          ) : violationTypeBreakdown.length > 0 ? (
            <ResponsiveContainer width="100%" height={220}>
              <PieChart>
                <Pie
                  data={violationTypeBreakdown}
                  dataKey="value"
                  nameKey="name"
                  cx="50%"
                  cy="50%"
                  outerRadius={80}
                  label={({ name, percent }) => `${name} (${(percent * 100).toFixed(0)}%)`}
                  labelLine={false}
                >
                  {violationTypeBreakdown.map(entry => (
                    <Cell key={entry.name} fill={VIOLATION_COLORS[entry.name] ?? '#6b7280'} />
                  ))}
                </Pie>
                <Tooltip
                  contentStyle={{ background: '#111827', border: '1px solid #374151', borderRadius: 8 }}
                  labelStyle={{ color: '#e5e7eb' }}
                />
                <Legend
                  wrapperStyle={{ fontSize: 11, color: '#9ca3af' }}
                />
              </PieChart>
            </ResponsiveContainer>
          ) : (
            <div className="h-52 flex items-center justify-center text-gray-600 text-sm">
              No violation data in this period
            </div>
          )}
        </div>
      </div>

      {/* Recent Interventions Table */}
      <section>
        <div className="flex items-center justify-between mb-4">
          <h2 className="text-sm font-semibold text-gray-400 uppercase tracking-wider flex items-center gap-2">
            <Search size={14} className="text-red-400" />
            Recent Interventions
          </h2>
          <div className="relative w-64">
            <Search className="absolute left-3 top-2.5 w-4 h-4 text-gray-500" />
            <input
              type="text"
              value={searchQuery}
              onChange={e => setSearchQuery(e.target.value)}
              placeholder="Search interventions..."
              className="w-full bg-gray-950 border border-gray-800 rounded-lg pl-9 pr-4 py-2 text-xs text-gray-200 focus:outline-none focus:border-red-500/50"
            />
          </div>
        </div>
        <div className="rounded-xl border border-gray-800 bg-gray-900 overflow-hidden">
          <table className="w-full text-sm">
            <thead>
              <tr className="border-b border-gray-800">
                {['Time', 'Virtual Key', 'Model', 'Violation Type', 'Latency', 'Input Preview'].map(h => (
                  <th key={h} className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                    {h}
                  </th>
                ))}
              </tr>
            </thead>
            <tbody>
              {isLoading ? (
                Array.from({ length: 5 }).map((_, i) => (
                  <tr key={i} className="border-b border-gray-800/50">
                    {Array.from({ length: 6 }).map((_, j) => (
                      <td key={j} className="px-4 py-3">
                        <Skeleton className="h-4 w-20" />
                      </td>
                    ))}
                  </tr>
                ))
              ) : filteredLogs.length === 0 ? (
                <tr>
                  <td colSpan={6} className="px-4 py-12 text-center text-gray-600">
                    {searchQuery ? 'No interventions match your search' : 'No guardrail interventions recorded in this period'}
                  </td>
                </tr>
              ) : (
                filteredLogs.map(log => {
                  const isExpanded = expandedId === log.id
                  const violation = classifyViolation(log)
                  return (
                    <tr key={log.id} className="border-b border-gray-800/50">
                      <td className="px-4 py-3">
                        <button
                          onClick={() => setExpandedId(isExpanded ? null : log.id)}
                          className="flex items-center gap-1.5 text-gray-400 hover:text-white transition-colors"
                        >
                          {isExpanded ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
                          <span className="font-mono text-xs">{formatTimestamp(log.timestamp)}</span>
                        </button>
                      </td>
                      <td className="px-4 py-3 text-gray-300 text-xs max-w-[120px] truncate">
                        {log.virtual_key ?? '—'}
                      </td>
                      <td className="px-4 py-3 text-gray-300 font-mono text-xs max-w-[180px] truncate">
                        {log.model}
                      </td>
                      <td className="px-4 py-3">
                        <span
                          className="text-xs px-2 py-0.5 rounded-full font-medium"
                          style={{
                            backgroundColor: `${VIOLATION_COLORS[violation] ?? '#6b7280'}20`,
                            color: VIOLATION_COLORS[violation] ?? '#6b7280',
                            border: `1px solid ${VIOLATION_COLORS[violation] ?? '#6b7280'}40`,
                          }}
                        >
                          {violation.replace(/_/g, ' ')}
                        </span>
                      </td>
                      <td className="px-4 py-3 text-gray-300 tabular-nums text-xs">
                        {formatLatency(log.latency_ms)}
                      </td>
                      <td className="px-4 py-3 text-gray-500 text-xs max-w-[250px] truncate">
                        {log.input_preview ?? '—'}
                      </td>
                    </tr>
                  )
                })
              )}
            </tbody>
          </table>
        </div>
      </section>
    </div>
  )
}
