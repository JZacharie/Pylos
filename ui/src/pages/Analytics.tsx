import { useState, useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import { logsApi, organizationsApi, teamsApi, usersApi, virtualKeysApi } from '../lib/api'
import type { Organization, Team, InternalUser, VirtualKey } from '../lib/api'
import { getCurrentUser, isCurrentUserAdmin } from '../lib/utils'
import {
  BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer, Cell,
} from 'recharts'
import {
  Activity, TrendingUp, Zap, Shield, DollarSign, Clock, AlertTriangle, Cpu,
  Building2, Users, User, Filter,
} from 'lucide-react'

const PERIODS = ['1h', '6h', '24h', '7d'] as const
type Period = typeof PERIODS[number]

type ViewMode = 'all' | 'org' | 'team' | 'user'

const GPT4O_PRICE_PER_1M_IN  = 5.0
const GPT4O_PRICE_PER_1M_OUT = 15.0

const PROVIDER_COLORS: Record<string, string> = {
  deepseek:   '#6366f1',
  ollama:     '#10b981',
  openrouter: '#f59e0b',
  graphon:    '#8b5cf6',
  lemonade:   '#ec4899',
}

function providerColor(p: string): string {
  const key = Object.keys(PROVIDER_COLORS).find(k => p.toLowerCase().includes(k))
  return key ? PROVIDER_COLORS[key] : '#6b7280'
}

interface ProviderStats {
  provider: string
  requests: number
  avgLatency: number
  avgTokens: number
  totalCost: number
  costPer1kTokens: number
  errorRate: number
  promptTokens: number
  completionTokens: number
  successCount: number
  latencies: number[]
}

function formatCost(v: number): string {
  if (v === 0) return '$0.00'
  if (v < 0.01) return `$${v.toFixed(5)}`
  return `$${v.toFixed(4)}`
}

function formatMs(v: number): string {
  if (v >= 1000) return `${(v / 1000).toFixed(1)}s`
  return `${v.toFixed(0)}ms`
}

function formatNumber(v: number): string {
  if (v >= 1_000_000) return `${(v / 1_000_000).toFixed(1)}M`
  if (v >= 1_000) return `${(v / 1_000).toFixed(1)}k`
  return v.toFixed(0)
}

function percentile(arr: number[], p: number): number {
  if (arr.length === 0) return 0
  const sorted = [...arr].sort((a, b) => a - b)
  const idx = Math.ceil((p / 100) * sorted.length) - 1
  return sorted[Math.max(0, idx)]
}

function Skeleton({ className }: { className?: string }) {
  return <div className={`animate-pulse bg-gray-800 rounded ${className ?? ''}`} />
}

function RumCard({
  label, value, sub, icon, accent = 'blue',
}: {
  label: string
  value: string
  sub?: string
  icon: React.ReactNode
  accent?: 'blue' | 'green' | 'yellow' | 'red' | 'indigo' | 'gray'
}) {
  const colors = {
    blue:   'text-blue-400 bg-blue-900/30',
    green:  'text-green-400 bg-green-900/30',
    yellow: 'text-yellow-400 bg-yellow-900/30',
    red:    'text-red-400 bg-red-900/30',
    indigo: 'text-indigo-400 bg-indigo-900/30',
    gray:   'text-gray-400 bg-gray-800',
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

function Select<T extends string>({
  value, onChange, options, placeholder, icon,
}: {
  value: T | null
  onChange: (v: T) => void
  options: { value: T; label: string }[]
  placeholder: string
  icon: React.ReactNode
}) {
  return (
    <div className="relative">
      <span className="absolute left-2.5 top-1/2 -translate-y-1/2 text-gray-500 pointer-events-none">
        {icon}
      </span>
      <select
        value={value ?? ''}
        onChange={e => onChange(e.target.value as T)}
        className="bg-gray-800 border border-gray-700 text-gray-200 text-xs rounded-lg pl-8 pr-3 py-2 appearance-none cursor-pointer hover:border-gray-600 focus:outline-none focus:ring-1 focus:ring-indigo-500"
      >
        <option value="">{placeholder}</option>
        {options.map(o => (
          <option key={o.value} value={o.value}>{o.label}</option>
        ))}
      </select>
    </div>
  )
}

function defaultUserState(): { viewMode: ViewMode; userId: string | null } {
  const user = getCurrentUser()
  if (user && !isCurrentUserAdmin()) {
    return { viewMode: 'user', userId: user.id }
  }
  return { viewMode: 'all', userId: null }
}

export default function Analytics() {
  const userState = defaultUserState()
  const [period, setPeriod] = useState<Period>('24h')
  const [viewMode, setViewMode] = useState<ViewMode>(userState.viewMode)
  const [selectedOrgId, setSelectedOrgId] = useState<string | null>(null)
  const [selectedTeamId, setSelectedTeamId] = useState<string | null>(null)
  const [selectedUserId, setSelectedUserId] = useState<string | null>(userState.userId)

  const logsQ = useQuery({
    queryKey: ['rum-logs', period],
    queryFn: () => logsApi.getLogs({ period, limit: 2000 }),
    refetchInterval: 60_000,
  })

  const statsQ = useQuery({
    queryKey: ['rum-stats', period],
    queryFn: () => logsApi.getStats({ period }),
    refetchInterval: 60_000,
  })

  const orgsQ = useQuery({
    queryKey: ['rum-orgs'],
    queryFn: () => organizationsApi.getAll(),
    staleTime: 120_000,
  })

  const teamsQ = useQuery({
    queryKey: ['rum-teams'],
    queryFn: () => teamsApi.getAll(),
    staleTime: 120_000,
  })

  const usersQ = useQuery({
    queryKey: ['rum-users'],
    queryFn: () => usersApi.getAll(),
    staleTime: 120_000,
  })

  const vksQ = useQuery({
    queryKey: ['rum-vks'],
    queryFn: () => virtualKeysApi.getAll(),
    staleTime: 120_000,
  })

  const logs = logsQ.data?.logs ?? []
  const stats = statsQ.data
  const orgs: Organization[] = orgsQ.data?.organizations ?? []
  const teams: Team[] = teamsQ.data?.teams ?? []
  const users: InternalUser[] = usersQ.data?.users ?? []
  const virtualKeys: VirtualKey[] = vksQ.data?.virtual_keys ?? []

  const filteredVkNames = useMemo(() => {
    if (viewMode === 'all') return null

    let matching: string[] = []

    if (viewMode === 'org' && selectedOrgId) {
      matching = virtualKeys
        .filter(vk => vk.organization_id === selectedOrgId)
        .map(vk => vk.name)
    } else if (viewMode === 'team' && selectedTeamId) {
      matching = virtualKeys
        .filter(vk => vk.team_id === selectedTeamId)
        .map(vk => vk.name)
    } else if (viewMode === 'user' && selectedUserId) {
      const user = users.find(u => u.id === selectedUserId)
      if (user) {
        matching = virtualKeys
          .filter(vk =>
            vk.user_id === selectedUserId ||
            vk.user_email === user.email ||
            (vk.team_id && user.team_ids.includes(vk.team_id))
          )
          .map(vk => vk.name)
      }
    }

    return matching.length > 0 ? new Set(matching) : null
  }, [viewMode, selectedOrgId, selectedTeamId, selectedUserId, virtualKeys, users])

  const filteredLogs = useMemo(() => {
    if (!filteredVkNames) return logs
    return logs.filter(l => l.virtual_key && filteredVkNames.has(l.virtual_key))
  }, [logs, filteredVkNames])

  const { providerStats, deepseekStats, savings, cacheHits, allLatencies } = useMemo(() => {
    const source = filteredLogs
    const map = new Map<string, ProviderStats>()

    for (const log of source) {
      const p = log.provider || 'unknown'
      if (!map.has(p)) {
        map.set(p, {
          provider: p, requests: 0, avgLatency: 0, avgTokens: 0,
          totalCost: 0, costPer1kTokens: 0, errorRate: 0,
          promptTokens: 0, completionTokens: 0, successCount: 0, latencies: [],
        })
      }
      const s = map.get(p)!
      s.requests++
      s.latencies.push(log.latency_ms)
      s.promptTokens += log.prompt_tokens
      s.completionTokens += log.completion_tokens
      s.totalCost += log.cost_usd
      if (log.status === 'success') s.successCount++
    }

    const providerStats: ProviderStats[] = []
    for (const [, s] of map) {
      const totalTokens = s.promptTokens + s.completionTokens
      s.avgLatency = s.latencies.reduce((a, b) => a + b, 0) / s.latencies.length
      s.avgTokens = totalTokens / s.requests
      s.costPer1kTokens = totalTokens > 0 ? (s.totalCost / totalTokens) * 1000 : 0
      s.errorRate = ((s.requests - s.successCount) / s.requests) * 100
      providerStats.push(s)
    }
    providerStats.sort((a, b) => b.requests - a.requests)

    const dsLogs = source.filter(l => l.provider?.toLowerCase().includes('deepseek'))
    const dsPrompt = dsLogs.reduce((a, l) => a + l.prompt_tokens, 0)
    const dsCompletion = dsLogs.reduce((a, l) => a + l.completion_tokens, 0)
    const dsCost = dsLogs.reduce((a, l) => a + l.cost_usd, 0)
    const deepseekStats = {
      requests: dsLogs.length,
      promptTokens: dsPrompt,
      completionTokens: dsCompletion,
      totalCost: dsCost,
      pct: source.length > 0 ? (dsLogs.length / source.length) * 100 : 0,
    }

    const equivalentGpt4oCost =
      (dsPrompt / 1_000_000) * GPT4O_PRICE_PER_1M_IN +
      (dsCompletion / 1_000_000) * GPT4O_PRICE_PER_1M_OUT
    const savings = Math.max(0, equivalentGpt4oCost - dsCost)

    const cacheHits = source.filter(l => l.cost_usd === 0 && l.total_tokens > 0).length

    const allLatencies = source.map(l => l.latency_ms)

    return { providerStats, deepseekStats, savings, cacheHits, allLatencies }
  }, [filteredLogs])

  const p50 = percentile(allLatencies, 50)
  const p95 = percentile(allLatencies, 95)
  const successRate = stats?.success_rate ?? 0

  const isLoading = logsQ.isLoading || statsQ.isLoading
  const isEmpty = !isLoading && filteredLogs.length === 0

  const activeFilterLabel = useMemo(() => {
    if (viewMode === 'org' && selectedOrgId) {
      const org = orgs.find(o => o.id === selectedOrgId)
      return org ? `Organisation: ${org.name}` : null
    }
    if (viewMode === 'team' && selectedTeamId) {
      const team = teams.find(t => t.id === selectedTeamId)
      return team ? `Équipe: ${team.name}` : null
    }
    if (viewMode === 'user' && selectedUserId) {
      const user = users.find(u => u.id === selectedUserId)
      return user ? `Utilisateur: ${user.name} (${user.email})` : null
    }
    return null
  }, [viewMode, selectedOrgId, selectedTeamId, selectedUserId, orgs, teams, users])

  const filteredTeams = useMemo(() => {
    if (viewMode === 'team') return teams
    if (viewMode === 'org' && selectedOrgId) {
      return teams.filter(t => t.organization_id === selectedOrgId)
    }
    return []
  }, [viewMode, selectedOrgId, teams])

  const filteredUsers = useMemo(() => {
    if (viewMode === 'user') return users
    if (viewMode === 'org' && selectedOrgId) {
      return users.filter(u => u.organization_id === selectedOrgId)
    }
    if (viewMode === 'team' && selectedTeamId) {
      return users.filter(u => u.team_ids.includes(selectedTeamId))
    }
    return []
  }, [viewMode, selectedOrgId, selectedTeamId, users])

  return (
    <div className="p-6 space-y-8 overflow-y-auto h-full">

      {/* Header */}
      <div className="flex items-center justify-between flex-wrap gap-3">
        <div>
          <h1 className="text-2xl font-bold text-white flex items-center gap-2">
            <Cpu size={22} className="text-indigo-400" />
            RUM Analytics
          </h1>
          <p className="text-sm text-gray-400 mt-1">
            Real User Monitoring · Optimisation DeepSeek via Pylos
          </p>
        </div>
        <div className="flex items-center gap-3">
          <div className="flex rounded-lg border border-gray-700 overflow-hidden">
            {PERIODS.map(p => (
              <button
                key={p}
                onClick={() => setPeriod(p)}
                className={`px-3 py-1.5 text-xs transition-colors ${
                  period === p
                    ? 'bg-indigo-600 text-white'
                    : 'text-gray-400 hover:text-white hover:bg-gray-800'
                }`}
              >
                {p}
              </button>
            ))}
          </div>
          <div className="flex items-center gap-1.5 text-xs text-gray-500">
            <div className="w-2 h-2 rounded-full bg-green-400 animate-pulse" />
            Live
          </div>
        </div>
      </div>

      {/* Filter bar */}
      <div className="flex flex-wrap items-center gap-3">
        <div className="flex rounded-lg border border-gray-700 overflow-hidden">
          {([
            { value: 'all' as ViewMode, label: 'Tout', icon: <Filter size={12} /> },
            { value: 'org' as ViewMode, label: 'Organisation', icon: <Building2 size={12} /> },
            { value: 'team' as ViewMode, label: 'Équipe', icon: <Users size={12} /> },
            { value: 'user' as ViewMode, label: 'Utilisateur', icon: <User size={12} /> },
          ]).map(m => (
            <button
              key={m.value}
              onClick={() => {
                setViewMode(m.value)
                setSelectedOrgId(null)
                setSelectedTeamId(null)
                setSelectedUserId(null)
              }}
              className={`flex items-center gap-1.5 px-3 py-1.5 text-xs transition-colors ${
                viewMode === m.value
                  ? 'bg-indigo-600 text-white'
                  : 'text-gray-400 hover:text-white hover:bg-gray-800'
              }`}
            >
              {m.icon} {m.label}
            </button>
          ))}
        </div>

        {viewMode === 'org' && (
          <Select
            value={selectedOrgId}
            onChange={setSelectedOrgId}
            options={orgs.filter(o => o.is_active).map(o => ({ value: o.id, label: o.name }))}
            placeholder="Sélectionner une organisation"
            icon={<Building2 size={14} />}
          />
        )}

        {(viewMode === 'team' || viewMode === 'org') && (
          <Select
            value={viewMode === 'team' ? selectedTeamId : selectedTeamId}
            onChange={v => setSelectedTeamId(v)}
            options={(viewMode === 'team' ? teams : filteredTeams)
              .filter(t => t.is_active)
              .map(t => ({ value: t.id, label: t.name }))}
            placeholder="Sélectionner une équipe"
            icon={<Users size={14} />}
          />
        )}

        {(viewMode === 'user' || viewMode === 'org' || viewMode === 'team') && (
          <Select
            value={selectedUserId}
            onChange={setSelectedUserId}
            options={(viewMode === 'user' ? users : filteredUsers)
              .filter(u => u.is_active)
              .map(u => ({ value: u.id, label: `${u.name} (${u.email})` }))}
            placeholder="Sélectionner un utilisateur"
            icon={<User size={14} />}
          />
        )}

        {activeFilterLabel && (
          <span className="text-xs text-indigo-400 bg-indigo-900/30 px-2.5 py-1 rounded-full border border-indigo-800 flex items-center gap-1.5">
            <Filter size={10} />
            {activeFilterLabel}
          </span>
        )}
      </div>

      {/* Empty state */}
      {isEmpty && (
        <div className="flex flex-col items-center justify-center py-24 text-gray-600">
          <Activity size={48} className="mb-4 opacity-30" />
          <p className="text-lg">Aucune donnée sur la période</p>
          <p className="text-sm mt-1">
            {filteredVkNames
              ? 'Aucune requête trouvée pour ce filtre'
              : 'Envoyez des requêtes via Pylos pour voir les métriques RUM'}
          </p>
        </div>
      )}

      {!isEmpty && (
        <>
          {/* Section 1 : Optimisation DeepSeek */}
          <section>
            <h2 className="text-sm font-semibold text-gray-400 uppercase tracking-wider mb-4 flex items-center gap-2">
              <Zap size={14} className="text-indigo-400" />
              Optimisation DeepSeek
            </h2>
            <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
              {isLoading ? (
                Array.from({ length: 4 }).map((_, i) => <Skeleton key={i} className="h-28" />)
              ) : (
                <>
                  <RumCard
                    label="Requêtes DeepSeek"
                    value={`${deepseekStats.pct.toFixed(1)}%`}
                    sub={`${formatNumber(deepseekStats.requests)} / ${formatNumber(filteredLogs.length)} ${
                      filteredVkNames ? 'filtrées' : 'total'
                    }`}
                    icon={<Zap size={14} />}
                    accent={deepseekStats.pct >= 30 ? 'green' : 'yellow'}
                  />
                  <RumCard
                    label="Coût DeepSeek"
                    value={formatCost(deepseekStats.totalCost)}
                    sub={`${formatNumber(deepseekStats.promptTokens + deepseekStats.completionTokens)} tokens`}
                    icon={<DollarSign size={14} />}
                    accent="indigo"
                  />
                  <RumCard
                    label="Économie vs GPT-4o"
                    value={formatCost(savings)}
                    sub={savings > 0 ? `${((savings / (savings + deepseekStats.totalCost)) * 100).toFixed(0)}% moins cher` : 'Aucun trafic DeepSeek'}
                    icon={<TrendingUp size={14} />}
                    accent={savings > 0 ? 'green' : 'gray'}
                  />
                  <RumCard
                    label="Cache Hits"
                    value={formatNumber(cacheHits)}
                    sub={filteredLogs.length > 0 ? `${((cacheHits / filteredLogs.length) * 100).toFixed(1)}% des requêtes` : '—'}
                    icon={<Shield size={14} />}
                    accent={cacheHits > 0 ? 'green' : 'gray'}
                  />
                </>
              )}
            </div>
          </section>

          {/* Section 2 : Coût par provider */}
          <section>
            <div className="rounded-xl border border-gray-800 bg-gray-900 p-5">
              <h2 className="text-sm font-semibold text-gray-300 mb-5 flex items-center gap-2">
                <DollarSign size={14} className="text-yellow-400" />
                Coût total par provider
              </h2>
              {isLoading ? (
                <Skeleton className="h-52" />
              ) : providerStats.length > 0 ? (
                <ResponsiveContainer width="100%" height={200}>
                  <BarChart data={providerStats} barSize={32}>
                    <XAxis
                      dataKey="provider"
                      tick={{ fill: '#9ca3af', fontSize: 11 }}
                      axisLine={false}
                      tickLine={false}
                    />
                    <YAxis
                      tickFormatter={v => formatCost(v)}
                      tick={{ fill: '#6b7280', fontSize: 10 }}
                      axisLine={false}
                      tickLine={false}
                    />
                    <Tooltip
                      contentStyle={{ background: '#111827', border: '1px solid #374151', borderRadius: 8 }}
                      labelStyle={{ color: '#e5e7eb' }}
                      formatter={(v) => [formatCost(Number(v ?? 0)), 'Coût']}
                    />
                    <Bar dataKey="totalCost" radius={[4, 4, 0, 0]}>
                      {providerStats.map((entry) => (
                        <Cell key={entry.provider} fill={providerColor(entry.provider)} />
                      ))}
                    </Bar>
                  </BarChart>
                </ResponsiveContainer>
              ) : (
                <div className="h-52 flex items-center justify-center text-gray-600 text-sm">
                  Aucune donnée
                </div>
              )}
            </div>
          </section>

          {/* Section 3 : Table performance par provider */}
          <section>
            <h2 className="text-sm font-semibold text-gray-400 uppercase tracking-wider mb-4 flex items-center gap-2">
              <Activity size={14} className="text-blue-400" />
              Performance par provider
            </h2>
            <div className="rounded-xl border border-gray-800 bg-gray-900 overflow-hidden">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-gray-800">
                    {['Provider', 'Requêtes', 'Latence moy.', 'Tokens moy.', 'Coût total', '$/1k tokens', 'Erreurs'].map(h => (
                      <th key={h} className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                        {h}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {isLoading ? (
                    Array.from({ length: 3 }).map((_, i) => (
                      <tr key={i} className="border-b border-gray-800/50">
                        {Array.from({ length: 7 }).map((_, j) => (
                          <td key={j} className="px-4 py-3">
                            <Skeleton className="h-4 w-20" />
                          </td>
                        ))}
                      </tr>
                    ))
                  ) : (
                    providerStats.map(s => {
                      const isDs = s.provider.toLowerCase().includes('deepseek')
                      return (
                        <tr
                          key={s.provider}
                          className={`border-b border-gray-800/50 hover:bg-gray-800/50 transition-colors ${
                            isDs ? 'bg-indigo-950/30' : ''
                          }`}
                        >
                          <td className="px-4 py-3">
                            <div className="flex items-center gap-2">
                              <div
                                className="w-2 h-2 rounded-full flex-shrink-0"
                                style={{ backgroundColor: providerColor(s.provider) }}
                              />
                              <span className={`font-medium ${isDs ? 'text-indigo-300' : 'text-white'}`}>
                                {s.provider}
                              </span>
                              {isDs && (
                                <span className="text-xs bg-indigo-900/60 text-indigo-300 px-1.5 py-0.5 rounded border border-indigo-800">
                                  DeepSeek
                                </span>
                              )}
                            </div>
                          </td>
                          <td className="px-4 py-3 text-gray-300 tabular-nums">{formatNumber(s.requests)}</td>
                          <td className="px-4 py-3 text-gray-300 tabular-nums">{formatMs(s.avgLatency)}</td>
                          <td className="px-4 py-3 text-gray-300 tabular-nums">{formatNumber(s.avgTokens)}</td>
                          <td className="px-4 py-3 text-gray-300 tabular-nums">{formatCost(s.totalCost)}</td>
                          <td className="px-4 py-3 text-gray-300 tabular-nums">{formatCost(s.costPer1kTokens)}</td>
                          <td className="px-4 py-3">
                            <span className={`text-xs px-2 py-0.5 rounded-full ${
                              s.errorRate === 0
                                ? 'bg-green-900/40 text-green-400'
                                : s.errorRate < 5
                                  ? 'bg-yellow-900/40 text-yellow-400'
                                  : 'bg-red-900/40 text-red-400'
                            }`}>
                              {s.errorRate.toFixed(1)}%
                            </span>
                          </td>
                        </tr>
                      )
                    })
                  )}
                </tbody>
              </table>
            </div>
          </section>

          {/* Section 4 : Token efficiency */}
          <section>
            <h2 className="text-sm font-semibold text-gray-400 uppercase tracking-wider mb-4 flex items-center gap-2">
              <Zap size={14} className="text-purple-400" />
              Efficacité tokens — ratio prompt / completion
            </h2>
            <div className="rounded-xl border border-gray-800 bg-gray-900 p-5 space-y-4">
              {isLoading ? (
                <Skeleton className="h-40" />
              ) : (
                providerStats.map(s => {
                  const total = s.promptTokens + s.completionTokens
                  if (total === 0) return null
                  const promptPct = (s.promptTokens / total) * 100
                  const completionPct = 100 - promptPct
                  return (
                    <div key={s.provider}>
                      <div className="flex justify-between text-xs text-gray-500 mb-1.5">
                        <span className="font-medium text-gray-300">{s.provider}</span>
                        <span>
                          <span className="text-blue-400">{formatNumber(s.promptTokens)} prompt</span>
                          {' / '}
                          <span className="text-purple-400">{formatNumber(s.completionTokens)} completion</span>
                        </span>
                      </div>
                      <div className="flex h-2 rounded-full overflow-hidden bg-gray-800">
                        <div
                          className="bg-blue-600 transition-all duration-500"
                          style={{ width: `${promptPct}%` }}
                        />
                        <div
                          className="bg-purple-600 transition-all duration-500"
                          style={{ width: `${completionPct}%` }}
                        />
                      </div>
                      <div className="flex justify-between text-xs text-gray-600 mt-0.5">
                        <span>{promptPct.toFixed(0)}% prompt</span>
                        <span>{completionPct.toFixed(0)}% completion</span>
                      </div>
                    </div>
                  )
                })
              )}
            </div>
          </section>

          {/* Section 5 : Santé du gateway */}
          <section>
            <h2 className="text-sm font-semibold text-gray-400 uppercase tracking-wider mb-4 flex items-center gap-2">
              <Shield size={14} className="text-green-400" />
              Santé du gateway
            </h2>
            <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
              {isLoading ? (
                Array.from({ length: 4 }).map((_, i) => <Skeleton key={i} className="h-28" />)
              ) : (
                <>
                  <RumCard
                    label="Uptime (success rate)"
                    value={`${successRate.toFixed(1)}%`}
                    sub="Basé sur le statut des requêtes"
                    icon={<Activity size={14} />}
                    accent={successRate >= 99 ? 'green' : successRate >= 95 ? 'yellow' : 'red'}
                  />
                  <RumCard
                    label="Latence P50"
                    value={formatMs(p50)}
                    sub="Médiane"
                    icon={<Clock size={14} />}
                    accent={p50 < 500 ? 'green' : p50 < 2000 ? 'yellow' : 'red'}
                  />
                  <RumCard
                    label="Latence P95"
                    value={formatMs(p95)}
                    sub="95e percentile"
                    icon={<Clock size={14} />}
                    accent={p95 < 2000 ? 'green' : p95 < 5000 ? 'yellow' : 'red'}
                  />
                  <RumCard
                    label="Requêtes analysées"
                    value={formatNumber(filteredLogs.length)}
                    sub={`Sur ${period}${filteredVkNames ? ' · filtrées' : ' · limit 2000'}`}
                    icon={<AlertTriangle size={14} />}
                    accent="blue"
                  />
                </>
              )}
            </div>
          </section>

          {/* Footer badge */}
          <div className="flex items-center justify-center pt-4 pb-2">
            <span className="text-xs text-gray-600 flex items-center gap-1.5">
              <Zap size={10} className="text-indigo-500" />
              Page générée par DeepSeek via Pylos · Coût ~$0.005
            </span>
          </div>
        </>
      )}
    </div>
  )
}
