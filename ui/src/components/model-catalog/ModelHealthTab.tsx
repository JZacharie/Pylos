import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { modelsApi, getErrorMessage } from '../../lib/api'
import { Activity, Play, RotateCcw } from 'lucide-react'

export function ModelHealthTab() {
  const qc = useQueryClient()
  const { data: healthData, isLoading } = useQuery({
    queryKey: ['modelHealth'],
    queryFn: () => modelsApi.getHealth(),
    refetchInterval: 30_000,
  })

  const checkAllMutation = useMutation({
    mutationFn: () => modelsApi.runAllHealthChecks(),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['modelHealth'] })
    },
    onError: (e: unknown) => {
      alert(`Health check failed: ${getErrorMessage(e)}`)
    }
  })

  const checkOneMutation = useMutation({
    mutationFn: ({ provider, model_id }: { provider: string; model_id: string }) =>
      modelsApi.runHealthCheck(provider, model_id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['modelHealth'] })
    },
    onError: (e: unknown) => {
      alert(`Health check failed: ${getErrorMessage(e)}`)
    }
  })

  function formatTime(ms: number | null): string {
    if (!ms) return '—'
    return new Date(ms).toLocaleString()
  }

  function StatusBadge({ status }: { status: string }) {
    if (status === 'healthy') {
      return (
        <span className="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-[10px] font-bold bg-emerald-900/30 text-emerald-400 border border-emerald-800/50 uppercase tracking-wider">
          <span className="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse" />
          Healthy
        </span>
      )
    }
    if (status === 'unhealthy') {
      return (
        <span className="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-[10px] font-bold bg-red-900/30 text-red-400 border border-red-800/50 uppercase tracking-wider">
          <span className="w-1.5 h-1.5 rounded-full bg-red-400" />
          Unhealthy
        </span>
      )
    }
    return (
      <span className="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-[10px] font-bold bg-zinc-800 text-zinc-500 border border-zinc-700/50 uppercase tracking-wider">
        <span className="w-1.5 h-1.5 rounded-full bg-zinc-500" />
        Unknown
      </span>
    )
  }

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h2 className="text-lg font-bold text-white">Health Status</h2>
          <p className="text-xs text-zinc-400 mt-1">
            Monitor model availability and run health checks
          </p>
        </div>
        <button
          onClick={() => checkAllMutation.mutate()}
          disabled={checkAllMutation.isPending}
          className="flex items-center gap-2 px-4 py-2.5 bg-indigo-600 hover:bg-indigo-500 text-white rounded-xl text-xs font-semibold disabled:opacity-50 transition-all active:scale-[0.98]"
        >
          {checkAllMutation.isPending ? (
            <RotateCcw size={14} className="animate-spin" />
          ) : (
            <Activity size={14} />
          )}
          Check All Models
        </button>
      </div>

      {isLoading ? (
        <div className="space-y-4">
          {Array.from({ length: 4 }).map((_, i) => (
            <div key={i} className="h-16 rounded-xl border border-zinc-800/50 bg-zinc-900/30 animate-pulse" />
          ))}
        </div>
      ) : !healthData || healthData.length === 0 ? (
        <div className="border border-zinc-800/80 rounded-2xl bg-zinc-900/20 p-12 text-center">
          <div className="flex flex-col items-center justify-center text-zinc-500 space-y-4">
            <div className="w-12 h-12 rounded-full bg-zinc-800/30 flex items-center justify-center">
              <Activity size={18} className="text-zinc-600" />
            </div>
            <div className="text-xs">No health data available yet</div>
            <button
              onClick={() => checkAllMutation.mutate()}
              disabled={checkAllMutation.isPending}
              className="px-4 py-2 bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 text-white rounded-xl text-xs font-semibold transition-all active:scale-[0.98]"
            >
              {checkAllMutation.isPending ? 'Checking...' : 'Run initial health check'}
            </button>
          </div>
        </div>
      ) : (
        <div className="border border-zinc-800/80 rounded-2xl bg-zinc-900/20 overflow-hidden shadow-2xl">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="bg-zinc-900/40 border-b border-zinc-800/80">
                <th className="px-6 py-4 text-[10px] font-bold text-zinc-500 uppercase tracking-widest">Provider</th>
                <th className="px-6 py-4 text-[10px] font-bold text-zinc-500 uppercase tracking-widest">Model ID</th>
                <th className="px-6 py-4 text-[10px] font-bold text-zinc-500 uppercase tracking-widest">Status</th>
                <th className="px-6 py-4 text-[10px] font-bold text-zinc-500 uppercase tracking-widest">Last Check</th>
                <th className="px-6 py-4 text-[10px] font-bold text-zinc-500 uppercase tracking-widest">Last Success</th>
                <th className="px-6 py-4 text-[10px] font-bold text-zinc-500 uppercase tracking-widest">Error</th>
                <th className="px-6 py-4 text-[10px] font-bold text-zinc-500 uppercase tracking-widest text-right">Actions</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-zinc-800/50">
              {healthData.map(entry => (
                <tr key={entry.id} className="hover:bg-zinc-900/40 transition-colors group">
                  <td className="px-6 py-4">
                    <span className="text-xs text-zinc-300 font-semibold capitalize">{entry.provider}</span>
                  </td>
                  <td className="px-6 py-4">
                    <span className="font-mono text-xs text-zinc-200">{entry.model_id}</span>
                  </td>
                  <td className="px-6 py-4">
                    <StatusBadge status={entry.health_status} />
                  </td>
                  <td className="px-6 py-4 text-xs text-zinc-400 font-mono">
                    {formatTime(entry.last_check_ms)}
                  </td>
                  <td className="px-6 py-4 text-xs text-zinc-400 font-mono">
                    {formatTime(entry.last_success_ms)}
                  </td>
                  <td className="px-6 py-4 text-xs text-zinc-500 max-w-[200px] truncate" title={entry.error_details ?? ''}>
                    {entry.error_details || '—'}
                  </td>
                  <td className="px-6 py-4 text-right">
                    <button
                      onClick={() => checkOneMutation.mutate({ provider: entry.provider, model_id: entry.model_id })}
                      disabled={checkOneMutation.isPending}
                      className="flex items-center gap-1.5 px-3 py-1.5 ml-auto text-[10px] font-semibold bg-zinc-800 hover:bg-zinc-700 disabled:opacity-50 text-zinc-300 hover:text-white rounded-lg transition-all active:scale-[0.98]"
                    >
                      {checkOneMutation.isPending ? (
                        <RotateCcw size={11} className="animate-spin" />
                      ) : (
                        <Play size={11} />
                      )}
                      Check
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}

// ─── Main page ────────────────────────────────────────────────────────────────

