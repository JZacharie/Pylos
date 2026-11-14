import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { modelsApi, getErrorMessage } from '../../lib/api'
import { RefreshCw, RotateCcw, Info } from 'lucide-react'

export function PriceDataManagementTab() {
  const qc = useQueryClient()
  const { data: status, isLoading, refetch } = useQuery({
    queryKey: ['pricingStatus'],
    queryFn: () => modelsApi.getPricingStatus(),
  })

  const reloadMutation = useMutation({
    mutationFn: () => modelsApi.reloadPricingData(),
    onSuccess: (res) => {
      qc.invalidateQueries({ queryKey: ['models'] })
      refetch()
      alert(res.message || "Successfully reloaded pricing data")
    },
    onError: (e: unknown) => {
      alert(`Failed to reload pricing data: ${getErrorMessage(e)}`)
    }
  })

  const scheduleMutation = useMutation({
    mutationFn: (schedule: string | null) => modelsApi.schedulePricingReload(schedule),
    onSuccess: () => {
      refetch()
      alert("Successfully updated schedule")
    },
    onError: (e: unknown) => {
      alert(`Failed to update schedule: ${getErrorMessage(e)}`)
    }
  })

  if (isLoading) {
    return (
      <div className="p-6 space-y-4 animate-pulse">
        <div className="h-8 bg-zinc-900 rounded w-1/4"></div>
        <div className="h-32 bg-zinc-900 rounded"></div>
      </div>
    )
  }

  const lastReload = status?.last_reload_ms 
    ? new Date(status.last_reload_ms).toLocaleString() 
    : 'Never'

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-bold text-white">Price Data Management</h2>
        <p className="text-xs text-zinc-400 mt-1">
          Manage model pricing data and configure automatic reload schedules
        </p>
      </div>

      <div className="flex gap-4">
        <button
          onClick={() => reloadMutation.mutate()}
          disabled={reloadMutation.isPending}
          className="flex items-center gap-2 px-4 py-2.5 bg-indigo-600 hover:bg-indigo-500 text-white rounded-xl text-xs font-semibold disabled:opacity-50 transition-all active:scale-[0.98]"
        >
          {reloadMutation.isPending ? (
            <RotateCcw size={14} className="animate-spin" />
          ) : (
            <RefreshCw size={14} />
          )}
          Reload Price Data
        </button>

        <button
          onClick={() => {
            const sched = window.prompt(
              "Enter periodic reload schedule (e.g. 'hourly', 'daily', 'weekly', or leave blank to disable):",
              status?.periodic_schedule || ''
            )
            if (sched !== null) {
              scheduleMutation.mutate(sched.trim() || null)
            }
          }}
          className="px-4 py-2.5 bg-zinc-900 border border-zinc-800 hover:border-zinc-700 text-zinc-300 hover:text-white rounded-xl text-xs font-semibold transition-all active:scale-[0.98]"
        >
          Set Up Periodic Reload
        </button>
      </div>

      <div className="border border-zinc-800/80 rounded-2xl bg-zinc-900/20 overflow-hidden shadow-2xl p-6 space-y-6">
        <div className="flex items-center gap-3 pb-4 border-b border-zinc-800/50">
          <span className="flex items-center justify-center w-8 h-8 rounded-full bg-indigo-500/15 text-indigo-400 shrink-0">
            <Info size={16} />
          </span>
          <div>
            <h3 className="text-sm font-semibold text-white">Pricing Data Source</h3>
            <p className="text-xs text-zinc-500 font-mono mt-0.5">
              {status?.source_url}
            </p>
          </div>
        </div>

        <div className="grid grid-cols-1 md:grid-cols-3 gap-6">
          <div className="bg-zinc-900/40 rounded-xl p-4 border border-zinc-800/50">
            <div className="text-[10px] font-bold text-zinc-500 uppercase tracking-wider">Models loaded</div>
            <div className="text-2xl font-bold text-white mt-1">
              {status?.models_count || 0}
            </div>
          </div>

          <div className="bg-zinc-900/40 rounded-xl p-4 border border-zinc-800/50">
            <div className="text-[10px] font-bold text-zinc-500 uppercase tracking-wider">Last Run</div>
            <div className="text-sm font-bold text-zinc-200 mt-2 truncate">
              {lastReload}
            </div>
          </div>

          <div className="bg-zinc-900/40 rounded-xl p-4 border border-zinc-800/50">
            <div className="text-[10px] font-bold text-zinc-500 uppercase tracking-wider">Reload Schedule</div>
            <div className="text-sm font-bold text-zinc-200 mt-2 capitalize">
              {status?.periodic_schedule || 'No periodic reload scheduled'}
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}

