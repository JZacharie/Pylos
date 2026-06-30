import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { logsApi, configApi } from '../lib/api'
import { StatCard } from '../components/StatCard'
import { RequestChart, TokenChart } from '../components/Charts'
import {
  formatLatency, formatCost, formatNumber, formatPercent,
} from '../lib/utils'
import { Activity, TrendingUp, Coins, Zap, Clock, Hash, Rocket } from 'lucide-react'

const PERIODS = ['1h', '6h', '24h', '7d', '30d'] as const
type Period = typeof PERIODS[number]

export default function Dashboard() {
  const [period, setPeriod] = useState<Period>('24h')
  const [isPromoting, setIsPromoting] = useState(false)
  const [promoteMessage, setPromoteMessage] = useState<string | null>(null)

  const handlePromote = async () => {
    if (!window.confirm("Are you sure you want to promote the current DEV version to PRODUCTION?")) {
      return
    }
    setIsPromoting(true)
    setPromoteMessage("Triggering promotion...")
    try {
      const res = await configApi.promote()
      setPromoteMessage(res.message || "Promotion started successfully!")
      setTimeout(() => setPromoteMessage(null), 5000)
    } catch (err: any) {
      const errMsg = err.response?.data?.error || err.message || "Failed to trigger promotion."
      setPromoteMessage(`Error: ${errMsg}`)
      setTimeout(() => setPromoteMessage(null), 7000)
    } finally {
      setIsPromoting(false)
    }
  }

  const statsQ = useQuery({
    queryKey: ['logs-stats', period],
    queryFn: () => logsApi.getStats({ period }),
    refetchInterval: 30_000,
  })

  const histQ = useQuery({
    queryKey: ['histogram', period],
    queryFn: () => logsApi.getHistogram({ period }),
    refetchInterval: 30_000,
  })

  const tokensQ = useQuery({
    queryKey: ['token-histogram', period],
    queryFn: () => logsApi.getTokenHistogram({ period }),
    refetchInterval: 30_000,
  })

  const configQ = useQuery({
    queryKey: ['config'],
    queryFn: configApi.get,
    refetchInterval: 60_000,
  })

  const isDev = configQ.data?.environment === 'dev'
  const s = statsQ.data

  return (
    <div className="p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between flex-wrap gap-3">
        <div>
          <h1 className="text-2xl font-bold text-white">Dashboard</h1>
          <p className="text-sm text-zinc-400 mt-1">Last {period}</p>
        </div>
        <div className="flex items-center gap-3">
          {/* Period selector */}
          <div className="flex rounded-lg border border-zinc-800/50 bg-zinc-900/50 overflow-hidden">
            {PERIODS.map(p => (
              <button
                key={p}
                onClick={() => setPeriod(p)}
                className={`px-3 py-1.5 text-xs transition-colors
                  ${period === p
                    ? 'bg-zinc-800 text-white font-medium'
                    : 'text-zinc-500 hover:text-zinc-300'
                  }`}
              >
                {p}
              </button>
            ))}
          </div>
          {isDev && (
            <button
              onClick={handlePromote}
              disabled={isPromoting}
              className={`flex items-center gap-2 px-3 py-1.5 rounded-lg border text-xs font-semibold transition-all duration-200
                ${isPromoting
                  ? 'bg-zinc-800 border-zinc-700 text-zinc-500 cursor-not-allowed'
                  : 'bg-gradient-to-r from-purple-600 to-indigo-600 border-indigo-500 hover:border-indigo-400 text-white shadow-lg hover:shadow-indigo-500/20 active:scale-95'
                }`}
            >
              <Rocket size={14} className={isPromoting ? 'animate-bounce' : ''} />
              {isPromoting ? 'Promoting...' : 'Promote to Prod'}
            </button>
          )}
          <div className="flex items-center gap-2 text-xs text-zinc-500">
            <div className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse" />
            Live
          </div>
        </div>
      </div>

      {promoteMessage && (
        <div className={`p-3 rounded-lg border text-xs font-medium ${
          promoteMessage.startsWith('Error')
            ? 'bg-red-500/10 border-red-500/20 text-red-400'
            : 'bg-indigo-500/10 border-indigo-500/20 text-indigo-400'
        }`}>
          {promoteMessage}
        </div>
      )}

      {/* KPI Cards */}
      <div className="grid grid-cols-2 md:grid-cols-3 xl:grid-cols-6 gap-4">
        <StatCard
          label="Total Requests"
          value={s ? formatNumber(s.total_requests) : '—'}
          color="blue"
          icon={<Activity size={14} />}
        />
        <StatCard
          label="Success Rate"
          value={s ? formatPercent(s.success_rate) : '—'}
          color={s && s.success_rate > 95 ? 'green' : 'red'}
          icon={<TrendingUp size={14} />}
        />
        <StatCard
          label="Avg Latency"
          value={s ? formatLatency(s.average_latency_ms) : '—'}
          color="yellow"
          icon={<Clock size={14} />}
        />
        <StatCard
          label="Total Tokens"
          value={s ? formatNumber(s.total_tokens) : '—'}
          color="purple"
          icon={<Hash size={14} />}
        />
        <StatCard
          label="Total Cost"
          value={s ? formatCost(s.total_cost_usd) : '—'}
          color="yellow"
          icon={<Coins size={14} />}
        />
        <StatCard
          label="Prompt / Completion"
          value={s ? `${formatNumber(s.total_prompt_tokens)} / ${formatNumber(s.total_completion_tokens)}` : '—'}
          color="blue"
          icon={<Zap size={14} />}
        />
        <StatCard
          label="Compression Saved"
          value={s ? formatNumber(s.total_compression_saved_bytes) + ' B' : '—'}
          color="green"
          icon={<Zap size={14} />}
        />
      </div>

      {/* Charts */}
      <div className="grid grid-cols-1 xl:grid-cols-2 gap-6">
        {/* Request Volume */}
        <div className="rounded-xl border border-zinc-800/50 bg-zinc-900/30 p-5">
          <h2 className="text-sm font-semibold text-zinc-300 mb-4">Request Volume</h2>
          {histQ.data?.buckets.length ? (
            <RequestChart
              buckets={histQ.data.buckets}
              bucketSecs={histQ.data.bucket_size_seconds}
            />
          ) : (
            <EmptyChart />
          )}
        </div>

        {/* Token Usage */}
        <div className="rounded-xl border border-zinc-800/50 bg-zinc-900/30 p-5">
          <h2 className="text-sm font-semibold text-zinc-300 mb-4">Token Usage</h2>
          {tokensQ.data?.buckets.length ? (
            <TokenChart
              buckets={tokensQ.data.buckets}
              bucketSecs={tokensQ.data.bucket_size_seconds}
            />
          ) : (
            <EmptyChart />
          )}
        </div>
      </div>
    </div>
  )
}

function EmptyChart() {
  return (
    <div className="h-[220px] flex items-center justify-center text-zinc-600 text-sm">
      No data yet — send some requests to see charts
    </div>
  )
}
