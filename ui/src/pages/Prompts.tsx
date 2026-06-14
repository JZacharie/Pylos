import { useState, useMemo } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { systemPromptsApi, type SystemPrompt } from '../lib/api'
import {
  Terminal, Plus, Pencil, Trash2, X, Check,
  AlertTriangle, RotateCw, Search, Copy,
} from 'lucide-react'

function DeleteConfirmModal({ name, onClose, onConfirm, isDeleting }: {
  name: string; onClose: () => void; onConfirm: () => void; isDeleting: boolean
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-sm mx-4 p-6">
        <div className="flex items-center gap-3 mb-4">
          <div className="w-9 h-9 rounded-full bg-red-500/15 flex items-center justify-center"><AlertTriangle size={16} className="text-red-400" /></div>
          <div><div className="font-semibold text-white">Delete prompt</div><div className="text-xs text-zinc-500">This action cannot be undone</div></div>
        </div>
        <p className="text-sm text-zinc-400 mb-5">Delete <span className="text-white font-medium">{name}</span>? This prompt will be removed from the system.</p>
        <div className="flex justify-end gap-3">
          <button onClick={onClose} className="px-4 py-2 text-sm text-zinc-400 hover:text-white">Cancel</button>
          <button onClick={onConfirm} disabled={isDeleting}
            className="px-4 py-2 text-sm bg-red-600 hover:bg-red-500 disabled:opacity-50 text-white rounded-lg flex items-center gap-2 transition-colors active:scale-[0.98]"
          >{isDeleting ? <RotateCw size={13} className="animate-spin" /> : <Trash2 size={13} />} Delete</button>
        </div>
      </div>
    </div>
  )
}

interface PromptFormState {
  id: string
  name: string
  prompt: string
}

const EMPTY_FORM: PromptFormState = { id: '', name: '', prompt: '' }

function PromptModal({ initial, isEdit, onClose, onSave, isSaving, error }: {
  initial: PromptFormState; isEdit: boolean; onClose: () => void
  onSave: (form: PromptFormState) => void; isSaving: boolean; error: string | null
}) {
  const [form, setForm] = useState<PromptFormState>(initial)
  const setField = <K extends keyof PromptFormState>(k: K, v: PromptFormState[K]) => setForm(f => ({ ...f, [k]: v }))

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-2xl mx-4 max-h-[90vh] overflow-y-auto">
        <div className="flex items-center justify-between p-5 border-b border-zinc-800/50">
          <h2 className="text-lg font-semibold text-white">{isEdit ? 'Edit prompt' : 'Create prompt'}</h2>
          <button onClick={onClose} className="text-zinc-500 hover:text-white transition-colors"><X size={18} /></button>
        </div>
        <div className="p-5 space-y-5">
          {!isEdit && (
            <div>
              <label className="block text-xs text-zinc-400 mb-1.5">ID *</label>
              <input type="text" value={form.id} onChange={e => setField('id', e.target.value)}
                placeholder="my-prompt-id"
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
              />
            </div>
          )}
          <div>
            <label className="block text-xs text-zinc-400 mb-1.5">Name *</label>
            <input type="text" value={form.name} onChange={e => setField('name', e.target.value)}
              placeholder="My Prompt"
              className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
            />
          </div>
          <div>
            <label className="block text-xs text-zinc-400 mb-1.5">Prompt Content *</label>
            <textarea value={form.prompt} onChange={e => setField('prompt', e.target.value)}
              placeholder="You are a helpful assistant..."
              rows={12}
              className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 font-mono focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20 resize-y"
            />
          </div>
          {error && (
            <div className="flex items-center gap-2 text-red-400 text-xs bg-red-900/20 border border-red-800/50 rounded-lg px-3 py-2">
              <AlertTriangle size={13} />{error}
            </div>
          )}
        </div>
        <div className="flex justify-end gap-3 px-5 py-4 border-t border-zinc-800/50">
          <button onClick={onClose} className="px-4 py-2 text-sm text-zinc-400 hover:text-white transition-colors">Cancel</button>
          <button onClick={() => onSave(form)}
            disabled={isSaving || !form.name.trim() || !form.prompt.trim() || (!isEdit && !form.id.trim())}
            className="px-4 py-2 text-sm bg-emerald-600 hover:bg-emerald-500 active:scale-[0.98] disabled:opacity-50 disabled:cursor-not-allowed text-white rounded-lg flex items-center gap-2 transition-colors"
          >
            {isSaving ? <RotateCw size={14} className="animate-spin" /> : <Check size={14} />}
            {isEdit ? 'Update' : 'Create'}
          </button>
        </div>
      </div>
    </div>
  )
}

