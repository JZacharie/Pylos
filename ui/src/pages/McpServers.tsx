import { useState, useMemo } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { mcpServersApi, type McpServer } from '../lib/api'
import {
  Cpu, Plus, Pencil, Trash2, X, Check, RotateCcw, AlertTriangle,
  Search, ExternalLink, Layers, KeyRound, Code
} from 'lucide-react'

interface McpFormState {
  name: string
  server_type: string
  target_url: string
  container_image: string
  env_vars: string
  virtual_key_id: string
  team_id: string
}

const DEFAULT_FORM: McpFormState = {
  name: '',
  server_type: 'python',
  target_url: '',
  container_image: '',
  env_vars: '{}',
  virtual_key_id: '',
  team_id: '',
}

function McpModal({ initial, isEdit, onClose, onSave, isSaving, error }: {
  initial: McpFormState; isEdit: boolean; onClose: () => void; onSave: (f: McpFormState) => void; isSaving: boolean; error: string | null
}) {
  const [form, setForm] = useState<McpFormState>(initial)
  const [mode, setMode] = useState<'url' | 'container'>(initial.target_url ? 'url' : 'container')

  const handleSubmit = () => {
    onSave({
      ...form,
      target_url: mode === 'url' ? form.target_url : '',
      container_image: mode === 'container' ? form.container_image : '',
    })
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
      <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-xl mx-4 max-h-[90vh] overflow-y-auto">
        <div className="flex items-center justify-between p-5 border-b border-zinc-800/50">
          <h2 className="text-lg font-semibold text-white">
            {isEdit ? 'Modifier le serveur MCP' : 'Créer un serveur MCP'}
          </h2>
          <button onClick={onClose} className="text-zinc-500 hover:text-white transition-colors">
            <X size={18} />
          </button>
        </div>

        <div className="p-5 space-y-5">
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-xs font-semibold text-zinc-400 uppercase tracking-wider mb-1.5">Nom *</label>
              <input
                type="text"
                value={form.name}
                onChange={e => setForm(f => ({ ...f, name: e.target.value }))}
                placeholder="Mon Serveur MCP"
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50"
              />
            </div>
            <div>
              <label className="block text-xs font-semibold text-zinc-400 uppercase tracking-wider mb-1.5">Type de Serveur *</label>
              <select
                value={form.server_type}
                onChange={e => setForm(f => ({ ...f, server_type: e.target.value }))}
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50"
              >
                <option value="python">Python</option>
                <option value="node">Node.js</option>
                <option value="custom">Custom / Autre</option>
              </select>
            </div>
          </div>

          <div>
            <label className="block text-xs font-semibold text-zinc-400 uppercase tracking-wider mb-2">Mode de connexion</label>
            <div className="grid grid-cols-2 gap-2 p-1 bg-zinc-950 rounded-lg border border-zinc-800">
              <button
                type="button"
                onClick={() => setMode('url')}
                className={`py-1.5 text-xs font-medium rounded-md transition-all ${
                  mode === 'url'
                    ? 'bg-zinc-800 text-white shadow-sm'
                    : 'text-zinc-400 hover:text-zinc-200'
                }`}
              >
                URL HTTP (SSE)
              </button>
              <button
                type="button"
                onClick={() => setMode('container')}
                className={`py-1.5 text-xs font-medium rounded-md transition-all ${
                  mode === 'container'
                    ? 'bg-zinc-800 text-white shadow-sm'
                    : 'text-zinc-400 hover:text-zinc-200'
                }`}
              >
                Image Docker
              </button>
            </div>
          </div>

          {mode === 'url' ? (
            <div>
              <label className="block text-xs font-semibold text-zinc-400 uppercase tracking-wider mb-1.5">URL Target *</label>
              <input
                type="url"
                value={form.target_url}
                onChange={e => setForm(f => ({ ...f, target_url: e.target.value }))}
                placeholder="http://localhost:8000/mcp"
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50"
              />
            </div>
          ) : (
            <div>
              <label className="block text-xs font-semibold text-zinc-400 uppercase tracking-wider mb-1.5">Image Docker / Container *</label>
              <input
                type="text"
                value={form.container_image}
                onChange={e => setForm(f => ({ ...f, container_image: e.target.value }))}
                placeholder="pylos-mcp-server-node:latest"
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50"
              />
            </div>
          )}

          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-xs font-semibold text-zinc-400 uppercase tracking-wider mb-1.5">Virtual Key ID (Optionnel)</label>
              <input
                type="text"
                value={form.virtual_key_id}
                onChange={e => setForm(f => ({ ...f, virtual_key_id: e.target.value }))}
                placeholder="vk-..."
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50"
              />
            </div>
            <div>
              <label className="block text-xs font-semibold text-zinc-400 uppercase tracking-wider mb-1.5">Team ID (Optionnel)</label>
              <input
                type="text"
                value={form.team_id}
                onChange={e => setForm(f => ({ ...f, team_id: e.target.value }))}
                placeholder="team-..."
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50"
              />
            </div>
          </div>

          <div>
            <label className="block text-xs font-semibold text-zinc-400 uppercase tracking-wider mb-1.5">
              Variables d'Environnement (JSON)
            </label>
            <textarea
              value={form.env_vars}
              onChange={e => setForm(f => ({ ...f, env_vars: e.target.value }))}
              rows={4}
              placeholder='{ "API_KEY": "secret_value" }'
              className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 font-mono focus:outline-none focus:border-emerald-500/50"
            />
          </div>

          {error && (
            <div className="flex items-center gap-2 text-red-400 text-xs bg-red-900/20 border border-red-800/50 rounded-lg px-3 py-2">
              <AlertTriangle size={13} />
              {error}
            </div>
          )}
        </div>

        <div className="flex justify-end gap-3 px-5 py-4 border-t border-zinc-800/50">
          <button onClick={onClose} className="px-4 py-2 text-sm text-zinc-400 hover:text-white transition-colors">
            Annuler
          </button>
          <button
            onClick={handleSubmit}
            disabled={isSaving || !form.name.trim() || (mode === 'url' ? !form.target_url.trim() : !form.container_image.trim())}
            className="px-4 py-2 text-sm bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-white rounded-lg flex items-center gap-2 transition-colors active:scale-[0.98]"
          >
            {isSaving ? <RotateCcw size={14} className="animate-spin" /> : <Check size={14} />}
            {isEdit ? 'Enregistrer' : 'Créer'}
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
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
      <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-sm mx-4 p-6">
        <div className="flex items-center gap-3 mb-4">
          <div className="w-9 h-9 rounded-full bg-red-500/15 flex items-center justify-center">
            <AlertTriangle size={16} className="text-red-400" />
          </div>
          <div>
            <div className="font-semibold text-white">Supprimer le serveur MCP</div>
            <div className="text-xs text-zinc-500">Cette action est irréversible</div>
          </div>
        </div>
        <p className="text-sm text-zinc-400 mb-5">
          Voulez-vous vraiment supprimer <span className="text-white font-medium">{name}</span> ?
        </p>
        <div className="flex justify-end gap-3">
          <button onClick={onClose} className="px-4 py-2 text-sm text-zinc-400 hover:text-white transition-colors">
            Annuler
          </button>
          <button
            onClick={onConfirm}
            disabled={isDeleting}
            className="px-4 py-2 text-sm bg-red-600 hover:bg-red-500 disabled:opacity-50 text-white rounded-lg flex items-center gap-2 transition-colors"
          >
            {isDeleting ? <RotateCcw size={13} className="animate-spin" /> : <Trash2 size={13} />}
            Supprimer
          </button>
        </div>
      </div>
    </div>
  )
}

