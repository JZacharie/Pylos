import { useState } from 'react'
import {
  Bot, Plus, Pencil, Trash2, X, Check, AlertTriangle, Wrench, RotateCcw, CheckCircle, XCircle,
} from 'lucide-react'


// ─── Types ────────────────────────────────────────────────────────────────────

interface Agent {
  id: string
  name: string
  description: string
  model: string
  system_prompt: string
  tools: string[]
  mcp_server_id: string | null
  status: 'active' | 'inactive'
  temperature: number
  max_tokens: number
}

type AgentFormState = Omit<Agent, 'id'>

const MODELS = [
  'gpt-4o',
  'gpt-4o-mini',
  'gpt-4-turbo',
  'claude-sonnet-4-20250514',
  'claude-3-5-haiku-20241022',
  'claude-3-opus-20240229',
  'gemini-1.5-pro',
  'gemini-1.5-flash',
  'llama-3.1-70b',
  'llama-3.1-8b',
  'mistral-large',
  'deepseek-chat',
]

const AVAILABLE_TOOLS = [
  'web_search',
  'file_read',
  'file_write',
  'code_execute',
  'image_generate',
  'database_query',
  'api_call',
  'email_send',
  'calendar_manage',
  'slack_post',
  'github_ops',
  'notion_read',
]

const DEFAULT_FORM: AgentFormState = {
  name: '',
  description: '',
  model: 'gpt-4o',
  system_prompt: '',
  tools: [],
  mcp_server_id: null,
  status: 'active',
  temperature: 0.7,
  max_tokens: 4096,
}

let nextId = 1
function genId() {
  return `agent_${Date.now()}_${nextId++}`
}

// ─── AgentModal ───────────────────────────────────────────────────────────────

