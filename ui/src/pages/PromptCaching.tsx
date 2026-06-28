import { useState, useEffect } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import {
  Database, Zap, Clock, Save, RotateCcw,
  HardDrive, Activity, Hash, BarChart3, Target, Server
} from 'lucide-react'
import { configApi } from '../lib/api'

interface PrefixCacheConfig {
  enabled: boolean
  ttl_seconds: number
  max_capacity: number
  min_prefix_length: number
  stats?: {
    hits: number
    misses: number
    hit_rate: number
  }
}

interface SemanticCacheConfig {
  enabled: boolean
  collection_name: string
  similarity_threshold: number
  ttl_seconds: number
  embedding_model: string
  stats?: {
    total_entries: number
    hits: number
    misses: number
    avg_similarity: number
  }
}

interface CacheAlignerConfig {
  enabled: boolean
}

interface CachePlugin {
  name: string
  enabled: boolean
  config: Record<string, any>
}

function ToggleSwitch({ enabled, onToggle, size = 'md' }: {
  enabled: boolean
  onToggle: () => void
  size?: 'sm' | 'md'
}) {
  const h = size === 'sm' ? 'h-5 w-9' : 'h-6 w-11'
  const dot = size === 'sm' ? 'h-3.5 w-3.5' : 'h-4 w-4'
  const translate = size === 'sm' ? 'translate-x-4' : 'translate-x-6'
  return (
    <button
      onClick={onToggle}
      className={`relative inline-flex ${h} items-center rounded-full transition-colors ${
        enabled ? 'bg-emerald-500' : 'bg-zinc-700'
      }`}
    >
      <span className={`inline-block ${dot} transform rounded-full bg-white transition ${
        enabled ? translate : 'translate-x-1'
      }`} />
    </button>
  )
}

function StatCard({ icon: Icon, label, value, sub }: {
  icon: any
  label: string
  value: string | number
  sub?: string
}) {
  return (
    <div className="bg-zinc-900/60 border border-zinc-800/60 rounded-xl p-4">
      <div className="flex items-center gap-2 mb-2">
        <Icon size={14} className="text-zinc-500" />
        <span className="text-xs text-zinc-500">{label}</span>
      </div>
      <div className="text-xl font-bold text-white font-mono">{value}</div>
      {sub && <div className="text-[11px] text-zinc-500 mt-1">{sub}</div>}
    </div>
  )
}