export default function Prompts() {
  const qc = useQueryClient()
  const [search, setSearch] = useState('')
  const [showCreate, setShowCreate] = useState(false)
  const [editingPrompt, setEditingPrompt] = useState<SystemPrompt | null>(null)
  const [deletingPrompt, setDeletingPrompt] = useState<SystemPrompt | null>(null)
  const [mutationError, setMutationError] = useState<string | null>(null)
  const [expandedId, setExpandedId] = useState<string | null>(null)

  const { data, isLoading, refetch, isFetching } = useQuery({
    queryKey: ['system-prompts'],
    queryFn: systemPromptsApi.getAll,
    refetchInterval: 30_000,
  })

  const invalidate = () => qc.invalidateQueries({ queryKey: ['system-prompts'] })

  const createMutation = useMutation({
    mutationFn: (form: PromptFormState) => systemPromptsApi.create({ id: form.id, name: form.name, prompt: form.prompt }),
    onSuccess: () => { invalidate(); setShowCreate(false); setMutationError(null) },
    onError: (e: Error) => setMutationError(e.message),
  })

  const updateMutation = useMutation({
    mutationFn: ({ id, form }: { id: string; form: PromptFormState }) => systemPromptsApi.update(id, { name: form.name, prompt: form.prompt }),
    onSuccess: () => { invalidate(); setEditingPrompt(null); setMutationError(null) },
    onError: (e: Error) => setMutationError(e.message),
  })

  const deleteMutation = useMutation({
    mutationFn: (id: string) => systemPromptsApi.remove(id),
    onSuccess: () => { invalidate(); setDeletingPrompt(null) },
  })

  const prompts = data?.system_prompts ?? []

  const filtered = useMemo(() => {
    if (!prompts.length) return []
    const q = search.toLowerCase()
    return prompts.filter(p =>
      p.id.toLowerCase().includes(q) ||
      p.name.toLowerCase().includes(q) ||
      p.prompt.toLowerCase().includes(q)
    )
  }, [prompts, search])

  return (
    <div className="p-6 space-y-6 overflow-y-auto h-full">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white">Prompts Management</h1>
          <p className="text-sm text-zinc-400 mt-1">{filtered.length} / {prompts.length} prompts</p>
        </div>
        <div className="flex items-center gap-3">
          <button onClick={() => refetch()} disabled={isFetching}
            className="flex items-center justify-center p-2 text-zinc-400 hover:text-white bg-zinc-950 hover:bg-zinc-900 border border-zinc-800 disabled:opacity-50 rounded-lg transition-colors" title="Refresh"
          ><RotateCw size={15} className={isFetching ? 'animate-spin' : ''} /></button>
          <button onClick={() => { setMutationError(null); setShowCreate(true) }}
            className="flex items-center gap-2 px-4 py-2 bg-emerald-600 hover:bg-emerald-500 active:scale-[0.98] text-white text-sm rounded-lg transition-colors"
          ><Plus size={15} /> Create prompt</button>
        </div>
      </div>

      {/* Search */}
      <div className="relative max-w-md">
        <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-500" />
        <input type="text" value={search} onChange={e => setSearch(e.target.value)}
          placeholder="Search prompts by ID, name, or content…"
          className="w-full bg-zinc-950 border border-zinc-800 rounded-lg pl-9 pr-8 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
        />
        {search && (
          <button onClick={() => setSearch('')} className="absolute right-3 top-1/2 -translate-y-1/2 text-zinc-500 hover:text-zinc-300">
            <X size={14} />
          </button>
        )}
      </div>

      {/* Prompt list */}
      <div className="space-y-3">
        {isLoading
          ? Array.from({ length: 3 }).map((_, i) => (
              <div key={i} className="bg-zinc-900/40 border border-zinc-800/50 rounded-xl p-5 animate-pulse">
                <div className="h-4 bg-zinc-800 rounded w-48 mb-3" />
                <div className="h-3 bg-zinc-800/50 rounded w-full mb-2" />
                <div className="h-3 bg-zinc-800/50 rounded w-3/4" />
              </div>
            ))
          : filtered.map(p => (
              <div key={p.id}
                className={`bg-zinc-900/40 border rounded-xl overflow-hidden transition-all hover:border-zinc-700/50 ${
                  expandedId === p.id ? 'border-emerald-500/30' : 'border-zinc-800/50'
                }`}
              >
                <div
                  className="flex items-center justify-between px-5 py-4 cursor-pointer"
                  onClick={() => setExpandedId(expandedId === p.id ? null : p.id)}
                >
                  <div className="flex items-center gap-3 min-w-0 flex-1">
                    <div className="w-8 h-8 rounded-lg bg-indigo-500/10 border border-indigo-500/20 flex items-center justify-center text-indigo-400 shrink-0">
                      <Terminal size={16} />
                    </div>
                    <div className="min-w-0 flex-1">
                      <div className="flex items-center gap-2">
                        <span className="font-medium text-white text-sm truncate">{p.name}</span>
                        <span className="text-[10px] font-mono text-zinc-500 bg-zinc-800/50 px-1.5 py-0.5 rounded shrink-0">{p.id}</span>
                      </div>
                      <p className="text-xs text-zinc-500 mt-0.5 truncate max-w-xl">{p.prompt}</p>
                    </div>
                  </div>
                  <div className="flex items-center gap-2 shrink-0 ml-4" onClick={e => e.stopPropagation()}>
                    <button onClick={() => { navigator.clipboard.writeText(p.prompt) }}
                      className="p-1.5 text-zinc-500 hover:text-emerald-400 hover:bg-emerald-400/10 rounded transition-all" title="Copy prompt"
                    ><Copy size={13} /></button>
                    <button onClick={() => { setMutationError(null); setEditingPrompt(p) }}
                      className="p-1.5 text-zinc-500 hover:text-emerald-400 hover:bg-emerald-400/10 rounded transition-all" title="Edit"
                    ><Pencil size={13} /></button>
                    <button onClick={() => setDeletingPrompt(p)}
                      className="p-1.5 text-zinc-500 hover:text-red-400 hover:bg-red-400/10 rounded transition-all" title="Delete"
                    ><Trash2 size={13} /></button>
                  </div>
                </div>
                {expandedId === p.id && (
                  <div className="px-5 pb-4 pt-0 border-t border-zinc-800/30 mt-0">
                    <div className="mt-3 bg-zinc-950/50 border border-zinc-800/50 rounded-lg p-4">
                      <pre className="text-xs text-zinc-300 font-mono whitespace-pre-wrap break-words">{p.prompt}</pre>
                    </div>
                  </div>
                )}
              </div>
            ))
        }
        {!isLoading && filtered.length === 0 && (
          <div className="text-center py-16 text-zinc-600">
            {search ? 'No prompts match your search' : 'No prompts created yet — create one to get started'}
          </div>
        )}
      </div>

      {/* Modals */}
      {showCreate && (
        <PromptModal initial={EMPTY_FORM} isEdit={false} onClose={() => { setShowCreate(false); setMutationError(null) }}
          onSave={form => createMutation.mutate(form)} isSaving={createMutation.isPending} error={mutationError} />
      )}
      {editingPrompt && (
        <PromptModal
          initial={{ id: editingPrompt.id, name: editingPrompt.name, prompt: editingPrompt.prompt }}
          isEdit={true} onClose={() => { setEditingPrompt(null); setMutationError(null) }}
          onSave={form => updateMutation.mutate({ id: editingPrompt.id, form })} isSaving={updateMutation.isPending} error={mutationError} />
      )}
      {deletingPrompt && (
        <DeleteConfirmModal name={deletingPrompt.name} onClose={() => setDeletingPrompt(null)}
          onConfirm={() => deleteMutation.mutate(deletingPrompt.id)} isDeleting={deleteMutation.isPending} />
      )}
    </div>
  )
}
