import { useState, useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import { logsApi } from '../lib/api'
import {
  BarChart, Bar, XAxis, YAxis, Tooltip, ResponsiveContainer, Cell, CartesianGrid,
} from 'recharts'
import { DollarSign, TrendingUp, Coins, Layers } from 'lucide-react'

// ─── Constants ────────────────────────────────────────────────────────────────

const PERIODS = ['1h', '6h', '24h', '7d', '30d'] as const
type Period = typeof PERIODS[number]

const PROVIDER_COLORS: Record<string, string> = {
  deepseek: '#6366f1',
  ollama: '#10b981',
  openrouter: '#f59e0b',
  graphon: '#8b5cf6',
  lemonade: '#ec4899',
}

const MODEL_COLORS = ['#6366f1', '#f59e0b', '#10b981', '#ec4899', '#8b5cf6', '#3b82f6', '#ef4444', '#14b8a6']

function providerColor(p: string): string {
  const key = Object.keys(PROVIDER_COLORS).find(k => p.toLowerCase().includes(k))
  return key ? PROVIDER_COLORS[key] : '#6b7280'
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

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

function shortTimestamp(ts: number): string {
  const d = new Date(ts * 1000)
  return `${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')}`
}

function shortDate(ts: number): string {
  const d = new Date(ts * 1000)
  return `${d.getMonth() + 1}/${d.getDate()} ${d.getHours().toString().padStart(2, '0')}:${d.getMinutes().toString().padStart(2, '0')}`
}

function bucketSecondsForPeriod(period: Period): string {
  switch (period) {
    case '1h': return '300'
    case '6h': return '900'
    case '24h': return '3600'
    case '7d': return '14400'
    case '30d': return '86400'
  }
}

// ─── Types ────────────────────────────────────────────────────────────────────

interface ModelCostEntry {
  model: string
  provider: string
  requests: number
  totalCost: number
  totalTokens: number
  avgCost: number
}

interface VkCostEntry {
  virtual_key: string
  requests: number
  totalCost: number
  totalTokens: number
}

// ─── Skeleton ─────────────────────────────────────────────────────────────────

function Skeleton({ className }: { className?: string }) {
  return <div className={`animate-pulse bg-gray-800 rounded ${className ?? ''}`} />
}

// ─── KPI Card ─────────────────────────────────────────────────────────────────

function KpiCard({
  label, value, sub, icon, accent = 'blue',
}: {
  label: string
  value: string
  sub?: string
  icon: React.ReactNode
  accent?: 'blue' | 'green' | 'yellow' | 'red' | 'indigo' | 'purple'
}) {
  const colors = {
    blue: 'text-blue-400 bg-blue-900/30',
    green: 'text-green-400 bg-green-900/30',
    yellow: 'text-yellow-400 bg-yellow-900/30',
    red: 'text-red-400 bg-red-900/30',
    indigo: 'text-indigo-400 bg-indigo-900/30',
    purple: 'text-purple-400 bg-purple-900/30',
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

// ─── Main page ────────────────────────────────────────────────────────────────

export default function CostTracking() {
  const [period, setPeriod] = useState<Period>('24h')

  const logsQ = useQuery({
    queryKey: ['cost-logs', period],
    queryFn: () => logsApi.getLogs({ period, limit: 2000 }),
    refetchInterval: 60_000,
  })

  const statsQ = useQuery({
    queryKey: ['cost-stats', period],
    queryFn: () => logsApi.getStats({ period }),
    refetchInterval: 60_000,
  })

  const histQ = useQuery({
    queryKey: ['cost-histogram', period],
    queryFn: () => logsApi.getTokenHistogram({ period, bucket_seconds: bucketSecondsForPeriod(period) }),
    refetchInterval: 60_000,
  })

  const logs = logsQ.data?.logs ?? []
  const stats = statsQ.data

  // ── Derived data ──────────────────────────────────────────────────────────

  const {
    totalCost, avgCostPerRequest, totalTokens, modelStats, vkStats, expensiveRequests,
    costByProvider, costOverTime, providerStats,
  } = useMemo(() => {
    let totalCost = 0
    let totalTokens = 0

    for (const log of logs) {
      totalCost += log.cost_usd
      totalTokens += log.total_tokens
    }

    const avgCostPerRequest = logs.length > 0 ? totalCost / logs.length : 0

    // Model breakdown
    const modelMap = new Map<string, ModelCostEntry>()
    for (const log of logs) {
      const key = `${log.provider}::${log.model}`
      if (!modelMap.has(key)) {
        modelMap.set(key, {
          model: log.model,
          provider: log.provider,
          requests: 0,
          totalCost: 0,
          totalTokens: 0,
          avgCost: 0,
        })
      }
      const s = modelMap.get(key)!
      s.requests++
      s.totalCost += log.cost_usd
      s.totalTokens += log.total_tokens
    }
    const modelStats = Array.from(modelMap.values())
      .map(s => ({ ...s, avgCost: s.requests > 0 ? s.totalCost / s.requests : 0 }))
      .sort((a, b) => b.totalCost - a.totalCost)

    // Virtual key breakdown
    const vkMap = new Map<string, VkCostEntry>()
    for (const log of logs) {
      const vk = log.virtual_key || 'default'
      if (!vkMap.has(vk)) {
        vkMap.set(vk, { virtual_key: vk, requests: 0, totalCost: 0, totalTokens: 0 })
      }
      const s = vkMap.get(vk)!
      s.requests++
      s.totalCost += log.cost_usd
      s.totalTokens += log.total_tokens
    }
    const vkStats = Array.from(vkMap.values()).sort((a, b) => b.totalCost - a.totalCost)

    // Cost by provider (for pie-style summary)
    const providerMap = new Map<string, number>()
    for (const log of logs) {
      const p = log.provider || 'unknown'
      providerMap.set(p, (providerMap.get(p) || 0) + log.cost_usd)
    }
    const costByProvider = Array.from(providerMap.entries())
      .map(([provider, cost]) => ({ provider, cost }))
      .sort((a, b) => b.cost - a.cost)

    // Provider stats for bar chart
    const pStatsMap = new Map<string, { provider: string; requests: number; totalCost: number }>()
    for (const log of logs) {
      const p = log.provider || 'unknown'
      if (!pStatsMap.has(p)) {
        pStatsMap.set(p, { provider: p, requests: 0, totalCost: 0 })
      }
      const s = pStatsMap.get(p)!
      s.requests++
      s.totalCost += log.cost_usd
    }
    const providerStats = Array.from(pStatsMap.values()).sort((a, b) => b.totalCost - a.totalCost)

    // Top 10 most expensive requests
    const expensiveRequests = [...logs]
      .sort((a, b) => b.cost_usd - a.cost_usd)
      .slice(0, 10)
      .map(l => ({
        id: l.id,
        timestamp: l.timestamp,
        provider: l.provider,
        model: l.model,
        cost_usd: l.cost_usd,
        total_tokens: l.total_tokens,
        latency_ms: l.latency_ms,
        status: l.status,
      }))

    // Cost over time (from histogram token data)
    const costOverTime = (histQ.data?.buckets ?? []).map(b => ({
      timestamp: b.timestamp,
      cost: (b.total_tokens / 1_000_000) * 2.5,
      tokens: b.total_tokens,
      label: period === '30d' || period === '7d'
        ? shortDate(b.timestamp)
        : shortTimestamp(b.timestamp),
    }))

    return {
      totalCost, avgCostPerRequest, totalTokens,
      modelStats, vkStats, expensiveRequests,
      costByProvider, costOverTime, providerStats,
    }
  }, [logs, histQ.data, period])

  const isLoading = logsQ.isLoading || statsQ.isLoading
  const isEmpty = !isLoading && logs.length === 0

  return (
    <div className="p-6 space-y-8 overflow-y-auto h-full">

      {/* Header */}
      <div className="flex items-center justify-between flex-wrap gap-3">
        <div>
          <h1 className="text-2xl font-bold text-white flex items-center gap-2">
            <DollarSign size={22} className="text-green-400" />
            Cost Tracking
          </h1>
          <p className="text-sm text-gray-400 mt-1">
            Cost analytics and usage breakdown
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
                    ? 'bg-green-600 text-white'
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

      {/* Empty state */}
      {isEmpty && (
        <div className="flex flex-col items-center justify-center py-24 text-gray-600">
          <DollarSign size={48} className="mb-4 opacity-30" />
          <p className="text-lg">No cost data for this period</p>
          <p className="text-sm mt-1">Send requests via Pylos to see cost metrics</p>
        </div>
      )}

      {!isEmpty && (
        <>
          {/* ── KPI Cards ───────────────────────────────────────────────────── */}
          <section>
            <h2 className="text-sm font-semibold text-gray-400 uppercase tracking-wider mb-4 flex items-center gap-2">
              <Layers size={14} className="text-green-400" />
              Overview
            </h2>
            <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
              {isLoading ? (
                Array.from({ length: 4 }).map((_, i) => <Skeleton key={i} className="h-28" />)
              ) : (
                <>
                  <KpiCard
                    label="Total Cost"
                    value={formatCost(totalCost)}
                    sub={`${logs.length} requests`}
                    icon={<DollarSign size={14} />}
                    accent="green"
                  />
                  <KpiCard
                    label="Avg Cost / Request"
                    value={formatCost(avgCostPerRequest)}
                    sub="Across all providers"
                    icon={<TrendingUp size={14} />}
                    accent="blue"
                  />
                  <KpiCard
                    label="Total Tokens"
                    value={formatNumber(totalTokens)}
                    sub={`${formatNumber(stats?.total_prompt_tokens ?? 0)} prompt / ${formatNumber(stats?.total_completion_tokens ?? 0)} completion`}
                    icon={<Coins size={14} />}
                    accent="purple"
                  />
                  <KpiCard
                    label="Providers Used"
                    value={`${costByProvider.length}`}
                    sub={costByProvider.map(c => c.provider).join(', ') || '—'}
                    icon={<Layers size={14} />}
                    accent="indigo"
                  />
                </>
              )}
            </div>
          </section>

          {/* ── Cost Over Time Chart ────────────────────────────────────────── */}
          <section>
            <div className="rounded-xl border border-gray-800 bg-gray-900 p-5">
              <h2 className="text-sm font-semibold text-gray-300 mb-5 flex items-center gap-2">
                <TrendingUp size={14} className="text-green-400" />
                Cost Over Time
              </h2>
              {isLoading ? (
                <Skeleton className="h-52" />
              ) : costOverTime.length > 0 ? (
                <ResponsiveContainer width="100%" height={240}>
                  <BarChart data={costOverTime} barSize={period === '30d' ? 16 : 24}>
                    <CartesianGrid strokeDasharray="3 3" stroke="#1f2937" vertical={false} />
                    <XAxis
                      dataKey="label"
                      tick={{ fill: '#6b7280', fontSize: 10 }}
                      axisLine={false}
                      tickLine={false}
                      interval="preserveStartEnd"
                    />
                    <YAxis
                      tickFormatter={v => formatCost(v)}
                      tick={{ fill: '#6b7280', fontSize: 10 }}
                      axisLine={false}
                      tickLine={false}
                      width={70}
                    />
                    <Tooltip
                      contentStyle={{ background: '#111827', border: '1px solid #374151', borderRadius: 8 }}
                      labelStyle={{ color: '#e5e7eb' }}
                      formatter={(v, name) => [name === 'cost' ? formatCost(Number(v)) : formatNumber(Number(v)), name === 'cost' ? 'Cost' : 'Tokens']}
                    />
                    <Bar dataKey="cost" radius={[4, 4, 0, 0]} fill="#22c55e" />
                  </BarChart>
                </ResponsiveContainer>
              ) : (
                <div className="h-52 flex items-center justify-center text-gray-600 text-sm">
                  No data
                </div>
              )}
            </div>
          </section>

          {/* ── Cost by Provider (bar chart + summary) ──────────────────────── */}
          <section>
            <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
              <div className="lg:col-span-2 rounded-xl border border-gray-800 bg-gray-900 p-5">
                <h2 className="text-sm font-semibold text-gray-300 mb-5 flex items-center gap-2">
                  <DollarSign size={14} className="text-yellow-400" />
                  Cost by Provider
                </h2>
                {isLoading ? (
                  <Skeleton className="h-44" />
                ) : providerStats.length > 0 ? (
                  <ResponsiveContainer width="100%" height={180}>
                    <BarChart data={providerStats} barSize={36} layout="vertical">
                      <CartesianGrid strokeDasharray="3 3" stroke="#1f2937" horizontal={false} />
                      <XAxis
                        type="number"
                        tickFormatter={v => formatCost(v)}
                        tick={{ fill: '#6b7280', fontSize: 10 }}
                        axisLine={false}
                        tickLine={false}
                      />
                      <YAxis
                        type="category"
                        dataKey="provider"
                        tick={{ fill: '#9ca3af', fontSize: 11 }}
                        axisLine={false}
                        tickLine={false}
                        width={100}
                      />
                      <Tooltip
                        contentStyle={{ background: '#111827', border: '1px solid #374151', borderRadius: 8 }}
                        labelStyle={{ color: '#e5e7eb' }}
                        formatter={(v) => [formatCost(Number(v ?? 0)), 'Cost']}
                      />
                      <Bar dataKey="totalCost" radius={[0, 4, 4, 0]}>
                        {providerStats.map((entry) => (
                          <Cell key={entry.provider} fill={providerColor(entry.provider)} />
                        ))}
                      </Bar>
                    </BarChart>
                  </ResponsiveContainer>
                ) : (
                  <div className="h-44 flex items-center justify-center text-gray-600 text-sm">
                    No data
                  </div>
                )}
              </div>

              <div className="rounded-xl border border-gray-800 bg-gray-900 p-5">
                <h2 className="text-sm font-semibold text-gray-300 mb-5">Breakdown</h2>
                {isLoading ? (
                  <Skeleton className="h-44" />
                ) : costByProvider.length > 0 ? (
                  <div className="space-y-3">
                    {costByProvider.map((entry) => {
                      const pct = totalCost > 0 ? (entry.cost / totalCost) * 100 : 0
                      return (
                        <div key={entry.provider}>
                          <div className="flex justify-between items-center text-xs mb-1.5">
                            <div className="flex items-center gap-2">
                              <div
                                className="w-2 h-2 rounded-full flex-shrink-0"
                                style={{ backgroundColor: providerColor(entry.provider) }}
                              />
                              <span className="text-gray-300 font-medium">{entry.provider}</span>
                            </div>
                            <span className="text-gray-400 tabular-nums">{formatCost(entry.cost)}</span>
                          </div>
                          <div className="flex h-2 rounded-full overflow-hidden bg-gray-800">
                            <div
                              className="transition-all duration-500 rounded-full"
                              style={{ width: `${pct}%`, backgroundColor: providerColor(entry.provider) }}
                            />
                          </div>
                          <div className="text-xs text-gray-600 mt-0.5">{pct.toFixed(1)}%</div>
                        </div>
                      )
                    })}
                  </div>
                ) : (
                  <div className="h-44 flex items-center justify-center text-gray-600 text-sm">
                    No data
                  </div>
                )}
              </div>
            </div>
          </section>

          {/* ── Cost by Model Table ─────────────────────────────────────────── */}
          <section>
            <h2 className="text-sm font-semibold text-gray-400 uppercase tracking-wider mb-4 flex items-center gap-2">
              <DollarSign size={14} className="text-indigo-400" />
              Cost by Model
            </h2>
            <div className="rounded-xl border border-gray-800 bg-gray-900 overflow-hidden">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-gray-800">
                    {['Model', 'Provider', 'Requests', 'Total Cost', 'Avg Cost', 'Total Tokens'].map(h => (
                      <th key={h} className="px-4 py-3 text-left text-xs font-medium text-gray-500 uppercase tracking-wider">
                        {h}
                      </th>
                    ))}
                  </tr>
                </thead>
                <tbody>
                  {isLoading ? (
                    Array.from({ length: 4 }).map((_, i) => (
                      <tr key={i} className="border-b border-gray-800/50">
                        {Array.from({ length: 6 }).map((_, j) => (
                          <td key={j} className="px-4 py-3">
                            <Skeleton className="h-4 w-20" />
                          </td>
                        ))}
                      </tr>
                    ))
                  ) : modelStats.length > 0 ? (
                    modelStats.map((s, i) => (
                      <tr
                        key={`${s.provider}::${s.model}`}
                        className="border-b border-gray-800/50 hover:bg-gray-800/50 transition-colors"
                      >
                        <td className="px-4 py-3">
                          <div className="flex items-center gap-2">
                            <div
                              className="w-2 h-2 rounded-full flex-shrink-0"
                              style={{ backgroundColor: MODEL_COLORS[i % MODEL_COLORS.length] }}
                            />
                            <span className="text-white font-medium truncate max-w-[260px]">{s.model}</span>
                          </div>
                        </td>
                        <td className="px-4 py-3 text-gray-300">{s.provider}</td>
                        <td className="px-4 py-3 text-gray-300 tabular-nums">{formatNumber(s.requests)}</td>
                        <td className="px-4 py-3 text-gray-300 tabular-nums">{formatCost(s.totalCost)}</td>
                        <td className="px-4 py-3 text-gray-300 tabular-nums">{formatCost(s.avgCost)}</td>
                        <td className="px-4 py-3 text-gray-300 tabular-nums">{formatNumber(s.totalTokens)}</td>
                      </tr>
                    ))
                  ) : (
                    <tr>
                      <td colSpan={6} className="px-4 py-8 text-center text-gray-600">No data</td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          </section>

          {/* ── Cost by Virtual Key Table ───────────────────────────────────── */}
          <section>
            <h2 className="text-sm font-semibold text-gray-400 uppercase tracking-wider mb-4 flex items-center gap-2">
              <Coins size={14} className="text-pink-400" />
              Cost by Virtual Key
            </h2>
            <div className="rounded-xl border border-gray-800 bg-gray-900 overflow-hidden">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-gray-800">
                    {['Virtual Key', 'Requests', 'Total Cost', 'Total Tokens'].map(h => (
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
                        {Array.from({ length: 4 }).map((_, j) => (
                          <td key={j} className="px-4 py-3">
                            <Skeleton className="h-4 w-20" />
                          </td>
                        ))}
                      </tr>
                    ))
                  ) : vkStats.length > 0 ? (
                    vkStats.map(s => {
                      const pct = totalCost > 0 ? (s.totalCost / totalCost) * 100 : 0
                      return (
                        <tr
                          key={s.virtual_key}
                          className="border-b border-gray-800/50 hover:bg-gray-800/50 transition-colors"
                        >
                          <td className="px-4 py-3">
                            <div className="flex items-center gap-2">
                              <span className="text-white font-medium truncate max-w-[320px]">
                                {s.virtual_key}
                              </span>
                              <span className="text-xs text-gray-600">{pct.toFixed(1)}%</span>
                            </div>
                          </td>
                          <td className="px-4 py-3 text-gray-300 tabular-nums">{formatNumber(s.requests)}</td>
                          <td className="px-4 py-3 text-gray-300 tabular-nums">{formatCost(s.totalCost)}</td>
                          <td className="px-4 py-3 text-gray-300 tabular-nums">{formatNumber(s.totalTokens)}</td>
                        </tr>
                      )
                    })
                  ) : (
                    <tr>
                      <td colSpan={4} className="px-4 py-8 text-center text-gray-600">No data</td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          </section>

          {/* ── Top 10 Most Expensive Requests ─────────────────────────────── */}
          <section>
            <h2 className="text-sm font-semibold text-gray-400 uppercase tracking-wider mb-4 flex items-center gap-2">
              <TrendingUp size={14} className="text-red-400" />
              Top 10 Most Expensive Requests
            </h2>
            <div className="rounded-xl border border-gray-800 bg-gray-900 overflow-hidden">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-gray-800">
                    {['Time', 'Provider', 'Model', 'Cost', 'Tokens', 'Latency', 'Status'].map(h => (
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
                        {Array.from({ length: 7 }).map((_, j) => (
                          <td key={j} className="px-4 py-3">
                            <Skeleton className="h-4 w-16" />
                          </td>
                        ))}
                      </tr>
                    ))
                  ) : expensiveRequests.length > 0 ? (
                    expensiveRequests.map((r, i) => (
                      <tr
                        key={r.id}
                        className="border-b border-gray-800/50 hover:bg-gray-800/50 transition-colors"
                      >
                        <td className="px-4 py-3 text-gray-400 tabular-nums text-xs">
                          {new Date(r.timestamp * 1000).toLocaleString()}
                        </td>
                        <td className="px-4 py-3">
                          <div className="flex items-center gap-2">
                            <div
                              className="w-2 h-2 rounded-full flex-shrink-0"
                              style={{ backgroundColor: providerColor(r.provider) }}
                            />
                            <span className="text-gray-300">{r.provider}</span>
                          </div>
                        </td>
                        <td className="px-4 py-3 text-gray-300 truncate max-w-[200px]">{r.model}</td>
                        <td className="px-4 py-3">
                          <span className={`font-medium tabular-nums ${
                            i === 0 ? 'text-red-400' : i < 3 ? 'text-orange-400' : 'text-gray-300'
                          }`}>
                            {formatCost(r.cost_usd)}
                          </span>
                        </td>
                        <td className="px-4 py-3 text-gray-300 tabular-nums">{formatNumber(r.total_tokens)}</td>
                        <td className="px-4 py-3 text-gray-300 tabular-nums">
                          {r.latency_ms >= 1000 ? `${(r.latency_ms / 1000).toFixed(1)}s` : `${r.latency_ms.toFixed(0)}ms`}
                        </td>
                        <td className="px-4 py-3">
                          <span className={`text-xs px-2 py-0.5 rounded-full ${
                            r.status === 'success'
                              ? 'bg-green-900/40 text-green-400'
                              : 'bg-red-900/40 text-red-400'
                          }`}>
                            {r.status}
                          </span>
                        </td>
                      </tr>
                    ))
                  ) : (
                    <tr>
                      <td colSpan={7} className="px-4 py-8 text-center text-gray-600">No data</td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          </section>
        </>
      )}
    </div>
  )
}
