import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { providersApi, type Provider } from '../lib/api'
import { providerColor, isCurrentUserAdmin } from '../lib/utils'
import { Key, Globe, RotateCcw, Plus, Pencil, Trash2, X, Check, AlertTriangle } from 'lucide-react'
import { ProviderIcon } from '../components/ProviderIcon'

// ─── Types ────────────────────────────────────────────────────────────────────

interface KeyEntry { name: string; value: string; models: string; weight: number }

interface ProviderFormState {
  name: string
  keys: KeyEntry[]
  base_url: string
  timeout_secs: number
  max_retries: number
}

const DEFAULT_FORM: ProviderFormState = {
  name: '',
  keys: [{ name: 'default', value: '', models: '*', weight: 1.0 }],
  base_url: '',
  timeout_secs: 30,
  max_retries: 3,
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

function formToPayload(form: ProviderFormState) {
  return {
    keys: form.keys.map(k => ({
      name: k.name,
      value: k.value,
      models: k.models.split(',').map(s => s.trim()).filter(Boolean),
      weight: k.weight,
    })),
    network: {
      base_url: form.base_url || null,
      timeout_secs: form.timeout_secs,
      max_retries: form.max_retries,
    },
  }
}

function providerToForm(p: Provider): ProviderFormState {
  return {
    name: p.name,
    keys: p.keys.map(k => ({
      name: k.name,
      value: '',
      models: k.models.join(', '),
      weight: k.weight,
    })),
    base_url: p.network.base_url ?? '',
    timeout_secs: p.network.timeout_secs,
    max_retries: p.network.max_retries,
  }
}

// ─── ProviderModal ────────────────────────────────────────────────────────────

function ProviderModal({
  initial,
  isEdit,
  onClose,
  onSave,
  isSaving,
  error,
}: {
  initial: ProviderFormState
  isEdit: boolean
  onClose: () => void
  onSave: (form: ProviderFormState) => void
  isSaving: boolean
  error: string | null
}) {
  const [form, setForm] = useState<ProviderFormState>(initial)

  const setField = <K extends keyof ProviderFormState>(k: K, v: ProviderFormState[K]) =>
    setForm(f => ({ ...f, [k]: v }))

  const setKey = (i: number, field: keyof KeyEntry, value: string | number) =>
    setForm(f => {
      const keys = [...f.keys]
      keys[i] = { ...keys[i], [field]: value }
      return { ...f, keys }
    })

  const addKey = () =>
    setForm(f => ({ ...f, keys: [...f.keys, { name: '', value: '', models: '*', weight: 1.0 }] }))

  const removeKey = (i: number) =>
    setForm(f => ({ ...f, keys: f.keys.filter((_, idx) => idx !== i) }))

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-lg mx-4 max-h-[90vh] overflow-y-auto">
        {/* Header */}
        <div className="flex items-center justify-between p-5 border-b border-zinc-800/50">
          <h2 className="text-lg font-semibold text-white">
            {isEdit ? 'Edit provider' : 'Add provider'}
          </h2>
          <button onClick={onClose} className="text-zinc-500 hover:text-white transition-colors">
            <X size={18} />
          </button>
        </div>

        <div className="p-5 space-y-5">
          {/* Provider name */}
          <div>
            <label className="block text-xs text-zinc-400 mb-1.5">Provider name *</label>
            <input
              type="text"
              value={form.name}
              onChange={e => setField('name', e.target.value)}
              disabled={isEdit}
              placeholder="openai, anthropic, ollama…"
              className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                disabled:opacity-50 disabled:cursor-not-allowed
                focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
            />
          </div>

          {/* Network */}
          <div className="grid grid-cols-3 gap-3">
            <div className="col-span-3">
              <label className="block text-xs text-zinc-400 mb-1.5">Base URL (optional)</label>
              <input
                type="text"
                value={form.base_url}
                onChange={e => setField('base_url', e.target.value)}
                placeholder="https://api.openai.com/v1"
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                  focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20 font-mono"
              />
            </div>
            <div>
              <label className="block text-xs text-zinc-400 mb-1.5">Timeout (s)</label>
              <input
                type="number"
                value={form.timeout_secs}
                onChange={e => setField('timeout_secs', Number(e.target.value))}
                min={1} max={300}
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                  focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
              />
            </div>
            <div>
              <label className="block text-xs text-zinc-400 mb-1.5">Max retries</label>
              <input
                type="number"
                value={form.max_retries}
                onChange={e => setField('max_retries', Number(e.target.value))}
                min={0} max={10}
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                  focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
              />
            </div>
          </div>

          {/* API Keys */}
          <div>
            <div className="flex items-center justify-between mb-2">
              <label className="text-xs text-zinc-400">API Keys</label>
              <button
                onClick={addKey}
                className="text-xs text-emerald-400 hover:text-emerald-300 flex items-center gap-1"
              >
                <Plus size={12} /> Add key
              </button>
            </div>
            <div className="space-y-3">
              {form.keys.map((k, i) => (
                <div key={i} className="bg-zinc-950/50 border border-zinc-800/50 rounded-lg p-3 space-y-2">
                  <div className="grid grid-cols-2 gap-2">
                    <div>
                      <label className="block text-xs text-zinc-500 mb-1">Name</label>
                      <input
                        value={k.name}
                        onChange={e => setKey(i, 'name', e.target.value)}
                        placeholder="default"
                        className="w-full bg-zinc-950 border border-zinc-800 rounded px-2 py-1.5 text-xs text-zinc-200
                          focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
                      />
                    </div>
                    <div>
                      <label className="block text-xs text-zinc-500 mb-1">Weight</label>
                      <input
                        type="number"
                        value={k.weight}
                        onChange={e => setKey(i, 'weight', Number(e.target.value))}
                        min={0.1} max={10} step={0.1}
                        className="w-full bg-zinc-950 border border-zinc-800 rounded px-2 py-1.5 text-xs text-zinc-200
                          focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
                      />
                    </div>
                  </div>
                  <div>
                    <label className="block text-xs text-zinc-500 mb-1">API Key value</label>
                    <input
                      type="password"
                      value={k.value}
                      onChange={e => setKey(i, 'value', e.target.value)}
                      placeholder={isEdit ? '(unchanged — leave blank to keep)' : 'sk-…'}
                      className="w-full bg-zinc-950 border border-zinc-800 rounded px-2 py-1.5 text-xs text-zinc-200
                        font-mono focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
                    />
                  </div>
                  <div>
                    <label className="block text-xs text-zinc-500 mb-1">Models (comma-sep, * for all)</label>
                    <input
                      value={k.models}
                      onChange={e => setKey(i, 'models', e.target.value)}
                      placeholder="*, gpt-4o, gpt-4o-mini"
                      className="w-full bg-zinc-950 border border-zinc-800 rounded px-2 py-1.5 text-xs text-zinc-200
                        font-mono focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
                    />
                  </div>
                  {form.keys.length > 1 && (
                    <button
                      onClick={() => removeKey(i)}
                      className="text-xs text-red-400 hover:text-red-300 flex items-center gap-1 mt-1"
                    >
                      <Trash2 size={11} /> Remove key
                    </button>
                  )}
                </div>
              ))}
            </div>
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
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm text-zinc-400 hover:text-white transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={() => onSave(form)}
            disabled={isSaving || !form.name.trim()}
            className="px-4 py-2 text-sm bg-emerald-600 hover:bg-emerald-500 active:scale-[0.98]
              disabled:opacity-50 disabled:cursor-not-allowed text-white rounded-lg flex items-center gap-2 transition-colors"
          >
            {isSaving ? (
              <RotateCcw size={14} className="animate-spin" />
            ) : (
              <Check size={14} />
            )}
            {isEdit ? 'Update' : 'Create'}
          </button>
        </div>
      </div>
    </div>
  )
}

