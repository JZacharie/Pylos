import { useState, useEffect } from 'react'
import {
  BookOpen, Code, Zap, Clock, BarChart, Shield, Server, Search, CheckCircle,
} from 'lucide-react'

// ─── Types ────────────────────────────────────────────────────────────────────

type Category = 'All' | 'Getting Started' | 'Guides' | 'Tutorials' | 'API Docs'
type Difficulty = 'beginner' | 'intermediate' | 'advanced'

interface Resource {
  id: string
  title: string
  description: string
  category: Category
  readingTime: number
  difficulty: Difficulty
  icon: React.ReactNode
}

// ─── Data ─────────────────────────────────────────────────────────────────────

const CATEGORIES: Category[] = ['All', 'Getting Started', 'Guides', 'Tutorials', 'API Docs']

const RESOURCES: Resource[] = [
  {
    id: 'getting-started',
    title: 'Getting Started with Pylos',
    description: 'Learn the fundamentals of Pylos, from installation to your first API call. Covers providers, virtual keys, and routing basics.',
    category: 'Getting Started',
    readingTime: 5,
    difficulty: 'beginner',
    icon: <BookOpen size={18} />,
  },
  {
    id: 'virtual-keys',
    title: 'Configuring Virtual Keys',
    description: 'Set up virtual keys with budget limits, rate controls, and model restrictions to securely manage client access.',
    category: 'Getting Started',
    readingTime: 3,
    difficulty: 'beginner',
    icon: <Shield size={18} />,
  },
  {
    id: 'rag-pipelines',
    title: 'Setting Up RAG Pipelines',
    description: 'Build retrieval-augmented generation workflows using embeddings, vector stores, and smart context injection.',
    category: 'Guides',
    readingTime: 10,
    difficulty: 'intermediate',
    icon: <Server size={18} />,
  },
  {
    id: 'advanced-routing',
    title: 'Advanced Routing Rules',
    description: 'Master conditional routing, failover chains, and cost-based model selection to optimize performance and spend.',
    category: 'Guides',
    readingTime: 8,
    difficulty: 'advanced',
    icon: <Zap size={18} />,
  },
  {
    id: 'mcp-integration',
    title: 'MCP Server Integration',
    description: 'Connect external MCP servers to expose custom tools and data sources to your LLM agents through the gateway.',
    category: 'Tutorials',
    readingTime: 7,
    difficulty: 'intermediate',
    icon: <Code size={18} />,
  },
  {
    id: 'guardrails',
    title: 'Guardrails Configuration',
    description: 'Enforce input/output policies, content filtering, and safety checks on every request flowing through Pylos.',
    category: 'Guides',
    readingTime: 4,
    difficulty: 'beginner',
    icon: <Shield size={18} />,
  },
  {
    id: 'cost-optimization',
    title: 'Cost Optimization Strategies',
    description: 'Reduce LLM spend with prompt caching, model fallbacks, token budgets, and smart routing strategies.',
    category: 'Tutorials',
    readingTime: 6,
    difficulty: 'intermediate',
    icon: <BarChart size={18} />,
  },
  {
    id: 'production-deployment',
    title: 'Production Deployment Guide',
    description: 'Deploy Pylos at scale with high availability, monitoring, logging, and security hardening best practices.',
    category: 'API Docs',
    readingTime: 12,
    difficulty: 'advanced',
    icon: <Server size={18} />,
  },
]

const DIFFICULTY_CONFIG: Record<Difficulty, { label: string; color: string }> = {
  beginner: { label: 'Beginner', color: 'text-emerald-400 bg-emerald-400/10 border-emerald-400/20' },
  intermediate: { label: 'Intermediate', color: 'text-amber-400 bg-amber-400/10 border-amber-400/20' },
  advanced: { label: 'Advanced', color: 'text-red-400 bg-red-400/10 border-red-400/20' },
}

const CATEGORY_ICONS: Record<Category, React.ReactNode> = {
  'All': <BookOpen size={14} />,
  'Getting Started': <Zap size={14} />,
  'Guides': <BookOpen size={14} />,
  'Tutorials': <Code size={14} />,
  'API Docs': <Server size={14} />,
}

const STORAGE_KEY = 'pylos-learning-progress'

// ─── Helpers ──────────────────────────────────────────────────────────────────

function getProgress(): Record<string, boolean> {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    return raw ? JSON.parse(raw) : {}
  } catch {
    return {}
  }
}

function setProgress(progress: Record<string, boolean>) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(progress))
}

// ─── Components ───────────────────────────────────────────────────────────────

