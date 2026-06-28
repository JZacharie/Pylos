import { useState } from 'react'
import {
  FileText, Plus, Pencil, History, Tag, ToggleLeft, ToggleRight,
  Copy, X, Check, Search, ChevronDown, ChevronRight, Beaker,
  Clock, BarChart3,
} from 'lucide-react'

interface PromptVersion {
  version: number
  content: string
  updatedAt: string
  updatedBy: string
}

interface Prompt {
  id: string
  name: string
  description: string
  content: string
  version: number
  tags: string[]
  isActive: boolean
  usageCount: number
  lastUpdated: string
  createdAt: string
  versions: PromptVersion[]
}

const SEED_PROMPTS: Prompt[] = [
  {
    id: 'prompt_001',
    name: 'Default System Prompt',
    description: 'General-purpose system prompt used across all standard interactions.',
    content: 'You are a helpful, harmless, and honest AI assistant. Provide clear, accurate, and concise responses. When you are uncertain about something, acknowledge the limitation rather than guessing. Follow ethical guidelines and respect user privacy.',
    version: 3,
    tags: ['general', 'default', 'safety'],
    isActive: true,
    usageCount: 14823,
    lastUpdated: '2026-06-25T14:30:00Z',
    createdAt: '2026-03-10T09:00:00Z',
    versions: [
      { version: 3, content: 'You are a helpful, harmless, and honest AI assistant. Provide clear, accurate, and concise responses. When you are uncertain about something, acknowledge the limitation rather than guessing. Follow ethical guidelines and respect user privacy.', updatedAt: '2026-06-25T14:30:00Z', updatedBy: 'admin@pylos.io' },
      { version: 2, content: 'You are a helpful AI assistant. Provide clear and accurate responses. When uncertain, acknowledge limitations.', updatedAt: '2026-05-12T10:00:00Z', updatedBy: 'admin@pylos.io' },
      { version: 1, content: 'You are a helpful AI assistant.', updatedAt: '2026-03-10T09:00:00Z', updatedBy: 'admin@pylos.io' },
    ],
  },
  {
    id: 'prompt_002',
    name: 'Code Assistant',
    description: 'Optimized prompt for code generation, debugging, and technical explanations.',
    content: 'You are a senior software engineer and coding assistant. Write clean, well-documented code following best practices. Always include error handling, explain your approach, and suggest improvements. When reviewing code, identify bugs, performance issues, and security vulnerabilities. Prefer readability over cleverness.',
    version: 2,
    tags: ['code', 'technical', 'engineering'],
    isActive: true,
    usageCount: 9451,
    lastUpdated: '2026-06-20T11:15:00Z',
    createdAt: '2026-04-05T08:00:00Z',
    versions: [
      { version: 2, content: 'You are a senior software engineer and coding assistant. Write clean, well-documented code following best practices. Always include error handling, explain your approach, and suggest improvements. When reviewing code, identify bugs, performance issues, and security vulnerabilities. Prefer readability over cleverness.', updatedAt: '2026-06-20T11:15:00Z', updatedBy: 'dev@pylos.io' },
      { version: 1, content: 'You are a coding assistant. Help users write and debug code. Explain your reasoning.', updatedAt: '2026-04-05T08:00:00Z', updatedBy: 'dev@pylos.io' },
    ],
  },
  {
    id: 'prompt_003',
    name: 'Data Analyst',
    description: 'Specialized prompt for data analysis, visualization recommendations, and statistical insights.',
    content: 'You are a data analyst expert. When presented with data, identify key trends, outliers, and patterns. Recommend appropriate visualizations and statistical methods. Explain findings in plain language with actionable insights. Always note data quality issues and suggest validation steps.',
    version: 1,
    tags: ['data', 'analytics', 'statistics'],
    isActive: true,
    usageCount: 3207,
    lastUpdated: '2026-06-18T09:45:00Z',
    createdAt: '2026-06-18T09:45:00Z',
    versions: [
      { version: 1, content: 'You are a data analyst expert. When presented with data, identify key trends, outliers, and patterns. Recommend appropriate visualizations and statistical methods. Explain findings in plain language with actionable insights. Always note data quality issues and suggest validation steps.', updatedAt: '2026-06-18T09:45:00Z', updatedBy: 'analyst@pylos.io' },
    ],
  },
  {
    id: 'prompt_004',
    name: 'Customer Support',
    description: 'Friendly, empathetic prompt for handling customer inquiries and support tickets.',
    content: 'You are a customer support specialist. Be warm, empathetic, and solution-oriented. Acknowledge the customer\'s concern before offering solutions. Use clear, non-technical language. If you cannot resolve an issue, explain next steps and escalation paths. Always aim to leave the customer feeling heard and valued.',
    version: 1,
    tags: ['support', 'customer-service', 'empathy'],
    isActive: false,
    usageCount: 5812,
    lastUpdated: '2026-06-10T16:20:00Z',
    createdAt: '2026-06-10T16:20:00Z',
    versions: [
      { version: 1, content: 'You are a customer support specialist. Be warm, empathetic, and solution-oriented. Acknowledge the customer\'s concern before offering solutions. Use clear, non-technical language. If you cannot resolve an issue, explain next steps and escalation paths. Always aim to leave the customer feeling heard and valued.', updatedAt: '2026-06-10T16:20:00Z', updatedBy: 'support@pylos.io' },
    ],
  },
]

