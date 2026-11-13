import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { policiesApi, type Policy } from '../lib/api'
import {
  FileBadge, Plus, Pencil, Trash2, X, Check, RotateCcw, AlertTriangle, CheckCircle, XCircle,
} from 'lucide-react'

interface PolicyFormState { name: string; description: string; policy_type: string; config: string; is_active: boolean }
const DEFAULT_FORM: PolicyFormState = { name: '', description: '', policy_type: 'rate_limit', config: '{}', is_active: true }

function PolicyModal({ initial, isEdit, onClose, onSave, isSaving, error }: {
  initial: PolicyFormState; isEdit: boolean; onClose: () => void; onSave: (f: PolicyFormState) => void; isSaving: boolean; error: string | null
}) {
  const [form, setForm] = useState<PolicyFormState>(initial)
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-lg mx-4">
        <div className="flex items-center justify-between p-5 border-b border-zinc-800/50">
          <h2 className="text-lg font-semibold text-white">{isEdit ? 'Edit policy' : 'Create policy'}</h2>
          <button onClick={onClose} className="text-zinc-500 hover:text-white"><X size={18} /></button>
        </div>
        <div className="p-5 space-y-5">
          <div><label className="block text-xs text-zinc-400 mb-1.5">Name *</label>
            <input type="text" value={form.name} onChange={e => setForm(f => ({ ...f, name: e.target.value }))}
              placeholder="My Policy" className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50" /></div>
          <div><label className="block text-xs text-zinc-400 mb-1.5">Description</label>
            <input type="text" value={form.description} onChange={e => setForm(f => ({ ...f, description: e.target.value }))}
              placeholder="Optional description" className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50" /></div>
          <div className="grid grid-cols-2 gap-3">
            <div><label className="block text-xs text-zinc-400 mb-1.5">Policy Type *</label>
              <select value={form.policy_type} onChange={e => setForm(f => ({ ...f, policy_type: e.target.value }))}
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50">
                <option value="rate_limit">Rate Limit</option>
                <option value="access_control">Access Control</option>
                <option value="routing">Routing</option>
                <option value="audit">Audit</option>
              </select></div>
          </div>
          <div><label className="block text-xs text-zinc-400 mb-1.5">Config (JSON)</label>
            <textarea value={form.config} onChange={e => setForm(f => ({ ...f, config: e.target.value }))}
              rows={4}
              className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 font-mono focus:outline-none focus:border-emerald-500/50" /></div>
          <div className="flex items-center gap-3">
            <button onClick={() => setForm(f => ({ ...f, is_active: !f.is_active }))}
              className={`relative w-10 h-5 rounded-full transition-colors ${form.is_active ? 'bg-emerald-600' : 'bg-zinc-700'}`}>
              <span className={`absolute top-0.5 w-4 h-4 rounded-full bg-white shadow transition-all ${form.is_active ? 'left-5' : 'left-0.5'}`} />
            </button>
            <span className="text-sm text-zinc-300">Active</span>
          </div>
          {error && <div className="flex items-center gap-2 text-red-400 text-xs bg-red-900/20 border border-red-800/50 rounded-lg px-3 py-2"><AlertTriangle size={13} />{error}</div>}
        </div>
        <div className="flex justify-end gap-3 px-5 py-4 border-t border-zinc-800/50">
          <button onClick={onClose} className="px-4 py-2 text-sm text-zinc-400 hover:text-white">Cancel</button>
          <button onClick={() => onSave(form)} disabled={isSaving || !form.name.trim()}
            className="px-4 py-2 text-sm bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-white rounded-lg flex items-center gap-2">
            {isSaving ? <RotateCcw size={14} className="animate-spin" /> : <Check size={14} />}
            {isEdit ? 'Update' : 'Create'}
          </button>
        </div>
      </div>
    </div>
  )
}

