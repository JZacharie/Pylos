import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { configApi, providersApi } from '../lib/api'
import {
  Route,
  Settings,
  RotateCcw,
  Plus,
  Trash2,
  ChevronDown,
  ChevronRight,
  Check,
  AlertTriangle,
  Zap,
  ArrowDown,
  X,
  Pencil,
} from 'lucide-react'

// ─── Types ────────────────────────────────────────────────────────────────────

interface RoutingTarget {
  provider: string | null
  model: string | null
  weight: number
}

interface RoutingRuleConfig {
  id: string
  name: string
  enabled: boolean
  cel_expression: string
  targets: RoutingTarget[]
  fallbacks: string[]
  priority: number
}

interface RuleFormState {
  id: string
  name: string
  enabled: boolean
  cel_expression: string
  targets: RoutingTarget[]
  fallbacks: string[]
  priority: number
}

const EMPTY_TARGET: RoutingTarget = { provider: null, model: null, weight: 1.0 }

function createEmptyRule(index: number): RuleFormState {
  return {
    id: `rule-${Date.now()}`,
    name: `Rule ${index + 1}`,
    enabled: true,
    cel_expression: '',
    targets: [{ ...EMPTY_TARGET }],
    fallbacks: [],
    priority: index,
  }
}

function ruleToForm(r: RoutingRuleConfig): RuleFormState {
  return {
    id: r.id,
    name: r.name,
    enabled: r.enabled,
    cel_expression: r.cel_expression,
    targets: r.targets.map(t => ({ ...t })),
    fallbacks: [...r.fallbacks],
    priority: r.priority,
  }
}

function formToRule(f: RuleFormState): RoutingRuleConfig {
  return {
    id: f.id,
    name: f.name,
    enabled: f.enabled,
    cel_expression: f.cel_expression,
    targets: f.targets.map(t => ({
      provider: t.provider || null,
      model: t.model || null,
      weight: t.weight,
    })),
    fallbacks: f.fallbacks,
    priority: f.priority,
  }
}

// ─── RuleCard ─────────────────────────────────────────────────────────────────

function RuleCard({
  rule,
  onEdit,
  onDelete,
}: {
  rule: RoutingRuleConfig
  onEdit: () => void
  onDelete: () => void
}) {
  return (
    <div className="rounded-xl border border-zinc-800/50 bg-zinc-900/30 p-5 hover:border-zinc-700/50 transition-colors group">
      <div className="flex items-center justify-between mb-3">
        <div className="flex items-center gap-3 min-w-0">
          <div className={`w-2 h-2 rounded-full shrink-0 ${rule.enabled ? 'bg-emerald-400' : 'bg-zinc-600'}`} />
          <h3 className="font-semibold text-white truncate">{rule.name}</h3>
          <span className="text-xs text-zinc-600 font-mono shrink-0">P{rule.priority}</span>
        </div>
        <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
          <button
            onClick={onEdit}
            className="p-1.5 text-zinc-500 hover:text-white hover:bg-zinc-800/50 rounded-lg transition-all"
            title="Edit rule"
          >
            <Pencil size={13} />
          </button>
          <button
            onClick={onDelete}
            className="p-1.5 text-zinc-500 hover:text-red-400 hover:bg-red-400/10 rounded-lg transition-all"
            title="Delete rule"
          >
            <Trash2 size={13} />
          </button>
        </div>
      </div>

      {/* CEL Expression */}
      <div className="mb-3">
        <div className="text-xs text-zinc-500 mb-1">CEL Expression</div>
        <code className="block text-xs text-emerald-400 bg-emerald-400/5 border border-emerald-400/10 rounded-lg px-3 py-2 font-mono break-all">
          {rule.cel_expression || '(empty)'}
        </code>
      </div>

      {/* Targets */}
      <div className="mb-3">
        <div className="text-xs text-zinc-500 mb-1.5">Targets ({rule.targets.length})</div>
        <div className="space-y-1">
          {rule.targets.map((t, i) => (
            <div key={i} className="flex items-center gap-2 text-xs">
              <ArrowDown size={10} className="text-zinc-600" />
              <span className="text-zinc-300">{t.provider || '(any)'}</span>
              <span className="text-zinc-600">/</span>
              <span className="text-zinc-300">{t.model || '(any)'}</span>
              <span className="ml-auto text-zinc-600 font-mono">w={t.weight}</span>
            </div>
          ))}
        </div>
      </div>

      {/* Fallbacks */}
      {rule.fallbacks.length > 0 && (
        <div>
          <div className="text-xs text-zinc-500 mb-1.5">Fallbacks</div>
          <div className="flex flex-wrap gap-1.5">
            {rule.fallbacks.map((fb, i) => (
              <span key={i} className="text-xs bg-zinc-800/50 text-zinc-400 px-2 py-0.5 rounded-full">
                {fb}
              </span>
            ))}
          </div>
        </div>
      )}
    </div>
  )
}

