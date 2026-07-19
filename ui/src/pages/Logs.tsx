import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { logsApi, type LogEntry } from '../lib/api'
import { formatTimestamp, formatLatency, formatCost, formatNumber, providerColor } from '../lib/utils'
import { RefreshCw, ChevronLeft, ChevronRight } from 'lucide-react'

const LIMIT = 50

export default function Logs() {
  const [offset, setOffset] = useState(0)
  const [period, setPeriod] = useState('1h')
  const [provider, setProvider] = useState('')
  const [status, setStatus] = useState('')
  const [virtualKey, setVirtualKey] = useState('')
  const [model, setModel] = useState('')
  const [selectedLog, setSelectedLog] = useState<LogEntry | null>(null)

  const { data: filterData } = useQuery({
    queryKey: ['logs-filterdata'],
    queryFn: () => logsApi.getFilterData(),
    staleTime: 30_000,
  })

  const { data, isLoading, refetch, isFetching } = useQuery({
    queryKey: ['logs', offset, period, provider, status, virtualKey, model],
    queryFn: () => logsApi.getLogs({
      limit: LIMIT,
      offset,
      period,
      ...(provider && { provider }),
      ...(status && { status }),
      ...(virtualKey && { virtual_key: virtualKey }),
      ...(model && { model }),
    }),
    refetchInterval: 10_000,
  })

  const total = data?.pagination.total_count ?? 0
  const totalPages = Math.ceil(total / LIMIT)
  const currentPage = Math.floor(offset / LIMIT) + 1

  const resetFilters = () => {
    setOffset(0)
  }

  return (
    <div className="p-6 h-full flex flex-col gap-4">
      {/* Header */}
      <div className="flex items-center justify-between flex-wrap gap-3">
        <div>
          <h1 className="text-2xl font-bold text-white">Logs</h1>
          <p className="text-sm text-zinc-400">
            {total.toLocaleString()} requests
          </p>
        </div>

        <div className="flex items-center gap-3 flex-wrap">
          {/* Period */}
          <select
            value={period}
            onChange={e => { setPeriod(e.target.value); resetFilters() }}
            className="bg-zinc-900 border border-zinc-800 rounded-lg px-3 py-1.5 text-sm text-zinc-200 focus:border-emerald-500/50 focus:outline-none"
          >
            {['1h', '6h', '24h', '7d', '30d'].map(p => (
              <option key={p} value={p}>{p}</option>
            ))}
          </select>

          {/* Provider */}
          <select
            value={provider}
            onChange={e => { setProvider(e.target.value); resetFilters() }}
            className="bg-zinc-900 border border-zinc-800 rounded-lg px-3 py-1.5 text-sm text-zinc-200 focus:border-emerald-500/50 focus:outline-none"
          >
            <option value="">All providers</option>
            {(filterData?.providers ?? []).map((p: string) => (
              <option key={p} value={p}>{p}</option>
            ))}
          </select>

          {/* Virtual Key */}
          <select
            value={virtualKey}
            onChange={e => { setVirtualKey(e.target.value); resetFilters() }}
            className="bg-zinc-900 border border-zinc-800 rounded-lg px-3 py-1.5 text-sm text-zinc-200 focus:border-emerald-500/50 focus:outline-none"
          >
            <option value="">All keys</option>
            {(filterData?.virtual_keys ?? []).map((vk: { id: string; name: string } | string) => {
              const id = typeof vk === 'string' ? vk : vk.id
              const name = typeof vk === 'string' ? vk : vk.name
              return <option key={id} value={name}>{name}</option>
            })}
          </select>

          {/* Model filter */}
          <input
            type="text"
            placeholder="Model…"
            value={model}
            onChange={e => { setModel(e.target.value); resetFilters() }}
            className="bg-zinc-900 border border-zinc-800 rounded-lg px-3 py-1.5 text-sm text-zinc-200 w-36 focus:border-emerald-500/50 focus:outline-none"
          />

          {/* Status */}
          <select
            value={status}
            onChange={e => { setStatus(e.target.value); resetFilters() }}
            className="bg-zinc-900 border border-zinc-800 rounded-lg px-3 py-1.5 text-sm text-zinc-200 focus:border-emerald-500/50 focus:outline-none"
          >
            <option value="">All status</option>
            <option value="success">Success</option>
            <option value="error">Error</option>
          </select>

          <button
            onClick={() => refetch()}
            className="p-1.5 rounded-lg bg-zinc-900 border border-zinc-800 text-zinc-400 hover:text-white hover:bg-zinc-800 transition-colors"
          >
            <RefreshCw size={14} className={isFetching ? 'animate-spin' : ''} />
          </button>
        </div>
      </div>

      {/* Quick Stats Summary */}
      {data?.stats && (
        <div className="grid grid-cols-2 sm:grid-cols-5 gap-3 bg-gray-950 border border-gray-800/80 rounded-xl p-4 text-sm">
          <div className="space-y-0.5 border-r border-gray-800/60 pr-2">
            <div className="text-gray-500 text-[10px] uppercase tracking-wider font-semibold">Total Requests</div>
            <div className="text-base font-bold text-white tabular-nums">{formatNumber(data.stats.total_requests)}</div>
          </div>
          <div className="space-y-0.5 sm:border-r border-gray-800/60 sm:px-2">
            <div className="text-gray-500 text-[10px] uppercase tracking-wider font-semibold">Success Rate</div>
            <div className={`text-base font-bold tabular-nums ${data.stats.success_rate > 95 ? 'text-green-400' : 'text-red-400'}`}>
              {data.stats.success_rate.toFixed(1)}%
            </div>
          </div>
          <div className="space-y-0.5 border-r border-gray-800/60 px-2">
            <div className="text-gray-500 text-[10px] uppercase tracking-wider font-semibold">Avg Latency</div>
            <div className="text-base font-bold text-yellow-500 tabular-nums">{formatLatency(data.stats.average_latency_ms)}</div>
          </div>
          <div className="space-y-0.5 border-r border-gray-800/60 px-2">
            <div className="text-gray-500 text-[10px] uppercase tracking-wider font-semibold">Total Tokens</div>
            <div className="text-base font-bold text-purple-400 tabular-nums">{formatNumber(data.stats.total_tokens)}</div>
          </div>
          <div className="space-y-0.5 pl-2">
            <div className="text-gray-500 text-[10px] uppercase tracking-wider font-semibold">Total Cost</div>
            <div className="text-base font-bold text-yellow-400 tabular-nums">{formatCost(data.stats.total_cost_usd)}</div>
          </div>
        </div>
      )}

      {/* Table */}
      <div className="flex-1 overflow-auto rounded-xl border border-zinc-800/50 bg-zinc-900/30">
        <table className="w-full text-sm">
          <thead className="sticky top-0 bg-zinc-900 border-b border-zinc-800/50">
            <tr>
              {['Time', 'Provider', 'Model', 'Status', 'Latency', 'Tokens', 'Cost', 'VK', 'Input'].map(h => (
                <th key={h} className="text-left px-4 py-3 text-xs text-zinc-500 uppercase tracking-wide font-medium">
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {isLoading
              ? Array.from({ length: 8 }).map((_, i) => (
                  <tr key={i} className="border-b border-zinc-800/30">
                    {Array.from({ length: 8 }).map((_, j) => (
                      <td key={j} className="px-4 py-3">
                        <div className="h-3 bg-zinc-800 rounded animate-pulse w-16" />
                      </td>
                    ))}
                  </tr>
                ))
              : data?.logs.map(log => (
                  <tr
                    key={log.id}
                    onClick={() => setSelectedLog(selectedLog?.id === log.id ? null : log)}
                    className={`border-b border-zinc-800/30 cursor-pointer transition-colors
                      ${selectedLog?.id === log.id
                        ? 'bg-emerald-500/5 border-l-2 border-l-emerald-500'
                        : 'hover:bg-zinc-800/30'
                      }`}
                  >
                    <td className="px-4 py-2.5 text-zinc-400 font-mono text-xs">
                      {formatTimestamp(log.timestamp)}
                    </td>
                    <td className="px-4 py-2.5">
                      <span
                        className="inline-flex items-center gap-1.5 text-xs font-medium"
                        style={{ color: providerColor(log.provider) }}
                      >
                        <span className="w-1.5 h-1.5 rounded-full" style={{ background: providerColor(log.provider) }} />
                        {log.provider}
                      </span>
                    </td>
                    <td className="px-4 py-2.5 text-zinc-300 font-mono text-xs max-w-[180px] truncate">
                      {log.model}
                    </td>
                    <td className="px-4 py-2.5">
                      <span className={`text-xs font-medium px-2 py-0.5 rounded-full
                        ${log.status === 'success'
                          ? 'bg-emerald-500/15 text-emerald-400'
                          : 'bg-red-500/15 text-red-400'
                        }`}>
                        {log.status}
                      </span>
                    </td>
                    <td className="px-4 py-2.5 text-zinc-300 tabular-nums text-xs">
                      {formatLatency(log.latency_ms)}
                    </td>
                    <td className="px-4 py-2.5 text-zinc-300 tabular-nums text-xs">
                      {formatNumber(log.total_tokens)}
                    </td>
                    <td className="px-4 py-2.5 text-zinc-300 tabular-nums text-xs">
                      {formatCost(log.cost_usd)}
                    </td>
                    <td className="px-4 py-2.5 text-zinc-500 text-xs max-w-[120px] truncate">
                      {log.virtual_key ?? '—'}
                    </td>
                    <td className="px-4 py-2.5 text-zinc-500 text-xs max-w-[200px] truncate">
                      {log.input_preview}
                    </td>
                  </tr>
                ))
            }
          </tbody>
        </table>

        {!isLoading && !data?.logs.length && (
          <div className="text-center py-16 text-zinc-600">
            No logs found — send some requests to get started
          </div>
        )}
      </div>

      {/* Pagination */}
      {totalPages > 1 && (
        <div className="flex items-center justify-between text-sm text-zinc-500">
          <span>Page {currentPage} / {totalPages} ({total.toLocaleString()} total)</span>
          <div className="flex gap-2">
            <button
              disabled={offset === 0}
              onClick={() => setOffset(Math.max(0, offset - LIMIT))}
              className="p-1.5 rounded-lg bg-zinc-900 border border-zinc-800 disabled:opacity-40 hover:bg-zinc-800 transition-colors"
            >
              <ChevronLeft size={14} />
            </button>
            <button
              disabled={offset + LIMIT >= total}
              onClick={() => setOffset(offset + LIMIT)}
              className="p-1.5 rounded-lg bg-zinc-900 border border-zinc-800 disabled:opacity-40 hover:bg-zinc-800 transition-colors"
            >
              <ChevronRight size={14} />
            </button>
          </div>
        </div>
      )}

      {/* Log detail panel */}
      {selectedLog && (
        <div className="fixed right-0 top-0 h-full w-[400px] bg-zinc-900 border-l border-zinc-800
          overflow-y-auto z-50 p-6 space-y-4">
          <div className="flex items-center justify-between">
            <h3 className="font-semibold text-white">Log Detail</h3>
            <button onClick={() => setSelectedLog(null)} className="text-zinc-500 hover:text-white text-lg">✕</button>
          </div>

          <div className="space-y-3 text-sm">
            <Row label="ID"       value={selectedLog.id} mono />
            <Row label="Provider" value={selectedLog.provider} />
            <Row label="Model"    value={selectedLog.model} mono />
            <Row label="Status"   value={selectedLog.status} />
            <Row label="Latency"  value={formatLatency(selectedLog.latency_ms)} />
            <Row label="Tokens"   value={`${selectedLog.prompt_tokens} + ${selectedLog.completion_tokens} = ${selectedLog.total_tokens}`} />
            <Row label="Cost"     value={formatCost(selectedLog.cost_usd)} />
            {selectedLog.virtual_key && <Row label="Virtual Key" value={selectedLog.virtual_key} />}
            {selectedLog.finish_reason && <Row label="Finish" value={selectedLog.finish_reason} />}
            {selectedLog.error_message && (
              <div>
                <div className="text-xs text-zinc-500 mb-1">Error</div>
                <div className="bg-red-900/20 border border-red-800/50 rounded p-2 text-red-300 text-xs font-mono break-all">
                  {selectedLog.error_message}
                </div>
              </div>
            )}
            {selectedLog.input_preview && (
              <div>
                <div className="text-xs text-zinc-500 mb-1">Input</div>
                <div className="bg-zinc-800 rounded p-2 text-zinc-300 text-xs break-words">
                  {selectedLog.input_preview}
                </div>
              </div>
            )}
            {selectedLog.obfuscated_input && selectedLog.obfuscated_input !== selectedLog.input_preview && (
              <div>
                <div className="text-xs text-zinc-500 mb-1">Obfuscated Input (Sent to LLM)</div>
                <div className="bg-zinc-800 rounded p-2 text-zinc-300 text-xs break-words">
                  {selectedLog.obfuscated_input}
                </div>
              </div>
            )}
            {selectedLog.pii_mapping && (
              <div>
                <div className="text-xs text-zinc-500 mb-1">PII Mapping</div>
                <div className="bg-zinc-800 rounded p-2 text-zinc-300 text-xs font-mono break-all whitespace-pre-wrap">
                  {(() => {
                    try {
                      const parsed = JSON.parse(selectedLog.pii_mapping);
                      return Object.entries(parsed).map(([k, v]) => `${k} -> ${v}`).join('\n');
                    } catch (e) {
                      return selectedLog.pii_mapping;
                    }
                  })()}
                </div>
              </div>
            )}
            {selectedLog.output_preview && (
              <div>
                <div className="text-xs text-zinc-500 mb-1">Output</div>
                <div className="bg-zinc-800 rounded p-2 text-zinc-300 text-xs break-words">
                  {selectedLog.output_preview}
                </div>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  )
}

function Row({ label, value, mono }: { label: string; value: string; mono?: boolean }) {
  return (
    <div className="flex justify-between gap-2">
      <span className="text-zinc-500 shrink-0">{label}</span>
      <span className={`text-zinc-200 text-right break-all ${mono ? 'font-mono text-xs' : ''}`}>
        {value}
      </span>
    </div>
  )
}