function ResourceCard({
  resource,
  isRead,
  onToggleRead,
}: {
  resource: Resource
  isRead: boolean
  onToggleRead: (id: string) => void
}) {
  const diff = DIFFICULTY_CONFIG[resource.difficulty]

  return (
    <div className="rounded-xl border border-zinc-800/50 bg-zinc-900/20 p-5 hover:border-zinc-700/50 transition-colors flex flex-col">
      <div className="flex items-start justify-between mb-3">
        <div className="p-2 bg-emerald-500/10 rounded-lg text-emerald-400 shrink-0">
          {resource.icon}
        </div>
        {isRead && (
          <CheckCircle size={16} className="text-emerald-400 shrink-0" />
        )}
      </div>
      <h3 className="text-sm font-semibold text-white mb-2 leading-snug">{resource.title}</h3>
      <p className="text-xs text-zinc-500 leading-relaxed mb-4 flex-1">{resource.description}</p>
      <div className="flex items-center gap-3 mb-4">
        <span className={`text-[11px] font-medium px-2 py-0.5 rounded-full border ${diff.color}`}>
          {diff.label}
        </span>
        <span className="flex items-center gap-1 text-[11px] text-zinc-500">
          <Clock size={11} />
          {resource.readingTime} min
        </span>
      </div>
      <button
        onClick={() => onToggleRead(resource.id)}
        className={`w-full px-3 py-2 rounded-lg text-xs font-medium transition-colors ${
          isRead
            ? 'bg-emerald-500/10 text-emerald-400 hover:bg-emerald-500/20 border border-emerald-500/20'
            : 'bg-zinc-800 text-zinc-300 hover:bg-zinc-700 border border-zinc-700/50'
        }`}
      >
        {isRead ? 'Mark as Unread' : 'Read'}
      </button>
    </div>
  )
}

// ─── Main ─────────────────────────────────────────────────────────────────────

export default function LearningResources() {
  const [search, setSearch] = useState('')
  const [activeCategory, setActiveCategory] = useState<Category>('All')
  const [progress, setProgressState] = useState<Record<string, boolean>>(getProgress)

  const readCount = Object.values(progress).filter(Boolean).length

  useEffect(() => {
    setProgress(progress)
  }, [progress])

  const toggleRead = (id: string) => {
    setProgressState(prev => ({ ...prev, [id]: !prev[id] }))
  }

  const filtered = RESOURCES.filter(r => {
    const matchesCategory = activeCategory === 'All' || r.category === activeCategory
    const matchesSearch =
      search === '' ||
      r.title.toLowerCase().includes(search.toLowerCase()) ||
      r.description.toLowerCase().includes(search.toLowerCase())
    return matchesCategory && matchesSearch
  })

  return (
    <div className="p-6 space-y-6 overflow-y-auto h-full">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <div className="flex items-center gap-3 mb-1">
            <div className="p-2 bg-emerald-500/10 rounded-lg">
              <BookOpen size={18} className="text-emerald-400" />
            </div>
            <h1 className="text-2xl font-bold text-white">Learning Resources</h1>
          </div>
          <p className="text-sm text-zinc-400 mt-1">
            {readCount} of {RESOURCES.length} resources completed
          </p>
        </div>
        <div className="flex items-center gap-2 text-xs text-zinc-500">
          <div className="w-32 h-1.5 bg-zinc-800 rounded-full overflow-hidden">
            <div
              className="h-full bg-emerald-500 rounded-full transition-all"
              style={{ width: `${(readCount / RESOURCES.length) * 100}%` }}
            />
          </div>
          <span>{Math.round((readCount / RESOURCES.length) * 100)}%</span>
        </div>
      </div>

      {/* Search */}
      <div className="relative">
        <Search size={15} className="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-500" />
        <input
          type="text"
          value={search}
          onChange={e => setSearch(e.target.value)}
          placeholder="Search resources..."
          className="w-full bg-zinc-950 border border-zinc-800 rounded-lg pl-9 pr-4 py-2.5 text-sm text-zinc-200 placeholder:text-zinc-600 focus:outline-none focus:border-emerald-500/50 transition-colors"
        />
      </div>

      {/* Category tabs */}
      <div className="flex items-center gap-1 p-1 bg-zinc-900/50 rounded-lg border border-zinc-800/50 w-fit">
        {CATEGORIES.map(cat => (
          <button
            key={cat}
            onClick={() => setActiveCategory(cat)}
            className={`flex items-center gap-1.5 px-3 py-1.5 rounded-md text-xs font-medium transition-all ${
              activeCategory === cat
                ? 'bg-zinc-800 text-white shadow-sm'
                : 'text-zinc-500 hover:text-zinc-300'
            }`}
          >
            <span className={activeCategory === cat ? 'text-emerald-400' : 'text-zinc-600'}>
              {CATEGORY_ICONS[cat]}
            </span>
            {cat}
          </button>
        ))}
      </div>

      {/* Resource grid */}
      {filtered.length > 0 ? (
        <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-4">
          {filtered.map(resource => (
            <ResourceCard
              key={resource.id}
              resource={resource}
              isRead={!!progress[resource.id]}
              onToggleRead={toggleRead}
            />
          ))}
        </div>
      ) : (
        <div className="text-center py-16 text-zinc-600">
          No resources found matching your search.
        </div>
      )}
    </div>
  )
}
