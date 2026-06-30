import { useState, useMemo } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { virtualKeysApi, providersApi, modelsApi, type VirtualKey, type VkBudgetResponse } from '../lib/api'
import { providerColor } from '../lib/utils'
import {
  KeyRound, CheckCircle, XCircle, Shield, TrendingUp,
  ChevronDown, ChevronUp, Plus, Pencil, Trash2, X, Check,
  AlertTriangle, RotateCcw, Copy, RotateCw, Search, Filter,
  Eye, EyeOff,
} from 'lucide-react'
import { ProviderIcon } from '../components/ProviderIcon'

function tsDisplay(ts: number | null): string {
  if (!ts) return '—'
  const d = new Date(ts)
  return d.toLocaleDateString('fr-FR', { day: '2-digit', month: '2-digit', year: 'numeric' })
}

function tsAgo(ts: number | null): string {
  if (!ts) return 'Never'
  const diff = Date.now() - ts
  const mins = Math.floor(diff / 60000)
  if (mins < 1) return 'Just now'
  if (mins < 60) return `${mins}m ago`
  const hrs = Math.floor(mins / 60)
  if (hrs < 24) return `${hrs}h ago`
  const days = Math.floor(hrs / 24)
  return `${days}d ago`
}

function fmtVal(v: string | null | undefined): string {
  if (!v) return '—'
  return v
}

// ─── Budget / Rate panel ──────────────────────────────────────────────────────

function BudgetBar({ used, max }: { used: number; max: number }) {
  const pct = max > 0 ? Math.min((used / max) * 100, 100) : 0
  const color = pct > 90 ? '#f43f5e' : pct > 70 ? '#f59e0b' : '#10b981'
  return (
    <div className="w-full">
      <div className="flex justify-between text-xs text-zinc-400 mb-1">
        <span>${used.toFixed(4)}</span>
        <span>${max.toFixed(2)}</span>
      </div>
      <div className="h-1.5 bg-zinc-800/50 rounded-full overflow-hidden">
        <div className="h-full rounded-full transition-all" style={{ width: `${pct}%`, background: color }} />
      </div>
    </div>
  )
}

function VkBudgetPanel({ vkId }: { vkId: string }) {
  const { data, isLoading } = useQuery<VkBudgetResponse>({
    queryKey: ['vk-budget', vkId],
    queryFn: () => virtualKeysApi.getBudget(vkId),
    refetchInterval: 30_000,
  })

  if (isLoading) return <div className="text-xs text-zinc-500">Loading…</div>
  if (!data || (data.budget.length === 0 && data.rate_limits.length === 0)) {
    return <div className="text-xs text-zinc-500">No governance configured</div>
  }

  return (
    <div className="grid grid-cols-2 gap-4 text-xs">
      {data.budget.length > 0 && (
        <div>
          <div className="text-zinc-400 font-medium mb-2">Budget</div>
          {data.budget.map(b => (
            <div key={b.period} className="mb-3">
              <div className="text-zinc-500 mb-1 capitalize">{b.period}</div>
              <BudgetBar used={b.current_usd} max={b.max_usd} />
            </div>
          ))}
        </div>
      )}
      {data.rate_limits.length > 0 && (
        <div>
          <div className="text-zinc-400 font-medium mb-2">Rate Limits</div>
          {data.rate_limits.map(rl => (
            <div key={rl.window_type} className="mb-3">
              <div className="text-zinc-500 mb-1 capitalize">{rl.window_type}</div>
              <div className="flex justify-between text-zinc-300">
                <span>{rl.current_value}</span>
                <span>/ {rl.max_value}</span>
              </div>
              <div className="h-1.5 bg-zinc-800/50 rounded-full overflow-hidden mt-1">
                <div
                  className="h-full rounded-full transition-all"
                  style={{
                    width: rl.max_value > 0 ? `${Math.min((rl.current_value / rl.max_value) * 100, 100)}%` : '0%',
                    background: rl.max_value > 0 && rl.current_value / rl.max_value > 0.9 ? '#f43f5e' : '#3b82f6',
                  }}
                />
              </div>
            </div>
          ))}
        </div>
      )}
    </div>
  )
}

// ─── Types ────────────────────────────────────────────────────────────────────

interface ProviderCfgEntry { providers: string[]; models: string; weight: number }

interface VkFormState {
  name: string
  description: string
  is_active: boolean
  provider_configs: ProviderCfgEntry[]
  team_alias: string
  team_id: string
  organization_id: string
  access_group_id: string
  user_email: string
  user_id: string
  created_by: string
  expires_at: string
}

const DEFAULT_FORM: VkFormState = {
  name: '',
  description: '',
  is_active: true,
  provider_configs: [],
  team_alias: '',
  team_id: '',
  organization_id: '',
  access_group_id: '',
  user_email: '',
  user_id: '',
  created_by: '',
  expires_at: '',
}

