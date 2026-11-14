import { useState } from 'react'
import { Link } from 'react-router-dom'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { Shield, Save, Check, RotateCcw, Search, Info, Activity } from 'lucide-react'
import { configApi, getErrorMessage } from '../lib/api'

interface GuardrailItem {
  id: string
  name: string
  description: string
  category: 'content_filter' | 'partner'
  tag?: string
  configKey?: string
}

const LITELLM_FILTERS: GuardrailItem[] = [
  { id: 'financial_advice', name: 'Denied Financial Advice', description: 'Detects requests for personalized financial advice, investment recommendations...', category: 'content_filter' },
  { id: 'insults', name: 'Insults & Personal Attacks', description: 'Detects insults, name-calling, and personal attacks directed at the chatbot, staff, or other...', category: 'content_filter' },
  { id: 'legal_advice', name: 'Denied Legal Advice', description: 'Detects requests for unauthorized legal advice, case analysis, or legal recommendations...', category: 'content_filter' },
  { id: 'medical_advice', name: 'Denied Medical Advice', description: 'Detects requests for medical diagnosis, treatment recommendations, or health advice...', category: 'content_filter' },
  { id: 'violence', name: 'Harmful Violence', description: 'Detects content related to violence, criminal planning, attacks, and violent threats.', category: 'content_filter' },
  { id: 'self_harm', name: 'Harmful Self-Harm', description: 'Detects content related to self-harm, suicide, and dangerous, self-destructive behavior.', category: 'content_filter' },
  { id: 'child_safety', name: 'Harmful Child Safety', description: 'Detects content that could endanger child safety or exploit minors.', category: 'content_filter' },
  { id: 'illegal_weapons', name: 'Harmful Illegal Weapons', description: 'Detects content related to illegal weapons manufacturing, distribution, or acquisition.', category: 'content_filter' },
  { id: 'bias_gender', name: 'Bias: Gender', description: 'Detects gender-based discrimination, stereotypes, and biased language.', category: 'content_filter' },
  { id: 'bias_racial', name: 'Bias: Racial', description: 'Detects social discrimination, stereotypes, and racially biased content.', category: 'content_filter' },
  { id: 'bias_religious', name: 'Bias: Religious', description: 'Detects religious discrimination, intolerance, and religiously biased content.', category: 'content_filter' },
  { id: 'bias_sexual_orientation', name: 'Bias: Sexual Orientation', description: 'Detects discrimination based on sexual orientation and related biased content.', category: 'content_filter' },
  { id: 'jailbreak', name: 'Prompt Injection: Jailbreak', description: 'Detects jailbreak attempts designed to bypass AI safety guidelines and restrictions.', category: 'content_filter', configKey: 'prevent_prompt_injection' },
  { id: 'data_exfiltration', name: 'Prompt Injection: Data Exfiltration', description: 'Detects attempts to extract sensitive data through prompt manipulation.', category: 'content_filter' },
  { id: 'sql_injection', name: 'Prompt Injection: SQL', description: 'Detects SQL injection attempts embedded in prompts.', category: 'content_filter' },
  { id: 'malicious_code', name: 'Prompt Injection: Malicious Code', description: 'Detects attempts to inject malicious code through prompts.', category: 'content_filter' },
  { id: 'system_prompt', name: 'Prompt Injection: System Prompt', description: 'Detects attempts to extract or override system prompts.', category: 'content_filter' },
  { id: 'toxic_language', name: 'Toxic & Abusive Language', description: 'Detects toxic, abusive, and hateful language across multiple languages (FN, DE, ES, IT).', category: 'content_filter' },
  { id: 'pattern_matching', name: 'Pattern Matching', description: 'Detect and block sensitive data patterns (e.g. SSNs, credit card numbers, API keys, and custom regex).', category: 'content_filter', configKey: 'mask_pii' },
  { id: 'keyword_blocking', name: 'Keyword Blocking', description: 'Block or mask queries containing specific keywords or phrases. Upload custom word lists.', category: 'content_filter' },
  { id: 'block_code_execution', name: 'Block Code Execution', description: 'Detect markdown fenced code blocks in requests and responses. Block or mask executable code.', category: 'content_filter' },
  { id: 'competitor_blocking', name: 'Competitor Name Blocking', description: 'Block or reframe competitor comparison and ranking requests. Detect when users ask to compare.', category: 'content_filter' },
]

