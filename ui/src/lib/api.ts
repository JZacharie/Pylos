import axios from 'axios'

const getBaseUrl = (): string => {
  const envUrl = import.meta.env.VITE_API_URL;
  if (envUrl) {
    // If the API points to localhost/127.0.0.1 but the frontend is loaded from a different domain,
    // use relative paths to go through Nginx/reverse proxy.
    if (
      typeof window !== 'undefined' &&
      (envUrl.includes('localhost') || envUrl.includes('127.0.0.1')) &&
      window.location.hostname !== 'localhost' &&
      window.location.hostname !== '127.0.0.1'
    ) {
      return '';
    }
    return envUrl;
  }
  return '';
};

export const api = axios.create({
  timeout: 30000,
})

// Request interceptor to attach the Admin Key from localStorage and dynamically set baseURL
api.interceptors.request.use((config) => {
  config.baseURL = getBaseUrl();
  const adminKey = typeof window !== 'undefined' ? localStorage.getItem('pylos_admin_key') : null;
  if (adminKey) {
    config.headers['Authorization'] = `Bearer ${adminKey}`;
  }
  return config;
}, (error) => {
  return Promise.reject(error);
});

// Response interceptor to handle 401/403 and prompt for the Admin Key
api.interceptors.response.use(
  (response) => response,
  async (error) => {
    const originalRequest = error.config;
    if (
      typeof window !== 'undefined' &&
      error.response &&
      (error.response.status === 401 || error.response.status === 403) &&
      !originalRequest._retry
    ) {
      originalRequest._retry = true;
      const currentKey = localStorage.getItem('pylos_admin_key');
      const promptMsg = error.response.status === 401
        ? "Administration key required. Please enter your PYLOS_ADMIN_KEY:"
        : "Invalid administration key. Please enter a valid PYLOS_ADMIN_KEY:";
      const adminKey = window.prompt(promptMsg, currentKey || '');
      if (adminKey !== null) {
        localStorage.setItem('pylos_admin_key', adminKey);
        originalRequest.headers['Authorization'] = `Bearer ${adminKey}`;
        return api(originalRequest);
      }
    }
    return Promise.reject(error);
  }
);


// ─── Types ────────────────────────────────────────────────────────────────────

export interface LogEntry {
  id: string
  timestamp: number
  provider: string
  model: string
  object: string
  status: 'success' | 'error'
  latency_ms: number
  prompt_tokens: number
  completion_tokens: number
  total_tokens: number
  cost_usd: number
  finish_reason: string | null
  error_message: string | null
  virtual_key: string | null
  is_stream: boolean
  input_preview: string | null
  output_preview: string | null
  obfuscated_input: string | null
  pii_mapping: string | null
  guardrail_triggered: boolean | null
  guardrail_type: string | null
  guardrail_detail: string | null
  moderation_scores: string | null
  caveman_input: string | null
}

export interface LogStats {
  total_requests: number
  success_rate: number
  average_latency_ms: number
  total_tokens: number
  total_cost_usd: number
  total_prompt_tokens: number
  total_completion_tokens: number
  total_compression_saved_bytes: number
}

export interface HistogramBucket {
  timestamp: number
  count: number
  success: number
  error: number
}

export interface TokenBucket {
  timestamp: number
  prompt_tokens: number
  completion_tokens: number
  total_tokens: number
}

export interface LogsResponse {
  logs: LogEntry[]
  pagination: { limit: number; offset: number; total_count: number }
  stats: LogStats
  has_logs: boolean
}

export interface Provider {
  name: string
  keys_count: number
  keys: Array<{ name: string; value: string; models: string[]; weight: number }>
  network: { base_url: string | null; timeout_secs: number; max_retries: number }
}

export interface VirtualKey {
  id: string
  name: string
  description: string | null
  is_active: boolean
  value: string
  rate_limit_id: string | null
  team_alias: string | null
  team_id: string | null
  organization_id: string | null
  access_group_id: string | null
  user_email: string | null
  user_id: string | null
  created_at: number | null
  created_by: string | null
  updated_at: number | null
  last_active: number | null
  expires_at: number | null
  provider_configs: Array<{ provider: string; allowed_models: string[]; weight: number }>
}

export interface BudgetUsage {
  period: string
  max_usd: number
  current_usd: number
  reset_at_ms: number
}

export interface RateLimitStatus {
  window_type: string
  max_value: number
  current_value: number
  reset_at_ms: number
}

