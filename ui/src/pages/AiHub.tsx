import { useState, useMemo } from 'react'
import {
  Star, Download, GitFork, Search, Sparkles, Filter,
  Play, Users, Brain, Code, BarChart3, Wand2, MessageSquare, Image as ImageIcon,
} from 'lucide-react'

// ─── Types ────────────────────────────────────────────────────────────────────

type Category = 'all' | 'prompts' | 'workflows' | 'evaluations'

interface HubItem {
  id: string
  title: string
  description: string
  author: string
  category: 'prompt' | 'workflow' | 'evaluation'
  rating: number
  ratingCount: number
  usageCount: number
  forkCount: number
  tags: string[]
  icon: React.ReactNode
  featured?: boolean
  createdAt: string
}

// ─── Seed Data ────────────────────────────────────────────────────────────────

const SEED_ITEMS: HubItem[] = [
  {
    id: '1',
    title: 'Code Review Assistant',
    description: 'Expert code reviewer that identifies bugs, suggests improvements, and ensures best practices across languages.',
    author: 'Pylos Team',
    category: 'prompt',
    rating: 4.8,
    ratingCount: 234,
    usageCount: 12840,
    forkCount: 89,
    tags: ['coding', 'review', 'quality'],
    icon: <Code size={20} />,
    featured: true,
    createdAt: '2025-03-15',
  },
  {
    id: '2',
    title: 'Data Analysis Pipeline',
    description: 'End-to-end workflow for cleaning, transforming, and analyzing datasets with automated visualization.',
    author: 'data_wizard',
    category: 'workflow',
    rating: 4.6,
    ratingCount: 187,
    usageCount: 8420,
    forkCount: 67,
    tags: ['analysis', 'data', 'pipeline'],
    icon: <BarChart3 size={20} />,
    featured: true,
    createdAt: '2025-04-02',
  },
  {
    id: '3',
    title: 'GPT-4 vs Claude Evaluation',
    description: 'Standardized benchmark comparing GPT-4 and Claude across reasoning, coding, and creative tasks.',
    author: 'benchmarks_ai',
    category: 'evaluation',
    rating: 4.9,
    ratingCount: 412,
    usageCount: 23100,
    forkCount: 156,
    tags: ['benchmark', 'comparison', 'testing'],
    icon: <Brain size={20} />,
    createdAt: '2025-02-28',
  },
  {
    id: '4',
    title: 'RAG Best Practices',
    description: 'Complete workflow for building production-ready Retrieval-Augmented Generation systems with vector stores.',
    author: 'ai_architect',
    category: 'workflow',
    rating: 4.7,
    ratingCount: 298,
    usageCount: 15600,
    forkCount: 112,
    tags: ['rag', 'retrieval', 'production'],
    icon: <Sparkles size={20} />,
    createdAt: '2025-03-20',
  },
  {
    id: '5',
    title: 'Customer Support Bot',
    description: 'Multilingual customer support prompt with context-aware responses and escalation handling.',
    author: 'enterprise_ai',
    category: 'prompt',
    rating: 4.5,
    ratingCount: 156,
    usageCount: 9870,
    forkCount: 78,
    tags: ['support', 'multilingual', 'enterprise'],
    icon: <MessageSquare size={20} />,
    createdAt: '2025-04-10',
  },
  {
    id: '6',
    title: 'Image Description Generator',
    description: 'Creates detailed, accessible image descriptions for alt text and content moderation.',
    author: 'vision_labs',
    category: 'prompt',
    rating: 4.4,
    ratingCount: 198,
    usageCount: 11200,
    forkCount: 45,
    tags: ['vision', 'accessibility', 'creative'],
    icon: <ImageIcon size={20} />,
    createdAt: '2025-03-28',
  },
  {
    id: '7',
    title: 'Multi-Model Reasoning Eval',
    description: 'Comprehensive evaluation suite testing logical reasoning, math, and common sense across 10+ models.',
    author: 'eval_suite',
    category: 'evaluation',
    rating: 4.8,
    ratingCount: 342,
    usageCount: 18900,
    forkCount: 201,
    tags: ['reasoning', 'benchmark', 'comprehensive'],
    icon: <Brain size={20} />,
    createdAt: '2025-01-15',
  },
  {
    id: '8',
    title: 'Prompt Chaining Framework',
    description: 'Meta-workflow for building complex multi-step prompt chains with error recovery and fallbacks.',
    author: 'prompt_master',
    category: 'workflow',
    rating: 4.3,
    ratingCount: 127,
    usageCount: 6540,
    forkCount: 34,
    tags: ['chaining', 'meta', 'framework'],
    icon: <Wand2 size={20} />,
    createdAt: '2025-04-18',
  },
]