function DeleteConfirmModal({ name, onClose, onConfirm, isDeleting }: {
  name: string; onClose: () => void; onConfirm: () => void; isDeleting: boolean
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-sm mx-4 p-6">
        <div className="flex items-center gap-3 mb-4"><div className="w-9 h-9 rounded-full bg-red-500/15 flex items-center justify-center"><AlertTriangle size={16} className="text-red-400" /></div><div><div className="font-semibold text-white">Delete policy</div><div className="text-xs text-zinc-500">This action cannot be undone</div></div></div>
        <p className="text-sm text-zinc-400 mb-5">Delete <span className="text-white font-medium">{name}</span>?</p>
        <div className="flex justify-end gap-3">
          <button onClick={onClose} className="px-4 py-2 text-sm text-zinc-400 hover:text-white">Cancel</button>
          <button onClick={onConfirm} disabled={isDeleting}
            className="px-4 py-2 text-sm bg-red-600 hover:bg-red-500 disabled:opacity-50 text-white rounded-lg flex items-center gap-2">
            {isDeleting ? <RotateCcw size={13} className="animate-spin" /> : <Trash2 size={13} />}Delete
          </button>
        </div>
      </div>
    </div>
  )
}

export default function Policies() {
  const qc = useQueryClient()
  const [showCreate, setShowCreate] = useState(false)
  const [editing, setEditing] = useState<Policy | null>(null)
  const [deleting, setDeleting] = useState<Policy | null>(null)
  const [mutationError, setMutationError] = useState<string | null>(null)

  const { data, isLoading } = useQuery({ queryKey: ['policies'], queryFn: policiesApi.getAll })
  const invalidate = () => qc.invalidateQueries({ queryKey: ['policies'] })

  const createMut = useMutation({
    mutationFn: (f: PolicyFormState) => {
      let config: Record<string, unknown> = {}
      try {
        config = JSON.parse(f.config)
      } catch {
        // Fallback to empty object if JSON parsing fails
      }
      return policiesApi.create({ name: f.name, description: f.description || null, policy_type: f.policy_type, config, is_active: f.is_active })
    },
    onSuccess: () => { invalidate(); setShowCreate(false); setMutationError(null) },
    onError: (e: Error) => setMutationError(e.message),
  })

  const seedCommonMut = useMutation({
    mutationFn: async () => {
      const templates = [
        {
          name: "Standard Tier Rate Limit",
          description: "Limits requests to 60 RPM and 50,000 tokens per minute.",
          policy_type: "rate_limit",
          config: { max_requests_per_minute: 60, max_tokens_per_minute: 50000 },
          is_active: true
        },
        {
          name: "Developer Access Control",
          description: "Restricts access to local development keys and models.",
          policy_type: "access_control",
          config: { allowed_roles: ["admin", "developer"], require_mfa: false },
          is_active: true
        },
        {
          name: "Cost Optimization Routing",
          description: "Routes requests dynamically to the cheapest available provider.",
          policy_type: "routing",
          config: { strategy: "lowest_cost", fallback_provider: "openai" },
          is_active: true
        },
        {
          name: "Security & PII Auditing",
          description: "Enforces log auditing, PII masking, and prompt injection checks.",
          policy_type: "audit",
          config: { mask_pii: true, log_payloads: true, prevent_injection: true },
          is_active: true
        }
      ]
      for (const t of templates) {
        await policiesApi.create(t)
      }
    },
    onSuccess: () => { invalidate() },
  })

  const updateMut = useMutation({
    mutationFn: ({ id, f }: { id: string; f: PolicyFormState }) => {
      let config: Record<string, unknown> = {}
      try {
        config = JSON.parse(f.config)
      } catch {
        // Fallback to empty object if JSON parsing fails
      }
      return policiesApi.update(id, { name: f.name, description: f.description || null, policy_type: f.policy_type, config, is_active: f.is_active })
    },
    onSuccess: () => { invalidate(); setEditing(null); setMutationError(null) },
    onError: (e: Error) => setMutationError(e.message),
  })
  const deleteMut = useMutation({ mutationFn: (id: string) => policiesApi.remove(id), onSuccess: () => { invalidate(); setDeleting(null) } })

  return (
    <div className="p-6 space-y-6 overflow-y-auto h-full">
      <div className="flex items-center justify-between">
        <div><h1 className="text-2xl font-bold text-white">Policies</h1><p className="text-sm text-zinc-400 mt-1">{data?.total ?? '—'} configured</p></div>
        <div className="flex items-center gap-3">
          <button
            onClick={() => seedCommonMut.mutate()}
            disabled={seedCommonMut.isPending}
            className="flex items-center gap-2 px-4 py-2 border border-zinc-800 hover:border-zinc-700 bg-zinc-900/50 hover:bg-zinc-900 active:scale-[0.98] text-zinc-300 hover:text-white text-sm rounded-lg transition-all"
          >
            {seedCommonMut.isPending ? <RotateCcw size={14} className="animate-spin" /> : <Plus size={14} />}
            Add Common Policies
          </button>
          <button onClick={() => { setMutationError(null); setShowCreate(true) }}
            className="flex items-center gap-2 px-4 py-2 bg-emerald-600 hover:bg-emerald-500 active:scale-[0.98] text-white text-sm rounded-lg"><Plus size={15} />Create policy</button>
        </div>
      </div>
      <div className="rounded-xl border border-zinc-800/50 bg-zinc-900/30 overflow-hidden">
        <table className="w-full text-sm">
          <thead className="border-b border-zinc-800/50">
            <tr>{['Name', 'Type', 'Description', 'Status', ''].map(h => <th key={h} className="text-left px-5 py-3.5 text-xs text-zinc-500 uppercase tracking-wide font-medium">{h}</th>)}</tr>
          </thead>
          <tbody>
            {isLoading ? Array.from({ length: 3 }).map((_, i) => (
              <tr key={i} className="border-b border-zinc-800/30">{Array.from({ length: 5 }).map((_, j) => <td key={j} className="px-5 py-3.5"><div className="h-3 bg-zinc-800 rounded animate-pulse w-24" /></td>)}</tr>
            )) : data?.policies.map(p => (
              <tr key={p.id} className="border-b border-zinc-800/30 transition-colors group hover:bg-zinc-800/30">
                <td className="px-5 py-3.5"><div className="flex items-center gap-2"><FileBadge size={14} className="text-emerald-400 shrink-0" /><span className="font-medium text-white">{p.name}</span></div></td>
                <td className="px-5 py-3.5"><span className="capitalize text-xs px-2 py-0.5 rounded-full bg-zinc-800 text-zinc-300 border border-zinc-700/50">{p.policy_type}</span></td>
                <td className="px-5 py-3.5 text-zinc-400 text-xs">{p.description || '—'}</td>
                <td className="px-5 py-3.5">{p.is_active ? <span className="flex items-center gap-1.5 text-emerald-400 text-xs"><CheckCircle size={12} /> Active</span> : <span className="flex items-center gap-1.5 text-zinc-500 text-xs"><XCircle size={12} /> Inactive</span>}</td>
                <td className="px-5 py-3.5"><div className="flex items-center gap-1.5 opacity-0 group-hover:opacity-100 transition-all">
                  <button onClick={() => { setMutationError(null); setEditing(p) }} className="p-1.5 text-zinc-500 hover:text-emerald-400 hover:bg-emerald-400/10 rounded-lg"><Pencil size={13} /></button>
                  <button onClick={() => setDeleting(p)} className="p-1.5 text-zinc-500 hover:text-red-400 hover:bg-red-400/10 rounded-lg"><Trash2 size={13} /></button>
                </div></td>
              </tr>
            ))}
          </tbody>
        </table>
        {!isLoading && !data?.policies.length && <div className="text-center py-16 text-zinc-600">No policies configured</div>}
      </div>
      {showCreate && <PolicyModal initial={DEFAULT_FORM} isEdit={false} onClose={() => { setShowCreate(false); setMutationError(null) }} onSave={f => createMut.mutate(f)} isSaving={createMut.isPending} error={mutationError} />}
      {editing && <PolicyModal initial={{ name: editing.name, description: editing.description || '', policy_type: editing.policy_type, config: JSON.stringify(editing.config, null, 2), is_active: editing.is_active }} isEdit={true} onClose={() => { setEditing(null); setMutationError(null) }} onSave={f => updateMut.mutate({ id: editing.id, f })} isSaving={updateMut.isPending} error={mutationError} />}
      {deleting && <DeleteConfirmModal name={deleting.name} onClose={() => setDeleting(null)} onConfirm={() => deleteMut.mutate(deleting.id)} isDeleting={deleteMut.isPending} />}
    </div>
  )
}