const AB_TEST_INFO = [
  { id: 'test_1', promptA: 'Default System Prompt v3', promptB: 'Default System Prompt v2', trafficA: 70, trafficB: 30, status: 'running', conversionsA: 842, conversionsB: 301 },
  { id: 'test_2', promptA: 'Code Assistant v2', promptB: 'Code Assistant v1', trafficA: 100, trafficB: 0, status: 'completed', conversionsA: 1205, conversionsB: 890 },
]

function tsAgo(dateStr: string): string {
  const diff = Date.now() - new Date(dateStr).getTime()
  const mins = Math.floor(diff / 60000)
  if (mins < 1) return 'Just now'
  if (mins < 60) return `${mins}m ago`
  const hrs = Math.floor(mins / 60)
  if (hrs < 24) return `${hrs}h ago`
  const days = Math.floor(hrs / 24)
  return `${days}d ago`
}

function tsDisplay(dateStr: string): string {
  return new Date(dateStr).toLocaleDateString('en-US', { month: 'short', day: 'numeric', year: 'numeric' })
}

function PromptFormModal({ initial, isEdit, onClose, onSave }: {
  initial: Partial<Prompt>; isEdit: boolean; onClose: () => void; onSave: (prompt: Partial<Prompt>) => void
}) {
  const [name, setName] = useState(initial.name ?? '')
  const [description, setDescription] = useState(initial.description ?? '')
  const [content, setContent] = useState(initial.content ?? '')
  const [tags, setTags] = useState<string[]>(initial.tags ?? [])
  const [isActive, setIsActive] = useState(initial.isActive ?? true)
  const [tagInput, setTagInput] = useState('')

  const nextVersion = isEdit && initial.version ? initial.version + 1 : 1

  const handleAddTag = () => {
    const t = tagInput.trim().toLowerCase()
    if (t && !tags.includes(t)) {
      setTags([...tags, t])
      setTagInput('')
    }
  }

  const handleRemoveTag = (t: string) => setTags(tags.filter(x => x !== t))

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-2xl mx-4 max-h-[90vh] overflow-y-auto">
        <div className="flex items-center justify-between p-5 border-b border-zinc-800/50">
          <h2 className="text-lg font-semibold text-white">{isEdit ? 'Edit prompt' : 'Create prompt'}</h2>
          <button onClick={onClose} className="text-zinc-500 hover:text-white transition-colors"><X size={18} /></button>
        </div>
        <div className="p-5 space-y-5">
          <div className="grid grid-cols-2 gap-4">
            <div>
              <label className="block text-xs text-zinc-400 mb-1.5">Name *</label>
              <input type="text" value={name} onChange={e => setName(e.target.value)} placeholder="My Prompt"
                className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
              />
            </div>
            <div>
              <label className="block text-xs text-zinc-400 mb-1.5">Version</label>
              <div className="w-full bg-zinc-950/60 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-400 font-mono">
                v{nextVersion} {isEdit ? <span className="text-emerald-400 text-xs ml-1">(auto-increment)</span> : <span className="text-xs ml-1">(new)</span>}
              </div>
            </div>
          </div>
          <div>
            <label className="block text-xs text-zinc-400 mb-1.5">Description</label>
            <input type="text" value={description} onChange={e => setDescription(e.target.value)} placeholder="What does this prompt do?"
              className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
            />
          </div>
          <div>
            <label className="block text-xs text-zinc-400 mb-1.5">Prompt Content *</label>
            <textarea value={content} onChange={e => setContent(e.target.value)} placeholder="Enter the system prompt..."
              className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-3 text-sm text-zinc-200 font-mono min-h-[160px] resize-y focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
            />
            <p className="text-[11px] text-zinc-600 mt-1.5">{content.length} characters</p>
          </div>
          <div>
            <label className="block text-xs text-zinc-400 mb-1.5">Tags</label>
            <div className="flex flex-wrap gap-1.5 p-2 bg-zinc-950 border border-zinc-800 rounded-lg min-h-[42px]">
              {tags.map(t => (
                <span key={t} className="flex items-center gap-1 px-2 py-0.5 bg-blue-500/10 text-blue-300 border border-blue-500/20 rounded text-xs font-medium">
                  <Tag size={10} />{t}
                  <button onClick={() => handleRemoveTag(t)} className="hover:text-white ml-0.5"><X size={10} /></button>
                </span>
              ))}
              <div className="flex items-center gap-1 flex-1 min-w-[120px]">
                <input type="text" value={tagInput} onChange={e => setTagInput(e.target.value)}
                  onKeyDown={e => { if (e.key === 'Enter') { e.preventDefault(); handleAddTag() } }}
                  placeholder="Add tag..."
                  className="flex-1 bg-transparent border-none text-xs text-zinc-300 focus:outline-none placeholder-zinc-600 px-1"
                />
                {tagInput && (
                  <button onClick={handleAddTag} className="text-emerald-400 hover:text-emerald-300"><Check size={12} /></button>
                )}
              </div>
            </div>
          </div>
          <div className="flex items-center gap-3">
            <button onClick={() => setIsActive(!isActive)}
              className={`relative w-10 h-5 rounded-full transition-colors ${isActive ? 'bg-emerald-600' : 'bg-zinc-700'}`}
            >
              <span className={`absolute top-0.5 w-4 h-4 rounded-full bg-white shadow transition-all ${isActive ? 'left-5' : 'left-0.5'}`} />
            </button>
            <span className="text-sm text-zinc-300">Active</span>
          </div>
        </div>
        <div className="flex justify-end gap-3 px-5 py-4 border-t border-zinc-800/50">
          <button onClick={onClose} className="px-4 py-2 text-sm text-zinc-400 hover:text-white transition-colors">Cancel</button>
          <button onClick={() => onSave({ name, description, content, tags, isActive, version: nextVersion })}
            disabled={!name.trim() || !content.trim()}
            className="px-4 py-2 text-sm bg-emerald-600 hover:bg-emerald-500 active:scale-[0.98] disabled:opacity-50 disabled:cursor-not-allowed text-white rounded-lg flex items-center gap-2 transition-colors"
          >
            <Check size={14} />
            {isEdit ? 'Save changes' : 'Create prompt'}
          </button>
        </div>
      </div>
    </div>
  )
}