function vkToForm(vk: VirtualKey): VkFormState {
  const groups: { [key: string]: { providers: string[]; allowed_models: string[]; weight: number } } = {}
  for (const pc of vk.provider_configs) {
    const sortedModels = [...pc.allowed_models].sort()
    const groupKey = `${sortedModels.join(',')}|${pc.weight}`
    if (!groups[groupKey]) {
      groups[groupKey] = { providers: [], allowed_models: pc.allowed_models, weight: pc.weight }
    }
    groups[groupKey].providers.push(pc.provider)
  }
  return {
    name: vk.name,
    description: vk.description ?? '',
    is_active: vk.is_active,
    provider_configs: Object.values(groups).map(g => ({
      providers: g.providers,
      models: g.allowed_models.join(', '),
      weight: g.weight,
    })),
    team_alias: vk.team_alias ?? '',
    team_id: vk.team_id ?? '',
    organization_id: vk.organization_id ?? '',
    access_group_id: vk.access_group_id ?? '',
    user_email: vk.user_email ?? '',
    user_id: vk.user_id ?? '',
    created_by: vk.created_by ?? '',
    expires_at: vk.expires_at ? tsDisplay(vk.expires_at) : '',
  }
}

function formToPayload(form: VkFormState) {
  const provider_configs: Array<{ provider: string; allowed_models: string[]; weight: number }> = []
  for (const pc of form.provider_configs) {
    const allowed_models = pc.models.split(',').map(s => s.trim()).filter(Boolean)
    for (const p of pc.providers) {
      provider_configs.push({ provider: p, allowed_models, weight: pc.weight })
    }
  }
  const expiresMs = form.expires_at ? new Date(form.expires_at).getTime() : null
  return {
    name: form.name,
    description: form.description || null,
    is_active: form.is_active,
    provider_configs,
    team_alias: form.team_alias || null,
    team_id: form.team_id || null,
    organization_id: form.organization_id || null,
    access_group_id: form.access_group_id || null,
    user_email: form.user_email || null,
    user_id: form.user_id || null,
    created_by: form.created_by || null,
    expires_at: expiresMs,
  }
}

// ─── Governance Sub-components ──────────────────────────────────────────────

function ProviderSelector({ value, onChange }: { value: string[]; onChange: (v: string[]) => void }) {
  const { data } = useQuery({ queryKey: ['providers'], queryFn: providersApi.getAll })
  const providers = data?.providers ?? []
  const toggleProvider = (pName: string) => {
    if (value.includes(pName)) onChange(value.filter(x => x !== pName))
    else onChange([...value, pName])
  }
  return (
    <div className="flex flex-wrap gap-1.5 p-2 bg-zinc-950 border border-zinc-800 rounded-lg min-h-[42px]">
      {providers.map(p => {
        const isSelected = value.includes(p.name)
        return (
          <button key={p.name} type="button" onClick={() => toggleProvider(p.name)}
            className={`flex items-center gap-1.5 px-2.5 py-1 rounded-md text-xs font-medium border transition-all ${
              isSelected
                ? 'bg-emerald-500/10 text-emerald-300 border-emerald-500/30 shadow-sm shadow-emerald-500/5'
                : 'bg-zinc-900/40 text-zinc-400 border-zinc-800 hover:text-zinc-200 hover:bg-zinc-800'
            }`}
          >
            <ProviderIcon name={p.name} size={12} />
            <span className="capitalize">{p.name}</span>
          </button>
        )
      })}
      {providers.length === 0 && <span className="text-xs text-zinc-500 p-1">Loading providers…</span>}
    </div>
  )
}

function ModelSelector({ providers, value, onChange }: { providers: string[]; value: string; onChange: (v: string) => void }) {
  const { data: modelsData } = useQuery({
    queryKey: ['models', 'all'],
    queryFn: () => modelsApi.getAll(),
    enabled: providers.length > 0,
  })
  const allModels = modelsData?.data ?? []
  const models = allModels.filter(m => providers.includes(m.provider))
  const selectedModels = value.split(',').map(s => s.trim()).filter(Boolean)
  const toggleModel = (m: string) => {
    let next: string[]
    if (m === '*') next = ['*']
    else {
      const c = selectedModels.filter(s => s !== '*')
      next = c.includes(m) ? c.filter(s => s !== m) : [...c, m]
    }
    onChange(next.join(', '))
  }
  return (
    <div className="space-y-2">
      <div className="flex flex-wrap gap-1.5 p-2 bg-zinc-950 border border-zinc-800 rounded-lg min-h-[42px]">
        {selectedModels.map(m => (
          <span key={m} className="flex items-center gap-1 px-2 py-0.5 bg-emerald-500/10 text-emerald-300 border border-emerald-500/20 rounded text-[10px] font-medium">
            {m}
            <button onClick={() => toggleModel(m)} className="hover:text-white"><X size={10} /></button>
          </span>
        ))}
        {selectedModels.length === 0 && <span className="text-xs text-zinc-600 px-1 py-0.5">Pick models…</span>}
      </div>
      {providers.length > 0 && (
        <div className="max-h-32 overflow-y-auto p-1 bg-zinc-950/50 border border-zinc-800/50 rounded-lg grid grid-cols-2 gap-1 custom-scrollbar">
          <button onClick={() => toggleModel('*')}
            className={`text-left px-2 py-1.5 rounded text-[10px] transition-colors ${
              selectedModels.includes('*') ? 'bg-emerald-500/10 text-emerald-400' : 'text-zinc-500 hover:bg-zinc-800'
            }`}
          >All Models (*)</button>
          {models.map(m => (
            <button key={m.id} onClick={() => toggleModel(m.pylos.model_id)}
              className={`text-left px-2 py-1.5 rounded text-[10px] truncate transition-colors ${
                selectedModels.includes(m.pylos.model_id) ? 'bg-emerald-500/10 text-emerald-400' : 'text-zinc-400 hover:bg-zinc-800'
              }`} title={m.pylos.model_id}
            >{m.pylos.model_id}</button>
          ))}
        </div>
      )}
    </div>
  )
}