const CATEGORIES: { key: Category; label: string }[] = [
  { key: 'all', label: 'All' },
  { key: 'prompts', label: 'Prompts' },
  { key: 'workflows', label: 'Workflows' },
  { key: 'evaluations', label: 'Evaluations' },
]

// ─── Helpers ──────────────────────────────────────────────────────────────────

function formatCount(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return String(n)
}

function StarRating({ rating, count }: { rating: number; count: number }) {
  return (
    <div className="flex items-center gap-1.5">
      <div className="flex items-center gap-0.5">
        {Array.from({ length: 5 }, (_, i) => (
          <Star
            key={i}
            size={12}
            className={i < Math.round(rating) ? 'fill-amber-400 text-amber-400' : 'text-zinc-700'}
          />
        ))}
      </div>
      <span className="text-xs text-zinc-400">{rating}</span>
      <span className="text-xs text-zinc-600">({formatCount(count)})</span>
    </div>
  )
}

function CategoryBadge({ category }: { category: HubItem['category'] }) {
  const styles: Record<string, string> = {
    prompt: 'bg-emerald-500/10 text-emerald-400 border-emerald-500/20',
    workflow: 'bg-blue-500/10 text-blue-400 border-blue-500/20',
    evaluation: 'bg-purple-500/10 text-purple-400 border-purple-500/20',
  }
  return (
    <span className={`inline-flex items-center px-2 py-0.5 rounded-full text-[10px] font-medium border ${styles[category]}`}>
      {category.charAt(0).toUpperCase() + category.slice(1)}
    </span>
  )
}

// ─── Components ───────────────────────────────────────────────────────────────

function FeaturedCard({ item }: { item: HubItem }) {
  return (
    <div className="relative overflow-hidden rounded-xl border border-zinc-800/50 bg-zinc-900/30 p-6 hover:border-emerald-500/30 transition-all duration-300 group">
      <div className="absolute top-0 left-0 w-full h-1 bg-gradient-to-r from-emerald-500 to-emerald-400" />
      <div className="flex items-start justify-between mb-4">
        <div className="flex items-center gap-3">
          <div className="p-2.5 rounded-lg bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
            {item.icon}
          </div>
          <div>
            <h3 className="text-base font-semibold text-white group-hover:text-emerald-400 transition-colors">
              {item.title}
            </h3>
            <div className="flex items-center gap-2 mt-1">
              <CategoryBadge category={item.category} />
              <span className="text-xs text-zinc-500">by {item.author}</span>
            </div>
          </div>
        </div>
        <div className="flex items-center gap-1 px-2 py-1 rounded-md bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
          <Sparkles size={12} />
          <span className="text-[10px] font-medium uppercase tracking-wide">Featured</span>
        </div>
      </div>
      <p className="text-sm text-zinc-400 mb-4 line-clamp-2">{item.description}</p>
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-4">
          <StarRating rating={item.rating} count={item.ratingCount} />
          <div className="flex items-center gap-1 text-xs text-zinc-500">
            <Download size={12} />
            {formatCount(item.usageCount)}
          </div>
          <div className="flex items-center gap-1 text-xs text-zinc-500">
            <GitFork size={12} />
            {formatCount(item.forkCount)}
          </div>
        </div>
        <div className="flex items-center gap-2">
          <button className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg border border-zinc-800 bg-zinc-900/50 text-xs text-zinc-300 hover:text-white hover:border-zinc-700 transition-colors">
            <GitFork size={12} />
            Fork
          </button>
          <button className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg bg-emerald-600 hover:bg-emerald-500 text-xs text-white font-medium transition-colors">
            <Play size={12} />
            Use
          </button>
        </div>
      </div>
    </div>
  )
}