function VersionHistoryPanel({ prompt, onClose }: { prompt: Prompt; onClose: () => void }) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-lg mx-4 max-h-[80vh] overflow-hidden flex flex-col">
        <div className="flex items-center justify-between p-5 border-b border-zinc-800/50 shrink-0">
          <div className="flex items-center gap-3">
            <History size={16} className="text-blue-400" />
            <div>
              <h2 className="text-lg font-semibold text-white">Version History</h2>
              <p className="text-xs text-zinc-500">{prompt.name}</p>
            </div>
          </div>
          <button onClick={onClose} className="text-zinc-500 hover:text-white transition-colors"><X size={18} /></button>
        </div>
        <div className="overflow-y-auto p-5 space-y-3 flex-1">
          {[...prompt.versions].reverse().map((v, i) => (
            <div key={v.version} className={`border rounded-xl p-4 transition-all ${
              i === 0 ? 'bg-emerald-500/5 border-emerald-500/20' : 'bg-zinc-900/40 border-zinc-800'
            }`}>
              <div className="flex items-center justify-between mb-2">
                <div className="flex items-center gap-2">
                  <span className="text-sm font-semibold text-white">v{v.version}</span>
                  {i === 0 && <span className="text-[10px] bg-emerald-500/10 text-emerald-400 px-1.5 py-0.5 rounded font-medium">Current</span>}
                </div>
                <span className="text-[11px] text-zinc-500">{tsDisplay(v.updatedAt)}</span>
              </div>
              <p className="text-xs text-zinc-400 font-mono bg-zinc-950/60 rounded-lg p-3 mt-2 leading-relaxed">{v.content}</p>
              <div className="flex items-center gap-2 mt-3 text-[11px] text-zinc-500">
                <Clock size={10} />
                {tsAgo(v.updatedAt)} by {v.updatedBy}
              </div>
            </div>
          ))}
        </div>
      </div>
    </div>
  )
}