function ProviderConfigItem({ pc, index, onUpdate, onRemove }: {
  pc: ProviderCfgEntry; index: number
  onUpdate: (i: number, field: keyof ProviderCfgEntry, value: string | number | string[]) => void
  onRemove: (i: number) => void
}) {
  const color = providerColor(pc.providers[0] || 'default')
  return (
    <div className="bg-zinc-950/40 border border-zinc-800 rounded-xl overflow-hidden transition-all hover:border-zinc-700/50">
      <div className="px-4 py-2 bg-zinc-900/30 border-b border-zinc-800 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <div className="w-1.5 h-1.5 rounded-full" style={{ backgroundColor: color }} />
          <span className="text-[10px] font-bold text-zinc-500 uppercase tracking-widest">Rule #{index + 1}</span>
        </div>
        <button onClick={() => onRemove(index)} className="p-1 text-zinc-600 hover:text-red-400 transition-colors"><Trash2 size={12} /></button>
      </div>
      <div className="p-4 space-y-4">
        <div className="grid grid-cols-12 gap-4">
          <div className="col-span-7 space-y-3">
            <div>
              <label className="block text-[10px] font-medium text-zinc-500 uppercase mb-1.5 ml-1">Providers</label>
              <ProviderSelector value={pc.providers} onChange={v => onUpdate(index, 'providers', v)} />
            </div>
            <div>
              <label className="block text-[10px] font-medium text-zinc-500 uppercase mb-1.5 ml-1">Allowed Models</label>
              <ModelSelector providers={pc.providers} value={pc.models} onChange={v => onUpdate(index, 'models', v)} />
            </div>
          </div>
          <div className="col-span-5">
            <label className="block text-[10px] font-medium text-zinc-500 uppercase mb-1.5 ml-1">Routing Weight</label>
            <div className="bg-zinc-950 border border-zinc-800 rounded-lg p-3 space-y-3">
              <input type="range" min="0" max="10" step="0.1" value={pc.weight}
                onChange={e => onUpdate(index, 'weight', parseFloat(e.target.value))}
                className="w-full h-1 bg-zinc-800 rounded-lg appearance-none cursor-pointer accent-emerald-500"
              />
              <div className="flex items-center justify-between">
                <span className="text-[10px] text-zinc-500">Low</span>
                <div className="bg-emerald-500/10 text-emerald-400 px-2 py-0.5 rounded text-xs font-mono font-bold">{pc.weight.toFixed(1)}</div>
                <span className="text-[10px] text-zinc-500">High</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}

// ─── VkModal ──────────────────────────────────────────────────────────────────

function VkModal({ initial, isEdit, onClose, onSave, isSaving, error, createdKey }: {
  initial: VkFormState; isEdit: boolean; onClose: () => void; onSave: (form: VkFormState) => void
  isSaving: boolean; error: string | null; createdKey?: string
}) {
  const [form, setForm] = useState<VkFormState>(initial)
  const [copied, setCopied] = useState(false)
  const setField = <K extends keyof VkFormState>(k: K, v: VkFormState[K]) => setForm(f => ({ ...f, [k]: v }))
  const setPc = (i: number, field: keyof ProviderCfgEntry, value: string | number | string[]) =>
    setForm(f => { const pcs = [...f.provider_configs]; pcs[i] = { ...pcs[i], [field]: value } as ProviderCfgEntry; return { ...f, provider_configs: pcs } })
  const addPc = () => setForm(f => ({ ...f, provider_configs: [...f.provider_configs, { providers: [], models: '*', weight: 1.0 }] }))
  const removePc = (i: number) => setForm(f => ({ ...f, provider_configs: f.provider_configs.filter((_, idx) => idx !== i) }))
  const copyKey = () => { if (createdKey) { navigator.clipboard.writeText(createdKey); setCopied(true); setTimeout(() => setCopied(false), 2000) } }

  if (createdKey) {
    return (
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
        <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-sm mx-4 p-6">
          <div className="flex items-center gap-3 mb-4">
            <div className="w-9 h-9 rounded-full bg-emerald-500/15 flex items-center justify-center"><Check size={16} className="text-emerald-400" /></div>
            <div><div className="font-semibold text-white">Virtual key created</div><div className="text-xs text-zinc-500">Copy it now — it won't be shown again</div></div>
          </div>
          <div className="bg-zinc-950/50 border border-zinc-800/50 rounded-lg p-3 flex items-center gap-2 mb-5">
            <span className="font-mono text-sm text-emerald-300 flex-1 break-all">{createdKey}</span>
            <button onClick={copyKey} className="shrink-0 text-zinc-400 hover:text-white transition-colors">{copied ? <Check size={14} className="text-emerald-400" /> : <Copy size={14} />}</button>
          </div>
          <button onClick={onClose} className="w-full py-2 bg-emerald-600 hover:bg-emerald-500 active:scale-[0.98] text-white text-sm rounded-lg transition-colors">Done</button>
        </div>
      </div>
    )
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-2xl mx-4 max-h-[90vh] overflow-y-auto">
        <div className="flex items-center justify-between p-5 border-b border-zinc-800/50">
          <h2 className="text-lg font-semibold text-white">{isEdit ? 'Edit virtual key' : 'Create virtual key'}</h2>
          <button onClick={onClose} className="text-zinc-500 hover:text-white transition-colors"><X size={18} /></button>
        </div>
        <div className="p-5 space-y-5">
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-xs text-zinc-400 mb-1.5">Name *</label>
              <input type="text" value={form.name} onChange={e => setField('name', e.target.value)} placeholder="My Project"
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
              />
            </div>
            <div>
              <label className="block text-xs text-zinc-400 mb-1.5">Key Alias</label>
              <input type="text" value={form.team_alias} onChange={e => setField('team_alias', e.target.value)} placeholder="kusanagi"
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
              />
            </div>
            <div>
              <label className="block text-xs text-zinc-400 mb-1.5">Team ID</label>
              <input type="text" value={form.team_id} onChange={e => setField('team_id', e.target.value)} placeholder="team-xxx"
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
              />
            </div>
            <div>
              <label className="block text-xs text-zinc-400 mb-1.5">Organization ID</label>
              <input type="text" value={form.organization_id} onChange={e => setField('organization_id', e.target.value)} placeholder="org-xxx"
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
              />
            </div>
            <div>
              <label className="block text-xs text-zinc-400 mb-1.5">Access Group ID</label>
              <input type="text" value={form.access_group_id} onChange={e => setField('access_group_id', e.target.value)} placeholder="ag-xxx"
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
              />
            </div>
            <div>
              <label className="block text-xs text-zinc-400 mb-1.5">User Email</label>
              <input type="email" value={form.user_email} onChange={e => setField('user_email', e.target.value)} placeholder="user@example.com"
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
              />
            </div>
            <div>
              <label className="block text-xs text-zinc-400 mb-1.5">User ID</label>
              <input type="text" value={form.user_id} onChange={e => setField('user_id', e.target.value)} placeholder="user-xxx"
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
              />
            </div>
            <div>
              <label className="block text-xs text-zinc-400 mb-1.5">Created By</label>
              <input type="text" value={form.created_by} onChange={e => setField('created_by', e.target.value)} placeholder="admin"
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
              />
            </div>
            <div>
              <label className="block text-xs text-zinc-400 mb-1.5">Expires At</label>
              <input type="date" value={form.expires_at} onChange={e => setField('expires_at', e.target.value)}
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
              />
            </div>
          </div>
          <div>
            <label className="block text-xs text-zinc-400 mb-1.5">Description (optional)</label>
            <input type="text" value={form.description} onChange={e => setField('description', e.target.value)} placeholder="Production key for service X"
              className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
            />
          </div>
          <div className="flex items-center gap-3">
            <button onClick={() => setField('is_active', !form.is_active)}
              className={`relative w-10 h-5 rounded-full transition-colors ${form.is_active ? 'bg-emerald-600' : 'bg-zinc-700'}`}
            >
              <span className={`absolute top-0.5 w-4 h-4 rounded-full bg-white shadow transition-all ${form.is_active ? 'left-5' : 'left-0.5'}`} />
            </button>
            <span className="text-sm text-zinc-300">Active</span>
          </div>
          <div>
            <div className="flex items-center justify-between mb-3">
              <label className="text-xs font-semibold text-zinc-300 uppercase tracking-wider">Governance Settings</label>
              <button onClick={addPc} className="text-xs bg-emerald-600/10 hover:bg-emerald-600/20 text-emerald-400 border border-emerald-500/30 px-2 py-1 rounded flex items-center gap-1 transition-all"><Plus size={12} /> Add Rule</button>
            </div>
            {form.provider_configs.length === 0 ? (
              <div className="bg-zinc-900/40 border border-dashed border-zinc-800 rounded-xl p-8 text-center">
                <Shield size={24} className="text-zinc-700 mx-auto mb-2" />
                <p className="text-xs text-zinc-500">No restrictions — all configured providers allowed</p>
                <button onClick={addPc} className="mt-4 text-xs text-emerald-400 hover:underline">Add your first governance rule</button>
              </div>
            ) : (
              <div className="space-y-4">{form.provider_configs.map((pc, i) => <ProviderConfigItem key={i} pc={pc} index={i} onUpdate={setPc} onRemove={removePc} />)}</div>
            )}
          </div>
          {error && (
            <div className="flex items-center gap-2 text-red-400 text-xs bg-red-900/20 border border-red-800/50 rounded-lg px-3 py-2">
              <AlertTriangle size={13} />{error}
            </div>
          )}
        </div>
        <div className="flex justify-end gap-3 px-5 py-4 border-t border-zinc-800/50">
          <button onClick={onClose} className="px-4 py-2 text-sm text-zinc-400 hover:text-white transition-colors">Cancel</button>
          <button onClick={() => onSave(form)} disabled={isSaving || !form.name.trim()}
            className="px-4 py-2 text-sm bg-emerald-600 hover:bg-emerald-500 active:scale-[0.98] disabled:opacity-50 disabled:cursor-not-allowed text-white rounded-lg flex items-center gap-2 transition-colors"
          >
            {isSaving ? <RotateCcw size={14} className="animate-spin" /> : <Check size={14} />}
            {isEdit ? 'Update' : 'Create'}
          </button>
        </div>
      </div>
    </div>
  )
}


// ─── DeleteConfirmModal ───────────────────────────────────────────────────────

function DeleteConfirmModal({ name, onClose, onConfirm, isDeleting, error }: {
  name: string; onClose: () => void; onConfirm: () => void; isDeleting: boolean; error: string | null
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-sm mx-4 p-6">
        <div className="flex items-center gap-3 mb-4">
          <div className="w-9 h-9 rounded-full bg-red-500/15 flex items-center justify-center"><AlertTriangle size={16} className="text-red-400" /></div>
          <div><div className="font-semibold text-white">Delete virtual key</div><div className="text-xs text-zinc-500">This action cannot be undone</div></div>
        </div>
        <p className="text-sm text-zinc-400 mb-4">Delete <span className="text-white font-medium">{name}</span>? All requests using this key will be rejected immediately.</p>
        
        {error && (
          <div className="flex items-center gap-2 text-red-400 text-xs bg-red-900/20 border border-red-800/50 rounded-lg px-3 py-2 mb-4">
            <AlertTriangle size={13} />{error}
          </div>
        )}

        <div className="flex justify-end gap-3">
          <button onClick={onClose} className="px-4 py-2 text-sm text-zinc-400 hover:text-white">Cancel</button>
          <button onClick={onConfirm} disabled={isDeleting}
            className="px-4 py-2 text-sm bg-red-600 hover:bg-red-500 disabled:opacity-50 text-white rounded-lg flex items-center gap-2 transition-colors active:scale-[0.98]"
          >{isDeleting ? <RotateCcw size={13} className="animate-spin" /> : <Trash2 size={13} />} Delete</button>
        </div>
      </div>
    </div>
  )
}

// ─── Search bar ───────────────────────────────────────────────────────────────

function SearchBar({ value, onChange, placeholder }: {
  value: string; onChange: (v: string) => void; placeholder?: string
}) {
  return (
    <div className="relative">
      <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-500" />
      <input type="text" value={value} onChange={e => onChange(e.target.value)}
        placeholder={placeholder ?? 'Search…'}
        className="w-full bg-zinc-950 border border-zinc-800 rounded-lg pl-9 pr-8 py-2 text-sm text-zinc-200
          focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
      />
      {value && (
        <button onClick={() => onChange('')} className="absolute right-3 top-1/2 -translate-y-1/2 text-zinc-500 hover:text-zinc-300">
          <X size={14} />
        </button>
      )}
    </div>
  )
}

// ─── Main page ────────────────────────────────────────────────────────────────

const COLUMNS = [
  { key: 'name', label: 'Key Alias', default: true },
  { key: 'id', label: 'Key ID' },
  { key: 'value', label: 'Secret Key' },
  { key: 'team_alias', label: 'Team Alias' },
  { key: 'team_id', label: 'Team ID' },
  { key: 'organization_id', label: 'Org ID' },
  { key: 'access_group_id', label: 'Group ID' },
  { key: 'user_email', label: 'User Email' },
  { key: 'user_id', label: 'User ID' },
  { key: 'created_at', label: 'Created At' },
  { key: 'created_by', label: 'Created By' },
  { key: 'updated_at', label: 'Updated At' },
  { key: 'last_active', label: 'Last Active' },
  { key: 'expires_at', label: 'Expires' },
  { key: 'providers', label: 'Providers' },
  { key: 'models', label: 'Models' },
]

export default function VirtualKeys() {
  const qc = useQueryClient()
  const [expandedId, setExpandedId] = useState<string | null>(null)
  const [showCreate, setShowCreate] = useState(false)
  const [editingVk, setEditingVk] = useState<VirtualKey | null>(null)
  const [deletingVk, setDeletingVk] = useState<VirtualKey | null>(null)
  const [mutationError, setMutationError] = useState<string | null>(null)
  const [newKeyValue, setNewKeyValue] = useState<string | null>(null)
  const [search, setSearch] = useState('')
  const [filterField, setFilterField] = useState('all')
  const [showFilters, setShowFilters] = useState(false)
  const [revealedKeys, setRevealedKeys] = useState<Record<string, string>>({})

  const { data, isLoading, refetch, isFetching } = useQuery({
    queryKey: ['virtual-keys'],
    queryFn: virtualKeysApi.getAll,
    refetchInterval: 30_000,
  })

  const invalidate = () => qc.invalidateQueries({ queryKey: ['virtual-keys'] })

  const createMutation = useMutation({
    mutationFn: (form: VkFormState) => virtualKeysApi.create(formToPayload(form)),
    onSuccess: (result) => { invalidate(); setShowCreate(false); setMutationError(null); if (result?.value) setNewKeyValue(result.value) },
    onError: (e: Error) => setMutationError(e.message),
  })

  const updateMutation = useMutation({
    mutationFn: ({ id, form }: { id: string; form: VkFormState }) => virtualKeysApi.update(id, formToPayload(form)),
    onSuccess: () => { invalidate(); setEditingVk(null); setMutationError(null) },
    onError: (e: Error) => setMutationError(e.message),
  })

  const deleteMutation = useMutation({
    mutationFn: (id: string) => virtualKeysApi.remove(id),
    onSuccess: () => { invalidate(); setDeletingVk(null); setMutationError(null); },
    onError: (e: Error) => setMutationError(e.message),
  })

  const filtered = useMemo(() => {
    if (!data?.virtual_keys) return []
    const q = search.toLowerCase()
    return data.virtual_keys.filter(vk => {
      if (!q) return true
      const fields: Record<string, string> = {
        all: [vk.id, vk.name, vk.team_alias ?? '', vk.team_id ?? '', vk.organization_id ?? '', vk.access_group_id ?? '',
              vk.user_email ?? '', vk.user_id ?? '', vk.created_by ?? '', vk.description ?? ''].join(' '),
        id: vk.id,
        name: vk.name,
        team_alias: vk.team_alias ?? '',
        team_id: vk.team_id ?? '',
        organization_id: vk.organization_id ?? '',
        access_group_id: vk.access_group_id ?? '',
        user_email: vk.user_email ?? '',
        user_id: vk.user_id ?? '',
        created_by: vk.created_by ?? '',
      }
      return fields[filterField].toLowerCase().includes(q)
    })
  }, [data, search, filterField])

  return (
    <div className="p-6 space-y-6 overflow-y-auto h-full">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white">Virtual Keys</h1>
          <p className="text-sm text-zinc-400 mt-1">{filtered.length} / {data?.total ?? '—'} configured</p>
        </div>
        <div className="flex items-center gap-3">
          <button onClick={() => refetch()} disabled={isFetching}
            className="flex items-center justify-center p-2 text-zinc-400 hover:text-white bg-zinc-950 hover:bg-zinc-900 border border-zinc-800 disabled:opacity-50 rounded-lg transition-colors" title="Refresh keys"
          ><RotateCw size={15} className={isFetching ? 'animate-spin' : ''} /></button>
          <button onClick={() => { setMutationError(null); setShowCreate(true) }}
            className="flex items-center gap-2 px-4 py-2 bg-emerald-600 hover:bg-emerald-500 active:scale-[0.98] text-white text-sm rounded-lg transition-colors"
          ><Plus size={15} /> Create key</button>
        </div>
      </div>

      {/* Search + Filters */}
      <div className="flex items-center gap-3">
        <div className="flex-1">
          <SearchBar value={search} onChange={setSearch} placeholder="Search by any field…" />
        </div>
        <button onClick={() => setShowFilters(!showFilters)}
          className={`flex items-center gap-1.5 px-3 py-2 text-xs border rounded-lg transition-colors ${
            showFilters || filterField !== 'all'
              ? 'bg-emerald-500/10 text-emerald-300 border-emerald-500/30'
              : 'bg-zinc-950 text-zinc-400 border-zinc-800 hover:text-zinc-200'
          }`}
        ><Filter size={13} /> Filter</button>
      </div>

      {showFilters && (
        <div className="flex flex-wrap gap-2 p-3 bg-zinc-900/40 border border-zinc-800/50 rounded-xl">
          {[
            { key: 'all', label: 'All fields' },
            { key: 'id', label: 'Key ID' },
            { key: 'name', label: 'Key Alias' },
            { key: 'team_alias', label: 'Team Alias' },
            { key: 'team_id', label: 'Team ID' },
            { key: 'organization_id', label: 'Org ID' },
            { key: 'access_group_id', label: 'Group ID' },
            { key: 'user_email', label: 'User Email' },
            { key: 'user_id', label: 'User ID' },
            { key: 'created_by', label: 'Created By' },
          ].map(f => (
            <button key={f.key} onClick={() => { setFilterField(f.key); setSearch('') }}
              className={`px-2.5 py-1 rounded text-[11px] font-medium border transition-all ${
                filterField === f.key
                  ? 'bg-emerald-500/10 text-emerald-300 border-emerald-500/30'
                  : 'bg-zinc-950 text-zinc-500 border-zinc-800 hover:text-zinc-200'
              }`}
            >{f.label}</button>
          ))}
        </div>
      )}

      {/* Table */}
      <div className="rounded-xl border border-zinc-800/50 bg-zinc-900/30 overflow-x-auto">
        <table className="w-full text-sm whitespace-nowrap">
          <thead className="border-b border-zinc-800/50">
            <tr>
              {COLUMNS.map(c => (
                <th key={c.key} className="text-left px-3 py-3 text-[10px] text-zinc-500 uppercase tracking-wide font-medium">{c.label}</th>
              ))}
              <th className="px-3 py-3 text-[10px] text-zinc-500 uppercase tracking-wide font-medium">Status</th>
              <th className="px-3 py-3 text-[10px] text-zinc-500 uppercase tracking-wide font-medium w-12"></th>
            </tr>
          </thead>
          <tbody>
            {isLoading
              ? Array.from({ length: 3 }).map((_, i) => (
                  <tr key={i} className="border-b border-zinc-800/30">
                    {Array.from({ length: COLUMNS.length + 2 }).map((_, j) => (
                      <td key={j} className="px-3 py-3"><div className="h-3 bg-zinc-800 rounded animate-pulse w-20" /></td>
                    ))}
                  </tr>
                ))
              : filtered.map(vk => (
                  <>
                    <tr key={vk.id}
                      className={`border-b border-zinc-800/30 transition-colors cursor-pointer group ${
                        expandedId === vk.id ? 'bg-emerald-500/5' : 'hover:bg-zinc-800/30'
                      }`}
                      onClick={() => setExpandedId(expandedId === vk.id ? null : vk.id)}
                    >
                      <td className="px-3 py-3">
                        <div className="flex items-center gap-2">
                          <KeyRound size={13} className="text-emerald-400 shrink-0" />
                          <div>
                            <div className="font-medium text-white text-xs">{vk.name || '—'}</div>
                            {vk.description && <div className="text-[10px] text-zinc-500 truncate max-w-[140px]">{vk.description}</div>}
                          </div>
                        </div>
                      </td>
                      <td className="px-3 py-3 font-mono text-[10px] text-zinc-500 truncate max-w-[160px]" title={vk.id}>{vk.id}</td>
                      <td className="px-3 py-3 font-mono text-[10px] text-zinc-400">
                        <div className="flex items-center gap-1.5">
                          <span className="truncate max-w-[120px]">
                            {revealedKeys[vk.id] || vk.value.substring(0, 12) + '...'}
                          </span>
                          <button
                            onClick={async (e) => {
                              e.stopPropagation()
                              if (revealedKeys[vk.id]) {
                                const next = { ...revealedKeys }
                                delete next[vk.id]
                                setRevealedKeys(next)
                              } else {
                                try {
                                  const res = await virtualKeysApi.revealValue(vk.id)
                                  setRevealedKeys(prev => ({ ...prev, [vk.id]: res.value }))
                                } catch (err: any) {
                                  alert('Failed to reveal key: ' + (err.response?.data?.error || err.message))
                                }
                              }
                            }}
                            className="text-zinc-600 hover:text-indigo-400 transition-colors p-0.5"
                            title={revealedKeys[vk.id] ? 'Hide key' : 'Reveal key'}
                          >
                            {revealedKeys[vk.id] ? <EyeOff size={12} /> : <Eye size={12} />}
                          </button>
                          {revealedKeys[vk.id] && (
                            <button
                              onClick={(e) => {
                                e.stopPropagation()
                                navigator.clipboard.writeText(revealedKeys[vk.id])
                              }}
                              className="text-zinc-600 hover:text-emerald-400 transition-colors p-0.5"
                              title="Copy key"
                            >
                              <Copy size={11} />
                            </button>
                          )}
                        </div>
                      </td>
                      <td className="px-3 py-3 text-[11px] text-zinc-300">{fmtVal(vk.team_alias)}</td>
                      <td className="px-3 py-3 text-[11px] text-zinc-500">{fmtVal(vk.team_id)}</td>
                      <td className="px-3 py-3 text-[11px] text-zinc-500">{fmtVal(vk.organization_id)}</td>
                      <td className="px-3 py-3 text-[11px] text-zinc-500">{fmtVal(vk.access_group_id)}</td>
                      <td className="px-3 py-3 text-[11px] text-zinc-300">{fmtVal(vk.user_email)}</td>
                      <td className="px-3 py-3 text-[11px] text-zinc-500">{fmtVal(vk.user_id)}</td>
                      <td className="px-3 py-3 text-[11px] text-zinc-400">{tsDisplay(vk.created_at)}</td>
                      <td className="px-3 py-3 text-[11px] text-zinc-400">{fmtVal(vk.created_by)}</td>
                      <td className="px-3 py-3 text-[11px] text-zinc-400">{tsDisplay(vk.updated_at)}</td>
                      <td className="px-3 py-3 text-[11px] text-zinc-400">{tsAgo(vk.last_active)}</td>
                      <td className="px-3 py-3 text-[11px] text-zinc-400">{vk.expires_at ? tsDisplay(vk.expires_at) : 'Never'}</td>
                      <td className="px-3 py-3">
                        <div className="flex flex-wrap gap-1">
                          {vk.provider_configs.map(pc => (
                            <span key={pc.provider} className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[9px] bg-zinc-800 text-zinc-300 border border-zinc-700/50">
                              <ProviderIcon name={pc.provider} size={8} />
                              <span className="capitalize">{pc.provider}</span>
                            </span>
                          ))}
                          {vk.provider_configs.length === 0 && <span className="text-[10px] text-zinc-600">all</span>}
                        </div>
                      </td>
                      <td className="px-3 py-3">
                        <div className="flex items-center gap-1 text-zinc-400 text-[10px]">
                          <Shield size={10} />
                          {vk.provider_configs.length === 0
                            ? 'All models'
                            : vk.provider_configs.some(pc => pc.allowed_models.includes('*'))
                              ? 'All models'
                              : vk.provider_configs.flatMap(pc => pc.allowed_models).slice(0, 2).join(', ')
                          }
                        </div>
                      </td>
                      <td className="px-3 py-3">
                        {vk.is_active ? (
                          <span className="flex items-center gap-1 text-emerald-400 text-[10px]"><CheckCircle size={10} /> Active</span>
                        ) : (
                          <span className="flex items-center gap-1 text-zinc-500 text-[10px]"><XCircle size={10} /> Inactive</span>
                        )}
                      </td>
                      <td className="px-3 py-3">
                        <div className="flex items-center gap-1" onClick={e => e.stopPropagation()}>
                          <button onClick={() => { setMutationError(null); setEditingVk(vk) }}
                            className="opacity-0 group-hover:opacity-100 p-1 text-zinc-500 hover:text-emerald-400 hover:bg-emerald-400/10 rounded transition-all" title="Edit"
                          ><Pencil size={12} /></button>
                          <button onClick={() => { setMutationError(null); setDeletingVk(vk); }}
                            className="opacity-0 group-hover:opacity-100 p-1 text-zinc-500 hover:text-red-400 hover:bg-red-400/10 rounded transition-all" title="Delete"
                          ><Trash2 size={12} /></button>
                          <span className="text-zinc-600 ml-1">{expandedId === vk.id ? <ChevronUp size={13} /> : <ChevronDown size={13} />}</span>
                        </div>
                      </td>
                    </tr>
                    {expandedId === vk.id && (
                      <tr key={`${vk.id}-budget`} className="bg-zinc-800/20">
                        <td colSpan={COLUMNS.length + 2} className="px-6 py-4">
                          <div className="flex items-center gap-2 text-xs text-zinc-400 mb-3"><TrendingUp size={12} /><span>Governance</span></div>
                          <VkBudgetPanel vkId={vk.id} />
                        </td>
                      </tr>
                    )}
                  </>
                ))
            }
          </tbody>
        </table>
        {!isLoading && filtered.length === 0 && (
          <div className="text-center py-16 text-zinc-600">
            {search ? 'No keys match your search' : 'No virtual keys configured — create one to enable governance'}
          </div>
        )}
      </div>

      {/* Modals */}
      {showCreate && (
        <VkModal initial={DEFAULT_FORM} isEdit={false} onClose={() => { setShowCreate(false); setMutationError(null) }}
          onSave={form => createMutation.mutate(form)} isSaving={createMutation.isPending} error={mutationError} />
      )}
      {editingVk && (
        <VkModal initial={vkToForm(editingVk)} isEdit={true} onClose={() => { setEditingVk(null); setMutationError(null) }}
          onSave={form => updateMutation.mutate({ id: editingVk.id, form })} isSaving={updateMutation.isPending} error={mutationError} />
      )}
      {deletingVk && (
        <DeleteConfirmModal name={deletingVk.name} onClose={() => setDeletingVk(null)}
          onConfirm={() => deleteMutation.mutate(deletingVk.id)} isDeleting={deleteMutation.isPending} error={mutationError} />
      )}
      {newKeyValue && (
        <VkModal initial={DEFAULT_FORM} isEdit={false} onClose={() => setNewKeyValue(null)} onSave={() => {}} isSaving={false} error={null} createdKey={newKeyValue} />
      )}
    </div>
  )
}
