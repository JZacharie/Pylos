import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { mcpServersApi, type McpServer } from '../lib/api'
import { Plus, Pencil, Trash2, X, Check, AlertTriangle, RotateCcw, Server, Globe, Box, Power, PowerOff } from 'lucide-react'

// ─── Types ────────────────────────────────────────────────────────────────────

interface EnvVarEntry { key: string; value: string }

interface McpServerFormState {
  name: string
  server_type: string
  target_url: string
  container_image: string
  env_vars: EnvVarEntry[]
  virtual_key_id: string
  team_id: string
}

const DEFAULT_FORM: McpServerFormState = {
  name: '',
  server_type: 'python',
  target_url: '',
  container_image: '',
  env_vars: [],
  virtual_key_id: '',
  team_id: '',
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

function formToPayload(form: McpServerFormState) {
  const envVars: Record<string, string> = {}
  for (const e of form.env_vars) {
    if (e.key.trim()) envVars[e.key.trim()] = e.value
  }
  return {
    name: form.name,
    server_type: form.server_type,
    target_url: form.target_url || undefined,
    container_image: form.container_image || undefined,
    env_vars: Object.keys(envVars).length > 0 ? envVars : undefined,
    virtual_key_id: form.virtual_key_id || undefined,
    team_id: form.team_id || undefined,
  }
}

function serverToForm(s: McpServer): McpServerFormState {
  return {
    name: s.name,
    server_type: s.server_type,
    target_url: s.target_url ?? '',
    container_image: s.container_image ?? '',
    env_vars: s.env_vars
      ? Object.entries(s.env_vars).map(([key, value]) => ({ key, value }))
      : [],
    virtual_key_id: s.virtual_key_id ?? '',
    team_id: s.team_id ?? '',
  }
}

function statusColor(status: string) {
  if (status === 'active') return 'bg-emerald-500'
  if (status === 'error') return 'bg-red-500'
  return 'bg-zinc-500'
}

function statusLabel(status: string) {
  return status.charAt(0).toUpperCase() + status.slice(1)
}

// ─── McpServerModal ───────────────────────────────────────────────────────────

function McpServerModal({
  initial,
  isEdit,
  onClose,
  onSave,
  isSaving,
  error,
}: {
  initial: McpServerFormState
  isEdit: boolean
  onClose: () => void
  onSave: (form: McpServerFormState) => void
  isSaving: boolean
  error: string | null
}) {
  const [form, setForm] = useState<McpServerFormState>(initial)

  const setField = <K extends keyof McpServerFormState>(k: K, v: McpServerFormState[K]) =>
    setForm(f => ({ ...f, [k]: v }))

  const setEnvVar = (i: number, field: keyof EnvVarEntry, value: string) =>
    setForm(f => {
      const env_vars = [...f.env_vars]
      env_vars[i] = { ...env_vars[i], [field]: value }
      return { ...f, env_vars }
    })

  const addEnvVar = () =>
    setForm(f => ({ ...f, env_vars: [...f.env_vars, { key: '', value: '' }] }))

  const removeEnvVar = (i: number) =>
    setForm(f => ({ ...f, env_vars: f.env_vars.filter((_, idx) => idx !== i) }))

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-lg mx-4 max-h-[90vh] overflow-y-auto">
        {/* Header */}
        <div className="flex items-center justify-between p-5 border-b border-zinc-800/50">
          <h2 className="text-lg font-semibold text-white">
            {isEdit ? 'Edit MCP server' : 'Add MCP server'}
          </h2>
          <button onClick={onClose} className="text-zinc-500 hover:text-white transition-colors">
            <X size={18} />
          </button>
        </div>

        <div className="p-5 space-y-5">
          {/* Name */}
          <div>
            <label className="block text-xs text-zinc-400 mb-1.5">Name *</label>
            <input
              type="text"
              value={form.name}
              onChange={e => setField('name', e.target.value)}
              disabled={isEdit}
              placeholder="my-mcp-server"
              className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                disabled:opacity-50 disabled:cursor-not-allowed
                focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
            />
          </div>

          {/* Server type */}
          <div>
            <label className="block text-xs text-zinc-400 mb-1.5">Server type</label>
            <select
              value={form.server_type}
              onChange={e => setField('server_type', e.target.value)}
              className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
            >
              <option value="python">Python</option>
              <option value="node">Node</option>
              <option value="custom">Custom</option>
            </select>
          </div>

          {/* Target URL */}
          <div>
            <label className="block text-xs text-zinc-400 mb-1.5">Target URL</label>
            <input
              type="text"
              value={form.target_url}
              onChange={e => setField('target_url', e.target.value)}
              placeholder="http://localhost:3000/mcp"
              className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                font-mono focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
            />
          </div>

          {/* Container image */}
          <div>
            <label className="block text-xs text-zinc-400 mb-1.5">Container image</label>
            <input
              type="text"
              value={form.container_image}
              onChange={e => setField('container_image', e.target.value)}
              placeholder="ghcr.io/org/mcp-server:latest"
              className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                font-mono focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
            />
          </div>

          {/* Virtual Key & Team */}
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-xs text-zinc-400 mb-1.5">Virtual Key ID</label>
              <input
                type="text"
                value={form.virtual_key_id}
                onChange={e => setField('virtual_key_id', e.target.value)}
                placeholder="vk_..."
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                  font-mono focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
              />
            </div>
            <div>
              <label className="block text-xs text-zinc-400 mb-1.5">Team ID</label>
              <input
                type="text"
                value={form.team_id}
                onChange={e => setField('team_id', e.target.value)}
                placeholder="team_..."
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                  font-mono focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
              />
            </div>
          </div>

          {/* Env vars */}
          <div>
            <div className="flex items-center justify-between mb-2">
              <label className="text-xs text-zinc-400">Environment variables</label>
              <button
                onClick={addEnvVar}
                className="text-xs text-emerald-400 hover:text-emerald-300 flex items-center gap-1"
              >
                <Plus size={12} /> Add var
              </button>
            </div>
            <div className="space-y-3">
              {form.env_vars.map((ev, i) => (
                <div key={i} className="bg-zinc-950/50 border border-zinc-800/50 rounded-lg p-3 space-y-2">
                  <div className="grid grid-cols-2 gap-2">
                    <div>
                      <label className="block text-xs text-zinc-500 mb-1">Key</label>
                      <input
                        value={ev.key}
                        onChange={e => setEnvVar(i, 'key', e.target.value)}
                        placeholder="API_KEY"
                        className="w-full bg-zinc-950 border border-zinc-800 rounded px-2 py-1.5 text-xs text-zinc-200
                          font-mono focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
                      />
                    </div>
                    <div>
                      <label className="block text-xs text-zinc-500 mb-1">Value</label>
                      <input
                        type="password"
                        value={ev.value}
                        onChange={e => setEnvVar(i, 'value', e.target.value)}
                        placeholder="sk-..."
                        className="w-full bg-zinc-950 border border-zinc-800 rounded px-2 py-1.5 text-xs text-zinc-200
                          font-mono focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
                      />
                    </div>
                  </div>
                  <button
                    onClick={() => removeEnvVar(i)}
                    className="text-xs text-red-400 hover:text-red-300 flex items-center gap-1 mt-1"
                  >
                    <Trash2 size={11} /> Remove
                  </button>
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
            <div className="font-semibold text-white">Delete MCP server</div>
            <div className="text-xs text-zinc-500">This action cannot be undone</div>
          </div>
        </div>
        <p className="text-sm text-zinc-400 mb-5">
          Remove <span className="text-white font-medium">{name}</span> from the gateway?
          Active tool calls will complete, but no new requests will be routed to this server.
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

// ─── McpServerCard ────────────────────────────────────────────────────────────

function McpServerCard({
  server,
  onEdit,
  onDelete,
  onToggleActive,
  isToggling,
}: {
  server: McpServer
  onEdit: () => void
  onDelete: () => void
  onToggleActive: () => void
  isToggling: boolean
}) {
  const isActive = server.status === 'active'

  return (
    <div className="rounded-xl border border-zinc-800/50 bg-zinc-900/30 p-5 hover:border-zinc-700/50
      transition-colors group">
      {/* Header */}
      <div className="flex items-center gap-3 mb-4">
        <div className="w-9 h-9 rounded-lg flex items-center justify-center bg-zinc-800/50">
          <Server size={16} className="text-zinc-400" />
        </div>
        <div className="min-w-0">
          <div className="font-semibold text-white truncate">{server.name}</div>
          <div className="flex items-center gap-2">
            <span className="inline-flex items-center gap-1 text-xs text-zinc-500">
              <Box size={11} />
              {server.server_type}
            </span>
          </div>
        </div>
        <div className="ml-auto flex items-center gap-2">
          {/* Status indicator */}
          <span
            className={`w-2 h-2 rounded-full ${statusColor(server.status)}`}
            title={statusLabel(server.status)}
          />
          <span className="text-xs text-zinc-500">{statusLabel(server.status)}</span>
        </div>
      </div>

      {/* Info */}
      <div className="space-y-2 text-xs">
        {server.target_url && (
          <div className="flex items-center gap-2 text-zinc-400">
            <Globe size={12} />
            <span className="truncate font-mono">{server.target_url}</span>
          </div>
        )}
        {server.container_image && (
          <div className="flex items-center gap-2 text-zinc-400">
            <Box size={12} />
            <span className="truncate font-mono">{server.container_image}</span>
          </div>
        )}
      </div>

      {/* Env vars preview */}
      {server.env_vars && Object.keys(server.env_vars).length > 0 && (
        <div className="mt-3 pt-3 border-t border-zinc-800/50 space-y-1.5">
          {Object.entries(server.env_vars).slice(0, 3).map(([k, v], i) => (
            <div key={i} className="flex items-center gap-2 text-xs">
              <span className="text-zinc-300 font-mono truncate">{k}</span>
              <span className="ml-auto text-zinc-600 font-mono shrink-0">
                {v.length > 8 ? v.slice(0, 8) + '…' : v}
              </span>
            </div>
          ))}
          {Object.keys(server.env_vars).length > 3 && (
            <div className="text-xs text-zinc-600">
              +{Object.keys(server.env_vars).length - 3} more
            </div>
          )}
        </div>
      )}

      {/* Actions */}
      <div className="mt-4 pt-3 border-t border-zinc-800/50 flex items-center gap-2">
        <button
          onClick={onToggleActive}
          disabled={isToggling}
          className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs transition-colors
            ${isActive
              ? 'text-amber-400 bg-amber-400/10 hover:bg-amber-400/20'
              : 'text-emerald-400 bg-emerald-400/10 hover:bg-emerald-400/20'
            }
            disabled:opacity-50`}
          title={isActive ? 'Deactivate' : 'Activate'}
        >
          {isToggling ? (
            <RotateCcw size={12} className="animate-spin" />
          ) : isActive ? (
            <PowerOff size={12} />
          ) : (
            <Power size={12} />
          )}
          {isActive ? 'Deactivate' : 'Activate'}
        </button>

        <div className="ml-auto flex items-center gap-1">
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
        </div>
      </div>
    </div>
  )
}

// ─── Main page ────────────────────────────────────────────────────────────────

export default function McpServers() {
  const qc = useQueryClient()
  const [showCreate, setShowCreate] = useState(false)
  const [editingServer, setEditingServer] = useState<McpServer | null>(null)
  const [deletingServer, setDeletingServer] = useState<McpServer | null>(null)
  const [mutationError, setMutationError] = useState<string | null>(null)

  const { data: servers, isLoading } = useQuery({
    queryKey: ['mcp-servers'],
    queryFn: mcpServersApi.getAll,
    refetchInterval: 30_000,
  })

  const invalidate = () => qc.invalidateQueries({ queryKey: ['mcp-servers'] })

  const createMutation = useMutation({
    mutationFn: (form: McpServerFormState) => {
      const payload = formToPayload(form)
      return mcpServersApi.create(payload)
    },
    onSuccess: () => { invalidate(); setShowCreate(false); setMutationError(null) },
    onError: (e: Error) => setMutationError(e.message),
  })

  const updateMutation = useMutation({
    mutationFn: ({ id, form }: { id: string; form: McpServerFormState }) => {
      const payload = formToPayload(form)
      return mcpServersApi.update(id, payload)
    },
    onSuccess: () => { invalidate(); setEditingServer(null); setMutationError(null) },
    onError: (e: Error) => setMutationError(e.message),
  })

  const deleteMutation = useMutation({
    mutationFn: (id: string) => mcpServersApi.remove(id),
    onSuccess: () => { invalidate(); setDeletingServer(null) },
  })

  const toggleMutation = useMutation({
    mutationFn: (server: McpServer) =>
      server.status === 'active'
        ? mcpServersApi.deactivate(server.id)
        : mcpServersApi.activate(server.id),
    onSuccess: () => invalidate(),
  })

  return (
    <div className="p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white">MCP Servers</h1>
          <p className="text-sm text-zinc-400 mt-1">
            {servers?.length ?? '—'} configured
          </p>
        </div>
        <button
          onClick={() => { setMutationError(null); setShowCreate(true) }}
          className="flex items-center gap-2 px-4 py-2 bg-emerald-600 hover:bg-emerald-500 active:scale-[0.98]
            text-white text-sm rounded-lg transition-colors"
        >
          <Plus size={15} />
          Add server
        </button>
      </div>

      {/* Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
        {isLoading
          ? Array.from({ length: 3 }).map((_, i) => (
              <div key={i} className="rounded-xl border border-zinc-800/50 bg-zinc-900/30 p-5 animate-pulse h-48" />
            ))
          : servers?.map(s => (
              <McpServerCard
                key={s.id}
                server={s}
                onEdit={() => { setMutationError(null); setEditingServer(s) }}
                onDelete={() => setDeletingServer(s)}
                onToggleActive={() => toggleMutation.mutate(s)}
                isToggling={toggleMutation.isPending}
              />
            ))
        }
        {!isLoading && !servers?.length && (
          <div className="col-span-full text-center py-16 text-zinc-600">
            No MCP servers configured — add one to get started
          </div>
        )}
      </div>

      {/* Create modal */}
      {showCreate && (
        <McpServerModal
          initial={DEFAULT_FORM}
          isEdit={false}
          onClose={() => setShowCreate(false)}
          onSave={form => createMutation.mutate(form)}
          isSaving={createMutation.isPending}
          error={mutationError}
        />
      )}

      {/* Edit modal */}
      {editingServer && (
        <McpServerModal
          initial={serverToForm(editingServer)}
          isEdit={true}
          onClose={() => setEditingServer(null)}
          onSave={form => updateMutation.mutate({ id: editingServer.id, form })}
          isSaving={updateMutation.isPending}
          error={mutationError}
        />
      )}

      {/* Delete confirm */}
      {deletingServer && (
        <DeleteConfirmModal
          name={deletingServer.name}
          onClose={() => setDeletingServer(null)}
          onConfirm={() => deleteMutation.mutate(deletingServer.id)}
          isDeleting={deleteMutation.isPending}
        />
      )}
    </div>
  )
}