export interface VkBudgetResponse {
  virtual_key_id: string
  budget: BudgetUsage[]
  rate_limits: RateLimitStatus[]
}

// ─── Access Control Types ─────────────────────────────────────────────────────

export interface Organization {
  id: string
  name: string
  description: string | null
  is_active: boolean
  tags: string[]
  created_at: number
  updated_at: number
}

export interface Team {
  id: string
  organization_id: string
  name: string
  description: string | null
  is_active: boolean
  tags: string[]
  created_at: number
  updated_at: number
}

export interface InternalUser {
  id: string
  email: string
  name: string
  role: string
  organization_id: string | null
  team_ids: string[]
  is_active: boolean
  created_at: number
  updated_at: number
}

export interface AccessGroup {
  id: string
  name: string
  description: string | null
  organization_id: string | null
  team_ids: string[]
  user_ids: string[]
  model_ids: string[]
  provider_ids: string[]
  is_active: boolean
  tags: string[]
  created_at: number
  updated_at: number
}

export interface Policy {
  id: string
  name: string
  description: string | null
  policy_type: string
  config: Record<string, unknown>
  is_active: boolean
  created_at: number
  updated_at: number
}

export interface SearchToolConfig {
  id: string
  name: string
  description: string | null
  tool_type: string
  config: Record<string, unknown>
  is_active: boolean
  created_at: number
  updated_at: number
}

export interface ToolPolicy {
  id: string
  name: string
  description: string | null
  tool_type: string
  allowed_models: string[]
  allowed_providers: string[]
  max_tokens_per_call: number | null
  max_calls_per_minute: number | null
  is_active: boolean
  created_at: number
  updated_at: number
}

export interface ModelInfo {
  id: string
  provider: string
  model_id: string
  display_name: string | null
  context_window: number
  max_output_tokens: number
  input_price_per_1m_usd: number
  output_price_per_1m_usd: number
  supports_vision: boolean
  supports_tools: boolean
  supports_streaming: boolean
  supports_embeddings: boolean
  is_deprecated: boolean
  enabled: boolean
  api_base: string | null
  tpm: number | null
  rpm: number | null
  max_retries: number | null
  timeout_secs: number | null
  stream_timeout_secs: number | null
  model_access_groups: string[] | null
  guardrails: string[] | null
  tags: string[] | null
  provider_params: Record<string, unknown> | null
  organization_id: string | null
}

export interface ModelHealthEntry {
  id: string
  provider: string
  model_id: string
  health_status: string
  error_details: string | null
  last_check_ms: number | null
  last_success_ms: number | null
}

export interface ModelListResponse {
  object: string
  data: Array<{
    id: string
    object: string
    owned_by: string
    provider: string        // direct field on each entry
    pylos: ModelInfo
  }>
}

// ─── API calls ────────────────────────────────────────────────────────────────

export const logsApi = {
  getLogs: (params: Record<string, string | number>) =>
    api.get<LogsResponse>('/api/logs', { params }).then(r => r.data),

  getStats: (params: Record<string, string>) =>
    api.get<LogStats>('/api/logs/stats', { params }).then(r => r.data),

  getHistogram: (params: Record<string, string>) =>
    api.get<{ buckets: HistogramBucket[]; bucket_size_seconds: number }>(
      '/api/logs/histogram', { params }
    ).then(r => r.data),

  getTokenHistogram: (params: Record<string, string>) =>
    api.get<{ buckets: TokenBucket[]; bucket_size_seconds: number }>(
      '/api/logs/histogram/tokens', { params }
    ).then(r => r.data),

  getFilterData: () => api.get('/api/logs/filterdata').then(r => r.data),
}

export const providersApi = {
  getAll: () =>
    api.get<{ providers: Provider[]; total: number }>('/providers').then(r => r.data),

  create: (data: { name: string } & Record<string, unknown>) =>
    api.post('/providers', data).then(r => r.data),

  update: (name: string, data: Record<string, unknown>) =>
    api.put(`/providers/${name}`, data).then(r => r.data),

  remove: (name: string) =>
    api.delete(`/providers/${name}`).then(r => r.data),

  test: (name: string) =>
    api.post(`/providers/${name}/test`).then(r => r.data),
}

