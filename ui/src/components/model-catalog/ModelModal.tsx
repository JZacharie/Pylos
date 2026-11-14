import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { providersApi } from '../../lib/api'
import { X, Key, Network, Sliders, FileJson, Tags, Globe, AlertTriangle, RotateCcw, Check } from 'lucide-react'
import { type ModelFormState } from './helpers'
import { ProviderField, ModelFormToggle } from './CatalogBadges'

export function ModelModal({
  initial,
  isEdit,
  onClose,
  onSave,
  isSaving,
  error,
}: {
  initial: ModelFormState
  isEdit: boolean
  onClose: () => void
  onSave: (form: ModelFormState) => void
  isSaving: boolean
  error: string | null
}) {
  const [form, setForm] = useState<ModelFormState>(initial)
  const [detailTab, setDetailTab] = useState('overview')
  const { data: providers } = useQuery({ queryKey: ['providers'], queryFn: providersApi.getAll })
  const [isCustom, setIsCustom] = useState(() => {
    if (!initial.provider) return false
    const standardProviders = ['ollama', 'lemonade', 'gemini', 'openai', 'anthropic', 'deepseek']
    const isStandardOrActive = providers?.providers?.some(p => p.name === initial.provider) || standardProviders.includes(initial.provider)
    return !isStandardOrActive
  })

  const set = <K extends keyof ModelFormState>(k: K, v: ModelFormState[K]) =>
    setForm(f => ({ ...f, [k]: v }))

  const selectedProvider = providers?.providers?.find(p => p.name === form.provider)

  const detailTabs = [
    { id: 'overview', label: 'Overview', icon: null },
    { id: 'credentials', label: 'Credentials', icon: Key },
    { id: 'network', label: 'Network', icon: Network },
    { id: 'capabilities', label: 'Capabilities', icon: Sliders },
    { id: 'params', label: 'Provider Params', icon: FileJson },
    { id: 'tags', label: 'Tags', icon: Tags },
  ]

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
      <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-2xl mx-4 max-h-[90vh] flex flex-col">
        {/* Header */}
        <div className="flex items-center justify-between p-5 border-b border-zinc-800/50 shrink-0">
          <div>
            <h2 className="text-lg font-semibold text-white">
              {isEdit ? form.display_name || form.model_id : 'Add custom model'}
            </h2>
            {isEdit && <p className="text-xs text-zinc-500 font-mono mt-0.5">{form.provider}/{form.model_id}</p>}
          </div>
          <button onClick={onClose} className="text-zinc-500 hover:text-white"><X size={18} /></button>
        </div>

        {/* Detail tabs */}
        <div className="flex items-center gap-1 px-5 pt-3 border-b border-zinc-800/50 shrink-0 overflow-x-auto">
          {detailTabs.map(tab => (
            <button
              key={tab.id}
              onClick={() => setDetailTab(tab.id)}
              className={`flex items-center gap-1.5 px-3 py-2 text-[11px] font-semibold whitespace-nowrap border-b-2 -mb-px transition-all
                ${detailTab === tab.id ? 'border-indigo-500 text-indigo-400' : 'border-transparent text-zinc-500 hover:text-zinc-300'}`}
            >
              {tab.icon && <tab.icon size={12} />}
              {tab.label}
            </button>
          ))}
        </div>

        {/* Body */}
        <div className="flex-1 overflow-y-auto p-5 space-y-5">

          {/* ── Overview tab ─────────────────────────────────────────────── */}
          {detailTab === 'overview' && (
            <>
              <div className="grid grid-cols-2 gap-4">
                <ProviderField label="Public Model Name">
                  <input value={form.display_name} onChange={e => set('display_name', e.target.value)}
                    placeholder="GPT-4o, Claude Sonnet..."
                    className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                      focus:outline-none focus:border-indigo-500/50 focus:ring-1 focus:ring-indigo-500/20" />
                </ProviderField>
                <ProviderField label="Provider *">
                  <div className="relative">
                    <select
                      value={isCustom ? '__custom__' : form.provider}
                      onChange={e => {
                        const val = e.target.value
                        if (val === '__custom__') {
                          setIsCustom(true)
                          set('provider', '')
                        } else {
                          setIsCustom(false)
                          set('provider', val)
                        }
                      }}
                      disabled={isEdit}
                      className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                        disabled:opacity-50 appearance-none focus:outline-none focus:border-indigo-500/50 focus:ring-1 focus:ring-indigo-500/20"
                    >
                      <option value="">Select provider...</option>
                      {/* Active providers */}
                      {providers?.providers?.map(p => (
                        <option key={p.name} value={p.name}>{p.name}</option>
                      ))}
                      {/* Standard providers (if not already active) */}
                      {!providers?.providers?.some(p => p.name === 'ollama') && <option value="ollama">ollama</option>}
                      {!providers?.providers?.some(p => p.name === 'lemonade') && <option value="lemonade">lemonade</option>}
                      {!providers?.providers?.some(p => p.name === 'gemini') && <option value="gemini">gemini</option>}
                      {!providers?.providers?.some(p => p.name === 'openai') && <option value="openai">openai</option>}
                      {!providers?.providers?.some(p => p.name === 'anthropic') && <option value="anthropic">anthropic</option>}
                      {!providers?.providers?.some(p => p.name === 'deepseek') && <option value="deepseek">deepseek</option>}
                      <option value="__custom__">── Custom ──</option>
                    </select>
                    {isCustom && (
                      <input value={form.provider} onChange={e => set('provider', e.target.value)}
                        placeholder="custom-provider"
                        className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                          focus:outline-none focus:border-indigo-500/50 focus:ring-1 focus:ring-indigo-500/20 mt-1" />
                    )}
                  </div>
                </ProviderField>
              </div>

              <div className="grid grid-cols-2 gap-4">
                <ProviderField label="Model ID *">
                  <input value={form.model_id} onChange={e => set('model_id', e.target.value)}
                    disabled={isEdit} placeholder="gpt-4o, claude-sonnet-4-5..."
                    className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 disabled:opacity-50 font-mono
                      focus:outline-none focus:border-indigo-500/50 focus:ring-1 focus:ring-indigo-500/20" />
                </ProviderField>
                <ProviderField label="Organization ID">
                  <input value={form.organization_id} onChange={e => set('organization_id', e.target.value)}
                    placeholder="org-xxx"
                    className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 font-mono
                      focus:outline-none focus:border-indigo-500/50 focus:ring-1 focus:ring-indigo-500/20" />
                </ProviderField>
              </div>

              {/* Inherited provider info */}
              {selectedProvider && (
                <div className="bg-zinc-950/50 border border-zinc-800/50 rounded-xl p-4 space-y-2">
                  <div className="text-[10px] font-bold text-zinc-500 uppercase tracking-wider mb-2">Inherited from provider</div>
                  <div className="flex items-center gap-2 text-xs text-zinc-400">
                    <Globe size={12} />
                    <span className="font-mono">{selectedProvider.network.base_url || 'default URL'}</span>
                  </div>
                  <div className="flex items-center gap-4 text-xs text-zinc-500">
                    <span>{selectedProvider.network.timeout_secs}s timeout</span>
                    <span>{selectedProvider.network.max_retries} retries</span>
                  </div>
                  <div className="flex items-center gap-2 text-xs text-zinc-500">
                    <Key size={11} />
                    <span>{selectedProvider.keys_count} key(s) configured</span>
                  </div>
                </div>
              )}

              {/* Pricing */}
              <div>
                <div className="text-[10px] font-bold text-zinc-500 uppercase tracking-wider mb-2">Pricing (USD / 1M tokens)</div>
                <div className="grid grid-cols-2 gap-4">
                  <ProviderField label="Input">
                    <input type="number" step="0.001" value={form.input_price_per_1m_usd}
                      onChange={e => set('input_price_per_1m_usd', e.target.value)}
                      className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                        focus:outline-none focus:border-indigo-500/50 focus:ring-1 focus:ring-indigo-500/20" />
                  </ProviderField>
                  <ProviderField label="Output">
                    <input type="number" step="0.001" value={form.output_price_per_1m_usd}
                      onChange={e => set('output_price_per_1m_usd', e.target.value)}
                      className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                        focus:outline-none focus:border-indigo-500/50 focus:ring-1 focus:ring-indigo-500/20" />
                  </ProviderField>
                </div>
              </div>

              {/* Context window */}
              <div className="grid grid-cols-2 gap-4">
                <ProviderField label="Context window (tokens)">
                  <input type="number" value={form.context_window} onChange={e => set('context_window', e.target.value)}
                    className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                      focus:outline-none focus:border-indigo-500/50 focus:ring-1 focus:ring-indigo-500/20" />
                </ProviderField>
                <ProviderField label="Max output tokens">
                  <input type="number" value={form.max_output_tokens} onChange={e => set('max_output_tokens', e.target.value)}
                    className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                      focus:outline-none focus:border-indigo-500/50 focus:ring-1 focus:ring-indigo-500/20" />
                </ProviderField>
              </div>
            </>
          )}

          {/* ── Credentials tab ──────────────────────────────────────────── */}
          {detailTab === 'credentials' && (
            <div className="space-y-4">
              <div className="text-xs text-zinc-400">
                This model will use the API keys from the selected provider.
                Override the API base URL if this model uses a different endpoint.
              </div>
              <ProviderField label="API Base URL (override)">
                <input value={form.api_base} onChange={e => set('api_base', e.target.value)}
                  placeholder={selectedProvider?.network?.base_url || 'https://api.openai.com/v1'}
                  className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 font-mono
                    focus:outline-none focus:border-indigo-500/50 focus:ring-1 focus:ring-indigo-500/20" />
              </ProviderField>
              {selectedProvider && (
                <div className="border border-zinc-800/50 rounded-xl p-4 space-y-2">
                  <div className="text-[10px] font-bold text-zinc-500 uppercase tracking-wider">Provider Keys</div>
                  {selectedProvider.keys.map((k, i) => (
                    <div key={i} className="flex items-center gap-2 text-xs text-zinc-400">
                      <Key size={11} className="text-zinc-600 shrink-0" />
                      <span className="text-zinc-300">{k.name}</span>
                      <span className="text-zinc-600 font-mono">{k.value}</span>
                      <span className="ml-auto text-zinc-600">weight: {k.weight}</span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}

          {/* ── Network tab ──────────────────────────────────────────────── */}
          {detailTab === 'network' && (
            <div className="space-y-4">
              <div className="text-xs text-zinc-400">
                Override network settings for this model. Leave empty to inherit from provider config.
              </div>
              <div className="grid grid-cols-3 gap-4">
                <ProviderField label="Timeout (seconds)">
                  <input type="number" value={form.timeout_secs} onChange={e => set('timeout_secs', e.target.value)}
                    placeholder={String(selectedProvider?.network?.timeout_secs ?? 30)}
                    className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                      focus:outline-none focus:border-indigo-500/50 focus:ring-1 focus:ring-indigo-500/20" />
                </ProviderField>
                <ProviderField label="Max Retries">
                  <input type="number" value={form.max_retries} onChange={e => set('max_retries', e.target.value)}
                    placeholder={String(selectedProvider?.network?.max_retries ?? 3)}
                    className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                      focus:outline-none focus:border-indigo-500/50 focus:ring-1 focus:ring-indigo-500/20" />
                </ProviderField>
                <ProviderField label="Stream Timeout (s)">
                  <input type="number" value={form.stream_timeout_secs} onChange={e => set('stream_timeout_secs', e.target.value)}
                    placeholder="30"
                    className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                      focus:outline-none focus:border-indigo-500/50 focus:ring-1 focus:ring-indigo-500/20" />
                </ProviderField>
              </div>

              <div className="border border-zinc-800/50 rounded-xl p-4 space-y-3">
                <div className="text-[10px] font-bold text-zinc-500 uppercase tracking-wider">Rate Limits</div>
                <div className="grid grid-cols-2 gap-4">
                  <ProviderField label="TPM (Tokens per Minute)">
                    <input type="number" value={form.tpm} onChange={e => set('tpm', e.target.value)}
                      placeholder="Unlimited"
                      className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                        focus:outline-none focus:border-indigo-500/50 focus:ring-1 focus:ring-indigo-500/20" />
                  </ProviderField>
                  <ProviderField label="RPM (Requests per Minute)">
                    <input type="number" value={form.rpm} onChange={e => set('rpm', e.target.value)}
                      placeholder="Unlimited"
                      className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                        focus:outline-none focus:border-indigo-500/50 focus:ring-1 focus:ring-indigo-500/20" />
                  </ProviderField>
                </div>
              </div>

              <ProviderField label="Model Access Groups (comma-separated)">
                <input value={form.model_access_groups} onChange={e => set('model_access_groups', e.target.value)}
                  placeholder="group-a, group-b"
                  className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                    focus:outline-none focus:border-indigo-500/50 focus:ring-1 focus:ring-indigo-500/20" />
              </ProviderField>

              <ProviderField label="Guardrails (comma-separated)">
                <input value={form.guardrails} onChange={e => set('guardrails', e.target.value)}
                  placeholder="guardrail-pii, guardrail-toxicity"
                  className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                    focus:outline-none focus:border-indigo-500/50 focus:ring-1 focus:ring-indigo-500/20" />
              </ProviderField>
            </div>
          )}

          {/* ── Capabilities tab ─────────────────────────────────────────── */}
          {detailTab === 'capabilities' && (
            <div className="space-y-4">
              <div className="text-xs text-zinc-400">Toggle model capabilities and status.</div>
              <div className="grid grid-cols-2 gap-4">
                {[
                  { k: 'supports_vision', label: 'Vision' },
                  { k: 'supports_tools', label: 'Tool calling' },
                  { k: 'supports_streaming', label: 'Streaming' },
                  { k: 'supports_embeddings', label: 'Embeddings' },
                ].map(({ k, label }) => (
                  <div key={k} className="flex items-center justify-between bg-zinc-950/50 border border-zinc-800/50 rounded-lg px-4 py-3">
                    <span className="text-sm text-zinc-300">{label}</span>
                    <ModelFormToggle
                      checked={Boolean(form[k as keyof ModelFormState])}
                      onToggle={() => set(k as keyof ModelFormState, !form[k as keyof ModelFormState] as ModelFormState[keyof ModelFormState])}
                    />
                  </div>
                ))}
              </div>
              <div className="flex items-center justify-between bg-zinc-950/50 border border-zinc-800/50 rounded-lg px-4 py-3">
                <div>
                  <span className="text-sm text-zinc-300">Deprecated</span>
                  <p className="text-[10px] text-zinc-500">Hide from model listings</p>
                </div>
                <ModelFormToggle checked={form.is_deprecated} onToggle={() => set('is_deprecated', !form.is_deprecated)} />
              </div>
              <div className="flex items-center justify-between bg-zinc-950/50 border border-zinc-800/50 rounded-lg px-4 py-3">
                <div>
                  <span className="text-sm text-zinc-300">Enabled</span>
                  <p className="text-[10px] text-zinc-500">Allow inference requests</p>
                </div>
                <ModelFormToggle checked={form.enabled} onToggle={() => set('enabled', !form.enabled)} />
              </div>
            </div>
          )}

          {/* ── Provider Params tab ──────────────────────────────────────── */}
          {detailTab === 'params' && (
            <div className="space-y-4">
              <div className="text-xs text-zinc-400">
                Provider-specific parameters as JSON. For example: <code className="text-indigo-400">{'{"aws_region_name": "us-east-1"}'}</code>
              </div>
              <ProviderField label="LiteLLM Params JSON">
                <textarea value={form.provider_params} onChange={e => set('provider_params', e.target.value)}
                  rows={10} placeholder='{"aws_region_name": "us-east-1"}'
                  className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-xs text-zinc-200 font-mono
                    focus:outline-none focus:border-indigo-500/50 focus:ring-1 focus:ring-indigo-500/20 resize-y" />
              </ProviderField>
              <ProviderField label="Model Info JSON (read-only)">
                <pre className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-xs text-emerald-400 font-mono overflow-x-auto max-h-40">
                  {JSON.stringify({
                    provider: form.provider,
                    model_id: form.model_id,
                    context_window: parseInt(form.context_window) || 0,
                    max_output_tokens: parseInt(form.max_output_tokens) || 0,
                    input_cost_per_1m: parseFloat(form.input_price_per_1m_usd) || 0,
                    output_cost_per_1m: parseFloat(form.output_price_per_1m_usd) || 0,
                    supports_vision: form.supports_vision,
                    supports_tools: form.supports_tools,
                    supports_streaming: form.supports_streaming,
                    supports_embeddings: form.supports_embeddings,
                  }, null, 2)}
                </pre>
              </ProviderField>
            </div>
          )}

          {/* ── Tags tab ─────────────────────────────────────────────────── */}
          {detailTab === 'tags' && (
            <div className="space-y-4">
              <div className="text-xs text-zinc-400">Add tags to organize and filter models.</div>
              <ProviderField label="Tags (comma-separated)">
                <input value={form.tags} onChange={e => set('tags', e.target.value)}
                  placeholder="production, critical, experimental"
                  className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                    focus:outline-none focus:border-indigo-500/50 focus:ring-1 focus:ring-indigo-500/20" />
              </ProviderField>
              {form.tags && form.tags.split(',').filter(t => t.trim()).length > 0 && (
                <div className="flex gap-1.5 flex-wrap">
                  {form.tags.split(',').filter(t => t.trim()).map((tag, i) => (
                    <span key={i} className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[10px] font-medium
                      bg-indigo-900/30 text-indigo-400 border border-indigo-800/50">
                      {tag.trim()}
                    </span>
                  ))}
                </div>
              )}
            </div>
          )}
        </div>

        {/* Error */}
        {error && (
          <div className="flex items-center gap-2 text-red-400 text-xs bg-red-900/20 border-t border-red-800/50 px-5 py-3">
            <AlertTriangle size={13} /> {error}
          </div>
        )}

        {/* Footer */}
        <div className="flex justify-between items-center px-5 py-4 border-t border-zinc-800/50 shrink-0">
          <button onClick={onClose} className="px-4 py-2 text-sm text-zinc-400 hover:text-white">Cancel</button>
          <button
            onClick={() => onSave(form)}
            disabled={isSaving || !form.model_id.trim()}
            className="px-4 py-2 text-sm bg-indigo-600 hover:bg-indigo-500 active:scale-[0.98] disabled:opacity-50
              text-white rounded-lg flex items-center gap-2 transition-colors"
          >
            {isSaving ? <RotateCcw size={14} className="animate-spin" /> : <Check size={14} />}
            {isEdit ? 'Update Model' : 'Add Model'}
          </button>
        </div>
      </div>
    </div>
  )
}

// ─── DeleteConfirm ────────────────────────────────────────────────────────────

