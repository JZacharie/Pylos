import { useState } from 'react'
import { Link } from 'react-router-dom'
import { useQuery } from '@tanstack/react-query'
import { guardrailsApi, type GuardrailsBreakdown, type GuardrailsTimeline, type GuardrailsEvent } from '../lib/api'
import { ShieldAlert, Ban, Search, FileText, AlertTriangle, RefreshCw, Shield } from 'lucide-react'

const PERIODS = [
  { label: '1h', value: '1h' },
  { label: '6h', value: '6h' },
  { label: '24h', value: '24h' },
  { label: '7d', value: '7d' },
  { label: '30d', value: '30d' },
]

function formatDate(ts: number) {
  return new Date(ts).toLocaleString()
}

function BreakdownCards({ breakdown }: { breakdown: GuardrailsBreakdown }) {
  const cards = [
    { label: 'Total Blocks', value: breakdown.total_blocks, icon: ShieldAlert, color: 'text-red-400 bg-red-500/10' },
    { label: 'Keyword Blocks', value: breakdown.keyword_blocks, icon: Ban, color: 'text-orange-400 bg-orange-500/10' },
    { label: 'Prompt Injection', value: breakdown.prompt_injection_blocks, icon: AlertTriangle, color: 'text-yellow-400 bg-yellow-500/10' },
    { label: 'Content Filter', value: breakdown.content_filter_blocks, icon: FileText, color: 'text-blue-400 bg-blue-500/10' },
  ]
  return (
    <div className="grid grid-cols-2 lg:grid-cols-4 gap-4">
      {cards.map(c => (
        <div key={c.label} className="bg-zinc-900 rounded-xl border border-zinc-800 p-4 flex items-center gap-3">
          <div className={`p-2.5 rounded-lg ${c.color}`}>
            <c.icon size={20} />
          </div>
          <div>
            <div className="text-2xl font-bold text-white">{c.value.toLocaleString()}</div>
            <div className="text-xs text-zinc-500">{c.label}</div>
          </div>
        </div>
      ))}
    </div>
  )
}