function ItemCard({ item }: { item: HubItem }) {
  return (
    <div className="rounded-xl border border-zinc-800/50 bg-zinc-900/30 p-5 hover:border-zinc-700/50 transition-all duration-200 group">
      <div className="flex items-start gap-3 mb-3">
        <div className="p-2 rounded-lg bg-zinc-800/50 text-zinc-400 group-hover:text-emerald-400 transition-colors">
          {item.icon}
        </div>
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2">
            <h3 className="text-sm font-semibold text-white truncate group-hover:text-emerald-400 transition-colors">
              {item.title}
            </h3>
            <CategoryBadge category={item.category} />
          </div>
          <p className="text-xs text-zinc-500 mt-0.5">by {item.author}</p>
        </div>
      </div>
      <p className="text-xs text-zinc-400 mb-3 line-clamp-2">{item.description}</p>
      <div className="flex flex-wrap gap-1.5 mb-3">
        {item.tags.map(tag => (
          <span key={tag} className="px-2 py-0.5 rounded-md bg-zinc-800/50 text-[10px] text-zinc-400 border border-zinc-800/50">
            {tag}
          </span>
        ))}
      </div>
      <div className="flex items-center justify-between pt-3 border-t border-zinc-800/30">
        <div className="flex items-center gap-3">
          <StarRating rating={item.rating} count={item.ratingCount} />
          <div className="flex items-center gap-1 text-xs text-zinc-500">
            <Download size={11} />
            {formatCount(item.usageCount)}
          </div>
        </div>
        <div className="flex items-center gap-1.5">
          <button className="p-1.5 rounded-md text-zinc-500 hover:text-zinc-300 hover:bg-zinc-800/50 transition-colors" title="Fork">
            <GitFork size={13} />
          </button>
          <button className="p-1.5 rounded-md text-zinc-500 hover:text-emerald-400 hover:bg-emerald-500/10 transition-colors" title="Use">
            <Play size={13} />
          </button>
        </div>
      </div>
    </div>
  )
}

// ─── Main Component ───────────────────────────────────────────────────────────