const PARTNER_GUARDRAILS: GuardrailItem[] = [
  { id: 'presidio', name: 'Presidio PII', description: 'Microsoft Presidio for PII detection and anonymization. Supports 30+ entity types with configurable actions.', category: 'partner', configKey: 'mask_pii' },
  { id: 'bedrock_guardrail', name: 'Bedrock Guardrail', description: 'AWS Bedrock Guardrails for content filtering, topic avoidance, and sensitive information detection.', category: 'partner' },
  { id: 'lakera', name: 'Lakera', description: 'AI security platform protecting against prompt injections, data leakage, and harmful content.', category: 'partner' },
  { id: 'openai_moderation', name: 'OpenAI Moderation', description: 'OpenAI\'s content moderation API for detecting harmful content across multiple categories.', category: 'partner' },
  { id: 'gcp_model_armor', name: 'Google Cloud Model Armor', description: 'Google Cloud\'s model protection service for safe and responsible AI deployments.', category: 'partner' },
  { id: 'guardrails_ai', name: 'Guardrails AI', description: 'Open-source framework for adding structural, type, and quality guarantees to LLM outputs.', category: 'partner' },
  { id: 'zscaler_guard', name: 'Zscaler AI Guard', description: 'Enterprise AI security from Zscaler for monitoring and protecting LLM workloads.', category: 'partner' },
  { id: 'prisma_ars', name: 'PANW Prisma ARS', description: 'Palo Alto Networks Prisma AI Runtime Security for securing AI applications in production.', category: 'partner' },
  { id: 'noma_security', name: 'Noma Security', description: 'AI security platform for detecting and preventing AI-specific threats and vulnerabilities.', category: 'partner' },
  { id: 'aporia_ai', name: 'Aporia AI', description: 'Real-time AI guardrails for hallucination detection, topic control, and policy enforcement.', category: 'partner' },
  { id: 'aim_guardrail', name: 'AIM Guardrail', description: 'AIM Security platform for comprehensive AI threat detection and mitigation.', category: 'partner' },
  { id: 'prompt_security', name: 'Prompt Security', description: 'Protect against prompt injection attacks, data leakage, and other LLM security threats.', category: 'partner' },
  { id: 'lasso_guardrail', name: 'Lasso Guardrail', description: 'Content moderation and safety guardrails for responsible AI deployments.', category: 'partner' },
  { id: 'pangea_guardrail', name: 'Pangea Guardrail', description: 'Pangea\'s AI guardrails for secure, compliant, and trustworthy AI applications.', category: 'partner' },
  { id: 'enkrypt_ai', name: 'EnkryptAI', description: 'AI security and governance platform for enterprise AI safety and compliance.', category: 'partner' },
  { id: 'javelin_guardrails', name: 'Javelin Guardrails', description: 'AI gateway with built-in guardrails for secure and compliant AI operations.', category: 'partner' },
  { id: 'pillar_guardrail', name: 'Pillar Guardrail', description: 'AI safety platform for monitoring, testing, and securing AI systems.', category: 'partner' },
]

