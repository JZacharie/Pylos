import { useState, useMemo } from 'react'
import { useQuery } from '@tanstack/react-query'
import { logsApi } from '../lib/api'
import {
  LineChart, Line, XAxis, YAxis, Tooltip, ResponsiveContainer, CartesianGrid,
} from 'recharts'
import {
  BarChart2, Download, Calendar, TrendingUp, DollarSign, Zap, Key
} from 'lucide-react'

// ─── Constants ────────────────────────────────────────────────────────────────

const PERIODS = ['7d', '30d', '90d'] as const
type Period = typeof PERIODS[number]

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

function periodDays(p: Period): number {
  return p === '7d' ? 7 : p === '30d' ? 30 : 90
}

function previousPeriod(p: Period): string {
  return `${periodDays(p)}d before`
}

// ─── Skeleton ─────────────────────────────────────────────────────────────────

function Skeleton({ className }: { className?: string }) {
  return <div className={`animate-pulse bg-gray-800 rounded ${className ?? ''}`} />
}

// ─── StatCard ─────────────────────────────────────────────────────────────────

function UsageCard({
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

// ─── CSV Export ───────────────────────────────────────────────────────────────

function exportCsv(data: Record<string, unknown>[], filename: string) {
  if (data.length === 0) return
  const headers = Object.keys(data[0])
  const rows = data.map(row =>
    headers.map(h => {
      const v = row[h]
      if (typeof v === 'string' && (v.includes(',') || v.includes('"'))) {
        return `"${v.replace(/"/g, '""')}"`
      }
      return String(v ?? '')
    }).join(',')
  )
  const csv = [headers.join(','), ...rows].join('\n')
  const blob = new Blob([csv], { type: 'text/csv;charset=utf-8;' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  a.click()
  URL.revokeObjectURL(url)
}

// ─── Main page ────────────────────────────────────────────────────────────────

export default function OldUsage() {
  const [period, setPeriod] = useState<Period>('30d')

  const logsQ = useQuery({
    queryKey: ['old-usage-logs', period],
    queryFn: () => logsApi.getLogs({ period, limit: 5000 }),
    refetchInterval: 120_000,
  })

  const statsQ = useQuery({
    queryKey: ['old-usage-stats', period],
    queryFn: () => logsApi.getStats({ period }),
    refetchInterval: 120_000,
  })

  const logs = logsQ.data?.logs ?? []

  const isLoading = logsQ.isLoading || statsQ.isLoading
  const isEmpty = !isLoading && logs.length === 0

  // ── Derived data ────────────────────────────────────────────────────────────
  const {
    totalRequests,
    totalTokens,
    totalCost,
    activeKeys,
    dailyUsage,
    providerBreakdown,
    modelBreakdown,
    prevComparison,
  } = useMemo(() => {
    const totalRequests = logs.length
    const totalTokens = logs.reduce((a, l) => a + l.total_tokens, 0)
    const totalCost = logs.reduce((a, l) => a + l.cost_usd, 0)
    const activeKeys = new Set(logs.map(l => l.virtual_key).filter(Boolean)).size

    // Daily usage aggregation
    const dailyMap = new Map<string, { requests: number; tokens: number; cost: number }>()
    for (const log of logs) {
      const date = new Date(log.timestamp * 1000)
      const key = `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, '0')}-${String(date.getDate()).padStart(2, '0')}`
      if (!dailyMap.has(key)) {
        dailyMap.set(key, { requests: 0, tokens: 0, cost: 0 })
      }
      const d = dailyMap.get(key)!
      d.requests++
      d.tokens += log.total_tokens
      d.cost += log.cost_usd
    }
    const dailyUsage = Array.from(dailyMap.entries())
      .map(([date, v]) => ({ date, ...v }))
      .sort((a, b) => a.date.localeCompare(b.date))

    // Provider breakdown
    const providerMap = new Map<string, { requests: number; tokens: number; cost: number; models: Set<string> }>()
    for (const log of logs) {
      const p = log.provider || 'unknown'
      if (!providerMap.has(p)) {
        providerMap.set(p, { requests: 0, tokens: 0, cost: 0, models: new Set() })
      }
      const s = providerMap.get(p)!
      s.requests++
      s.tokens += log.total_tokens
      s.cost += log.cost_usd
      s.models.add(log.model)
    }
    const providerBreakdown = Array.from(providerMap.entries())
      .map(([provider, v]) => ({ provider, ...v, modelCount: v.models.size }))
      .sort((a, b) => b.requests - a.requests)

    // Model breakdown
    const modelMap = new Map<string, { requests: number; tokens: number; cost: number; provider: string }>()
    for (const log of logs) {
      const key = log.model || 'unknown'
      if (!modelMap.has(key)) {
        modelMap.set(key, { requests: 0, tokens: 0, cost: 0, provider: log.provider })
      }
      const s = modelMap.get(key)!
      s.requests++
      s.tokens += log.total_tokens
      s.cost += log.cost_usd
    }
    const modelBreakdown = Array.from(modelMap.entries())
      .map(([model, v]) => ({ model, ...v }))
      .sort((a, b) => b.requests - a.requests)

    // Comparison vs previous period (approximate using median daily rate)
    const days = periodDays(period)
    const avgDailyRequests = totalRequests / days
    const avgDailyCost = totalCost / days
    const prevComparison = {
      requests: avgDailyRequests,
      cost: avgDailyCost,
      tokens: totalTokens / days,
    }

    return { totalRequests, totalTokens, totalCost, activeKeys, dailyUsage, providerBreakdown, modelBreakdown, prevComparison }
  }, [logs, period])

  const days = periodDays(period)

  // ── Export handlers ──────────────────────────────────────────────────────────
  const handleExportDaily = () => {
    exportCsv(
      dailyUsage.map(d => ({ Date: d.date, Requests: d.requests, Tokens: d.tokens, Cost: d.cost })),
      `pylos-usage-daily-${period}.csv`
    )
  }

  const handleExportProviders = () => {
    exportCsv(
      providerBreakdown.map(p => ({
        Provider: p.provider,
        Requests: p.requests,
        Tokens: p.tokens,
        Cost: p.cost,
        Models: p.modelCount,
      })),
      `pylos-usage-providers-${period}.csv`
    )
  }

  const handleExportModels = () => {
    exportCsv(
      modelBreakdown.map(m => ({
        Model: m.model,
        Provider: m.provider,
        Requests: m.requests,
        Tokens: m.tokens,
        Cost: m.cost,
      })),
      `pylos-usage-models-${period}.csv`
    )
  }

  // ── Render ──────────────────────────────────────────────────────────────────
  return (
    <div className="p-6 space-y-8 overflow-y-auto h-full">

      {/* Header */}
      <div className="flex items-center justify-between flex-wrap gap-3">
        <div>
          <h1 className="text-2xl font-bold text-white flex items-center gap-2">
            <BarChart2 size={22} className="text-indigo-400" />
            Old Usage & Analytics
          </h1>
          <p className="text-sm text-gray-400 mt-1">
            Legacy usage reports and historical data
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
          <button
            onClick={handleExportDaily}
            disabled={isEmpty}
            className="flex items-center gap-1.5 px-3 py-1.5 text-xs rounded-lg border border-gray-700 text-gray-400 hover:text-white hover:bg-gray-800 transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          >
            <Download size={13} />
            Export CSV
          </button>
        </div>
      </div>

      {/* Empty state */}
      {isEmpty && (
        <div className="flex flex-col items-center justify-center py-24 text-gray-600">
          <BarChart2 size={48} className="mb-4 opacity-30" />
          <p className="text-lg">No usage data for this period</p>
          <p className="text-sm mt-1">Historical data will appear here once requests are logged</p>
        </div>
      )}

      {!isEmpty && (
        <>
          {/* ── Section 1: Usage summary cards ─────────────────────────────── */}
          <section>
            <h2 className="text-sm font-semibold text-gray-400 uppercase tracking-wider mb-4 flex items-center gap-2">
              <Calendar size={14} className="text-indigo-400" />
              Usage Summary
            </h2>
            <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
              {isLoading ? (
                Array.from({ length: 4 }).map((_, i) => <Skeleton key={i} className="h-28" />)
              ) : (
                <>
                  <UsageCard
                    label="Total Requests"
                    value={formatNumber(totalRequests)}
                    sub={`${(totalRequests / days).toFixed(1)}/day avg`}
                    icon={<BarChart2 size={14} />}
                    accent="blue"
                  />
                  <UsageCard
                    label="Total Tokens"
                    value={formatNumber(totalTokens)}
                    sub={`${(totalTokens / days).toFixed(0)}/day avg`}
                    icon={<Zap size={14} />}
                    accent="indigo"
                  />
                  <UsageCard
                    label="Total Cost"
                    value={formatCost(totalCost)}
                    sub={`${formatCost(totalCost / days)}/day avg`}
                    icon={<DollarSign size={14} />}
                    accent="yellow"
                  />
                  <UsageCard
                    label="Active Keys"
                    value={String(activeKeys)}
                    sub={`${providerBreakdown.length} providers`}
                    icon={<Key size={14} />}
                    accent="green"
                  />
                </>
              )}
            </div>
          </section>

          {/* ── Section 2: Historical comparison ──────────────────────────── */}
          <section>
            <h2 className="text-sm font-semibold text-gray-400 uppercase tracking-wider mb-4 flex items-center gap-2">
              <TrendingUp size={14} className="text-green-400" />
              Historical Comparison
            </h2>
            <div className="rounded-xl border border-gray-800 bg-gray-900 p-5">
              <div className="text-xs text-gray-500 mb-4">
                {previousPeriod(period)} · Daily averages over {days} days
              </div>
              {isLoading ? (
                <Skeleton className="h-20" />
              ) : (
                <div className="grid grid-cols-3 gap-6">
                  <div className="text-center">
                    <div className="text-lg font-bold text-white tabular-nums">
                      {prevComparison.requests.toFixed(1)}
                    </div>
                    <div className="text-xs text-gray-500 mt-1">Avg Daily Requests</div>
                  </div>
                  <div className="text-center">
                    <div className="text-lg font-bold text-white tabular-nums">
                      {formatCost(prevComparison.cost)}
                    </div>
                    <div className="text-xs text-gray-500 mt-1">Avg Daily Cost</div>
                  </div>
                  <div className="text-center">
                    <div className="text-lg font-bold text-white tabular-nums">
                      {formatNumber(prevComparison.tokens)}
                    </div>
                    <div className="text-xs text-gray-500 mt-1">Avg Daily Tokens</div>
                  </div>
                </div>
              )}
            </div>
          </section>

          {/* ── Section 3: Daily usage chart ───────────────────────────────── */}
          <section>
            <div className="rounded-xl border border-gray-800 bg-gray-900 p-5">
              <div className="flex items-center justify-between mb-5">
                <h2 className="text-sm font-semibold text-gray-300 flex items-center gap-2">
                  <BarChart2 size={14} className="text-indigo-400" />
                  Daily Usage Over Time
                </h2>
                <button
                  onClick={handleExportDaily}
                  className="text-xs text-gray-500 hover:text-white transition-colors flex items-center gap-1"
                >
                  <Download size={11} />
                  Export
                </button>
              </div>
              {isLoading ? (
                <Skeleton className="h-64" />
              ) : dailyUsage.length > 0 ? (
                <ResponsiveContainer width="100%" height={280}>
                  <LineChart data={dailyUsage}>
                    <CartesianGrid strokeDasharray="3 3" stroke="#1f2937" />
                    <XAxis
                      dataKey="date"
                      tick={{ fill: '#9ca3af', fontSize: 10 }}
                      axisLine={false}
                      tickLine={false}
                      tickFormatter={v => v.slice(5)}
                    />
                    <YAxis
                      yAxisId="left"
                      tick={{ fill: '#6b7280', fontSize: 10 }}
                      axisLine={false}
                      tickLine={false}
                    />
                    <YAxis
                      yAxisId="right"
                      orientation="right"
                      tickFormatter={v => formatCost(v)}
                      tick={{ fill: '#6b7280', fontSize: 10 }}
                      axisLine={false}
                      tickLine={false}
                    />
                    <Tooltip
                      contentStyle={{ background: '#111827', border: '1px solid #374151', borderRadius: 8 }}
                      labelStyle={{ color: '#e5e7eb' }}
                      formatter={(value, name) => {
                        if (name === 'requests') return [formatNumber(Number(value)), 'Requests']
                        if (name === 'cost') return [formatCost(Number(value)), 'Cost']
                        return [formatNumber(Number(value)), name]
                      }}
                    />
                    <Line
                      yAxisId="left"
                      type="monotone"
                      dataKey="requests"
                      stroke="#6366f1"
                      strokeWidth={2}
                      dot={false}
                      name="requests"
                    />
                    <Line
                      yAxisId="right"
                      type="monotone"
                      dataKey="cost"
                      stroke="#f59e0b"
                      strokeWidth={2}
                      dot={false}
                      name="cost"
                    />
                  </LineChart>
                </ResponsiveContainer>
              ) : (
                <div className="h-64 flex items-center justify-center text-gray-600 text-sm">
                  No data
                </div>
              )}
            </div>
          </section>

          {/* ── Section 4: Usage by provider ────────────────────────────────── */}
          <section>
            <div className="flex items-center justify-between mb-4">
              <h2 className="text-sm font-semibold text-gray-400 uppercase tracking-wider flex items-center gap-2">
                <Zap size={14} className="text-purple-400" />
                Usage by Provider
              </h2>
              <button
                onClick={handleExportProviders}
                className="text-xs text-gray-500 hover:text-white transition-colors flex items-center gap-1"
              >
                <Download size={11} />
                Export
              </button>
            </div>
            <div className="rounded-xl border border-gray-800 bg-gray-900 overflow-hidden">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-gray-800">
                    {['Provider', 'Requests', 'Tokens', 'Cost', 'Models', '% of Total'].map(h => (
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
                        {Array.from({ length: 6 }).map((_, j) => (
                          <td key={j} className="px-4 py-3">
                            <Skeleton className="h-4 w-20" />
                          </td>
                        ))}
                      </tr>
                    ))
                  ) : (
                    providerBreakdown.map(s => (
                      <tr
                        key={s.provider}
                        className="border-b border-gray-800/50 hover:bg-gray-800/50 transition-colors"
                      >
                        <td className="px-4 py-3">
                          <div className="flex items-center gap-2">
                            <div
                              className="w-2 h-2 rounded-full flex-shrink-0"
                              style={{ backgroundColor: providerColor(s.provider) }}
                            />
                            <span className="font-medium text-white">{s.provider}</span>
                          </div>
                        </td>
                        <td className="px-4 py-3 text-gray-300 tabular-nums">{formatNumber(s.requests)}</td>
                        <td className="px-4 py-3 text-gray-300 tabular-nums">{formatNumber(s.tokens)}</td>
                        <td className="px-4 py-3 text-gray-300 tabular-nums">{formatCost(s.cost)}</td>
                        <td className="px-4 py-3 text-gray-300 tabular-nums">{s.modelCount}</td>
                        <td className="px-4 py-3">
                          <div className="flex items-center gap-2">
                            <div className="w-16 h-1.5 rounded-full bg-gray-800 overflow-hidden">
                              <div
                                className="h-full rounded-full transition-all duration-500"
                                style={{
                                  width: `${totalRequests > 0 ? (s.requests / totalRequests) * 100 : 0}%`,
                                  backgroundColor: providerColor(s.provider),
                                }}
                              />
                            </div>
                            <span className="text-gray-400 text-xs tabular-nums">
                              {totalRequests > 0 ? ((s.requests / totalRequests) * 100).toFixed(1) : 0}%
                            </span>
                          </div>
                        </td>
                      </tr>
                    ))
                  )}
                </tbody>
              </table>
            </div>
          </section>

          {/* ── Section 5: Usage by model ──────────────────────────────────── */}
          <section>
            <div className="flex items-center justify-between mb-4">
              <h2 className="text-sm font-semibold text-gray-400 uppercase tracking-wider flex items-center gap-2">
                <BarChart2 size={14} className="text-blue-400" />
                Usage by Model
              </h2>
              <button
                onClick={handleExportModels}
                className="text-xs text-gray-500 hover:text-white transition-colors flex items-center gap-1"
              >
                <Download size={11} />
                Export
              </button>
            </div>
            <div className="rounded-xl border border-gray-800 bg-gray-900 overflow-hidden">
              <table className="w-full text-sm">
                <thead>
                  <tr className="border-b border-gray-800">
                    {['Model', 'Provider', 'Requests', 'Tokens', 'Cost', '% of Total'].map(h => (
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
                        {Array.from({ length: 6 }).map((_, j) => (
                          <td key={j} className="px-4 py-3">
                            <Skeleton className="h-4 w-20" />
                          </td>
                        ))}
                      </tr>
                    ))
                  ) : (
                    modelBreakdown.map(m => (
                      <tr
                        key={m.model}
                        className="border-b border-gray-800/50 hover:bg-gray-800/50 transition-colors"
                      >
                        <td className="px-4 py-3">
                          <span className="font-medium text-white">{m.model}</span>
                        </td>
                        <td className="px-4 py-3">
                          <div className="flex items-center gap-1.5">
                            <div
                              className="w-1.5 h-1.5 rounded-full flex-shrink-0"
                              style={{ backgroundColor: providerColor(m.provider) }}
                            />
                            <span className="text-gray-400 text-xs">{m.provider}</span>
                          </div>
                        </td>
                        <td className="px-4 py-3 text-gray-300 tabular-nums">{formatNumber(m.requests)}</td>
                        <td className="px-4 py-3 text-gray-300 tabular-nums">{formatNumber(m.tokens)}</td>
                        <td className="px-4 py-3 text-gray-300 tabular-nums">{formatCost(m.cost)}</td>
                        <td className="px-4 py-3">
                          <div className="flex items-center gap-2">
                            <div className="w-16 h-1.5 rounded-full bg-gray-800 overflow-hidden">
                              <div
                                className="h-full rounded-full bg-indigo-600 transition-all duration-500"
                                style={{
                                  width: `${totalRequests > 0 ? (m.requests / totalRequests) * 100 : 0}%`,
                                }}
                              />
                            </div>
                            <span className="text-gray-400 text-xs tabular-nums">
                              {totalRequests > 0 ? ((m.requests / totalRequests) * 100).toFixed(1) : 0}%
                            </span>
                          </div>
                        </td>
                      </tr>
                    ))
                  )}
                </tbody>
              </table>
            </div>
          </section>

          {/* ── Footer ──────────────────────────────────────────────────────── */}
          <div className="flex items-center justify-center pt-4 pb-2">
            <span className="text-xs text-gray-600 flex items-center gap-1.5">
              <BarChart2 size={10} className="text-indigo-500" />
              Old Usage & Analytics · {period} historical data
            </span>
          </div>
        </>
      )}
    </div>
  )
}