function TimelineChart({ timeline }: { timeline: GuardrailsTimeline[] }) {
  const maxVal = Math.max(...timeline.map(t => t.total), 1)
  return (
    <div className="bg-zinc-900 rounded-xl border border-zinc-800 p-5">
      <h3 className="text-sm font-semibold text-white mb-4">Blocks Over Time</h3>
      <div className="flex items-end gap-1 h-32">
        {timeline.map(t => {
          const h = (t.total / maxVal) * 100
          return (
            <div key={t.timestamp} className="flex-1 flex flex-col items-center justify-end h-full group relative">
              <div className="w-full flex flex-col-reverse" style={{ height: `${Math.max(h, 2)}%` }}>
                {t.prompt_injection > 0 && (
                  <div
                    className="w-full bg-yellow-500/80 rounded-t-sm transition-all"
                    style={{ height: `${(t.prompt_injection / t.total) * 100}%` }}
                    title={`Prompt injection: ${t.prompt_injection}`}
                  />
                )}
                {t.keyword_blocks > 0 && (
                  <div
                    className="w-full bg-orange-500/80 transition-all"
                    style={{ height: `${(t.keyword_blocks / t.total) * 100}%` }}
                    title={`Keyword blocks: ${t.keyword_blocks}`}
                  />
                )}
                {t.content_filter > 0 && (
                  <div
                    className="w-full bg-blue-500/80 transition-all"
                    style={{ height: `${(t.content_filter / t.total) * 100}%` }}
                    title={`Content filter: ${t.content_filter}`}
                  />
                )}
              </div>
              <span className="text-[9px] text-zinc-600 mt-1 truncate w-full text-center">
                {new Date(t.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
              </span>
            </div>
          )
        })}
      </div>
      <div className="flex items-center gap-4 mt-3 text-xs text-zinc-500">
        <span className="flex items-center gap-1.5"><span className="w-2.5 h-2.5 rounded bg-orange-500/80" /> Keyword</span>
        <span className="flex items-center gap-1.5"><span className="w-2.5 h-2.5 rounded bg-yellow-500/80" /> Injection</span>
        <span className="flex items-center gap-1.5"><span className="w-2.5 h-2.5 rounded bg-blue-500/80" /> Content Filter</span>
      </div>
    </div>
  )
}

function TopKeywords({ breakdown }: { breakdown: GuardrailsBreakdown }) {
  if (!breakdown.top_keywords?.length) return null
  const maxCount = Math.max(...breakdown.top_keywords.map(k => k.count), 1)
  return (
    <div className="bg-zinc-900 rounded-xl border border-zinc-800 p-5">
      <h3 className="text-sm font-semibold text-white mb-4">Top Blocked Keywords</h3>
      <div className="space-y-2">
        {breakdown.top_keywords.map(k => (
          <div key={k.keyword} className="flex items-center gap-3">
            <span className="text-sm text-zinc-300 w-32 truncate">{k.keyword}</span>
            <div className="flex-1 h-3 bg-zinc-800 rounded-full overflow-hidden">
              <div
                className="h-full bg-orange-500/70 rounded-full transition-all"
                style={{ width: `${(k.count / maxCount) * 100}%` }}
              />
            </div>
            <span className="text-xs text-zinc-500 w-8 text-right">{k.count}</span>
          </div>
        ))}
      </div>
    </div>
  )
}

function EventsTable({ events, loading }: { events: GuardrailsEvent[]; loading: boolean }) {
  if (loading) {
    return (
      <div className="bg-zinc-900 rounded-xl border border-zinc-800 p-5">
        <div className="animate-pulse space-y-3">
          {[...Array(5)].map((_, i) => (
            <div key={i} className="h-10 bg-zinc-800 rounded" />
          ))}
        </div>
      </div>
    )
  }

  return (
    <div className="bg-zinc-900 rounded-xl border border-zinc-800 overflow-hidden">
      <div className="p-4 border-b border-zinc-800">
        <h3 className="text-sm font-semibold text-white">Recent Guardrail Events</h3>
      </div>
      <div className="overflow-x-auto">
        <table className="w-full text-xs">
          <thead>
            <tr className="text-zinc-500 border-b border-zinc-800">
              <th className="text-left p-3 font-medium">Time</th>
              <th className="text-left p-3 font-medium">Type</th>
              <th className="text-left p-3 font-medium">Model</th>
              <th className="text-left p-3 font-medium">Detail</th>
              <th className="text-left p-3 font-medium">Preview</th>
            </tr>
          </thead>
          <tbody>
            {events.length === 0 ? (
              <tr>
                <td colSpan={5} className="p-8 text-center text-zinc-600">
                  <ShieldAlert size={32} className="mx-auto mb-2 opacity-50" />
                  <p>No guardrail events recorded yet</p>
                </td>
              </tr>
            ) : (
              events.map((e) => (
                <tr key={e.id} className="border-b border-zinc-800/50 hover:bg-zinc-800/30 transition-colors">
                  <td className="p-3 text-zinc-400 whitespace-nowrap">{formatDate(e.timestamp)}</td>
                  <td className="p-3">
                    <span className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium ${
                      e.guardrail_type === 'keyword_block' ? 'bg-orange-500/20 text-orange-400' :
                      e.guardrail_type === 'prompt_injection' ? 'bg-yellow-500/20 text-yellow-400' :
                      'bg-blue-500/20 text-blue-400'
                    }`}>
                      {e.guardrail_type === 'keyword_block' ? 'Keyword' :
                       e.guardrail_type === 'prompt_injection' ? 'Injection' :
                       e.guardrail_type || 'Content Filter'}
                    </span>
                  </td>
                  <td className="p-3 text-zinc-300">{e.model}</td>
                  <td className="p-3 text-zinc-400 max-w-[200px] truncate">{e.guardrail_detail || '-'}</td>
                  <td className="p-3 text-zinc-500 max-w-[200px] truncate">{e.input_preview || '-'}</td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>
    </div>
  )
}

export default function GuardrailsMonitor() {
  const [period, setPeriod] = useState('24h')
  const [typeFilter, setTypeFilter] = useState('')

  const { data: statsData, refetch: refetchStats } = useQuery({
    queryKey: ['guardrails-stats', period],
    queryFn: () => guardrailsApi.getStats({ period }),
    refetchInterval: 30_000,
  })

  const { data: eventsData, isLoading: eventsLoading } = useQuery({
    queryKey: ['guardrails-events', period, typeFilter],
    queryFn: () => guardrailsApi.getEvents({ limit: 50, period, guardrail_type: typeFilter || undefined }),
    refetchInterval: 15_000,
  })

  const breakdown = statsData?.breakdown
  const timeline = statsData?.timeline || []
  const events = eventsData?.events || []

  return (
    <div className="p-5 max-w-7xl mx-auto space-y-5">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-lg font-bold text-white flex items-center gap-2">
            <ShieldAlert size={20} className="text-emerald-400" />
            Guardrails Monitor
          </h1>
          <p className="text-xs text-zinc-500 mt-0.5">
            Real-time monitoring of guardrail interventions and content filtering
          </p>
        </div>
        <div className="flex items-center gap-3">
          <Link
            to="/guardrails"
            className="px-3 py-1.5 text-xs font-medium bg-zinc-900 border border-zinc-800 rounded-lg text-zinc-300 hover:text-white hover:bg-zinc-800 transition-colors flex items-center gap-1.5"
          >
            <Shield size={14} className="text-emerald-400" />
            <span>Configure Guardrails</span>
          </Link>
          <div className="flex bg-zinc-900 rounded-lg border border-zinc-800 overflow-hidden">
            {PERIODS.map(p => (
              <button
                key={p.value}
                onClick={() => setPeriod(p.value)}
                className={`px-3 py-1.5 text-xs font-medium transition-colors ${
                  period === p.value
                    ? 'bg-emerald-600 text-white'
                    : 'text-zinc-400 hover:text-white hover:bg-zinc-800'
                }`}
              >
                {p.label}
              </button>
            ))}
          </div>
          <button
            onClick={() => { refetchStats() }}
            className="p-1.5 text-zinc-500 hover:text-white hover:bg-zinc-800 rounded-lg transition-colors"
            title="Refresh"
          >
            <RefreshCw size={16} />
          </button>
        </div>
      </div>

      {/* Breakdown Cards */}
      {breakdown && <BreakdownCards breakdown={breakdown} />}

      {/* Charts Row */}
      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        <div className="lg:col-span-2">
          {timeline.length > 0 && <TimelineChart timeline={timeline} />}
        </div>
        <div>
          {breakdown && <TopKeywords breakdown={breakdown} />}
        </div>
      </div>

      {/* Filter bar */}
      <div className="flex items-center gap-3">
        <div className="relative flex-1 max-w-xs">
          <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-500" />
          <select
            value={typeFilter}
            onChange={e => setTypeFilter(e.target.value)}
            className="w-full bg-zinc-900 border border-zinc-800 rounded-lg pl-9 pr-3 py-2 text-xs text-zinc-300 focus:outline-none focus:border-emerald-600 appearance-none cursor-pointer"
          >
            <option value="">All types</option>
            <option value="keyword_block">Keyword Block</option>
            <option value="prompt_injection">Prompt Injection</option>
            <option value="content_filter">Content Filter</option>
          </select>
        </div>
        <span className="text-xs text-zinc-600">
          {eventsData?.pagination.total_count ?? 0} events
        </span>
      </div>

      {/* Events Table */}
      <EventsTable events={events} loading={eventsLoading} />
    </div>
  )
}
