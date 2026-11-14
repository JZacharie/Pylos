import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { providersApi } from '../../lib/api'
import { Plus, Trash2, Key, Globe } from 'lucide-react'

export function LlmCredentialsTab() {
  const { data: providers } = useQuery({ queryKey: ['providers'], queryFn: () => providersApi.getAll() }) ?? { data: undefined }
  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-bold text-white">LLM Credentials</h2>
        <p className="text-xs text-zinc-400 mt-1">Re-use credentials across models. Each model inherits from its provider.</p>
      </div>
      {!providers || providers.total === 0 ? (
        <div className="border border-zinc-800/80 rounded-2xl bg-zinc-900/20 p-12 text-center">
          <div className="flex flex-col items-center text-zinc-500 gap-3">
            <Key size={24} className="text-zinc-700" />
            <div className="text-xs">No providers configured. Add one in the Providers section.</div>
          </div>
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          {providers.providers.map(p => (
            <div key={p.name} className="rounded-xl border border-zinc-800/50 bg-zinc-900/20 p-5 space-y-3">
              <div className="flex items-center gap-2">
                <Key size={14} className="text-indigo-400" />
                <span className="font-semibold text-white capitalize">{p.name}</span>
                <span className="text-[10px] text-zinc-500">{p.keys_count} key(s)</span>
              </div>
              <div className="flex flex-wrap gap-1.5">
                {p.keys.map((k, i) => (
                  <span key={i} className="inline-flex items-center gap-1 px-2 py-0.5 rounded text-[10px] bg-zinc-800 text-zinc-300 border border-zinc-700/50">
                    {k.name}: <span className="text-zinc-500 font-mono">{k.value}</span>
                  </span>
                ))}
              </div>
              <div className="text-[10px] text-zinc-500 font-mono">{p.network.base_url || 'default URL'}</div>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

// ─── Pass-Through Endpoints Tab ──────────────────────────────────────────────

export function PassThroughEndpointsTab() {
  const { data: providers } = useQuery({ queryKey: ['providers'], queryFn: () => providersApi.getAll() }) ?? { data: undefined }
  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-bold text-white">Pass-Through Endpoints</h2>
        <p className="text-xs text-zinc-400 mt-1">Models can be accessed directly at their provider endpoints via Pylos.</p>
      </div>
      {!providers || providers.total === 0 ? (
        <div className="border border-zinc-800/80 rounded-2xl bg-zinc-900/20 p-12 text-center">
          <div className="flex flex-col items-center text-zinc-500 gap-3">
            <Globe size={24} className="text-zinc-700" />
            <div className="text-xs">No endpoints configured.</div>
          </div>
        </div>
      ) : (
        <div className="border border-zinc-800/80 rounded-2xl bg-zinc-900/20 overflow-hidden shadow-2xl">
          <table className="w-full text-left border-collapse">
            <thead>
              <tr className="bg-zinc-900/40 border-b border-zinc-800/80">
                <th className="px-6 py-4 text-[10px] font-bold text-zinc-500 uppercase tracking-widest">Provider</th>
                <th className="px-6 py-4 text-[10px] font-bold text-zinc-500 uppercase tracking-widest">Endpoint</th>
                <th className="px-6 py-4 text-[10px] font-bold text-zinc-500 uppercase tracking-widest">Auth</th>
                <th className="px-6 py-4 text-[10px] font-bold text-zinc-500 uppercase tracking-widest">Statu</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-zinc-800/50">
              {providers.providers.map(p => (
                <tr key={p.name} className="hover:bg-zinc-900/40 transition-colors">
                  <td className="px-6 py-4">
                    <span className="text-xs text-zinc-300 font-semibold capitalize">{p.name}</span>
                  </td>
                  <td className="px-6 py-4">
                    <span className="font-mono text-xs text-zinc-400">{p.network.base_url || 'default'}</span>
                  </td>
                  <td className="px-6 py-4">
                    <span className="text-[10px] text-zinc-500">{p.keys_count} key(s)</span>
                  </td>
                  <td className="px-6 py-4">
                    <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-bold bg-emerald-900/30 text-emerald-400 border border-emerald-800/50 uppercase tracking-wider">
                      Active
                    </span>
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

// ─── Model Retry Settings Tab ────────────────────────────────────────────────

export function ModelRetrySettingsTab() {
  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-bold text-white">Model Retry Settings</h2>
        <p className="text-xs text-zinc-400 mt-1">
          Configure retry policies per model or per provider. When a model fails, Pylos will attempt fallback providers.
        </p>
      </div>

      <div className="grid grid-cols-1 md:grid-cols-3 gap-4">
        <div className="rounded-xl border border-zinc-800/50 bg-zinc-900/20 p-5 space-y-2">
          <div className="text-[10px] font-bold text-zinc-500 uppercase tracking-wider">Per-Provider Retries</div>
          <div className="text-2xl font-bold text-white">3</div>
          <div className="text-[10px] text-zinc-500">default max retries</div>
        </div>
        <div className="rounded-xl border border-zinc-800/50 bg-zinc-900/20 p-5 space-y-2">
          <div className="text-[10px] font-bold text-zinc-500 uppercase tracking-wider">Backoff (initial)</div>
          <div className="text-2xl font-bold text-white">100ms</div>
          <div className="text-[10px] text-zinc-500">exponential with jitter</div>
        </div>
        <div className="rounded-xl border border-zinc-800/50 bg-zinc-900/20 p-5 space-y-2">
          <div className="text-[10px] font-bold text-zinc-500 uppercase tracking-wider">Circuit Breaker</div>
          <div className="text-2xl font-bold text-white">5</div>
          <div className="text-[10px] text-zinc-500">max failures before 30s cooldown</div>
        </div>
      </div>

      <div className="border border-zinc-800/80 rounded-2xl bg-zinc-900/20 p-6 space-y-4 shadow-2xl">
        <h3 className="text-sm font-semibold text-white">How retries work</h3>
        <ol className="list-decimal list-inside space-y-2 text-xs text-zinc-400">
          <li><strong className="text-zinc-300">Per-provider retry</strong> — If a provider returns a retriable error (timeout, 429, 5xx), the request is retried up to <code className="text-indigo-400">max_retries</code> times with exponential backoff + jitter.</li>
          <li><strong className="text-zinc-300">Provider fallback</strong> — After exhausting retries, Pylos tries the next provider in the ordered list (<code className="text-indigo-400">select_and_order_providers</code>). Providers supporting the model are tried first.</li>
          <li><strong className="text-zinc-300">Circuit breaker</strong> — After <code className="text-indigo-400">5 consecutive failures</code>, the provider is skipped for <code className="text-indigo-400">30 seconds</code> to allow recovery.</li>
          <li><strong className="text-zinc-300">Model mapping</strong> — If the model name isn't supported by the fallback provider, <code className="text-indigo-400">map_model_for_provider</code> translates it (e.g., <code className="text-indigo-400">gpt-4o</code> → <code className="text-indigo-400">anthropic.claude-3-5-sonnet</code>).</li>
          <li><strong className="text-zinc-300">Request queuing</strong> — If the concurrency limit is reached, requests are queued with a configurable timeout (default: 30s).</li>
        </ol>
      </div>
    </div>
  )
}


interface AliasEntry {
  alias_name: string;
  target_model: string;
}

export function ModelGroupAliasTab() {
  const [aliases, setAliases] = useState<AliasEntry[]>(() => {
    const saved = localStorage.getItem('pylos_model_group_aliases')
    return saved ? JSON.parse(saved) : []
  })

  const [aliasName, setAliasName] = useState('')
  const [targetModel, setTargetModel] = useState('')

  const saveAliases = (newAliases: AliasEntry[]) => {
    setAliases(newAliases)
    localStorage.setItem('pylos_model_group_aliases', JSON.stringify(newAliases))
  }

  const handleAdd = () => {
    if (!aliasName.trim() || !targetModel.trim()) return
    if (aliases.some(a => a.alias_name.toLowerCase() === aliasName.trim().toLowerCase())) {
      alert("This alias already exists.")
      return
    }
    const newAliases = [...aliases, { alias_name: aliasName.trim(), target_model: targetModel.trim() }]
    saveAliases(newAliases)
    setAliasName('')
    setTargetModel('')
  }

  const handleDelete = (name: string) => {
    const newAliases = aliases.filter(a => a.alias_name !== name)
    saveAliases(newAliases)
  }

  const generateYaml = () => {
    if (aliases.length === 0) {
      return `router_settings:
  model_group_alias:
    # no aliases configured yet`
    }
    
    const aliasLines = aliases.map(
      a => `    - model_alias: ${a.alias_name}
      target_model: ${a.target_model}`
    ).join('\n')

    return `router_settings:
  model_group_alias:
${aliasLines}`
  }

  return (
    <div className="space-y-6">
      <div>
        <h2 className="text-lg font-bold text-white">Model Group Alias Settings</h2>
        <p className="text-xs text-zinc-400 mt-1">
          Create aliases for your model groups to simplify API calls. For example, you can create an alias 'gpt-4o' that points to 'gpt-4o-mini-openai' model group.
        </p>
      </div>

      <div className="border border-zinc-800/80 rounded-2xl bg-zinc-900/20 p-6 space-y-4 shadow-2xl">
        <h3 className="text-sm font-semibold text-white">Add New Alias</h3>
        <div className="flex items-end gap-3 flex-wrap md:flex-nowrap">
          <div className="flex-1 min-w-[200px]">
            <label className="block text-[10px] font-bold text-zinc-500 uppercase tracking-wider mb-1.5 ml-1">Alias Name</label>
            <input
              type="text"
              value={aliasName}
              onChange={e => setAliasName(e.target.value)}
              placeholder="e.g., gpt-4o"
              className="w-full bg-zinc-950 border border-zinc-800 rounded-xl px-3.5 py-2 text-xs text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-indigo-500/50 focus:ring-1 focus:ring-indigo-500/20"
            />
          </div>
          <div className="flex-1 min-w-[200px]">
            <label className="block text-[10px] font-bold text-zinc-500 uppercase tracking-wider mb-1.5 ml-1">Target Model Group</label>
            <input
              type="text"
              value={targetModel}
              onChange={e => setTargetModel(e.target.value)}
              placeholder="e.g., gpt-4o-mini-openai"
              className="w-full bg-zinc-950 border border-zinc-800 rounded-xl px-3.5 py-2 text-xs text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-indigo-500/50 focus:ring-1 focus:ring-indigo-500/20"
            />
          </div>
          <button
            onClick={handleAdd}
            disabled={!aliasName.trim() || !targetModel.trim()}
            className="flex items-center gap-1.5 px-4 py-2 bg-indigo-600 hover:bg-indigo-500 disabled:opacity-50 text-white rounded-xl text-xs font-semibold transition-all active:scale-[0.98] h-[38px] shrink-0"
          >
            <Plus size={14} />
            Add Alias
          </button>
        </div>
      </div>

      <div className="border border-zinc-800/80 rounded-2xl bg-zinc-900/20 overflow-hidden shadow-2xl">
        <div className="px-6 py-4 border-b border-zinc-800/80">
          <h3 className="text-sm font-semibold text-white">Manage Existing Aliases</h3>
        </div>
        <table className="w-full text-left border-collapse">
          <thead>
            <tr className="bg-zinc-900/40 border-b border-zinc-800/80">
              <th className="px-6 py-3.5 text-[10px] font-bold text-zinc-500 uppercase tracking-widest w-[40%]">Alias Name</th>
              <th className="px-6 py-3.5 text-[10px] font-bold text-zinc-500 uppercase tracking-widest w-[40%]">Target Model Group</th>
              <th className="px-6 py-3.5 text-[10px] font-bold text-zinc-500 uppercase tracking-widest text-right">Actions</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-zinc-800/50">
            {aliases.length === 0 ? (
              <tr>
                <td colSpan={3} className="px-6 py-12 text-center text-xs text-zinc-500 font-medium">
                  No aliases added yet. Add a new alias above.
                </td>
              </tr>
            ) : (
              aliases.map(a => (
                <tr key={a.alias_name} className="hover:bg-zinc-900/20 transition-colors">
                  <td className="px-6 py-4 text-xs font-mono text-indigo-400 font-semibold">
                    {a.alias_name}
                  </td>
                  <td className="px-6 py-4 text-xs font-mono text-zinc-300">
                    {a.target_model}
                  </td>
                  <td className="px-6 py-4 text-right">
                    <button
                      onClick={() => handleDelete(a.alias_name)}
                      className="p-1.5 text-zinc-500 hover:text-red-400 hover:bg-red-400/10 rounded-lg transition-all"
                      title="Delete Alias"
                    >
                      <Trash2 size={13} />
                    </button>
                  </td>
                </tr>
              ))
            )}
          </tbody>
        </table>
      </div>

      <div className="border border-zinc-800/80 rounded-2xl bg-zinc-900/20 p-6 space-y-4 shadow-2xl">
        <div>
          <h3 className="text-sm font-semibold text-white">Configuration Example</h3>
          <p className="text-xs text-zinc-500 mt-1">
            Here's how your current aliases would look in the config.yaml:
          </p>
        </div>
        <pre className="bg-zinc-950/80 rounded-xl p-4 border border-zinc-800/80 font-mono text-xs text-emerald-400 overflow-x-auto leading-relaxed">
          {generateYaml()}
        </pre>
      </div>
    </div>
  )
}

// ─── ModelHealthTab ───────────────────────────────────────────────────────────