export default function AiHub() {
  const [search, setSearch] = useState('')
  const [category, setCategory] = useState<Category>('all')
  const [sortBy, setSortBy] = useState<'popular' | 'rating' | 'newest'>('popular')

  const filteredItems = useMemo(() => {
    const items = SEED_ITEMS.filter(item => {
      const matchesCategory = category === 'all' || item.category === category.slice(0, -1)
      const query = search.toLowerCase()
      const matchesSearch = !query ||
        item.title.toLowerCase().includes(query) ||
        item.description.toLowerCase().includes(query) ||
        item.tags.some(t => t.includes(query)) ||
        item.author.toLowerCase().includes(query)
      return matchesCategory && matchesSearch
    })

    items.sort((a, b) => {
      if (sortBy === 'popular') return b.usageCount - a.usageCount
      if (sortBy === 'rating') return b.rating - a.rating
      return new Date(b.createdAt).getTime() - new Date(a.createdAt).getTime()
    })

    return items
  }, [search, category, sortBy])

  const featuredItems = SEED_ITEMS.filter(i => i.featured)

  const stats = [
    { label: 'Total Items', value: SEED_ITEMS.length, icon: <Sparkles size={14} /> },
    { label: 'Total Users', value: '24.8K', icon: <Users size={14} /> },
    { label: 'Total Forks', value: formatCount(SEED_ITEMS.reduce((s, i) => s + i.forkCount, 0)), icon: <GitFork size={14} /> },
  ]

  return (
    <div className="p-6 space-y-6 bg-zinc-950 min-h-screen">
      {/* Header */}
      <div className="flex items-center justify-between flex-wrap gap-4">
        <div>
          <h1 className="text-2xl font-bold text-white flex items-center gap-3">
            <div className="p-2 rounded-lg bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
              <Sparkles size={20} />
            </div>
            AI Hub
          </h1>
          <p className="text-sm text-zinc-400 mt-1">
            Discover and share prompts, workflows, and evaluations
          </p>
        </div>
        <div className="flex items-center gap-3">
          {stats.map(stat => (
            <div key={stat.label} className="flex items-center gap-2 px-3 py-1.5 rounded-lg border border-zinc-800/50 bg-zinc-900/30">
              <span className="text-zinc-500">{stat.icon}</span>
              <span className="text-sm font-medium text-white">{stat.value}</span>
              <span className="text-xs text-zinc-500">{stat.label}</span>
            </div>
          ))}
        </div>
      </div>

      {/* Featured Section */}
      <div>
        <div className="flex items-center gap-2 mb-4">
          <div className="w-1 h-5 rounded-full bg-emerald-500" />
          <h2 className="text-sm font-semibold text-zinc-300 uppercase tracking-wide">Featured</h2>
        </div>
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          {featuredItems.map(item => (
            <FeaturedCard key={item.id} item={item} />
          ))}
        </div>
      </div>

      {/* Search and Filters */}
      <div className="flex items-center gap-4 flex-wrap">
        <div className="relative flex-1 min-w-[280px]">
          <Search size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-500" />
          <input
            type="text"
            value={search}
            onChange={e => setSearch(e.target.value)}
            placeholder="Search prompts, workflows, evaluations..."
            className="w-full pl-9 pr-4 py-2.5 rounded-lg border border-zinc-800/50 bg-zinc-900/30 text-sm text-zinc-100 placeholder:text-zinc-500 focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20 transition-colors"
          />
        </div>
        <div className="flex rounded-lg border border-zinc-800/50 bg-zinc-900/50 overflow-hidden">
          {CATEGORIES.map(c => (
            <button
              key={c.key}
              onClick={() => setCategory(c.key)}
              className={`px-4 py-2 text-xs font-medium transition-colors
                ${category === c.key
                  ? 'bg-zinc-800 text-white'
                  : 'text-zinc-500 hover:text-zinc-300'
                }`}
            >
              {c.label}
            </button>
          ))}
        </div>
        <select
          value={sortBy}
          onChange={e => setSortBy(e.target.value as typeof sortBy)}
          className="px-3 py-2.5 rounded-lg border border-zinc-800/50 bg-zinc-900/30 text-xs text-zinc-300 focus:outline-none focus:border-emerald-500/50 cursor-pointer"
        >
          <option value="popular">Most Popular</option>
          <option value="rating">Highest Rated</option>
          <option value="newest">Newest</option>
        </select>
      </div>

      {/* Results Count */}
      <div className="flex items-center gap-2 text-xs text-zinc-500">
        <Filter size={12} />
        {filteredItems.length} {filteredItems.length === 1 ? 'item' : 'items'} found
      </div>

      {/* Card Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
        {filteredItems.map(item => (
          <ItemCard key={item.id} item={item} />
        ))}
      </div>

      {/* Empty State */}
      {filteredItems.length === 0 && (
        <div className="flex flex-col items-center justify-center py-20 text-center">
          <div className="p-4 rounded-full bg-zinc-900/50 text-zinc-600 mb-4">
            <Search size={32} />
          </div>
          <p className="text-sm text-zinc-400 mb-1">No items found</p>
          <p className="text-xs text-zinc-600">Try adjusting your search or filters</p>
        </div>
      )}
    </div>
  )
}
