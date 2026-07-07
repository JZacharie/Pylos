import { useState, Fragment } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { modelsApi, providersApi, type ModelInfo } from '../lib/api'
import {
  ChevronDown, Pencil, Trash2, X, Check,
  AlertTriangle, RotateCcw, AlertCircle, Search, Filter, Info, RefreshCw, Plus,
  Activity, Play, Globe, Key, Network, Sliders, Tags, FileJson
} from 'lucide-react'

const PROVIDERS = ['all', 'anthropic', 'gemini', 'groq', 'mistral', 'deepseek', 'ollama-jo3', 'lemonade-jo3', 'lemonade-optimus']

function formatPrice(price: number): string {
  if (price === 0) return 'Free'
  return `$${price.toFixed(2)}`
}

function formatContext(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${Math.round(n / 1_000)}K`
  return String(n)
}

function CapBadge({ ok, label }: { ok: boolean; label: string }) {
  return ok ? (
    <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] bg-zinc-800 text-zinc-300 border border-zinc-700/50 font-medium">
      {label}
    </span>
  ) : null
}

// Simple deterministic hash to simulate LiteLLM's hex Model IDs
function getModelHash(modelId: string): string {
  let hash = 0;
  for (let i = 0; i < modelId.length; i++) {
    hash = (hash << 5) - hash + modelId.charCodeAt(i);
    hash |= 0;
  }
  const hex = Math.abs(hash).toString(16).padEnd(24, 'f');
  return hex.substring(0, 24);
}

// ─── Types form ───────────────────────────────────────────────────────────────

interface ModelFormState {
  provider: string
  model_id: string
  display_name: string
  context_window: string
  max_output_tokens: string
  input_price_per_1m_usd: string
  output_price_per_1m_usd: string
  supports_vision: boolean
  supports_tools: boolean
  supports_streaming: boolean
  supports_embeddings: boolean
  is_deprecated: boolean
  enabled: boolean
  api_base: string
  tpm: string
  rpm: string
  max_retries: string
  timeout_secs: string
  stream_timeout_secs: string
  model_access_groups: string
  guardrails: string
  tags: string
  provider_params: string
  organization_id: string
}

const DEFAULT_FORM: ModelFormState = {
  provider: '',
  model_id: '',
  display_name: '',
  context_window: '0',
  max_output_tokens: '0',
  input_price_per_1m_usd: '0',
  output_price_per_1m_usd: '0',
  supports_vision: false,
  supports_tools: true,
  supports_streaming: true,
  supports_embeddings: false,
  is_deprecated: false,
  enabled: true,
  api_base: '',
  tpm: '',
  rpm: '',
  max_retries: '',
  timeout_secs: '',
  stream_timeout_secs: '',
  model_access_groups: '',
  guardrails: '',
  tags: '',
  provider_params: '',
  organization_id: '',
}

function formToPayload(f: ModelFormState) {
  return {
    provider: f.provider,
    model_id: f.model_id,
    display_name: f.display_name || null,
    context_window: parseInt(f.context_window) || 0,
    max_output_tokens: parseInt(f.max_output_tokens) || 0,
    input_price_per_1m_usd: parseFloat(f.input_price_per_1m_usd) || 0,
    output_price_per_1m_usd: parseFloat(f.output_price_per_1m_usd) || 0,
    supports_vision: f.supports_vision,
    supports_tools: f.supports_tools,
    supports_streaming: f.supports_streaming,
    supports_embeddings: f.supports_embeddings,
    is_deprecated: f.is_deprecated,
    enabled: f.enabled,
    api_base: f.api_base || null,
    tpm: f.tpm ? parseInt(f.tpm) : null,
    rpm: f.rpm ? parseInt(f.rpm) : null,
    max_retries: f.max_retries ? parseInt(f.max_retries) : null,
    timeout_secs: f.timeout_secs ? parseInt(f.timeout_secs) : null,
    stream_timeout_secs: f.stream_timeout_secs ? parseInt(f.stream_timeout_secs) : null,
    model_access_groups: f.model_access_groups ? f.model_access_groups.split(',').map(s => s.trim()).filter(Boolean) : null,
    guardrails: f.guardrails ? f.guardrails.split(',').map(s => s.trim()).filter(Boolean) : null,
    tags: f.tags ? f.tags.split(',').map(s => s.trim()).filter(Boolean) : null,
    provider_params: f.provider_params ? (() => { try { return JSON.parse(f.provider_params) } catch { return null } })() : null,
    organization_id: f.organization_id || null,
  }
}

function pylosToForm(p: ModelInfo): ModelFormState {
  return {
    provider: p.provider,
    model_id: p.model_id,
    display_name: p.display_name ?? '',
    context_window: String(p.context_window),
    max_output_tokens: String(p.max_output_tokens),
    input_price_per_1m_usd: String(p.input_price_per_1m_usd),
    output_price_per_1m_usd: String(p.output_price_per_1m_usd),
    supports_vision: p.supports_vision,
    supports_tools: p.supports_tools,
    supports_streaming: p.supports_streaming,
    supports_embeddings: p.supports_embeddings,
    is_deprecated: p.is_deprecated,
    enabled: p.enabled,
    api_base: p.api_base ?? '',
    tpm: p.tpm?.toString() ?? '',
    rpm: p.rpm?.toString() ?? '',
    max_retries: p.max_retries?.toString() ?? '',
    timeout_secs: p.timeout_secs?.toString() ?? '',
    stream_timeout_secs: p.stream_timeout_secs?.toString() ?? '',
    model_access_groups: p.model_access_groups?.join(', ') ?? '',
    guardrails: p.guardrails?.join(', ') ?? '',
    tags: p.tags?.join(', ') ?? '',
    provider_params: p.provider_params ? JSON.stringify(p.provider_params, null, 2) : '',
    organization_id: p.organization_id ?? '',
  }
}

// ─── Shared sub-components (stable references to avoid focus loss) ───────────

const ProviderField = ({ label, children }: { label: string; children: React.ReactNode }) => (
  <div>
    <label className="block text-[10px] font-bold text-zinc-500 uppercase tracking-wider mb-1">{label}</label>
    {children}
  </div>
)

// ─── ModelDetailModal ─── LiteLLM-style unified model detail view ────────────

function ModelModal({
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

  const set = <K extends keyof ModelFormState>(k: K, v: ModelFormState[K]) =>
    setForm(f => ({ ...f, [k]: v }))

  const Toggle = ({ k }: { k: keyof ModelFormState }) => (
    <button
      type="button"
      onClick={() => set(k, !form[k] as ModelFormState[typeof k])}
      className={`relative w-9 h-5 rounded-full transition-colors ${form[k] ? 'bg-indigo-600' : 'bg-zinc-700'}`}
    >
      <span className={`absolute top-0.5 w-4 h-4 rounded-full bg-white shadow transition-all ${form[k] ? 'left-4' : 'left-0.5'}`} />
    </button>
  )

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
                    <select value={form.provider} onChange={e => set('provider', e.target.value)}
                      disabled={isEdit}
                      className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                        disabled:opacity-50 appearance-none focus:outline-none focus:border-indigo-500/50 focus:ring-1 focus:ring-indigo-500/20">
                      <option value="">Select provider...</option>
                      {providers?.providers?.map(p => (
                        <option key={p.name} value={p.name}>{p.name}</option>
                      ))}
                      <option value="__custom__">── Custom ──</option>
                    </select>
                    {form.provider && form.provider !== '__custom__' && !providers?.providers?.find(p => p.name === form.provider) && (
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
                    <Toggle k={k as keyof ModelFormState} />
                  </div>
                ))}
              </div>
              <div className="flex items-center justify-between bg-zinc-950/50 border border-zinc-800/50 rounded-lg px-4 py-3">
                <div>
                  <span className="text-sm text-zinc-300">Deprecated</span>
                  <p className="text-[10px] text-zinc-500">Hide from model listings</p>
                </div>
                <Toggle k="is_deprecated" />
              </div>
              <div className="flex items-center justify-between bg-zinc-950/50 border border-zinc-800/50 rounded-lg px-4 py-3">
                <div>
                  <span className="text-sm text-zinc-300">Enabled</span>
                  <p className="text-[10px] text-zinc-500">Allow inference requests</p>
                </div>
                <Toggle k="enabled" />
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

function DeleteConfirm({
  modelId,
  onClose,
  onConfirm,
  isDeleting,
}: {
  modelId: string
  onClose: () => void
  onConfirm: () => void
  isDeleting: boolean
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
      <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-sm mx-4 p-6">
        <div className="flex items-center gap-3 mb-4">
          <div className="w-9 h-9 rounded-full bg-red-500/15 flex items-center justify-center">
            <AlertTriangle size={16} className="text-red-400" />
          </div>
          <div>
            <div className="font-semibold text-white">Remove from catalog</div>
            <div className="text-xs text-zinc-500">This removes only the catalog entry</div>
          </div>
        </div>
        <p className="text-sm text-zinc-400 mb-5">
          Remove <span className="text-white font-mono text-xs">{modelId}</span> from the model catalog?
        </p>
        <div className="flex justify-end gap-3">
          <button onClick={onClose} className="px-4 py-2 text-sm text-zinc-400 hover:text-white">Cancel</button>
          <button
            onClick={onConfirm}
            disabled={isDeleting}
            className="px-4 py-2 text-sm bg-red-600 hover:bg-red-500 disabled:opacity-50 text-white rounded-lg
              flex items-center gap-2 active:scale-[0.98]"
          >
            {isDeleting ? <RotateCcw size={13} className="animate-spin" /> : <Trash2 size={13} />}
            Remove
          </button>
        </div>
      </div>
    </div>
  )
}

// ─── LLM Credentials Tab — Re-use credentials from providers ─────────────────

function LlmCredentialsTab() {
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

function PassThroughEndpointsTab() {
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

function ModelRetrySettingsTab() {
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

function PriceDataManagementTab() {
  const qc = useQueryClient()
  const { data: status, isLoading, refetch } = useQuery({
    queryKey: ['pricingStatus'],
    queryFn: () => modelsApi.getPricingStatus(),
  })

  const reloadMutation = useMutation({
    mutationFn: () => modelsApi.reloadPricingData(),
    onSuccess: (res: any) => {
      qc.invalidateQueries({ queryKey: ['models'] })
      refetch()
      alert(res.message || "Successfully reloaded pricing data")
    },
    onError: (e: any) => {
      alert(`Failed to reload pricing data: ${e.message}`)
    }
  })

  const scheduleMutation = useMutation({
    mutationFn: (schedule: string | null) => modelsApi.schedulePricingReload(schedule),
    onSuccess: () => {
      refetch()
      alert("Successfully updated schedule")
    },
    onError: (e: any) => {
      alert(`Failed to update schedule: ${e.message}`)
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

interface AliasEntry {
  alias_name: string;
  target_model: string;
}

function ModelGroupAliasTab() {
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

function ModelHealthTab() {
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
    onError: (e: any) => {
      alert(`Health check failed: ${e.message}`)
    }
  })

  const checkOneMutation = useMutation({
    mutationFn: ({ provider, model_id }: { provider: string; model_id: string }) =>
      modelsApi.runHealthCheck(provider, model_id),
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ['modelHealth'] })
    },
    onError: (e: any) => {
      alert(`Health check failed: ${e.message}`)
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

export default function ModelCatalog() {
  const qc = useQueryClient()
  const [provider, setProvider] = useState('all')
  const [search, setSearch] = useState('')
  const [expandedId, setExpandedId] = useState<string | null>(null)
  const [showCreate, setShowCreate] = useState(false)
  const [editingModel, setEditingModel] = useState<ModelInfo | null>(null)
  const [deletingModel, setDeletingModel] = useState<{ provider: string; model_id: string } | null>(null)
  const [mutationError, setMutationError] = useState<string | null>(null)
  
  // Navigation Tabs state
  const [activeTab, setActiveTab] = useState('all-models')

  const { data, isLoading, isError, refetch } = useQuery({
    queryKey: ['models', provider],
    queryFn: () => modelsApi.getAll(provider === 'all' ? undefined : provider),
    refetchInterval: 60_000,
  })

  const invalidate = () => qc.invalidateQueries({ queryKey: ['models'] })

  const upsertMutation = useMutation({
    mutationFn: (form: ModelFormState) => modelsApi.upsert(formToPayload(form)),
    onSuccess: () => {
      invalidate()
      setShowCreate(false)
      setEditingModel(null)
      setMutationError(null)
    },
    onError: (e: Error) => setMutationError(e.message),
  })

  const deleteMutation = useMutation({
    mutationFn: ({ provider, model_id }: { provider: string; model_id: string }) =>
      modelsApi.remove(provider, model_id),
    onSuccess: () => { invalidate(); setDeletingModel(null) },
  })

  const pullMutation = useMutation({
    mutationFn: (p: string) => modelsApi.pull(p),
    onSuccess: (res) => {
      invalidate()
      alert(res.message || "Successfully pulled models from provider")
    },
    onError: (e: any) => {
      const msg = e.response?.data?.error || e.message
      alert(`Failed to pull models: ${msg}`)
    }
  })

  const models = (data?.data ?? []).filter(Boolean)
  const filtered = models.filter(m => {
    if (!search) return true
    const q = search.toLowerCase()
    return m.id.toLowerCase().includes(q) ||
      (m.pylos?.display_name?.toLowerCase().includes(q) ?? false)
  })

  return (
    <div className="p-8 space-y-6 overflow-y-auto h-full bg-zinc-950 text-zinc-100">
      
      {/* Title Header with LiteLLM missing provider alert */}
      <div className="flex flex-col gap-4">
        <div className="flex items-center justify-between">
          <div>
            <h1 className="text-2xl font-bold tracking-tight text-white">Model Management</h1>
            <p className="text-xs text-zinc-400 mt-1">
              Add and manage models for the proxy
            </p>
          </div>
        </div>


      </div>

      {/* Navigation Tabs */}
      <div className="flex items-center border-b border-zinc-800/80 gap-1 overflow-x-auto">
        {(
          [
            { id: 'all-models', label: 'All Models' },
            { id: 'add-model', label: 'Add Model', onClick: () => { setMutationError(null); setShowCreate(true); } },
            { id: 'llm-credentials', label: 'LLM Credentials' },
            { id: 'endpoints', label: 'Pass-Through Endpoints' },
            { id: 'health', label: 'Health Status' },
            { id: 'retry', label: 'Model Retry Settings' },
            { id: 'alias', label: 'Model Group Alias' },
            { id: 'reload', label: 'Price Data Reload' },
          ]
        ).map(tab => (
          <button
            key={tab.id}
            onClick={() => {
              if (tab.onClick) {
                tab.onClick();
              } else {
                setActiveTab(tab.id);
              }
            }}
            className={`px-4 py-2.5 text-xs font-semibold whitespace-nowrap transition-all border-b-2 -mb-px
              ${activeTab === tab.id 
                ? 'border-indigo-500 text-indigo-400 font-bold' 
                : 'border-transparent text-zinc-500 hover:text-zinc-300'}`}
          >
            {tab.label}
          </button>
        ))}
      </div>

      {activeTab === 'reload' ? (
        <PriceDataManagementTab />
      ) : activeTab === 'alias' ? (
        <ModelGroupAliasTab />
      ) : activeTab === 'health' ? (
        <ModelHealthTab />
      ) : activeTab === 'llm-credentials' ? (
        <LlmCredentialsTab />
      ) : activeTab === 'endpoints' ? (
        <PassThroughEndpointsTab />
      ) : activeTab === 'retry' ? (
        <ModelRetrySettingsTab />
      ) : (
        <>
          {/* Dropdown Filters (Current Team & View) */}
          <div className="flex items-center gap-6 text-xs text-zinc-400 bg-zinc-900/20 p-1 rounded-lg">
            <div className="flex items-center gap-2">
              <span className="font-medium text-zinc-500">Current Team:</span>
              <div className="relative group">
                <button className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-zinc-900 border border-zinc-800 text-zinc-200 font-semibold hover:border-zinc-700 transition-colors">
                  <span className="w-1.5 h-1.5 rounded-full bg-indigo-500" />
                  Personal
                  <ChevronDown size={12} className="text-zinc-500" />
                </button>
              </div>
            </div>

            <div className="flex items-center gap-2 ml-auto">
              <span className="font-medium text-zinc-500">View:</span>
              <button className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-zinc-900 border border-zinc-800 text-zinc-200 font-semibold hover:border-zinc-700 transition-colors">
                Current Team Models
                <ChevronDown size={12} className="text-zinc-500" />
              </button>
            </div>
          </div>

          {/* Search and Filters Bar */}
          <div className="flex gap-3 items-center flex-wrap">
            <div className="relative flex-1 min-w-[240px]">
              <Search size={14} className="absolute left-3.5 top-3 text-zinc-500" />
              <input
                type="text"
                placeholder="Search model names..."
                value={search}
                onChange={e => setSearch(e.target.value)}
                className="w-full pl-10 pr-4 py-2 bg-zinc-900 border border-zinc-800 rounded-xl text-xs text-white placeholder-zinc-500 focus:outline-none focus:border-indigo-500/50 focus:ring-1 focus:ring-indigo-500/20 transition-all"
              />
            </div>
            
            <div className="flex items-center gap-2">
              <button className="flex items-center gap-1.5 px-4 py-2 bg-zinc-900 border border-zinc-800 rounded-xl text-xs text-zinc-300 hover:text-white hover:border-zinc-700 transition-colors">
                <Filter size={13} className="text-zinc-400" />
                Filters
              </button>
              <button 
                onClick={() => { setSearch(''); setProvider('all') }}
                className="px-4 py-2 bg-zinc-900 border border-zinc-800 rounded-xl text-xs text-zinc-300 hover:text-white hover:border-zinc-700 transition-colors"
              >
                Reset Filters
              </button>
              {provider !== 'all' && (
                <button
                  disabled={pullMutation.isPending}
                  onClick={() => pullMutation.mutate(provider)}
                  className="flex items-center gap-1.5 px-4 py-2 bg-indigo-600 hover:bg-indigo-500 text-white rounded-xl text-xs font-semibold disabled:opacity-50 transition-all active:scale-[0.98]"
                >
                  {pullMutation.isPending ? (
                    <RotateCcw size={12} className="animate-spin text-indigo-300" />
                  ) : (
                    <RefreshCw size={12} />
                  )}
                  Sync {provider}
                </button>
              )}
            </div>
          </div>

          {/* Sync buttons per provider */}
          <div className="flex gap-1.5 flex-wrap py-1">
            {PROVIDERS.map(p => (
              <button
                key={p}
                onClick={() => setProvider(p)}
                className={`px-3 py-1.5 rounded-lg text-[10px] font-bold uppercase tracking-wider transition-colors
                  ${provider === p
                    ? 'bg-zinc-800 text-white border border-zinc-700'
                    : 'bg-zinc-900/50 text-zinc-500 border border-zinc-800/80 hover:text-zinc-300 hover:border-zinc-700/50'}`}
              >
                {p}
              </button>
            ))}
          </div>

          {/* Error state */}
          {isError && (
            <div className="flex items-center gap-3 bg-red-900/20 border border-red-800/50 rounded-xl p-4 text-red-300">
              <AlertCircle size={16} className="shrink-0" />
              <span className="text-sm">Failed to load models. </span>
              <button onClick={() => refetch()} className="text-sm underline hover:no-underline">Retry</button>
            </div>
          )}

          {/* Models Table Listing */}
          {isLoading ? (
            <div className="space-y-4">
              {Array.from({ length: 5 }).map((_, i) => (
                <div key={i} className="h-16 rounded-xl border border-zinc-800/50 bg-zinc-900/30 animate-pulse" />
              ))}
            </div>
          ) : (
            <div className="border border-zinc-800/80 rounded-2xl bg-zinc-900/20 overflow-hidden shadow-2xl">
              <table className="w-full text-left border-collapse">
                <thead>
                  <tr className="bg-zinc-900/40 border-b border-zinc-800/80">
                    <th className="px-6 py-4 text-[10px] font-bold text-zinc-500 uppercase tracking-widest w-[25%]">Model ID</th>
                    <th className="px-6 py-4 text-[10px] font-bold text-zinc-500 uppercase tracking-widest w-[25%]">Model Information</th>
                    <th className="px-6 py-4 text-[10px] font-bold text-zinc-500 uppercase tracking-widest">Credentials</th>
                    <th className="px-6 py-4 text-[10px] font-bold text-zinc-500 uppercase tracking-widest">Created By</th>
                    <th className="px-6 py-4 text-[10px] font-bold text-zinc-500 uppercase tracking-widest">Updated At</th>
                    <th className="px-6 py-4 text-[10px] font-bold text-zinc-500 uppercase tracking-widest text-right">Costs</th>
                    <th className="px-6 py-4 text-[10px] font-bold text-zinc-500 uppercase tracking-widest text-center">Team ID</th>
                    <th className="px-6 py-4 text-[10px] font-bold text-zinc-500 uppercase tracking-widest text-center">Access Group</th>
                    <th className="px-6 py-4 text-[10px] font-bold text-zinc-500 uppercase tracking-widest text-center">Status</th>
                    <th className="px-6 py-4 w-20 text-right">Actions</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-zinc-800/50">
                  {filtered.map(({ id, pylos }) => {
                    const modelHash = getModelHash(pylos?.model_id ?? id);
                    return (
                      <Fragment key={id}>
                        <tr
                          className={`transition-colors cursor-pointer group hover:bg-zinc-900/40
                            ${expandedId === id ? 'bg-zinc-900/30' : ''}
                            ${pylos?.enabled === false ? 'opacity-40 grayscale' : ''}`}
                          onClick={() => setExpandedId(expandedId === id ? null : id)}
                        >
                          {/* Model ID */}
                          <td className="px-6 py-4">
                            <span className="font-mono text-xs text-indigo-400 hover:text-indigo-300 font-semibold truncate block max-w-[180px]">
                              {modelHash}
                            </span>
                          </td>

                          {/* Model Information */}
                          <td className="px-6 py-4">
                            <div className="flex flex-col">
                              <span className="font-bold text-zinc-100 group-hover:text-white transition-colors text-xs">
                                {pylos?.display_name || pylos?.model_id || id}
                              </span>
                              <span className="text-[10px] text-zinc-500 font-mono mt-0.5">
                                {pylos?.provider ?? 'unknown'}/{pylos?.model_id ?? id}
                              </span>
                            </div>
                          </td>

                          {/* Credentials */}
                          <td className="px-6 py-4">
                            <span className="inline-flex items-center gap-1.5 px-2.5 py-0.5 rounded-full text-[10px] font-medium bg-zinc-800 text-zinc-400 border border-zinc-700/50">
                              Manual
                            </span>
                          </td>

                          {/* Created By */}
                          <td className="px-6 py-4 text-xs text-zinc-400">
                            Defined in config
                          </td>

                          {/* Updated At */}
                          <td className="px-6 py-4 text-xs text-zinc-500 font-mono">
                            -
                          </td>

                          {/* Costs */}
                          <td className="px-6 py-4 text-right">
                            <div className="flex flex-col items-end gap-0.5">
                              <span className="text-[10px] font-mono text-zinc-300">
                                In: <span className="font-semibold text-emerald-400">{formatPrice(pylos?.input_price_per_1m_usd ?? 0)}</span>
                              </span>
                              <span className="text-[10px] font-mono text-zinc-400">
                                Out: <span className="font-semibold text-blue-400">{formatPrice(pylos?.output_price_per_1m_usd ?? 0)}</span>
                              </span>
                            </div>
                          </td>

                          {/* Team ID */}
                          <td className="px-6 py-4 text-center text-xs text-zinc-500">
                            -
                          </td>

                          {/* Access Group */}
                          <td className="px-6 py-4 text-center text-xs text-zinc-500">
                            -
                          </td>

                          {/* Status */}
                          <td className="px-6 py-4 text-center">
                            <span className="inline-flex items-center px-2 py-0.5 rounded text-[10px] font-bold bg-zinc-800 text-zinc-400 border border-zinc-700/60 uppercase tracking-wider">
                              Config Model
                            </span>
                          </td>

                          {/* Actions */}
                          <td className="px-6 py-4 text-right">
                            <div className="flex items-center justify-end gap-2" onClick={e => e.stopPropagation()}>
                              {pylos && (
                                <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                                  <button
                                    onClick={() => { setMutationError(null); setEditingModel(pylos) }}
                                    className="p-1.5 text-zinc-400 hover:text-emerald-400 hover:bg-emerald-400/10 rounded-lg transition-all"
                                    title="Edit Model"
                                  >
                                    <Pencil size={13} />
                                  </button>
                                  <button
                                    onClick={() => setDeletingModel({ provider: pylos.provider, model_id: pylos.model_id })}
                                    className="p-1.5 text-zinc-400 hover:text-red-400 hover:bg-red-400/10 rounded-lg transition-all"
                                    title="Delete Model"
                                  >
                                    <Trash2 size={13} />
                                  </button>
                                </div>
                              )}
                              <div className={`text-zinc-600 transition-transform duration-200 ${expandedId === id ? 'rotate-180' : ''}`}>
                                <ChevronDown size={14} />
                              </div>
                            </div>
                          </td>
                        </tr>

                        {/* Expanded view */}
                        {expandedId === id && pylos && (
                          <tr className="bg-zinc-900/10 border-l-2 border-l-indigo-500">
                            <td colSpan={10} className="px-8 py-6">
                              <div className="grid grid-cols-1 md:grid-cols-4 gap-8">
                                <div>
                                  <div className="text-[10px] font-bold text-zinc-500 uppercase tracking-widest mb-2">Technical Details</div>
                                  <div className="space-y-1.5">
                                    <div className="flex justify-between text-xs">
                                      <span className="text-zinc-400">Context Window</span>
                                      <span className="text-zinc-200 font-semibold font-mono">{formatContext(pylos.context_window)} tokens</span>
                                    </div>
                                    <div className="flex justify-between text-xs">
                                      <span className="text-zinc-400">Max Output</span>
                                      <span className="text-zinc-200 font-semibold font-mono">{formatContext(pylos.max_output_tokens)} tokens</span>
                                    </div>
                                    <div className="flex justify-between text-xs">
                                      <span className="text-zinc-400">Provider</span>
                                      <span className="text-zinc-200 font-semibold capitalize">{pylos.provider}</span>
                                    </div>
                                    {pylos.api_base && (
                                      <div className="flex justify-between text-xs">
                                        <span className="text-zinc-400">API Base</span>
                                        <span className="text-zinc-200 font-mono text-[10px]">{pylos.api_base}</span>
                                      </div>
                                    )}
                                    {pylos.tpm && (
                                      <div className="flex justify-between text-xs">
                                        <span className="text-zinc-400">TPM</span>
                                        <span className="text-zinc-200 font-mono">{pylos.tpm}</span>
                                      </div>
                                    )}
                                    {pylos.rpm && (
                                      <div className="flex justify-between text-xs">
                                        <span className="text-zinc-400">RPM</span>
                                        <span className="text-zinc-200 font-mono">{pylos.rpm}</span>
                                      </div>
                                    )}
                                    {pylos.max_retries && (
                                      <div className="flex justify-between text-xs">
                                        <span className="text-zinc-400">Max Retries</span>
                                        <span className="text-zinc-200 font-mono">{pylos.max_retries}</span>
                                      </div>
                                    )}
                                  </div>
                                </div>
                                <div>
                                  <div className="text-[10px] font-bold text-zinc-500 uppercase tracking-widest mb-2 font-semibold">Capabilities</div>
                                  <div className="flex gap-1.5 flex-wrap">
                                    <CapBadge ok={pylos.supports_vision} label="Vision" />
                                    <CapBadge ok={pylos.supports_tools} label="Tools" />
                                    <CapBadge ok={pylos.supports_embeddings} label="Embed" />
                                    <CapBadge ok={pylos.supports_streaming} label="Stream" />
                                  </div>
                                  {pylos.tags && pylos.tags.length > 0 && (
                                    <div className="mt-3">
                                      <div className="text-[10px] font-bold text-zinc-500 uppercase tracking-widest mb-2">Tags</div>
                                      <div className="flex gap-1 flex-wrap">
                                        {pylos.tags.map((tag, i) => (
                                          <span key={i} className="inline-flex items-center px-1.5 py-0.5 rounded text-[10px] bg-indigo-900/30 text-indigo-400 border border-indigo-800/50">
                                            {tag}
                                          </span>
                                        ))}
                                      </div>
                                    </div>
                                  )}
                                </div>
                                <div>
                                  <div className="text-[10px] font-bold text-zinc-500 uppercase tracking-widest mb-2">Cost Estimation</div>
                                  <div className="space-y-1 text-xs">
                                    <div className="text-zinc-400">Avg. request cost</div>
                                    <div className="text-base font-mono font-bold text-white">
                                      ${((pylos.input_price_per_1m_usd / 1000) + (pylos.output_price_per_1m_usd / 2000)).toFixed(4)}
                                    </div>
                                    <div className="text-[10px] text-zinc-500">(1K prompt + 500 completion)</div>
                                  </div>
                                </div>
                                <div>
                                  <div className="text-[10px] font-bold text-zinc-500 uppercase tracking-widest mb-2">Integration Snippet</div>
                                  <div className="bg-zinc-950/80 rounded-xl p-3 border border-zinc-800/80 font-mono text-[10px] text-emerald-400 overflow-x-auto">
                                    {`model: "${pylos.model_id}", // Managed by Pylos`}
                                  </div>
                                </div>
                              </div>
                            </td>
                          </tr>
                        )}
                      </Fragment>
                    )
                  })}

                  {filtered.length === 0 && !isError && (
                    <tr>
                      <td colSpan={10} className="py-24 text-center">
                        <div className="flex flex-col items-center justify-center text-zinc-500 space-y-4">
                          <div className="w-12 h-12 rounded-full bg-zinc-800/30 flex items-center justify-center">
                            <Search size={18} className="text-zinc-600" />
                          </div>
                          <div className="text-xs">No models found matching your criteria</div>
                          <button 
                            onClick={() => { setSearch(''); setProvider('all') }}
                            className="text-xs text-indigo-400 hover:underline"
                          >
                            Clear all filters
                          </button>
                        </div>
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          )}
        </>
      )}

      {/* Modals */}
      {showCreate && (
        <ModelModal
          initial={DEFAULT_FORM}
          isEdit={false}
          onClose={() => { setShowCreate(false); setMutationError(null) }}
          onSave={form => upsertMutation.mutate(form)}
          isSaving={upsertMutation.isPending}
          error={mutationError}
        />
      )}

      {editingModel && (
        <ModelModal
          initial={pylosToForm(editingModel)}
          isEdit={true}
          onClose={() => { setEditingModel(null); setMutationError(null) }}
          onSave={form => upsertMutation.mutate(form)}
          isSaving={upsertMutation.isPending}
          error={mutationError}
        />
      )}

      {deletingModel && (
        <DeleteConfirm
          modelId={`${deletingModel.provider}/${deletingModel.model_id}`}
          onClose={() => setDeletingModel(null)}
          onConfirm={() => deleteMutation.mutate(deletingModel)}
          isDeleting={deleteMutation.isPending}
        />
      )}
    </div>
  )
}