export const virtualKeysApi = {
  getAll: () =>
    api.get<{ virtual_keys: VirtualKey[]; total: number }>('/virtual-keys').then(r => r.data),

  create: (data: Record<string, unknown>) =>
    api.post<{ id: string; name: string; value: string }>('/virtual-keys', data).then(r => r.data),

  update: (id: string, data: Record<string, unknown>) =>
    api.put(`/virtual-keys/${id}`, data).then(r => r.data),

  remove: (id: string) =>
    api.delete(`/virtual-keys/${id}`).then(r => r.data),

  getBudget: (id: string) =>
    api.get<VkBudgetResponse>(`/virtual-keys/${id}/budget`).then(r => r.data),

  revealValue: (id: string) =>
    api.get<{ id: string; value: string }>(`/virtual-keys/${id}/value`).then(r => r.data),
}

export const modelsApi = {
  getAll: (provider?: string) =>
    api.get<ModelListResponse>('/v1/models', {
      params: provider ? { provider } : {}
    }).then(r => r.data),

  upsert: (data: Record<string, unknown>) =>
    api.post('/v1/models/catalog', data).then(r => r.data),

  remove: (provider: string, model_id: string) =>
    api.delete(`/v1/models/catalog/${provider}/${model_id}`).then(r => r.data),

  removeProviderModels: (provider: string) =>
    api.delete<{ message: string; deleted_count: number }>(`/v1/models/catalog/${provider}`).then(r => r.data),

  pull: (provider: string) =>
    api.post(`/v1/models/pull/${provider}`).then(r => r.data),

  getPricingStatus: () =>
    api.get<{ source_url: string; last_reload_ms: number | null; models_count: number; periodic_schedule: string | null }>('/v1/models/pricing/status').then(r => r.data),

  reloadPricingData: () =>
    api.post<{ success: boolean; message: string; status: any }>('/v1/models/pricing/reload').then(r => r.data),

  schedulePricingReload: (schedule: string | null) =>
    api.post<{ success: boolean; status: any }>('/v1/models/pricing/schedule', { schedule }).then(r => r.data),

  getHealth: () =>
    api.get<ModelHealthEntry[]>('/v1/models/health').then(r => r.data),

  runHealthCheck: (provider: string, model_id: string) =>
    api.post<ModelHealthEntry>('/v1/models/health/check', { provider, model_id }).then(r => r.data),

  runAllHealthChecks: () =>
    api.post<ModelHealthEntry[]>('/v1/models/health/check_all').then(r => r.data),
}

export const healthApi = {
  check: () => api.get('/health').then(r => r.data),
  getRoot: () => api.get('/').then(r => r.data),
}

export const organizationsApi = {
  getAll: (tag?: string) =>
    api.get<{ organizations: Organization[]; total: number }>('/api/organizations', { params: tag ? { tag } : {} }).then(r => r.data),
  get: (id: string) =>
    api.get<Organization>(`/api/organizations/${id}`).then(r => r.data),
  create: (data: { name: string; description?: string | null; is_active?: boolean; tags?: string[] }) =>
    api.post<Organization>('/api/organizations', data).then(r => r.data),
  update: (id: string, data: { name?: string; description?: string | null; is_active?: boolean; tags?: string[] }) =>
    api.put<Organization>(`/api/organizations/${id}`, data).then(r => r.data),
  remove: (id: string) =>
    api.delete(`/api/organizations/${id}`).then(r => r.data),
}

export const teamsApi = {
  getAll: (tag?: string) =>
    api.get<{ teams: Team[]; total: number }>('/api/teams', { params: tag ? { tag } : {} }).then(r => r.data),
  get: (id: string) =>
    api.get<Team>(`/api/teams/${id}`).then(r => r.data),
  create: (data: { organization_id: string; name: string; description?: string | null; is_active?: boolean; tags?: string[] }) =>
    api.post<Team>('/api/teams', data).then(r => r.data),
  update: (id: string, data: { name?: string; description?: string | null; is_active?: boolean; tags?: string[] }) =>
    api.put<Team>(`/api/teams/${id}`, data).then(r => r.data),
  remove: (id: string) =>
    api.delete(`/api/teams/${id}`).then(r => r.data),
}

export const usersApi = {
  getAll: () =>
    api.get<{ users: InternalUser[]; total: number }>('/api/users').then(r => r.data),
  get: (id: string) =>
    api.get<InternalUser>(`/api/users/${id}`).then(r => r.data),
  create: (data: { email: string; name: string; role?: string; organization_id?: string | null; team_ids?: string[]; is_active?: boolean }) =>
    api.post<InternalUser>('/api/users', data).then(r => r.data),
  update: (id: string, data: { email?: string; name?: string; role?: string; organization_id?: string | null; team_ids?: string[]; is_active?: boolean }) =>
    api.put<InternalUser>(`/api/users/${id}`, data).then(r => r.data),
  remove: (id: string) =>
    api.delete(`/api/users/${id}`).then(r => r.data),
}