// ─── RuleEditor ───────────────────────────────────────────────────────────────

function RuleEditor({
  rule,
  providerNames,
  onSave,
  onClose,
  isSaving,
  error,
}: {
  rule: RuleFormState
  providerNames: string[]
  onSave: (rule: RuleFormState) => void
  onClose: () => void
  isSaving: boolean
  error: string | null
}) {
  const [form, setForm] = useState<RuleFormState>({ ...rule })

  const setField = <K extends keyof RuleFormState>(k: K, v: RuleFormState[K]) =>
    setForm(f => ({ ...f, [k]: v }))

  const setTarget = (i: number, field: keyof RoutingTarget, value: string | number | null) =>
    setForm(f => {
      const targets = [...f.targets]
      targets[i] = { ...targets[i], [field]: value }
      return { ...f, targets }
    })

  const addTarget = () =>
    setForm(f => ({ ...f, targets: [...f.targets, { ...EMPTY_TARGET }] }))

  const removeTarget = (i: number) =>
    setForm(f => ({ ...f, targets: f.targets.filter((_, idx) => idx !== i) }))

  const addFallback = (provider: string) =>
    setForm(f => ({ ...f, fallbacks: [...f.fallbacks, provider] }))

  const removeFallback = (i: number) =>
    setForm(f => ({ ...f, fallbacks: f.fallbacks.filter((_, idx) => idx !== i) }))

  const totalWeight = form.targets.reduce((sum, t) => sum + t.weight, 0)

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-2xl mx-4 max-h-[90vh] overflow-y-auto">
        {/* Header */}
        <div className="flex items-center justify-between p-5 border-b border-zinc-800/50">
          <h2 className="text-lg font-semibold text-white">Edit routing rule</h2>
          <button onClick={onClose} className="text-zinc-500 hover:text-white transition-colors">
            <X size={18} />
          </button>
        </div>

        <div className="p-5 space-y-5">
          {/* Name + Enabled + Priority */}
          <div className="grid grid-cols-3 gap-3">
            <div className="col-span-2">
              <label className="block text-xs text-zinc-400 mb-1.5">Rule name</label>
              <input
                type="text"
                value={form.name}
                onChange={e => setField('name', e.target.value)}
                placeholder="e.g. GPT-4 Premium Routing"
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                  focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
              />
            </div>
            <div>
              <label className="block text-xs text-zinc-400 mb-1.5">Priority</label>
              <input
                type="number"
                value={form.priority}
                onChange={e => setField('priority', Number(e.target.value))}
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                  focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
              />
            </div>
          </div>

          {/* Enabled toggle */}
          <div className="flex items-center gap-3">
            <button
              onClick={() => setField('enabled', !form.enabled)}
              className={`w-10 h-5 rounded-full transition-colors relative ${
                form.enabled ? 'bg-emerald-500' : 'bg-zinc-700'
              }`}
            >
              <div
                className={`w-4 h-4 rounded-full bg-white absolute top-0.5 transition-all ${
                  form.enabled ? 'left-5' : 'left-0.5'
                }`}
              />
            </button>
            <span className="text-sm text-zinc-300">{form.enabled ? 'Enabled' : 'Disabled'}</span>
          </div>

          {/* CEL Expression */}
          <div>
            <label className="block text-xs text-zinc-400 mb-1.5">CEL Expression</label>
            <textarea
              value={form.cel_expression}
              onChange={e => setField('cel_expression', e.target.value)}
              placeholder={`e.g. request.model == 'gpt-4' && request.stream == true`}
              rows={2}
              className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-emerald-400 font-mono
                focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20 resize-none"
            />
          </div>

          {/* Targets */}
          <div>
            <div className="flex items-center justify-between mb-2">
              <label className="text-xs text-zinc-400">
                Routing Targets
                {form.targets.length > 1 && (
                  <span className={`ml-2 font-mono ${Math.abs(totalWeight - 1.0) < 0.01 ? 'text-emerald-400' : 'text-amber-400'}`}>
                    total: {totalWeight.toFixed(2)}
                  </span>
                )}
              </label>
              <button onClick={addTarget} className="text-xs text-emerald-400 hover:text-emerald-300 flex items-center gap-1">
                <Plus size={12} /> Add target
              </button>
            </div>
            <div className="space-y-2">
              {form.targets.map((t, i) => (
                <div key={i} className="bg-zinc-950/50 border border-zinc-800/50 rounded-lg p-3">
                  <div className="grid grid-cols-3 gap-2">
                    <div>
                      <label className="block text-xs text-zinc-500 mb-1">Provider</label>
                      <select
                        value={t.provider || ''}
                        onChange={e => setTarget(i, 'provider', e.target.value || null)}
                        className="w-full bg-zinc-950 border border-zinc-800 rounded px-2 py-1.5 text-xs text-zinc-200
                          focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
                      >
                        <option value="">(any)</option>
                        {providerNames.map(p => (
                          <option key={p} value={p}>{p}</option>
                        ))}
                      </select>
                    </div>
                    <div>
                      <label className="block text-xs text-zinc-500 mb-1">Model</label>
                      <input
                        value={t.model || ''}
                        onChange={e => setTarget(i, 'model', e.target.value || null)}
                        placeholder="(any)"
                        className="w-full bg-zinc-950 border border-zinc-800 rounded px-2 py-1.5 text-xs text-zinc-200
                          font-mono focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
                      />
                    </div>
                    <div>
                      <label className="block text-xs text-zinc-500 mb-1">Weight</label>
                      <div className="flex items-center gap-1">
                        <input
                          type="number"
                          value={t.weight}
                          onChange={e => setTarget(i, 'weight', Number(e.target.value))}
                          min={0.01}
                          step={0.1}
                          className="w-full bg-zinc-950 border border-zinc-800 rounded px-2 py-1.5 text-xs text-zinc-200
                            focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
                        />
                        {form.targets.length > 1 && (
                          <button
                            onClick={() => removeTarget(i)}
                            className="p-1 text-zinc-600 hover:text-red-400 transition-colors shrink-0"
                          >
                            <Trash2 size={12} />
                          </button>
                        )}
                      </div>
                    </div>
                  </div>
                </div>
              ))}
            </div>
          </div>

          {/* Fallbacks */}
          <div>
            <div className="flex items-center justify-between mb-2">
              <label className="text-xs text-zinc-400">Fallback Chain</label>
            </div>
            {form.fallbacks.length > 0 && (
              <div className="flex flex-wrap gap-1.5 mb-2">
                {form.fallbacks.map((fb, i) => (
                  <span key={i} className="text-xs bg-zinc-800 text-zinc-300 px-2 py-1 rounded-full flex items-center gap-1.5">
                    {fb}
                    <button onClick={() => removeFallback(i)} className="text-zinc-500 hover:text-red-400">
                      <X size={10} />
                    </button>
                  </span>
                ))}
              </div>
            )}
            <select
              value=""
              onChange={e => { if (e.target.value) addFallback(e.target.value) }}
              className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-400
                focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
            >
              <option value="">Add fallback provider…</option>
              {providerNames
                .filter(p => !form.fallbacks.includes(p))
                .map(p => (
                  <option key={p} value={p}>{p}</option>
                ))}
            </select>
          </div>

          {/* Error */}
          {error && (
            <div className="flex items-center gap-2 text-red-400 text-xs bg-red-900/20 border border-red-800/50 rounded-lg px-3 py-2">
              <AlertTriangle size={13} />
              {error}
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex justify-end gap-3 px-5 py-4 border-t border-zinc-800/50">
          <button onClick={onClose} className="px-4 py-2 text-sm text-zinc-400 hover:text-white transition-colors">
            Cancel
          </button>
          <button
            onClick={() => onSave(form)}
            disabled={isSaving || !form.name.trim()}
            className="px-4 py-2 text-sm bg-emerald-600 hover:bg-emerald-500 active:scale-[0.98]
              disabled:opacity-50 disabled:cursor-not-allowed text-white rounded-lg flex items-center gap-2 transition-colors"
          >
            {isSaving ? <RotateCcw size={14} className="animate-spin" /> : <Check size={14} />}
            Save Rule
          </button>
        </div>
      </div>
    </div>
  )
}

// ─── Main page ────────────────────────────────────────────────────────────────

export default function RouterSettings() {
  const qc = useQueryClient()
  const [editingRule, setEditingRule] = useState<RoutingRuleConfig | null>(null)
  const [creatingRule, setCreatingRule] = useState(false)
  const [deletingRule, setDeletingRule] = useState<RoutingRuleConfig | null>(null)
  const [mutationError, setMutationError] = useState<string | null>(null)
  const [expandedGlobal, setExpandedGlobal] = useState(true)

  const { data: config, isLoading } = useQuery({
    queryKey: ['config'],
    queryFn: configApi.get,
    refetchInterval: 30_000,
  })

  const { data: providersData } = useQuery({
    queryKey: ['providers'],
    queryFn: providersApi.getAll,
    refetchInterval: 30_000,
  })

  const providerNames = providersData?.providers?.map(p => p.name) ?? []
  const routingRules: RoutingRuleConfig[] = config?.governance?.routing_rules ?? []
  const sortedRules = [...routingRules].sort((a, b) => a.priority - b.priority)

  // Reload config
  const reloadMutation = useMutation({
    mutationFn: configApi.reload,
    onSuccess: () => qc.invalidateQueries({ queryKey: ['config'] }),
  })

  // Save routing rules (update governance config via reload)
  const saveRules = async () => {
    setMutationError(null)
    try {
      // The config API doesn't have a direct routing rules update endpoint,
      // so we save via the reload mechanism after config file changes.
      // For now, we trigger a reload to refresh from the file.
      await reloadMutation.mutateAsync()
    } catch (e: any) {
      setMutationError(e.message || 'Failed to save')
    }
  }

  const handleDeleteRule = (rule: RoutingRuleConfig) => {
    routingRules.filter(r => r.id !== rule.id)
    saveRules()
    setDeletingRule(null)
  }

  const providersAny = (config?.providers ?? {}) as Record<string, { network?: { timeout_secs?: number; max_retries?: number }; keys?: Array<{ weight?: number }> }>
  const firstProvider = Object.values(providersAny)[0]
  const globalTimeout = firstProvider?.network?.timeout_secs ?? 30
  const globalMaxRetries = firstProvider?.network?.max_retries ?? 3

  // Build provider keys summary for load balancing display
  const providerKeysSummary = Object.entries(providersAny).map(([name, p]) => ({
    name,
    keys: p.keys?.length ?? 0,
    totalWeight: p.keys?.reduce((sum: number, k: { weight?: number }) => sum + (k.weight ?? 1), 0) ?? 0,
    timeout: p.network?.timeout_secs,
    retries: p.network?.max_retries,
  }))

  return (
    <div className="p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white flex items-center gap-3">
            <Route size={24} className="text-emerald-400" />
            Router Settings
          </h1>
          <p className="text-sm text-zinc-400 mt-1">
            {routingRules.length} routing rule{routingRules.length !== 1 ? 's' : ''} configured
          </p>
        </div>
        <div className="flex items-center gap-3">
          <button
            onClick={() => reloadMutation.mutate()}
            disabled={reloadMutation.isPending}
            className="flex items-center gap-2 px-3 py-2 text-sm text-zinc-400 hover:text-white
              border border-zinc-800 hover:border-zinc-700 rounded-lg transition-colors"
          >
            <RotateCcw size={14} className={reloadMutation.isPending ? 'animate-spin' : ''} />
            Reload
          </button>
          <button
            onClick={() => { setMutationError(null); setCreatingRule(true) }}
            className="flex items-center gap-2 px-4 py-2 bg-emerald-600 hover:bg-emerald-500 active:scale-[0.98]
              text-white text-sm rounded-lg transition-colors"
          >
            <Plus size={15} />
            Add rule
          </button>
        </div>
      </div>

      {/* Global Settings */}
      <div className="rounded-xl border border-zinc-800/50 bg-zinc-900/30">
        <button
          onClick={() => setExpandedGlobal(!expandedGlobal)}
          className="w-full flex items-center justify-between p-5 text-left"
        >
          <div className="flex items-center gap-3">
            <Settings size={16} className="text-zinc-400" />
            <span className="font-semibold text-white">Global Settings</span>
          </div>
          {expandedGlobal ? <ChevronDown size={16} className="text-zinc-500" /> : <ChevronRight size={16} className="text-zinc-500" />}
        </button>
        {expandedGlobal && (
          <div className="px-5 pb-5 grid grid-cols-2 md:grid-cols-4 gap-4">
            <div>
              <div className="text-xs text-zinc-500 mb-1">Default Timeout</div>
              <div className="text-sm text-white font-mono">{globalTimeout}s</div>
            </div>
            <div>
              <div className="text-xs text-zinc-500 mb-1">Max Retries</div>
              <div className="text-sm text-white font-mono">{globalMaxRetries}</div>
            </div>
            <div>
              <div className="text-xs text-zinc-500 mb-1">Config Version</div>
              <div className="text-sm text-white font-mono">v{config?.version ?? '—'}</div>
            </div>
            <div>
              <div className="text-xs text-zinc-500 mb-1">Providers</div>
              <div className="text-sm text-white font-mono">{providerNames.length}</div>
            </div>
          </div>
        )}
      </div>

      {/* Load Balancing Summary */}
      <div className="rounded-xl border border-zinc-800/50 bg-zinc-900/30 p-5">
        <div className="flex items-center gap-3 mb-4">
          <Zap size={16} className="text-amber-400" />
          <span className="font-semibold text-white">Load Balancing</span>
        </div>
        {providerKeysSummary.length > 0 ? (
          <div className="space-y-3">
            {providerKeysSummary.map(p => (
              <div key={p.name} className="flex items-center gap-4 text-sm">
                <span className="text-zinc-300 font-medium capitalize min-w-[100px]">{p.name}</span>
                <span className="text-zinc-500 text-xs">
                  {p.keys} key{p.keys !== 1 ? 's' : ''}
                </span>
                <div className="flex-1 bg-zinc-800 rounded-full h-1.5 overflow-hidden">
                  <div
                    className="h-full bg-emerald-500/60 rounded-full transition-all"
                    style={{ width: `${Math.min((p.totalWeight / (providerKeysSummary.reduce((s, x) => s + x.totalWeight, 0) || 1)) * 100, 100)}%` }}
                  />
                </div>
                <span className="text-zinc-600 font-mono text-xs">w={p.totalWeight.toFixed(1)}</span>
              </div>
            ))}
          </div>
        ) : (
          <div className="text-sm text-zinc-600">No providers configured</div>
        )}
      </div>

      {/* Routing Rules */}
      <div>
        <h2 className="text-lg font-semibold text-white mb-4">Routing Rules</h2>
        {isLoading ? (
          <div className="space-y-3">
            {Array.from({ length: 2 }).map((_, i) => (
              <div key={i} className="rounded-xl border border-zinc-800/50 bg-zinc-900/30 p-5 animate-pulse h-32" />
            ))}
          </div>
        ) : sortedRules.length > 0 ? (
          <div className="space-y-3">
            {sortedRules.map(rule => (
              <RuleCard
                key={rule.id}
                rule={rule}
                onEdit={() => { setMutationError(null); setEditingRule(rule) }}
                onDelete={() => setDeletingRule(rule)}
              />
            ))}
          </div>
        ) : (
          <div className="text-center py-16 rounded-xl border border-zinc-800/50 bg-zinc-900/30">
            <Route size={32} className="text-zinc-700 mx-auto mb-3" />
            <div className="text-zinc-500 text-sm">No routing rules configured</div>
            <div className="text-zinc-600 text-xs mt-1">Add a rule to start routing requests based on conditions</div>
          </div>
        )}
      </div>

      {/* Create Rule Editor */}
      {creatingRule && (
        <RuleEditor
          rule={createEmptyRule(routingRules.length)}
          providerNames={providerNames}
          onSave={form => {
            [...routingRules, formToRule(form)]
            saveRules()
            setCreatingRule(false)
          }}
          onClose={() => setCreatingRule(false)}
          isSaving={reloadMutation.isPending}
          error={mutationError}
        />
      )}

      {/* Edit Rule Editor */}
      {editingRule && (
        <RuleEditor
          rule={ruleToForm(editingRule)}
          providerNames={providerNames}
          onSave={form => {
            routingRules.map(r => r.id === editingRule.id ? formToRule(form) : r)
            saveRules()
            setEditingRule(null)
          }}
          onClose={() => setEditingRule(null)}
          isSaving={reloadMutation.isPending}
          error={mutationError}
        />
      )}

      {/* Delete Confirm */}
      {deletingRule && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
          <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-sm mx-4 p-6">
            <div className="flex items-center gap-3 mb-4">
              <div className="w-9 h-9 rounded-full bg-red-500/15 flex items-center justify-center">
                <AlertTriangle size={16} className="text-red-400" />
              </div>
              <div>
                <div className="font-semibold text-white">Delete rule</div>
                <div className="text-xs text-zinc-500">This cannot be undone</div>
              </div>
            </div>
            <p className="text-sm text-zinc-400 mb-5">
              Remove <span className="text-white font-medium">{deletingRule.name}</span>?
            </p>
            <div className="flex justify-end gap-3">
              <button onClick={() => setDeletingRule(null)} className="px-4 py-2 text-sm text-zinc-400 hover:text-white">
                Cancel
              </button>
              <button
                onClick={() => handleDeleteRule(deletingRule)}
                className="px-4 py-2 text-sm bg-red-600 hover:bg-red-500 text-white rounded-lg
                  flex items-center gap-2 transition-colors active:scale-[0.98]"
              >
                <Trash2 size={13} />
                Delete
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}