export default function Guardrails() {
  const queryClient = useQueryClient()
  const [activeTab, setActiveTab] = useState<'garden' | 'config'>('garden')
  const [searchQuery, setSearchQuery] = useState('')
  const [showLessFilters, setShowLessFilters] = useState(false)

  const { data: configData, isLoading } = useQuery({
    queryKey: ['config'],
    queryFn: configApi.get,
  })

  const [enabled, setEnabled] = useState(false)
  const [configValues, setConfigValues] = useState<Record<string, unknown>>({
    mask_pii: true,
    mask_secrets: false,
    prevent_prompt_injection: false,
    blocked_keywords: [],
  })

  // Sync state when config loads
  const [prevConfigData, setPrevConfigData] = useState(configData)
  if (configData !== prevConfigData) {
    setPrevConfigData(configData)
    if (configData && configData.plugins) {
      const guardrailsPlugin = configData.plugins.find((p: { name: string; enabled: boolean; config?: Record<string, unknown> }) => p.name === 'guardrails')
      if (guardrailsPlugin) {
        setEnabled(guardrailsPlugin.enabled)
        setConfigValues(guardrailsPlugin.config || {})
      }
    }
  }

  const mutation = useMutation({
    mutationFn: configApi.updateGuardrails,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['config'] })
      alert('Guardrails configuration updated successfully')
    },
    onError: (error: unknown) => {
      alert(`Failed to update guardrails: ${getErrorMessage(error)}`)
    }
  })

  const handleToggleItem = (item: GuardrailItem) => {
    const key = item.configKey || `toggle_${item.id}`
    setConfigValues(prev => ({
      ...prev,
      [key]: !prev[key]
    }))
  }

  const isItemActive = (item: GuardrailItem) => {
    const key = item.configKey || `toggle_${item.id}`
    return !!configValues[key]
  }

  const handleSave = () => {
    mutation.mutate({
      enabled,
      config: configValues
    })
  }

  if (isLoading) {
    return (
      <div className="flex-1 flex items-center justify-center p-8">
        <div className="text-zinc-500 animate-pulse">Loading configuration...</div>
      </div>
    )
  }

  const filteredFilters = LITELLM_FILTERS.filter(
    item => item.name.toLowerCase().includes(searchQuery.toLowerCase()) || 
            item.description.toLowerCase().includes(searchQuery.toLowerCase())
  )

  const displayedFilters = showLessFilters ? filteredFilters.slice(0, 12) : filteredFilters

  const filteredPartners = PARTNER_GUARDRAILS.filter(
    item => item.name.toLowerCase().includes(searchQuery.toLowerCase()) || 
            item.description.toLowerCase().includes(searchQuery.toLowerCase())
  )

  return (
    <div className="flex-1 flex flex-col h-full bg-zinc-950 text-zinc-100 overflow-y-auto">
      {/* Header and Tabs */}
      <header className="border-b border-zinc-800 bg-zinc-900/30 shrink-0">
        <div className="px-6 py-4 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="p-2 bg-emerald-500/10 rounded-lg">
              <Shield className="text-emerald-400 w-5 h-5" />
            </div>
            <h1 className="text-lg font-semibold text-white">Guardrails</h1>
          </div>
          
          <div className="flex bg-zinc-900 border border-zinc-800 rounded-lg p-0.5">
            <button
              onClick={() => setActiveTab('garden')}
              className={`px-3 py-1.5 rounded-md text-xs font-medium transition-all ${
                activeTab === 'garden' ? 'bg-zinc-800 text-white shadow' : 'text-zinc-400 hover:text-zinc-200'
              }`}
            >
              Guardrail Garden
            </button>
            <button
              onClick={() => setActiveTab('config')}
              className={`px-3 py-1.5 rounded-md text-xs font-medium transition-all ${
                activeTab === 'config' ? 'bg-zinc-800 text-white shadow' : 'text-zinc-400 hover:text-zinc-200'
              }`}
            >
              Classic Settings
            </button>
            <Link
              to="/guardrails-monitor"
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium text-emerald-400 hover:text-emerald-300 hover:bg-zinc-800/60 transition-all"
            >
              <Activity size={13} className="animate-pulse" />
              <span>Live Monitor</span>
            </Link>
          </div>
        </div>
      </header>

      {activeTab === 'garden' ? (
        <div className="p-6 space-y-8 max-w-[1600px] mx-auto w-full">
          {/* Controls Bar */}
          <div className="flex items-center justify-between gap-4 bg-zinc-900/30 border border-zinc-800/80 rounded-xl p-4">
            <div className="relative w-80">
              <Search className="absolute left-3 top-2.5 w-4 h-4 text-zinc-500" />
              <input
                type="text"
                value={searchQuery}
                onChange={e => setSearchQuery(e.target.value)}
                placeholder="Search guardrails..."
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg pl-9 pr-4 py-2 text-xs text-zinc-200 focus:outline-none focus:border-emerald-500/50"
              />
            </div>

            <div className="flex items-center gap-4">
              <div className="flex items-center gap-3">
                <span className="text-xs text-zinc-400">Global Guardrails Status</span>
                <button
                  onClick={() => setEnabled(!enabled)}
                  className={`relative inline-flex h-6 w-11 items-center rounded-full transition-colors ${
                    enabled ? 'bg-emerald-500' : 'bg-zinc-700'
                  }`}
                >
                  <span className={`inline-block h-4 w-4 transform rounded-full bg-white transition ${
                    enabled ? 'translate-x-6' : 'translate-x-1'
                  }`} />
                </button>
              </div>

              <button
                onClick={handleSave}
                disabled={mutation.isPending}
                className="flex items-center gap-2 px-4 py-2 bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-white text-xs font-semibold rounded-lg transition-all"
              >
                {mutation.isPending ? <RotateCcw size={14} className="animate-spin" /> : <Save size={14} />}
                Save Garden
              </button>
            </div>
          </div>

          {/* Section: LiteLLM Filters */}
          <div className="space-y-4">
            <div className="flex items-center justify-between">
              <div>
                <h2 className="text-base font-bold text-white">LiteLLM Content Filter</h2>
                <p className="text-xs text-zinc-500 mt-0.5">Built-in guardrails powered by LiteLLM. Zero latency, no external dependencies.</p>
              </div>
              <button 
                onClick={() => setShowLessFilters(!showLessFilters)} 
                className="text-xs text-emerald-400 hover:text-emerald-300 font-medium"
              >
                {showLessFilters ? 'Show all' : 'Show less'}
              </button>
            </div>

            <div className="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4">
              {displayedFilters.map(item => {
                const active = isItemActive(item)
                return (
                  <div
                    key={item.id}
                    onClick={() => handleToggleItem(item)}
                    className={`border rounded-xl p-4 cursor-pointer transition-all flex flex-col justify-between min-h-[140px] hover:scale-[1.01] hover:shadow-lg ${
                      active
                        ? 'bg-emerald-500/10 border-emerald-500/30 text-white shadow-emerald-500/5'
                        : 'bg-zinc-900/40 border-zinc-800/80 text-zinc-400 hover:border-zinc-700'
                    }`}
                  >
                    <div>
                      <div className="flex items-start justify-between gap-2">
                        <span className={`font-semibold text-xs transition-colors ${active ? 'text-emerald-400' : 'text-zinc-200'}`}>
                          {item.name}
                        </span>
                        <div className={`w-3.5 h-3.5 rounded-full border flex items-center justify-center shrink-0 ${
                          active ? 'border-emerald-400 bg-emerald-400 text-zinc-950' : 'border-zinc-700'
                        }`}>
                          {active && <Check className="w-2.5 h-2.5 stroke-[3]" />}
                        </div>
                      </div>
                      <p className="text-[11px] text-zinc-500 mt-2 leading-relaxed">
                        {item.description}
                      </p>
                    </div>
                    {active && (
                      <div className="mt-3 flex items-center gap-1 text-[10px] text-emerald-400 font-medium font-mono">
                        <CheckCircle size={10} /> Active Filter
                      </div>
                    )}
                  </div>
                )
              })}
            </div>
          </div>

          {/* Section: Partner Guardrails */}
          <div className="space-y-4 pt-4">
            <div>
              <h2 className="text-base font-bold text-white">Partner Guardrails</h2>
              <p className="text-xs text-zinc-500 mt-0.5">Third party guardrail integrations from leading AI security providers.</p>
            </div>

            <div className="grid grid-cols-1 sm:grid-cols-2 md:grid-cols-3 lg:grid-cols-4 gap-4">
              {filteredPartners.map(item => {
                const active = isItemActive(item)
                return (
                  <div
                    key={item.id}
                    onClick={() => handleToggleItem(item)}
                    className={`border rounded-xl p-4 cursor-pointer transition-all flex flex-col justify-between min-h-[140px] hover:scale-[1.01] hover:shadow-lg ${
                      active
                        ? 'bg-blue-500/10 border-blue-500/30 text-white shadow-blue-500/5'
                        : 'bg-zinc-900/40 border-zinc-800/80 text-zinc-400 hover:border-zinc-700'
                    }`}
                  >
                    <div>
                      <div className="flex items-start justify-between gap-2">
                        <span className={`font-semibold text-xs transition-colors ${active ? 'text-blue-400' : 'text-zinc-200'}`}>
                          {item.name}
                        </span>
                        <div className={`w-3.5 h-3.5 rounded-full border flex items-center justify-center shrink-0 ${
                          active ? 'border-blue-400 bg-blue-400 text-zinc-950' : 'border-zinc-700'
                        }`}>
                          {active && <Check className="w-2.5 h-2.5 stroke-[3]" />}
                        </div>
                      </div>
                      <p className="text-[11px] text-zinc-500 mt-2 leading-relaxed">
                        {item.description}
                      </p>
                    </div>
                    {active && (
                      <div className="mt-3 flex items-center gap-1 text-[10px] text-blue-400 font-medium font-mono">
                        <CheckCircle size={10} /> Enabled Integration
                      </div>
                    )}
                  </div>
                )
              })}
            </div>
          </div>
        </div>
      ) : (
        /* Classic Settings view */
        <div className="px-6 py-8 max-w-4xl mx-auto w-full space-y-6">
          <div className="bg-zinc-900 border border-zinc-800/50 rounded-2xl p-5 shadow-xl flex items-center gap-3">
            <Info className="text-zinc-400 w-5 h-5 shrink-0" />
            <p className="text-xs text-zinc-400">
              Classic settings maps directly to the active system values. Changes saved here will synchronize with the Guardrail Garden catalog switches.
            </p>
          </div>

          <div className="bg-zinc-900 border border-zinc-800/50 rounded-2xl p-6 shadow-xl relative overflow-hidden">
            <div className="relative flex items-center justify-between">
              <div>
                <h2 className="text-base font-semibold text-white mb-1">Enable Guardrails</h2>
                <p className="text-zinc-500 text-xs">
                  Globally enable or disable all guardrail features for the gateway.
                </p>
              </div>
              <button
                onClick={() => setEnabled(!enabled)}
                className={`relative inline-flex h-7 w-12 items-center rounded-full transition-colors ${
                  enabled ? 'bg-emerald-500' : 'bg-zinc-700'
                }`}
              >
                <span className={`inline-block h-5 w-5 transform rounded-full bg-white transition ${
                  enabled ? 'translate-x-6' : 'translate-x-1'
                }`} />
              </button>
            </div>
          </div>

          <div className={`space-y-6 transition-opacity ${enabled ? 'opacity-100' : 'opacity-40 pointer-events-none'}`}>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
              {/* PII Masking */}
              <div className="bg-zinc-900 border border-zinc-800/50 rounded-2xl p-6">
                <h3 className="text-sm font-medium text-white mb-1.5">PII Masking</h3>
                <p className="text-zinc-500 text-xs mb-6">
                  Automatically mask Personally Identifiable Information (emails, phone numbers, credit cards).
                </p>
                <label className="flex items-center cursor-pointer gap-3">
                  <input
                    type="checkbox"
                    checked={!!configValues.mask_pii}
                    onChange={(e) => setConfigValues(prev => ({ ...prev, mask_pii: e.target.checked }))}
                    className="w-4 h-4 rounded border-zinc-700 bg-zinc-800 text-blue-500 focus:ring-blue-500 focus:ring-offset-zinc-900"
                  />
                  <span className="text-zinc-300 text-sm">Mask PII</span>
                </label>
              </div>

              {/* Secrets Masking */}
              <div className="bg-zinc-900 border border-zinc-800/50 rounded-2xl p-6">
                <h3 className="text-sm font-medium text-white mb-1.5">Secrets Masking</h3>
                <p className="text-zinc-500 text-xs mb-6">
                  Detect and mask API keys, JWT tokens, and private keys in the prompts.
                </p>
                <label className="flex items-center cursor-pointer gap-3">
                  <input
                    type="checkbox"
                    checked={!!configValues.mask_secrets}
                    onChange={(e) => setConfigValues(prev => ({ ...prev, mask_secrets: e.target.checked }))}
                    className="w-4 h-4 rounded border-zinc-700 bg-zinc-800 text-purple-500 focus:ring-purple-500 focus:ring-offset-zinc-900"
                  />
                  <span className="text-zinc-300 text-sm">Mask Secrets</span>
                </label>
              </div>
            </div>

            {/* Prompt Injection */}
            <div className="bg-zinc-900 border border-zinc-800/50 rounded-2xl p-6">
              <h3 className="text-sm font-medium text-white mb-1.5">Prompt Injection Prevention</h3>
              <p className="text-zinc-500 text-xs mb-6">
                Block requests that contain common prompt injection patterns.
              </p>
              <label className="flex items-center cursor-pointer gap-3">
                <input
                  type="checkbox"
                  checked={!!configValues.prevent_prompt_injection}
                  onChange={(e) => setConfigValues(prev => ({ ...prev, prevent_prompt_injection: e.target.checked }))}
                  className="w-4 h-4 rounded border-zinc-700 bg-zinc-800 text-amber-500 focus:ring-amber-500 focus:ring-offset-zinc-900"
                />
                <span className="text-zinc-300 text-sm">Prevent Prompt Injections</span>
              </label>
            </div>

            {/* Blocked Keywords */}
            <div className="bg-zinc-900 border border-zinc-800/50 rounded-2xl p-6">
              <h3 className="text-sm font-medium text-white mb-1.5">Blocked Keywords</h3>
              <p className="text-zinc-500 text-xs mb-4">
                Comma-separated list of keywords. If any of these are found in a user's prompt, the request will be blocked.
              </p>
              <textarea
                value={Array.isArray(configValues.blocked_keywords) ? configValues.blocked_keywords.join(', ') : ''}
                onChange={(e) => {
                  const arr = e.target.value.split(',').map(s => s.trim()).filter(Boolean)
                  setConfigValues(prev => ({ ...prev, blocked_keywords: arr }))
                }}
                placeholder="e.g. hate, violence, confidential_project_x"
                className="w-full bg-zinc-950 border border-zinc-800 rounded-xl p-4 text-xs text-zinc-200 placeholder-zinc-700 focus:outline-none min-h-[120px] resize-y font-mono"
              />
            </div>
          </div>

          <div className="flex justify-end pt-4">
            <button
              onClick={handleSave}
              disabled={mutation.isPending}
              className="flex items-center gap-2 px-6 py-3 bg-white hover:bg-zinc-200 text-zinc-950 font-semibold text-xs rounded-xl transition-colors disabled:opacity-50"
            >
              {mutation.isPending ? <RotateCcw size={16} className="animate-spin" /> : <Save size={16} />}
              Save Configuration
            </button>
          </div>
        </div>
      )}
    </div>
  )
}

function CheckCircle({ size }: { size: number }) {
  return (
    <svg xmlns="http://www.w3.org/2000/svg" width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round" className="lucide lucide-check-circle-2">
      <path d="M12 22c5.523 0 10-4.477 10-10S17.523 2 12 2 2 6.477 2 12s4.477 10 10 10z"/>
      <path d="m9 12 2 2 4-4"/>
    </svg>
  )
}