export const accessGroupsApi = {
  getAll: (tag?: string) =>
    api.get<{ access_groups: AccessGroup[]; total: number }>('/api/access-groups', { params: tag ? { tag } : {} }).then(r => r.data),
  get: (id: string) =>
    api.get<AccessGroup>(`/api/access-groups/${id}`).then(r => r.data),
  create: (data: { name: string; description?: string | null; organization_id?: string | null; team_ids?: string[]; user_ids?: string[]; model_ids?: string[]; provider_ids?: string[]; is_active?: boolean; tags?: string[] }) =>
    api.post<AccessGroup>('/api/access-groups', data).then(r => r.data),
  update: (id: string, data: { name?: string; description?: string | null; model_ids?: string[]; provider_ids?: string[]; is_active?: boolean; tags?: string[] }) =>
    api.put<AccessGroup>(`/api/access-groups/${id}`, data).then(r => r.data),
  remove: (id: string) =>
    api.delete(`/api/access-groups/${id}`).then(r => r.data),
}

export const policiesApi = {
  getAll: () =>
    api.get<{ policies: Policy[]; total: number }>('/api/policies').then(r => r.data),
  create: (data: { name: string; description?: string | null; policy_type: string; config?: Record<string, unknown>; is_active?: boolean }) =>
    api.post<Policy>('/api/policies', data).then(r => r.data),
  update: (id: string, data: { name?: string; description?: string | null; policy_type?: string; config?: Record<string, unknown>; is_active?: boolean }) =>
    api.put<Policy>(`/api/policies/${id}`, data).then(r => r.data),
  remove: (id: string) =>
    api.delete(`/api/policies/${id}`).then(r => r.data),
}

export const toolPoliciesApi = {
  getAll: () =>
    api.get<{ tool_policies: ToolPolicy[]; total: number }>('/api/tool-policies').then(r => r.data),
  create: (data: { name: string; description?: string | null; tool_type: string; allowed_models?: string[]; allowed_providers?: string[]; max_tokens_per_call?: number | null; max_calls_per_minute?: number | null; is_active?: boolean }) =>
    api.post<ToolPolicy>('/api/tool-policies', data).then(r => r.data),
  update: (id: string, data: { name?: string; description?: string | null; tool_type?: string; allowed_models?: string[]; allowed_providers?: string[]; is_active?: boolean }) =>
    api.put<ToolPolicy>(`/api/tool-policies/${id}`, data).then(r => r.data),
  remove: (id: string) =>
    api.delete(`/api/tool-policies/${id}`).then(r => r.data),
}

export const searchToolsApi = {
  getAll: () =>
    api.get<{ search_tools: SearchToolConfig[]; total: number }>('/api/search-tools').then(r => r.data),
  create: (data: { name: string; description?: string | null; tool_type: string; config?: Record<string, unknown>; is_active?: boolean }) =>
    api.post<SearchToolConfig>('/api/search-tools', data).then(r => r.data),
  update: (id: string, data: { name?: string; description?: string | null; tool_type?: string; config?: Record<string, unknown>; is_active?: boolean }) =>
    api.put<SearchToolConfig>(`/api/search-tools/${id}`, data).then(r => r.data),
  remove: (id: string) =>
    api.delete(`/api/search-tools/${id}`).then(r => r.data),
}

export const configApi = {
  get: () => api.get('/config').then(r => r.data),
  reload: () => api.post('/config/reload').then(r => r.data),
  promote: () => api.post<{ success: boolean; message: string }>('/api/github/promote').then(r => r.data),
  updateGuardrails: (data: { enabled: boolean; config: Record<string, any> }) => 
    api.put('/config/guardrails', data).then(r => r.data),
}

export const authApi = {
  getConfig: () =>
    api.get<{ google_auth_enabled: boolean; google_client_id: string | null; google_redirect_uri: string | null }>('/api/auth/config').then(r => r.data),
  googleCallback: (code: string, redirectUri?: string) =>
    api.post<{ token: string; user: { email: string; name: string; role: string } }>('/api/auth/google/callback', { code, redirect_uri: redirectUri }).then(r => r.data),
  logout: () =>
    api.post<{ status: string; message: string }>('/api/auth/logout').then(r => r.data),
}