export default function McpServers() {
  const qc = useQueryClient()
  const [showCreate, setShowCreate] = useState(false)
  const [editing, setEditing] = useState<McpServer | null>(null)
  const [deleting, setDeleting] = useState<McpServer | null>(null)
  const [mutationError, setMutationError] = useState<string | null>(null)
  const [search, setSearch] = useState('')

  const { data, isLoading } = useQuery<McpServer[]>({
    queryKey: ['mcp-servers'],
    queryFn: mcpServersApi.getAll,
  })

  const invalidate = () => qc.invalidateQueries({ queryKey: ['mcp-servers'] })

  function parseForm(f: McpFormState) {
    let env_vars: Record<string, unknown> | null = null
    try {
      env_vars = JSON.parse(f.env_vars)
    } catch {
      env_vars = null
    }
    return {
      name: f.name,
      server_type: f.server_type,
      target_url: f.target_url || null,
      container_image: f.container_image || null,
      env_vars,
      virtual_key_id: f.virtual_key_id || null,
      team_id: f.team_id || null,
    }
  }

  const createMut = useMutation({
    mutationFn: (f: McpFormState) => mcpServersApi.create(parseForm(f)),
    onSuccess: () => { invalidate(); setShowCreate(false); setMutationError(null) },
    onError: (e: Error) => setMutationError(e.message),
  })

  const updateMut = useMutation({
    mutationFn: ({ id, f }: { id: string; f: McpFormState }) => mcpServersApi.update(id, parseForm(f)),
    onSuccess: () => { invalidate(); setEditing(null); setMutationError(null) },
    onError: (e: Error) => setMutationError(e.message),
  })

  const deleteMut = useMutation({
    mutationFn: (id: string) => mcpServersApi.remove(id),
    onSuccess: () => { invalidate(); setDeleting(null) },
  })

  const toggleStatusMut = useMutation({
    mutationFn: ({ id, active }: { id: string; active: boolean }) =>
      active ? mcpServersApi.deactivate(id) : mcpServersApi.activate(id),
    onSuccess: () => invalidate(),
  })

  const filtered = useMemo(() => {
    if (!data) return []
    const q = search.toLowerCase().trim()
    if (!q) return data
    return data.filter(s =>
      s.name.toLowerCase().includes(q) ||
      s.server_type.toLowerCase().includes(q) ||
      (s.target_url && s.target_url.toLowerCase().includes(q)) ||
      (s.container_image && s.container_image.toLowerCase().includes(q))
    )
  }, [data, search])

  const stats = useMemo(() => {
    if (!data) return { total: 0, active: 0 }
    return {
      total: data.length,
      active: data.filter(s => s.status === 'active').length,
    }
  }, [data])

  return (
    <div className="p-6 space-y-6 overflow-y-auto h-full">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white flex items-center gap-2">
            <Cpu className="text-emerald-400" size={24} /> Serviteurs MCP
          </h1>
          <p className="text-sm text-zinc-400 mt-1">
            {stats.total} configurés &bull; {stats.active} actifs
          </p>
        </div>
        <button
          onClick={() => { setMutationError(null); setShowCreate(true) }}
          className="flex items-center gap-2 px-4 py-2 bg-emerald-600 hover:bg-emerald-500 active:scale-[0.98] text-white text-sm rounded-lg transition-all shadow-lg shadow-emerald-900/10 font-medium"
        >
          <Plus size={15} /> Ajouter un serveur
        </button>
      </div>

      <div className="relative">
        <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-500" />
        <input
          type="text"
          value={search}
          onChange={e => setSearch(e.target.value)}
          placeholder="Rechercher par nom, type, URL, container..."
          className="w-full bg-zinc-950 border border-zinc-800 rounded-lg pl-9 pr-8 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
        />
        {search && (
          <button
            onClick={() => setSearch('')}
            className="absolute right-3 top-1/2 -translate-y-1/2 text-zinc-500 hover:text-zinc-300"
          >
            <X size={14} />
          </button>
        )}
      </div>

      <div className="rounded-xl border border-zinc-800/50 bg-zinc-900/30 overflow-hidden">
        <table className="w-full text-sm">
          <thead className="border-b border-zinc-800/50">
            <tr>
              {['Serveur', 'Type', 'Mode / Destination', 'Virtual Key', 'Team ID', 'Statut', ''].map(h => (
                <th key={h} className="text-left px-5 py-3.5 text-xs text-zinc-500 uppercase tracking-wide font-medium">
                  {h}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {isLoading ? (
              Array.from({ length: 3 }).map((_, i) => (
                <tr key={i} className="border-b border-zinc-800/30">
                  {Array.from({ length: 7 }).map((_, j) => (
                    <td key={j} className="px-5 py-3.5">
                      <div className="h-3 bg-zinc-800 rounded animate-pulse w-24" />
                    </td>
                  ))}
                </tr>
              ))
            ) : filtered.map(server => (
              <tr key={server.id} className="border-b border-zinc-800/30 transition-colors group hover:bg-zinc-800/30">
                <td className="px-5 py-3.5">
                  <div className="flex items-center gap-2">
                    <Cpu size={14} className="text-emerald-400 shrink-0" />
                    <div>
                      <div className="font-semibold text-white text-xs">{server.name}</div>
                      <div className="text-[10px] text-zinc-500 font-mono truncate max-w-[140px]" title={server.id}>
                        {server.id}
                      </div>
                    </div>
                  </div>
                </td>
                <td className="px-5 py-3.5">
                  <span className="capitalize text-xs px-2.5 py-0.5 rounded-full bg-zinc-800/80 text-zinc-300 border border-zinc-700/30">
                    {server.server_type}
                  </span>
                </td>
                <td className="px-5 py-3.5 font-mono text-xs text-zinc-400">
                  {server.target_url ? (
                    <span className="flex items-center gap-1 hover:text-emerald-400 transition-colors">
                      <ExternalLink size={12} /> {server.target_url}
                    </span>
                  ) : server.container_image ? (
                    <span className="flex items-center gap-1 hover:text-emerald-400 transition-colors">
                      <Layers size={12} /> {server.container_image}
                    </span>
                  ) : (
                    <span className="text-zinc-600">—</span>
                  )}
                </td>
                <td className="px-5 py-3.5 font-mono text-xs text-zinc-400">
                  {server.virtual_key_id ? (
                    <span className="flex items-center gap-1">
                      <KeyRound size={12} className="text-zinc-500" /> {server.virtual_key_id}
                    </span>
                  ) : (
                    <span className="text-zinc-600">—</span>
                  )}
                </td>
                <td className="px-5 py-3.5 font-mono text-xs text-zinc-400">
                  {server.team_id ? (
                    <span className="flex items-center gap-1">
                      <Code size={12} className="text-zinc-500" /> {server.team_id}
                    </span>
                  ) : (
                    <span className="text-zinc-600">—</span>
                  )}
                </td>
                <td className="px-5 py-3.5">
                  <div className="flex items-center gap-2">
                    <button
                      onClick={() => toggleStatusMut.mutate({ id: server.id, active: server.status === 'active' })}
                      disabled={toggleStatusMut.isPending}
                      className={`relative w-9 h-5 rounded-full transition-colors flex items-center ${
                        server.status === 'active' ? 'bg-emerald-600' : 'bg-zinc-700'
                      }`}
                    >
                      <span
                        className={`absolute w-3.5 h-3.5 rounded-full bg-white shadow transition-all ${
                          server.status === 'active' ? 'left-5' : 'left-0.5'
                        }`}
                      />
                    </button>
                    <span className="text-xs text-zinc-400 font-medium">
                      {server.status === 'active' ? 'Actif' : server.status === 'inactive' ? 'Inactif' : 'Erreur'}
                    </span>
                  </div>
                </td>
                <td className="px-5 py-3.5">
                  <div className="flex items-center gap-1.5 opacity-0 group-hover:opacity-100 transition-all">
                    <button
                      onClick={() => { setMutationError(null); setEditing(server) }}
                      className="p-1.5 text-zinc-500 hover:text-emerald-400 hover:bg-emerald-400/10 rounded-lg"
                      title="Modifier"
                    >
                      <Pencil size={13} />
                    </button>
                    <button
                      onClick={() => setDeleting(server)}
                      className="p-1.5 text-zinc-500 hover:text-red-400 hover:bg-red-400/10 rounded-lg"
                      title="Supprimer"
                    >
                      <Trash2 size={13} />
                    </button>
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
        {!isLoading && !filtered.length && (
          <div className="text-center py-16 text-zinc-600 flex flex-col items-center justify-center gap-2">
            <Cpu size={36} className="text-zinc-700" />
            <span>Aucun serveur MCP trouvé</span>
          </div>
        )}
      </div>

      {showCreate && (
        <McpModal
          initial={DEFAULT_FORM}
          isEdit={false}
          onClose={() => { setShowCreate(false); setMutationError(null) }}
          onSave={f => createMut.mutate(f)}
          isSaving={createMut.isPending}
          error={mutationError}
        />
      )}

      {editing && (
        <McpModal
          initial={{
            name: editing.name,
            server_type: editing.server_type,
            target_url: editing.target_url || '',
            container_image: editing.container_image || '',
            env_vars: editing.env_vars ? JSON.stringify(editing.env_vars, null, 2) : '{}',
            virtual_key_id: editing.virtual_key_id || '',
            team_id: editing.team_id || '',
          }}
          isEdit={true}
          onClose={() => { setEditing(null); setMutationError(null) }}
          onSave={f => updateMut.mutate({ id: editing.id, f })}
          isSaving={updateMut.isPending}
          error={mutationError}
        />
      )}

      {deleting && (
        <DeleteConfirmModal
          name={deleting.name}
          onClose={() => setDeleting(null)}
          onConfirm={() => deleteMut.mutate(deleting.id)}
          isDeleting={deleteMut.isPending}
        />
      )}
    </div>
  )
}