// ─── DeleteConfirmModal ───────────────────────────────────────────────────────

function DeleteConfirmModal({
  name,
  onClose,
  onConfirm,
  isDeleting,
}: {
  name: string
  onClose: () => void
  onConfirm: () => void
  isDeleting: boolean
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-sm mx-4 p-6">
        <div className="flex items-center gap-3 mb-4">
          <div className="w-9 h-9 rounded-full bg-red-500/15 flex items-center justify-center">
            <AlertTriangle size={16} className="text-red-400" />
          </div>
          <div>
            <div className="font-semibold text-white">Delete provider</div>
            <div className="text-xs text-zinc-500">This action cannot be undone</div>
          </div>
        </div>
        <p className="text-sm text-zinc-400 mb-5">
          Remove <span className="text-white font-medium">{name}</span> from the gateway?
          Active inference calls will complete, but no new requests will be routed to this provider.
        </p>
        <div className="flex justify-end gap-3">
          <button onClick={onClose} className="px-4 py-2 text-sm text-zinc-400 hover:text-white">Cancel</button>
          <button
            onClick={onConfirm}
            disabled={isDeleting}
            className="px-4 py-2 text-sm bg-red-600 hover:bg-red-500 disabled:opacity-50 text-white rounded-lg
              flex items-center gap-2 transition-colors active:scale-[0.98]"
          >
            {isDeleting ? <RotateCcw size={13} className="animate-spin" /> : <Trash2 size={13} />}
            Delete
          </button>
        </div>
      </div>
    </div>
  )
}