export interface GuardrailsEvent {
  id: string
  timestamp: number
  provider: string
  model: string
  finish_reason: string | null
  input_preview: string | null
  output_preview: string | null
  guardrail_triggered: boolean | null
  guardrail_type: string | null
  guardrail_detail: string | null
}

export interface GuardrailsBreakdown {
  total_blocks: number
  keyword_blocks: number
  prompt_injection_blocks: number
  content_filter_blocks: number
  top_keywords: Array<{ keyword: string; count: number }>
}

export interface GuardrailsTimeline {
  timestamp: number
  total: number
  keyword_blocks: number
  prompt_injection: number
  content_filter: number
}

export interface GuardrailsEventsResponse {
  events: GuardrailsEvent[]
  pagination: { limit: number; offset: number; total_count: number }
  stats: GuardrailsBreakdown
  has_events: boolean
}

export interface GuardrailsStatsResponse {
  breakdown: GuardrailsBreakdown
  timeline: GuardrailsTimeline[]
}

export const guardrailsApi = {
  getEvents: (params?: { limit?: number; offset?: number; period?: string; guardrail_type?: string }) =>
    api.get<GuardrailsEventsResponse>('/api/guardrails/events', { params }).then(r => r.data),
  getStats: (params?: { period?: string; since_ms?: number; until_ms?: number }) =>
    api.get<GuardrailsStatsResponse>('/api/guardrails/stats', { params }).then(r => r.data),
}

export interface VectorCollection {
  name: string
  status: string
  points_count: number
  vector_size: number
  distance: string
}

export const vectorStoresApi = {
  getAll: () =>
    api.get<{ collections: VectorCollection[] }>('/api/vector-stores/collections').then(r => r.data),
  create: (data: { name: string; vector_size: number; distance: string }) =>
    api.post('/api/vector-stores/collections', data).then(r => r.data),
  remove: (name: string) =>
    api.delete(`/api/vector-stores/collections/${name}`).then(r => r.data),
  addDocument: (name: string, data: { text: string; embedding_model: string; payload?: Record<string, unknown> }) =>
    api.post(`/api/vector-stores/collections/${name}/points`, data).then(r => r.data),
  search: (name: string, data: { query: string; embedding_model: string; limit?: number }) =>
    api.post<Array<{ id: string | number; score: number; payload: Record<string, unknown> }>>(`/api/vector-stores/collections/${name}/search`, data).then(r => r.data),
}

// ─── MCP Server Types & API ──────────────────────────────────────────────────

export interface McpServer {
  id: string
  name: string
  server_type: string
  status: string
  target_url: string | null
  container_image: string | null
  env_vars: Record<string, string> | null
  virtual_key_id: string | null
  team_id: string | null
  created_at: number
  updated_at: number
}

export const mcpServersApi = {
  getAll: () =>
    api.get<McpServer[]>('/api/mcp-servers').then(r => r.data),
  get: (id: string) =>
    api.get<McpServer>(`/api/mcp-servers/${id}`).then(r => r.data),
  create: (data: { name: string; server_type: string; target_url?: string; container_image?: string; env_vars?: Record<string, string>; virtual_key_id?: string; team_id?: string }) =>
    api.post<McpServer>('/api/mcp-servers', data).then(r => r.data),
  update: (id: string, data: Partial<{ name: string; server_type: string; status: string; target_url: string; container_image: string; env_vars: Record<string, string>; virtual_key_id: string; team_id: string }>) =>
    api.put<McpServer>(`/api/mcp-servers/${id}`, data).then(r => r.data),
  remove: (id: string) =>
    api.delete(`/api/mcp-servers/${id}`).then(r => r.data),
  activate: (id: string) =>
    api.post<McpServer>(`/api/mcp-servers/${id}/activate`).then(r => r.data),
  deactivate: (id: string) =>
    api.post<McpServer>(`/api/mcp-servers/${id}/deactivate`).then(r => r.data),
}

export const setupApi = {
  getStatus: () =>
    api.get<{ setup_required: boolean }>('/api/setup/status').then(r => r.data),
  setup: (password: string, confirmPassword: string) =>
    api.post<{ status: string; message: string }>('/api/setup', { password, confirm_password: confirmPassword }).then(r => r.data),
}