function ABTestPanel({ tests }: { tests: typeof AB_TEST_INFO }) {
  return (
    <div className="space-y-3">
      {tests.map(test => (
        <div key={test.id} className="bg-zinc-900/40 border border-zinc-800 rounded-xl p-4">
          <div className="flex items-center justify-between mb-3">
            <span className={`text-[10px] font-bold uppercase tracking-wider px-2 py-0.5 rounded ${
              test.status === 'running' ? 'bg-emerald-500/10 text-emerald-400' : 'bg-zinc-700 text-zinc-400'
            }`}>
              {test.status}
            </span>
            <span className="text-[10px] text-zinc-500">{test.id}</span>
          </div>
          <div className="grid grid-cols-2 gap-4">
            <div className="bg-zinc-950/60 rounded-lg p-3">
              <div className="text-[10px] text-zinc-500 uppercase tracking-wider mb-1">Variant A ({test.trafficA}%)</div>
              <div className="text-xs text-white font-medium">{test.promptA}</div>
              <div className="text-[11px] text-zinc-400 mt-1">{test.conversionsA.toLocaleString()} conversions</div>
              <div className="h-1.5 bg-zinc-800 rounded-full overflow-hidden mt-2">
                <div className="h-full bg-emerald-500 rounded-full" style={{ width: `${test.trafficA}%` }} />
              </div>
            </div>
            <div className="bg-zinc-950/60 rounded-lg p-3">
              <div className="text-[10px] text-zinc-500 uppercase tracking-wider mb-1">Variant B ({test.trafficB}%)</div>
              <div className="text-xs text-white font-medium">{test.promptB}</div>
              <div className="text-[11px] text-zinc-400 mt-1">{test.conversionsB.toLocaleString()} conversions</div>
              <div className="h-1.5 bg-zinc-800 rounded-full overflow-hidden mt-2">
                <div className="h-full bg-blue-500 rounded-full" style={{ width: `${test.trafficB}%` }} />
              </div>
            </div>
          </div>
        </div>
      ))}
    </div>
  )
}