export default function PromptCaching() {
  const queryClient = useQueryClient()

  const { data: configData, isLoading } = useQuery({
    queryKey: ['config'],
    queryFn: configApi.get,
  })

  const [prefixCache, setPrefixCache] = useState<PrefixCacheConfig>({
    enabled: false,
    ttl_seconds: 3600,
    max_capacity: 10000,
    min_prefix_length: 10,
  })

  const [semanticCache, setSemanticCache] = useState<SemanticCacheConfig>({
    enabled: false,
    collection_name: 'semantic_cache',
    similarity_threshold: 0.85,
    ttl_seconds: 7200,
    embedding_model: 'nomic-embed-text',
  })

  const [cacheAligner, setCacheAligner] = useState<CacheAlignerConfig>({
    enabled: false,
  })

  useEffect(() => {
    if (configData && configData.plugins) {
      const prefixPlugin = configData.plugins.find((p: CachePlugin) => p.name === 'prefix_cache')
      if (prefixPlugin) {
        setPrefixCache({
          enabled: prefixPlugin.enabled,
          ttl_seconds: prefixPlugin.config?.ttl_seconds ?? 3600,
          max_capacity: prefixPlugin.config?.max_capacity ?? 10000,
          min_prefix_length: prefixPlugin.config?.min_prefix_length ?? 10,
          stats: prefixPlugin.config?.stats,
        })
      }

      const semanticPlugin = configData.plugins.find((p: CachePlugin) => p.name === 'semantic_cache')
      if (semanticPlugin) {
        setSemanticCache({
          enabled: semanticPlugin.enabled,
          collection_name: semanticPlugin.config?.collection_name ?? 'semantic_cache',
          similarity_threshold: semanticPlugin.config?.similarity_threshold ?? 0.85,
          ttl_seconds: semanticPlugin.config?.ttl_seconds ?? 7200,
          embedding_model: semanticPlugin.config?.embedding_model ?? 'nomic-embed-text',
          stats: semanticPlugin.config?.stats,
        })
      }

      const alignerPlugin = configData.plugins.find((p: CachePlugin) => p.name === 'cache_aligner')
      if (alignerPlugin) {
        setCacheAligner({ enabled: alignerPlugin.enabled })
      }
    }
  }, [configData])

  const reloadMutation = useMutation({
    mutationFn: configApi.reload,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['config'] })
      alert('Configuration reloaded successfully')
    },
    onError: (error: any) => {
      alert(`Failed to reload: ${error.message}`)
    }
  })

  const handleSaveAndReload = () => {
    reloadMutation.mutate()
  }

  if (isLoading) {
    return (
      <div className="flex-1 flex items-center justify-center p-8">
        <div className="text-zinc-500 animate-pulse">Loading cache configuration...</div>
      </div>
    )
  }

  return (
    <div className="flex-1 flex flex-col h-full bg-zinc-950 text-zinc-100 overflow-y-auto">
      <header className="border-b border-zinc-800 bg-zinc-900/30 shrink-0">
        <div className="px-6 py-4 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="p-2 bg-amber-500/10 rounded-lg">
              <HardDrive className="text-amber-400 w-5 h-5" />
            </div>
            <div>
              <h1 className="text-lg font-semibold text-white">Prompt Caching</h1>
              <p className="text-xs text-zinc-500">Configure prefix caching, semantic caching, and cache alignment</p>
            </div>
          </div>

          <button
            onClick={handleSaveAndReload}
            disabled={reloadMutation.isPending}
            className="flex items-center gap-2 px-4 py-2 bg-amber-600 hover:bg-amber-500 disabled:opacity-50 text-white text-xs font-semibold rounded-lg transition-all"
          >
            {reloadMutation.isPending ? (
              <RotateCcw size={14} className="animate-spin" />
            ) : (
              <Save size={14} />
            )}
            Save & Reload
          </button>
        </div>
      </header>

      <div className="p-6 space-y-8 max-w-5xl mx-auto w-full">
        {/* ─── Prefix Cache ─────────────────────────────────────────────── */}
        <section className="bg-zinc-900 border border-zinc-800/50 rounded-2xl p-6 shadow-xl">
          <div className="flex items-center justify-between mb-6">
            <div className="flex items-center gap-3">
              <div className="p-2 bg-blue-500/10 rounded-lg">
                <Database className="text-blue-400 w-5 h-5" />
              </div>
              <div>
                <h2 className="text-base font-semibold text-white">Prefix Cache</h2>
                <p className="text-xs text-zinc-500 mt-0.5">
                  Cache responses based on matching prompt prefixes to reduce redundant computation
                </p>
              </div>
            </div>
            <ToggleSwitch
              enabled={prefixCache.enabled}
              onToggle={() => setPrefixCache(p => ({ ...p, enabled: !p.enabled }))}
            />
          </div>

          <div className={`space-y-6 transition-opacity ${prefixCache.enabled ? 'opacity-100' : 'opacity-40 pointer-events-none'}`}>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-5">
              <div>
                <label className="flex items-center gap-2 text-xs text-zinc-400 mb-2">
                  <Clock size={12} />
                  TTL (seconds)
                </label>
                <input
                  type="number"
                  min={60}
                  value={prefixCache.ttl_seconds}
                  onChange={e => setPrefixCache(p => ({ ...p, ttl_seconds: Number(e.target.value) }))}
                  className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3.5 py-2.5 text-sm text-zinc-200 font-mono focus:outline-none focus:border-blue-500/50"
                />
                <p className="text-[11px] text-zinc-600 mt-1.5">Default: 3600s (1 hour)</p>
              </div>

              <div>
                <label className="flex items-center gap-2 text-xs text-zinc-400 mb-2">
                  <Hash size={12} />
                  Max Capacity
                </label>
                <input
                  type="number"
                  min={100}
                  value={prefixCache.max_capacity}
                  onChange={e => setPrefixCache(p => ({ ...p, max_capacity: Number(e.target.value) }))}
                  className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3.5 py-2.5 text-sm text-zinc-200 font-mono focus:outline-none focus:border-blue-500/50"
                />
                <p className="text-[11px] text-zinc-600 mt-1.5">Max cached entries in memory</p>
              </div>

              <div>
                <label className="flex items-center gap-2 text-xs text-zinc-400 mb-2">
                  <Zap size={12} />
                  Min Prefix Length
                </label>
                <input
                  type="number"
                  min={1}
                  value={prefixCache.min_prefix_length}
                  onChange={e => setPrefixCache(p => ({ ...p, min_prefix_length: Number(e.target.value) }))}
                  className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3.5 py-2.5 text-sm text-zinc-200 font-mono focus:outline-none focus:border-blue-500/50"
                />
                <p className="text-[11px] text-zinc-600 mt-1.5">Minimum tokens to qualify for caching</p>
              </div>
            </div>

            {/* Prefix Cache Stats */}
            {prefixCache.stats && (
              <div className="pt-4 border-t border-zinc-800/50">
                <div className="flex items-center gap-2 mb-3">
                  <BarChart3 size={14} className="text-zinc-500" />
                  <span className="text-xs font-medium text-zinc-400 uppercase tracking-wider">Cache Statistics</span>
                </div>
                <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
                  <StatCard
                    icon={Target}
                    label="Hits"
                    value={prefixCache.stats.hits.toLocaleString()}
                  />
                  <StatCard
                    icon={Activity}
                    label="Misses"
                    value={prefixCache.stats.misses.toLocaleString()}
                  />
                  <StatCard
                    icon={Zap}
                    label="Hit Rate"
                    value={`${(prefixCache.stats.hit_rate * 100).toFixed(1)}%`}
                  />
                  <StatCard
                    icon={Database}
                    label="Total Requests"
                    value={(prefixCache.stats.hits + prefixCache.stats.misses).toLocaleString()}
                  />
                </div>
              </div>
            )}
          </div>
        </section>

        {/* ─── Semantic Cache ───────────────────────────────────────────── */}
        <section className="bg-zinc-900 border border-zinc-800/50 rounded-2xl p-6 shadow-xl">
          <div className="flex items-center justify-between mb-6">
            <div className="flex items-center gap-3">
              <div className="p-2 bg-purple-500/10 rounded-lg">
                <Zap className="text-purple-400 w-5 h-5" />
              </div>
              <div>
                <h2 className="text-base font-semibold text-white">Semantic Cache</h2>
                <p className="text-xs text-zinc-500 mt-0.5">
                  Cache responses based on semantic similarity of prompts using vector embeddings
                </p>
              </div>
            </div>
            <ToggleSwitch
              enabled={semanticCache.enabled}
              onToggle={() => setSemanticCache(p => ({ ...p, enabled: !p.enabled }))}
            />
          </div>

          <div className={`space-y-6 transition-opacity ${semanticCache.enabled ? 'opacity-100' : 'opacity-40 pointer-events-none'}`}>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-5">
              <div>
                <label className="flex items-center gap-2 text-xs text-zinc-400 mb-2">
                  <Server size={12} />
                  Collection Name
                </label>
                <input
                  type="text"
                  value={semanticCache.collection_name}
                  onChange={e => setSemanticCache(p => ({ ...p, collection_name: e.target.value }))}
                  className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3.5 py-2.5 text-sm text-zinc-200 font-mono focus:outline-none focus:border-purple-500/50"
                />
                <p className="text-[11px] text-zinc-600 mt-1.5">Qdrant collection for semantic vectors</p>
              </div>

              <div>
                <label className="flex items-center gap-2 text-xs text-zinc-400 mb-2">
                  <Target size={12} />
                  Similarity Threshold
                </label>
                <div className="flex items-center gap-3">
                  <input
                    type="range"
                    min={0}
                    max={1}
                    step={0.01}
                    value={semanticCache.similarity_threshold}
                    onChange={e => setSemanticCache(p => ({ ...p, similarity_threshold: Number(e.target.value) }))}
                    className="flex-1 accent-purple-500"
                  />
                  <span className="text-sm font-mono text-zinc-300 w-12 text-right">
                    {semanticCache.similarity_threshold.toFixed(2)}
                  </span>
                </div>
                <p className="text-[11px] text-zinc-600 mt-1.5">Minimum similarity score (0.0 - 1.0)</p>
              </div>
            </div>

            <div className="grid grid-cols-1 md:grid-cols-2 gap-5">
              <div>
                <label className="flex items-center gap-2 text-xs text-zinc-400 mb-2">
                  <Clock size={12} />
                  TTL (seconds)
                </label>
                <input
                  type="number"
                  min={60}
                  value={semanticCache.ttl_seconds}
                  onChange={e => setSemanticCache(p => ({ ...p, ttl_seconds: Number(e.target.value) }))}
                  className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3.5 py-2.5 text-sm text-zinc-200 font-mono focus:outline-none focus:border-purple-500/50"
                />
                <p className="text-[11px] text-zinc-600 mt-1.5">Default: 7200s (2 hours)</p>
              </div>

              <div>
                <label className="flex items-center gap-2 text-xs text-zinc-400 mb-2">
                  <Zap size={12} />
                  Embedding Model
                </label>
                <select
                  value={semanticCache.embedding_model}
                  onChange={e => setSemanticCache(p => ({ ...p, embedding_model: e.target.value }))}
                  className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3.5 py-2.5 text-sm text-zinc-200 focus:outline-none focus:border-purple-500/50"
                >
                  <option value="nomic-embed-text">nomic-embed-text</option>
                  <option value="text-embedding-3-small">text-embedding-3-small</option>
                  <option value="text-embedding-3-large">text-embedding-3-large</option>
                  <option value="text-embedding-ada-002">text-embedding-ada-002</option>
                  <option value="voyage-2">voyage-2</option>
                </select>
                <p className="text-[11px] text-zinc-600 mt-1.5">Model used for generating embeddings</p>
              </div>
            </div>

            {/* Semantic Cache Stats */}
            {semanticCache.stats && (
              <div className="pt-4 border-t border-zinc-800/50">
                <div className="flex items-center gap-2 mb-3">
                  <BarChart3 size={14} className="text-zinc-500" />
                  <span className="text-xs font-medium text-zinc-400 uppercase tracking-wider">Cache Statistics</span>
                </div>
                <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
                  <StatCard
                    icon={Database}
                    label="Total Entries"
                    value={semanticCache.stats.total_entries.toLocaleString()}
                  />
                  <StatCard
                    icon={Target}
                    label="Hits"
                    value={semanticCache.stats.hits.toLocaleString()}
                  />
                  <StatCard
                    icon={Activity}
                    label="Misses"
                    value={semanticCache.stats.misses.toLocaleString()}
                  />
                  <StatCard
                    icon={Zap}
                    label="Avg Similarity"
                    value={semanticCache.stats.avg_similarity.toFixed(3)}
                  />
                </div>
              </div>
            )}
          </div>
        </section>

        {/* ─── Cache Aligner ────────────────────────────────────────────── */}
        <section className="bg-zinc-900 border border-zinc-800/50 rounded-2xl p-6 shadow-xl">
          <div className="flex items-center justify-between">
            <div className="flex items-center gap-3">
              <div className="p-2 bg-emerald-500/10 rounded-lg">
                <Activity className="text-emerald-400 w-5 h-5" />
              </div>
              <div>
                <h2 className="text-base font-semibold text-white">Cache Aligner</h2>
                <p className="text-xs text-zinc-500 mt-0.5">
                  Synchronize cache state across multiple gateway instances for distributed deployments
                </p>
              </div>
            </div>
            <ToggleSwitch
              enabled={cacheAligner.enabled}
              onToggle={() => setCacheAligner(p => ({ ...p, enabled: !p.enabled }))}
            />
          </div>
        </section>
      </div>
    </div>
  )
}