// ─── Main page ────────────────────────────────────────────────────────────────

export default function Providers() {
  const qc = useQueryClient()
  const [showCreate, setShowCreate] = useState(false)
  const [editingProvider, setEditingProvider] = useState<Provider | null>(null)
  const [deletingProvider, setDeletingProvider] = useState<Provider | null>(null)
  const [mutationError, setMutationError] = useState<string | null>(null)

  const { data, isLoading } = useQuery({
    queryKey: ['providers'],
    queryFn: providersApi.getAll,
    refetchInterval: 30_000,
  })

  const invalidate = () => qc.invalidateQueries({ queryKey: ['providers'] })

  const createMutation = useMutation({
    mutationFn: (form: ProviderFormState) => {
      const payload = { name: form.name, ...formToPayload(form) }
      return providersApi.create(payload)
    },
    onSuccess: () => { invalidate(); setShowCreate(false); setMutationError(null) },
    onError: (e: Error) => setMutationError(e.message),
  })

  const updateMutation = useMutation({
    mutationFn: ({ name, form }: { name: string; form: ProviderFormState }) => {
      const payload = formToPayload(form)
      return providersApi.update(name, payload)
    },
    onSuccess: () => { invalidate(); setEditingProvider(null); setMutationError(null) },
    onError: (e: Error) => setMutationError(e.message),
  })

  const deleteMutation = useMutation({
    mutationFn: (name: string) => providersApi.remove(name),
    onSuccess: () => { invalidate(); setDeletingProvider(null) },
  })

  return (
    <div className="p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white">Providers</h1>
          <p className="text-sm text-zinc-400 mt-1">
            {data?.total ?? '—'} configured
          </p>
        </div>
        {isCurrentUserAdmin() && (
          <button
            onClick={() => { setMutationError(null); setShowCreate(true) }}
            className="flex items-center gap-2 px-4 py-2 bg-emerald-600 hover:bg-emerald-500 active:scale-[0.98]
              text-white text-sm rounded-lg transition-colors"
          >
            <Plus size={15} />
            Add provider
          </button>
        )}
      </div>

      {/* Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
        {isLoading
          ? Array.from({ length: 3 }).map((_, i) => (
              <div key={i} className="rounded-xl border border-zinc-800/50 bg-zinc-900/30 p-5 animate-pulse h-40" />
            ))
          : data?.providers.map(p => (
              <ProviderCard
                key={p.name}
                provider={p}
                onEdit={() => { setMutationError(null); setEditingProvider(p) }}
                onDelete={() => setDeletingProvider(p)}
              />
            ))
        }
        {!isLoading && !data?.providers.length && (
          <div className="col-span-full text-center py-16 text-zinc-600">
            No providers configured — add one to get started
          </div>
        )}
      </div>

      {/* Create modal */}
      {showCreate && (
        <ProviderModal
          initial={DEFAULT_FORM}
          isEdit={false}
          onClose={() => setShowCreate(false)}
          onSave={form => createMutation.mutate(form)}
          isSaving={createMutation.isPending}
          error={mutationError}
        />
      )}

      {/* Edit modal */}
      {editingProvider && (
        <ProviderModal
          initial={providerToForm(editingProvider)}
          isEdit={true}
          onClose={() => setEditingProvider(null)}
          onSave={form => updateMutation.mutate({ name: editingProvider.name, form })}
          isSaving={updateMutation.isPending}
          error={mutationError}
        />
      )}

      {/* Delete confirm */}
      {deletingProvider && (
        <DeleteConfirmModal
          name={deletingProvider.name}
          onClose={() => setDeletingProvider(null)}
          onConfirm={() => deleteMutation.mutate(deletingProvider.name)}
          isDeleting={deleteMutation.isPending}
        />
      )}
    </div>
  )
}

