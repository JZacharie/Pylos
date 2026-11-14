import type { ModelInfo } from '../../lib/api'

export const PROVIDERS = ['all', 'antigravity', 'anthropic', 'gemini', 'groq', 'mistral', 'deepseek', 'ollama-jo3', 'lemonade-jo3', 'lemonade-optimus']

export function formatPrice(price: number): string {
  if (price === 0) return 'Free'
  return `$${price.toFixed(2)}`
}

export function formatContext(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${Math.round(n / 1_000)}K`
  return String(n)
}

export function getModelHash(modelId: string): string {
  let hash = 2166136261
  for (let i = 0; i < modelId.length; i++) {
    hash ^= modelId.charCodeAt(i)
    hash = Math.imul(hash, 16777619)
  }
  return (hash >>> 0).toString(16).padStart(8, '0')
}
export interface ModelFormState {
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

export const DEFAULT_FORM: ModelFormState = {
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

export function formToPayload(f: ModelFormState) {
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

export function pylosToForm(p: ModelInfo): ModelFormState {
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