export default function PromptsManagement() {
  const [prompts, setPrompts] = useState<Prompt[]>(SEED_PROMPTS)
  const [search, setSearch] = useState('')
  const [showCreate, setShowCreate] = useState(false)
  const [editingPrompt, setEditingPrompt] = useState<Prompt | null>(null)
  const [viewingVersions, setViewingVersions] = useState<Prompt | null>(null)
  const [selectedId, setSelectedId] = useState<string | null>(null)
  const [activeTab, setActiveTab] = useState<'all' | 'active' | 'inactive'>('all')
  const [showABTests, setShowABTests] = useState(false)

  const filteredPrompts = prompts.filter(p => {
    if (activeTab === 'active' && !p.isActive) return false
    if (activeTab === 'inactive' && p.isActive) return false
    if (search) {
      const q = search.toLowerCase()
      return p.name.toLowerCase().includes(q) || p.description.toLowerCase().includes(q) || p.tags.some(t => t.includes(q))
    }
    return true
  })

  const selectedPrompt = prompts.find(p => p.id === selectedId) ?? null

  const handleCreate = (data: Partial<Prompt>) => {
    const newPrompt: Prompt = {
      id: `prompt_${Date.now()}`,
      name: data.name!,
      description: data.description ?? '',
      content: data.content!,
      version: data.version ?? 1,
      tags: data.tags ?? [],
      isActive: data.isActive ?? true,
      usageCount: 0,
      lastUpdated: new Date().toISOString(),
      createdAt: new Date().toISOString(),
      versions: [
        { version: data.version ?? 1, content: data.content!, updatedAt: new Date().toISOString(), updatedBy: 'current-user' },
      ],
    }
    setPrompts(prev => [newPrompt, ...prev])
    setShowCreate(false)
    setSelectedId(newPrompt.id)
  }

  const handleUpdate = (data: Partial<Prompt>) => {
    if (!editingPrompt) return
    setPrompts(prev => prev.map(p => {
      if (p.id !== editingPrompt.id) return p
      const newVersion = data.version ?? p.version
      return {
        ...p,
        name: data.name ?? p.name,
        description: data.description ?? p.description,
        content: data.content ?? p.content,
        tags: data.tags ?? p.tags,
        isActive: data.isActive ?? p.isActive,
        version: newVersion,
        lastUpdated: new Date().toISOString(),
        versions: [
          { version: newVersion, content: data.content ?? p.content, updatedAt: new Date().toISOString(), updatedBy: 'current-user' },
          ...p.versions,
        ],
      }
    }))
    setEditingPrompt(null)
  }

  const handleToggleActive = (id: string) => {
    setPrompts(prev => prev.map(p => p.id === id ? { ...p, isActive: !p.isActive } : p))
  }

  const handleDuplicate = (prompt: Prompt) => {
    const dup: Prompt = {
      ...prompt,
      id: `prompt_${Date.now()}`,
      name: `${prompt.name} (Copy)`,
      version: 1,
      usageCount: 0,
      isActive: false,
      lastUpdated: new Date().toISOString(),
      createdAt: new Date().toISOString(),
      versions: [
        { version: 1, content: prompt.content, updatedAt: new Date().toISOString(), updatedBy: 'current-user' },
      ],
    }
    setPrompts(prev => [dup, ...prev])
    setSelectedId(dup.id)
  }

  return (
    <div className="flex-1 flex h-full bg-zinc-950 text-zinc-100 overflow-hidden">
      {/* Sidebar / Prompt List */}
      <div className="w-80 shrink-0 border-r border-zinc-800 flex flex-col overflow-hidden">
        <div className="p-4 border-b border-zinc-800 shrink-0">
          <div className="flex items-center justify-between mb-3">
            <div className="flex items-center gap-2">
              <FileText size={18} className="text-blue-400" />
              <h1 className="text-base font-semibold text-white">Prompts</h1>
            </div>
            <button onClick={() => { setEditingPrompt(null); setShowCreate(true) }}
              className="p-1.5 bg-emerald-600 hover:bg-emerald-500 rounded-lg transition-colors"
            >
              <Plus size={14} />
            </button>
          </div>
          <div className="relative">
            <Search size={13} className="absolute left-2.5 top-1/2 -translate-y-1/2 text-zinc-500" />
            <input type="text" value={search} onChange={e => setSearch(e.target.value)} placeholder="Search prompts..."
              className="w-full bg-zinc-950 border border-zinc-800 rounded-lg pl-8 pr-3 py-1.5 text-xs text-zinc-200 focus:outline-none focus:border-blue-500/50"
            />
          </div>
          <div className="flex gap-1 mt-3">
            {(['all', 'active', 'inactive'] as const).map(tab => (
              <button key={tab} onClick={() => setActiveTab(tab)}
                className={`flex-1 px-2 py-1 rounded text-[10px] font-medium transition-all ${
                  activeTab === tab ? 'bg-zinc-800 text-white' : 'text-zinc-500 hover:text-zinc-300'
                }`}
              >
                {tab.charAt(0).toUpperCase() + tab.slice(1)}
              </button>
            ))}
          </div>
        </div>
        <div className="overflow-y-auto flex-1">
          {filteredPrompts.map(p => (
            <div key={p.id}
              onClick={() => { setSelectedId(p.id); setEditingPrompt(null) }}
              className={`px-4 py-3 border-b border-zinc-800/50 cursor-pointer transition-all ${
                selectedId === p.id ? 'bg-blue-500/5 border-l-2 border-l-blue-500' : 'hover:bg-zinc-900/50 border-l-2 border-l-transparent'
              }`}
            >
              <div className="flex items-center justify-between mb-1">
                <span className="text-xs font-medium text-white truncate">{p.name}</span>
                <div className="flex items-center gap-1.5 shrink-0">
                  <span className="text-[10px] font-mono text-zinc-500">v{p.version}</span>
                  <div className={`w-1.5 h-1.5 rounded-full ${p.isActive ? 'bg-emerald-400' : 'bg-zinc-600'}`} />
                </div>
              </div>
              <p className="text-[10px] text-zinc-500 truncate">{p.description}</p>
              <div className="flex items-center gap-2 mt-1.5">
                <span className="text-[9px] text-zinc-600">{p.usageCount.toLocaleString()} uses</span>
                <span className="text-[9px] text-zinc-700">&middot;</span>
                <span className="text-[9px] text-zinc-600">{tsAgo(p.lastUpdated)}</span>
              </div>
            </div>
          ))}
          {filteredPrompts.length === 0 && (
            <div className="p-8 text-center text-zinc-600 text-xs">No prompts found</div>
          )}
        </div>
        <div className="p-3 border-t border-zinc-800 shrink-0">
          <button onClick={() => setShowABTests(!showABTests)}
            className="flex items-center gap-2 w-full px-3 py-2 bg-zinc-900/60 border border-zinc-800 rounded-lg text-xs text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800 transition-all"
          >
            <Beaker size={13} className="text-amber-400" />
            A/B Testing
            {showABTests ? <ChevronDown size={12} className="ml-auto" /> : <ChevronRight size={12} className="ml-auto" />}
          </button>
          {showABTests && (
            <div className="mt-2 space-y-2">
              <ABTestPanel tests={AB_TEST_INFO} />
            </div>
          )}
        </div>
      </div>

      {/* Main Content */}
      <div className="flex-1 flex flex-col overflow-hidden">
        {selectedPrompt ? (
          <>
            <header className="border-b border-zinc-800 bg-zinc-900/30 shrink-0">
              <div className="px-6 py-4 flex items-center justify-between">
                <div className="flex items-center gap-3">
                  <div className="p-2 bg-blue-500/10 rounded-lg">
                    <FileText className="text-blue-400 w-5 h-5" />
                  </div>
                  <div>
                    <h1 className="text-lg font-semibold text-white">{selectedPrompt.name}</h1>
                    <p className="text-xs text-zinc-500">v{selectedPrompt.version} &middot; {selectedPrompt.isActive ? 'Active' : 'Inactive'}</p>
                  </div>
                </div>
                <div className="flex items-center gap-2">
                  <button onClick={() => setViewingVersions(selectedPrompt)}
                    className="flex items-center gap-1.5 px-3 py-1.5 bg-zinc-900 border border-zinc-800 rounded-lg text-xs text-zinc-400 hover:text-white hover:border-zinc-700 transition-all"
                  >
                    <History size={13} /> History
                  </button>
                  <button onClick={() => handleDuplicate(selectedPrompt)}
                    className="flex items-center gap-1.5 px-3 py-1.5 bg-zinc-900 border border-zinc-800 rounded-lg text-xs text-zinc-400 hover:text-white hover:border-zinc-700 transition-all"
                  >
                    <Copy size={13} /> Duplicate
                  </button>
                  <button onClick={() => setEditingPrompt(selectedPrompt)}
                    className="flex items-center gap-1.5 px-3 py-1.5 bg-emerald-600 hover:bg-emerald-500 rounded-lg text-xs text-white font-medium transition-all"
                  >
                    <Pencil size={13} /> Edit
                  </button>
                </div>
              </div>
            </header>

            <div className="flex-1 overflow-y-auto p-6 space-y-6">
              {/* Stats */}
              <div className="grid grid-cols-4 gap-4">
                <div className="bg-zinc-900/60 border border-zinc-800/60 rounded-xl p-4">
                  <div className="flex items-center gap-2 mb-2">
                    <BarChart3 size={13} className="text-zinc-500" />
                    <span className="text-[10px] text-zinc-500 uppercase tracking-wider">Version</span>
                  </div>
                  <div className="text-xl font-bold text-white font-mono">v{selectedPrompt.version}</div>
                </div>
                <div className="bg-zinc-900/60 border border-zinc-800/60 rounded-xl p-4">
                  <div className="flex items-center gap-2 mb-2">
                    <BarChart3 size={13} className="text-zinc-500" />
                    <span className="text-[10px] text-zinc-500 uppercase tracking-wider">Usage Count</span>
                  </div>
                  <div className="text-xl font-bold text-white font-mono">{selectedPrompt.usageCount.toLocaleString()}</div>
                </div>
                <div className="bg-zinc-900/60 border border-zinc-800/60 rounded-xl p-4">
                  <div className="flex items-center gap-2 mb-2">
                    <Clock size={13} className="text-zinc-500" />
                    <span className="text-[10px] text-zinc-500 uppercase tracking-wider">Last Updated</span>
                  </div>
                  <div className="text-sm font-semibold text-white">{tsDisplay(selectedPrompt.lastUpdated)}</div>
                  <div className="text-[10px] text-zinc-500 mt-0.5">{tsAgo(selectedPrompt.lastUpdated)}</div>
                </div>
                <div className="bg-zinc-900/60 border border-zinc-800/60 rounded-xl p-4">
                  <div className="flex items-center gap-2 mb-2">
                    <History size={13} className="text-zinc-500" />
                    <span className="text-[10px] text-zinc-500 uppercase tracking-wider">Versions</span>
                  </div>
                  <div className="text-xl font-bold text-white font-mono">{selectedPrompt.versions.length}</div>
                </div>
              </div>

              {/* Toggle Active */}
              <div className="bg-zinc-900 border border-zinc-800/50 rounded-2xl p-5 flex items-center justify-between">
                <div className="flex items-center gap-3">
                  {selectedPrompt.isActive ? <ToggleRight size={20} className="text-emerald-400" /> : <ToggleLeft size={20} className="text-zinc-600" />}
                  <div>
                    <h3 className="text-sm font-semibold text-white">Status</h3>
                    <p className="text-xs text-zinc-500">{selectedPrompt.isActive ? 'This prompt is active and available for use' : 'This prompt is inactive and hidden from selection'}</p>
                  </div>
                </div>
                <button onClick={() => handleToggleActive(selectedPrompt.id)}
                  className={`relative w-12 h-6 rounded-full transition-colors ${selectedPrompt.isActive ? 'bg-emerald-600' : 'bg-zinc-700'}`}
                >
                  <span className={`absolute top-0.5 w-5 h-5 rounded-full bg-white shadow transition-all ${selectedPrompt.isActive ? 'left-6' : 'left-0.5'}`} />
                </button>
              </div>

              {/* Description */}
              <div className="bg-zinc-900 border border-zinc-800/50 rounded-2xl p-5">
                <h3 className="text-xs text-zinc-500 uppercase tracking-wider mb-2">Description</h3>
                <p className="text-sm text-zinc-300">{selectedPrompt.description || 'No description'}</p>
              </div>

              {/* Tags */}
              <div className="bg-zinc-900 border border-zinc-800/50 rounded-2xl p-5">
                <h3 className="text-xs text-zinc-500 uppercase tracking-wider mb-3">Tags</h3>
                <div className="flex flex-wrap gap-1.5">
                  {selectedPrompt.tags.length > 0 ? selectedPrompt.tags.map(t => (
                    <span key={t} className="flex items-center gap-1 px-2 py-0.5 bg-blue-500/10 text-blue-300 border border-blue-500/20 rounded text-xs font-medium">
                      <Tag size={10} />{t}
                    </span>
                  )) : <span className="text-xs text-zinc-600">No tags</span>}
                </div>
              </div>

              {/* Prompt Content */}
              <div className="bg-zinc-900 border border-zinc-800/50 rounded-2xl p-5">
                <div className="flex items-center justify-between mb-3">
                  <h3 className="text-xs text-zinc-500 uppercase tracking-wider">Prompt Content</h3>
                  <button onClick={() => navigator.clipboard.writeText(selectedPrompt.content)}
                    className="flex items-center gap-1 px-2 py-1 text-[10px] text-zinc-500 hover:text-zinc-300 bg-zinc-950 border border-zinc-800 rounded transition-colors"
                  >
                    <Copy size={10} /> Copy
                  </button>
                </div>
                <pre className="bg-zinc-950/60 border border-zinc-800/50 rounded-xl p-4 text-sm text-zinc-300 font-mono leading-relaxed whitespace-pre-wrap">{selectedPrompt.content}</pre>
              </div>

              {/* Version Timeline */}
              <div className="bg-zinc-900 border border-zinc-800/50 rounded-2xl p-5">
                <div className="flex items-center gap-2 mb-4">
                  <History size={14} className="text-zinc-500" />
                  <h3 className="text-xs text-zinc-500 uppercase tracking-wider">Version Timeline</h3>
                </div>
                <div className="space-y-2">
                  {selectedPrompt.versions.map((v, i) => (
                    <div key={v.version} className="flex items-start gap-3">
                      <div className="flex flex-col items-center">
                        <div className={`w-2 h-2 rounded-full shrink-0 mt-1 ${i === 0 ? 'bg-emerald-400' : 'bg-zinc-700'}`} />
                        {i < selectedPrompt.versions.length - 1 && <div className="w-px h-6 bg-zinc-800" />}
                      </div>
                      <div className="pb-2">
                        <div className="flex items-center gap-2">
                          <span className="text-xs font-semibold text-white">v{v.version}</span>
                          {i === 0 && <span className="text-[9px] bg-emerald-500/10 text-emerald-400 px-1.5 py-0.5 rounded">Current</span>}
                        </div>
                        <p className="text-[11px] text-zinc-500 mt-0.5">{tsAgo(v.updatedAt)} by {v.updatedBy}</p>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            </div>
          </>
        ) : (
          <div className="flex-1 flex items-center justify-center">
            <div className="text-center">
              <FileText size={40} className="text-zinc-800 mx-auto mb-3" />
              <p className="text-sm text-zinc-500">Select a prompt from the sidebar</p>
              <p className="text-xs text-zinc-600 mt-1">or create a new one with the + button</p>
            </div>
          </div>
        )}
      </div>

      {/* Modals */}
      {showCreate && (
        <PromptFormModal initial={{}} isEdit={false} onClose={() => setShowCreate(false)} onSave={handleCreate} />
      )}
      {editingPrompt && (
        <PromptFormModal initial={editingPrompt} isEdit={true} onClose={() => setEditingPrompt(null)} onSave={handleUpdate} />
      )}
      {viewingVersions && (
        <VersionHistoryPanel prompt={viewingVersions} onClose={() => setViewingVersions(null)} />
      )}
    </div>
  )
}