function AgentModal({
  initial,
  isEdit,
  onClose,
  onSave,
}: {
  initial: AgentFormState
  isEdit: boolean
  onClose: () => void
  onSave: (form: AgentFormState) => void
}) {
  const [form, setForm] = useState<AgentFormState>(initial)
  const [saving, setSaving] = useState(false)

  const setField = <K extends keyof AgentFormState>(k: K, v: AgentFormState[K]) =>
    setForm(f => ({ ...f, [k]: v }))

  const toggleTool = (tool: string) =>
    setForm(f => ({
      ...f,
      tools: f.tools.includes(tool) ? f.tools.filter(t => t !== tool) : [...f.tools, tool],
    }))

  const handleSave = () => {
    setSaving(true)
    setTimeout(() => {
      onSave(form)
      setSaving(false)
    }, 300)
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-2xl mx-4 max-h-[90vh] overflow-y-auto">
        {/* Header */}
        <div className="flex items-center justify-between p-5 border-b border-zinc-800/50">
          <h2 className="text-lg font-semibold text-white">
            {isEdit ? 'Edit agent' : 'Create agent'}
          </h2>
          <button onClick={onClose} className="text-zinc-500 hover:text-white transition-colors">
            <X size={18} />
          </button>
        </div>

        <div className="p-5 space-y-5">
          {/* Name & Model */}
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-xs text-zinc-400 mb-1.5">Name *</label>
              <input
                type="text"
                value={form.name}
                onChange={e => setField('name', e.target.value)}
                placeholder="e.g. Code Assistant"
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                  focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
              />
            </div>
            <div>
              <label className="block text-xs text-zinc-400 mb-1.5">Model *</label>
              <select
                value={form.model}
                onChange={e => setField('model', e.target.value)}
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                  focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
              >
                {MODELS.map(m => (
                  <option key={m} value={m}>{m}</option>
                ))}
              </select>
            </div>
          </div>

          {/* Description */}
          <div>
            <label className="block text-xs text-zinc-400 mb-1.5">Description</label>
            <input
              type="text"
              value={form.description}
              onChange={e => setField('description', e.target.value)}
              placeholder="What does this agent do?"
              className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
            />
          </div>

          {/* System Prompt */}
          <div>
            <label className="block text-xs text-zinc-400 mb-1.5">System Prompt</label>
            <textarea
              rows={4}
              value={form.system_prompt}
              onChange={e => setField('system_prompt', e.target.value)}
              placeholder="You are a helpful assistant that..."
              className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20 resize-y font-mono"
            />
          </div>

          {/* Tools */}
          <div>
            <label className="block text-xs text-zinc-400 mb-1.5">Assigned Tools</label>
            <div className="flex flex-wrap gap-1.5 p-2.5 bg-zinc-950 border border-zinc-800 rounded-lg min-h-[42px]">
              {AVAILABLE_TOOLS.map(tool => {
                const sel = form.tools.includes(tool)
                return (
                  <button
                    key={tool}
                    type="button"
                    onClick={() => toggleTool(tool)}
                    className={`px-2.5 py-1 rounded-md text-xs font-medium border transition-all ${
                      sel
                        ? 'bg-emerald-500/10 text-emerald-300 border-emerald-500/30'
                        : 'bg-zinc-900/40 text-zinc-400 border-zinc-800 hover:text-zinc-200'
                    }`}
                  >
                    {tool}
                  </button>
                )
              })}
            </div>
            {form.tools.length > 0 && (
              <div className="flex flex-wrap gap-1 mt-2">
                {form.tools.map(t => (
                  <span key={t} className="flex items-center gap-1 text-xs bg-zinc-800 text-zinc-200 px-2 py-0.5 rounded-full">
                    {t}
                    <button type="button" onClick={() => toggleTool(t)} className="text-zinc-500 hover:text-white">
                      <X size={11} />
                    </button>
                  </span>
                ))}
              </div>
            )}
          </div>

          {/* MCP Server */}
          <div>
            <label className="block text-xs text-zinc-400 mb-1.5">MCP Server (optional)</label>
            <input
              type="text"
              value={form.mcp_server_id ?? ''}
              onChange={e => setField('mcp_server_id', e.target.value || null)}
              placeholder="e.g. mcp-server-prod"
              className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 font-mono
                focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
            />
          </div>

          {/* Temperature & Max Tokens */}
          <div className="grid grid-cols-2 gap-3">
            <div>
              <label className="block text-xs text-zinc-400 mb-1.5">
                Temperature <span className="text-zinc-600">({form.temperature})</span>
              </label>
              <input
                type="range"
                min={0}
                max={2}
                step={0.1}
                value={form.temperature}
                onChange={e => setField('temperature', Number(e.target.value))}
                className="w-full accent-emerald-500"
              />
              <div className="flex justify-between text-[10px] text-zinc-600 mt-0.5">
                <span>Precise</span>
                <span>Creative</span>
              </div>
            </div>
            <div>
              <label className="block text-xs text-zinc-400 mb-1.5">Max Tokens</label>
              <input
                type="number"
                value={form.max_tokens}
                onChange={e => setField('max_tokens', Number(e.target.value))}
                min={256}
                max={128000}
                step={256}
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                  focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
              />
            </div>
          </div>

          {/* Status Toggle */}
          <div className="flex items-center gap-3">
            <button
              onClick={() => setField('status', form.status === 'active' ? 'inactive' : 'active')}
              className={`relative w-10 h-5 rounded-full transition-colors ${
                form.status === 'active' ? 'bg-emerald-600' : 'bg-zinc-700'
              }`}
            >
              <span
                className={`absolute top-0.5 w-4 h-4 rounded-full bg-white shadow transition-all ${
                  form.status === 'active' ? 'left-5' : 'left-0.5'
                }`}
              />
            </button>
            <span className="text-sm text-zinc-300">
              {form.status === 'active' ? 'Active' : 'Inactive'}
            </span>
          </div>
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
            onClick={handleSave}
            disabled={saving || !form.name.trim()}
            className="px-4 py-2 text-sm bg-emerald-600 hover:bg-emerald-500 active:scale-[0.98]
              disabled:opacity-50 disabled:cursor-not-allowed text-white rounded-lg flex items-center gap-2 transition-colors"
          >
            {saving ? (
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
}: {
  name: string
  onClose: () => void
  onConfirm: () => void
}) {
  const [deleting, setDeleting] = useState(false)

  const handleDelete = () => {
    setDeleting(true)
    setTimeout(() => {
      onConfirm()
      setDeleting(false)
    }, 300)
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-sm mx-4 p-6">
        <div className="flex items-center gap-3 mb-4">
          <div className="w-9 h-9 rounded-full bg-red-500/15 flex items-center justify-center">
            <AlertTriangle size={16} className="text-red-400" />
          </div>
          <div>
            <div className="font-semibold text-white">Delete agent</div>
            <div className="text-xs text-zinc-500">This action cannot be undone</div>
          </div>
        </div>
        <p className="text-sm text-zinc-400 mb-5">
          Delete <span className="text-white font-medium">{name}</span>?
          Any active sessions using this agent will be terminated.
        </p>
        <div className="flex justify-end gap-3">
          <button onClick={onClose} className="px-4 py-2 text-sm text-zinc-400 hover:text-white">
            Cancel
          </button>
          <button
            onClick={handleDelete}
            disabled={deleting}
            className="px-4 py-2 text-sm bg-red-600 hover:bg-red-500 disabled:opacity-50 text-white rounded-lg
              flex items-center gap-2 transition-colors active:scale-[0.98]"
          >
            {deleting ? <RotateCcw size={13} className="animate-spin" /> : <Trash2 size={13} />}
            Delete
          </button>
        </div>
      </div>
    </div>
  )
}

// ─── AgentCard ────────────────────────────────────────────────────────────────

function AgentCard({
  agent,
  onEdit,
  onDelete,
  onToggleStatus,
}: {
  agent: Agent
  onEdit: () => void
  onDelete: () => void
  onToggleStatus: () => void
}) {
  return (
    <div className="rounded-xl border border-zinc-800/50 bg-zinc-900/30 p-5 hover:border-zinc-700/50
      transition-colors group">
      {/* Header */}
      <div className="flex items-center gap-3 mb-3">
        <div className="w-9 h-9 rounded-lg bg-emerald-500/10 flex items-center justify-center">
          <Bot size={16} className="text-emerald-400" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="font-semibold text-white truncate">{agent.name}</div>
          <div className="text-xs text-zinc-500 font-mono truncate">{agent.model}</div>
        </div>
        <div className="flex items-center gap-1.5">
          <button
            onClick={e => { e.stopPropagation(); onToggleStatus() }}
            className={`relative w-9 h-[18px] rounded-full transition-colors ${
              agent.status === 'active' ? 'bg-emerald-600' : 'bg-zinc-700'
            }`}
            title={agent.status === 'active' ? 'Deactivate' : 'Activate'}
          >
            <span
              className={`absolute top-[2px] w-3.5 h-3.5 rounded-full bg-white shadow transition-all ${
                agent.status === 'active' ? 'left-[18px]' : 'left-[2px]'
              }`}
            />
          </button>
        </div>
      </div>

      {/* Description */}
      {agent.description && (
        <p className="text-xs text-zinc-400 mb-3 line-clamp-2">{agent.description}</p>
      )}

      {/* Meta */}
      <div className="flex items-center gap-3 text-xs text-zinc-500 mb-3">
        <span className={`flex items-center gap-1.5 ${
          agent.status === 'active' ? 'text-emerald-400' : 'text-zinc-500'
        }`}>
          {agent.status === 'active' ? <CheckCircle size={12} /> : <XCircle size={12} />}
          {agent.status === 'active' ? 'Active' : 'Inactive'}
        </span>
        <span className="text-zinc-700">|</span>
        <span>Temp: {agent.temperature}</span>
        <span className="text-zinc-700">|</span>
        <span>{agent.max_tokens.toLocaleString()} tokens</span>
      </div>

      {/* Tools */}
      {agent.tools.length > 0 && (
        <div className="flex items-center gap-2 pt-3 border-t border-zinc-800/50">
          <Wrench size={11} className="text-zinc-600 shrink-0" />
          <div className="flex flex-wrap gap-1">
            {agent.tools.slice(0, 4).map(t => (
              <span key={t} className="text-[10px] bg-zinc-800/60 text-zinc-400 px-1.5 py-0.5 rounded">
                {t}
              </span>
            ))}
            {agent.tools.length > 4 && (
              <span className="text-[10px] text-zinc-600">
                +{agent.tools.length - 4} more
              </span>
            )}
          </div>
        </div>
      )}

      {/* Actions */}
      <div className="flex items-center gap-1.5 mt-3 pt-3 border-t border-zinc-800/50 opacity-0 group-hover:opacity-100 transition-all">
        <button
          onClick={onEdit}
          className="flex-1 flex items-center justify-center gap-1.5 py-1.5 text-xs text-zinc-400 hover:text-emerald-400
            hover:bg-emerald-400/10 rounded-lg transition-all"
        >
          <Pencil size={12} />
          Edit
        </button>
        <button
          onClick={onDelete}
          className="flex-1 flex items-center justify-center gap-1.5 py-1.5 text-xs text-zinc-400 hover:text-red-400
            hover:bg-red-400/10 rounded-lg transition-all"
        >
          <Trash2 size={12} />
          Delete
        </button>
      </div>
    </div>
  )
}

// ─── Main page ────────────────────────────────────────────────────────────────

const SEED_AGENTS: Agent[] = [
  {
    id: 'agent_seed_1',
    name: 'Code Assistant',
    description: 'Helps with code generation, debugging, and refactoring across multiple languages.',
    model: 'gpt-4o',
    system_prompt: 'You are an expert software engineer. Help users write, debug, and improve code.',
    tools: ['code_execute', 'file_read', 'file_write', 'github_ops'],
    mcp_server_id: null,
    status: 'active',
    temperature: 0.3,
    max_tokens: 8192,
  },
  {
    id: 'agent_seed_2',
    name: 'Research Analyst',
    description: 'Conducts web research, summarizes findings, and compiles reports.',
    model: 'claude-sonnet-4-20250514',
    system_prompt: 'You are a diligent research analyst. Search the web, gather data, and produce well-cited summaries.',
    tools: ['web_search', 'file_write', 'email_send'],
    mcp_server_id: null,
    status: 'active',
    temperature: 0.5,
    max_tokens: 4096,
  },
  {
    id: 'agent_seed_3',
    name: 'Data Explorer',
    description: 'Queries databases, analyzes datasets, and generates visualizations.',
    model: 'gpt-4o-mini',
    system_prompt: 'You are a data analyst. Help users explore, query, and visualize their data.',
    tools: ['database_query', 'code_execute', 'file_write'],
    mcp_server_id: 'mcp-db-prod',
    status: 'inactive',
    temperature: 0.2,
    max_tokens: 4096,
  },
]

export default function Agents() {
  const [agents, setAgents] = useState<Agent[]>(SEED_AGENTS)
  const [showCreate, setShowCreate] = useState(false)
  const [editingAgent, setEditingAgent] = useState<Agent | null>(null)
  const [deletingAgent, setDeletingAgent] = useState<Agent | null>(null)
  const [searchQuery, setSearchQuery] = useState('')
  const [filterStatus, setFilterStatus] = useState<'all' | 'active' | 'inactive'>('all')

  const filtered = agents.filter(a => {
    const matchSearch = !searchQuery ||
      a.name.toLowerCase().includes(searchQuery.toLowerCase()) ||
      a.description.toLowerCase().includes(searchQuery.toLowerCase()) ||
      a.model.toLowerCase().includes(searchQuery.toLowerCase())
    const matchStatus = filterStatus === 'all' || a.status === filterStatus
    return matchSearch && matchStatus
  })

  const handleCreate = (form: AgentFormState) => {
    const newAgent: Agent = { ...form, id: genId() }
    setAgents(prev => [...prev, newAgent])
    setShowCreate(false)
  }

  const handleUpdate = (form: AgentFormState) => {
    if (!editingAgent) return
    setAgents(prev => prev.map(a => a.id === editingAgent.id ? { ...a, ...form } : a))
    setEditingAgent(null)
  }

  const handleDelete = () => {
    if (!deletingAgent) return
    setAgents(prev => prev.filter(a => a.id !== deletingAgent.id))
    setDeletingAgent(null)
  }

  const handleToggleStatus = (agent: Agent) => {
    setAgents(prev =>
      prev.map(a =>
        a.id === agent.id
          ? { ...a, status: a.status === 'active' ? 'inactive' : 'active' }
          : a
      )
    )
  }

  return (
    <div className="p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white">Agents</h1>
          <p className="text-sm text-zinc-400 mt-1">
            {agents.length} agent{agents.length !== 1 ? 's' : ''} configured
          </p>
        </div>
        <button
          onClick={() => setShowCreate(true)}
          className="flex items-center gap-2 px-4 py-2 bg-emerald-600 hover:bg-emerald-500 active:scale-[0.98]
            text-white text-sm rounded-lg transition-colors"
        >
          <Plus size={15} />
          Create agent
        </button>
      </div>

      {/* Filters */}
      <div className="flex items-center gap-3">
        <div className="flex-1 max-w-sm">
          <input
            type="text"
            value={searchQuery}
            onChange={e => setSearchQuery(e.target.value)}
            placeholder="Search agents..."
            className="w-full bg-zinc-900/30 border border-zinc-800/50 rounded-lg px-3 py-2 text-sm text-zinc-200
              placeholder-zinc-600 focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
          />
        </div>
        <div className="flex bg-zinc-900/30 border border-zinc-800/50 rounded-lg p-0.5">
          {(['all', 'active', 'inactive'] as const).map(s => (
            <button
              key={s}
              onClick={() => setFilterStatus(s)}
              className={`px-3 py-1.5 rounded-md text-xs font-medium transition-all ${
                filterStatus === s
                  ? 'bg-zinc-800 text-white shadow'
                  : 'text-zinc-500 hover:text-zinc-300'
              }`}
            >
              {s.charAt(0).toUpperCase() + s.slice(1)}
            </button>
          ))}
        </div>
      </div>

      {/* Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
        {filtered.map(agent => (
          <AgentCard
            key={agent.id}
            agent={agent}
            onEdit={() => setEditingAgent(agent)}
            onDelete={() => setDeletingAgent(agent)}
            onToggleStatus={() => handleToggleStatus(agent)}
          />
        ))}
      </div>

      {!filtered.length && (
        <div className="text-center py-16 text-zinc-600">
          {agents.length === 0
            ? 'No agents yet — create one to get started'
            : 'No agents match your filters'}
        </div>
      )}

      {/* Create modal */}
      {showCreate && (
        <AgentModal
          initial={DEFAULT_FORM}
          isEdit={false}
          onClose={() => setShowCreate(false)}
          onSave={handleCreate}
        />
      )}

      {/* Edit modal */}
      {editingAgent && (
        <AgentModal
          initial={{
            name: editingAgent.name,
            description: editingAgent.description,
            model: editingAgent.model,
            system_prompt: editingAgent.system_prompt,
            tools: editingAgent.tools,
            mcp_server_id: editingAgent.mcp_server_id,
            status: editingAgent.status,
            temperature: editingAgent.temperature,
            max_tokens: editingAgent.max_tokens,
          }}
          isEdit={true}
          onClose={() => setEditingAgent(null)}
          onSave={handleUpdate}
        />
      )}

      {/* Delete confirm */}
      {deletingAgent && (
        <DeleteConfirmModal
          name={deletingAgent.name}
          onClose={() => setDeletingAgent(null)}
          onConfirm={handleDelete}
        />
      )}
    </div>
  )
}