// ─── ProviderCard ─────────────────────────────────────────────────────────────

function ProviderCard({
  provider,
  onEdit,
  onDelete,
}: {
  provider: Provider
  onEdit: () => void
  onDelete: () => void
}) {
  const color = providerColor(provider.name)
  const [testing, setTesting] = useState(false)
  const [testResult, setTestResult] = useState<'success' | 'error' | null>(null)
  const [testError, setTestError] = useState<string | null>(null)

  const handleTest = async (e: React.MouseEvent) => {
    e.stopPropagation()
    setTesting(true)
    setTestResult(null)
    setTestError(null)
    try {
      await providersApi.test(provider.name)
      setTestResult('success')
    } catch (err: any) {
      setTestResult('error')
      setTestError(err.response?.data?.error || err.message || 'Connection failed')
    } finally {
      setTesting(false)
    }
  }

  const isAdmin = isCurrentUserAdmin()
  return (
    <div className="rounded-xl border border-zinc-800/50 bg-zinc-900/30 p-5 hover:border-zinc-700/50
      transition-colors group">
      {/* Header */}
      <div className="flex items-center gap-3 mb-4">
        <div
          className="w-9 h-9 rounded-lg flex items-center justify-center"
          style={{ background: color + '20', color }}
        >
          <ProviderIcon name={provider.name} size={16} />
        </div>
        <div className="min-w-0">
          <div className="font-semibold text-white capitalize truncate">{provider.name}</div>
          <div className="text-xs text-zinc-500">
            {provider.keys_count} key{provider.keys_count !== 1 ? 's' : ''}
          </div>
        </div>
        <div className="ml-auto flex items-center gap-2">
          {testResult === 'success' && (
            <span className="text-emerald-400" title="Connection OK">
              <Check size={14} />
            </span>
          )}
          {testResult === 'error' && (
            <span className="text-red-400" title={testError || 'Connection failed'}>
              <AlertTriangle size={14} />
            </span>
          )}
          <button
            onClick={handleTest}
            disabled={testing}
            className={`p-1.5 rounded-lg transition-all ${
              testing
                ? 'text-emerald-400 bg-emerald-400/10'
                : 'text-zinc-500 hover:text-emerald-400 hover:bg-emerald-400/10'
            }`}
            title="Test Connection"
          >
            <RotateCcw size={13} className={testing ? 'animate-spin' : ''} />
          </button>
          {/* Actions — visible on hover */}
          {isAdmin && (
            <>
              <button
                onClick={e => { e.stopPropagation(); onEdit() }}
                className="opacity-0 group-hover:opacity-100 p-1.5 text-zinc-500 hover:text-white
                  hover:bg-zinc-800/50 rounded-lg transition-all"
                title="Edit"
              >
                <Pencil size={13} />
              </button>
              <button
                onClick={e => { e.stopPropagation(); onDelete() }}
                className="opacity-0 group-hover:opacity-100 p-1.5 text-zinc-500 hover:text-red-400
                  hover:bg-red-400/10 rounded-lg transition-all"
                title="Delete"
              >
                <Trash2 size={13} />
              </button>
            </>
          )}
        </div>
      </div>

      {/* Network info */}
      <div className="space-y-2 text-xs">
        {provider.network.base_url && (
          <div className="flex items-center gap-2 text-zinc-400">
            <Globe size={12} />
            <span className="truncate font-mono">{provider.network.base_url}</span>
          </div>
        )}
        <div className="flex items-center gap-4 text-zinc-500">
          <span className="flex items-center gap-1">
            <RotateCcw size={11} />
            {provider.network.max_retries} retries
          </span>
          <span>{provider.network.timeout_secs}s timeout</span>
        </div>
      </div>

      {/* Keys preview */}
      {provider.keys.length > 0 && (
        <div className="mt-3 pt-3 border-t border-zinc-800/50 space-y-1.5">
          {provider.keys.slice(0, 3).map((k, i) => (
            <div key={i} className="flex items-center gap-2 text-xs">
              <Key size={11} className="text-zinc-600 shrink-0" />
              <span className="text-zinc-300 truncate">{k.name}</span>
              <span className="ml-auto text-zinc-600 font-mono shrink-0">{k.value}</span>
            </div>
          ))}
          {provider.keys.length > 3 && (
            <div className="text-xs text-zinc-600 pl-5">
              +{provider.keys.length - 3} more
            </div>
          )}
        </div>
      )}
    </div>
  )
}
