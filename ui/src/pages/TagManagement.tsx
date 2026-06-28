import { useState, useMemo } from 'react'
import {
  Plus, Pencil, Trash2, X, Check, Palette,
  RotateCcw, AlertTriangle, KeyRound, FileText, CheckSquare,
} from 'lucide-react'

interface TagDef {
  id: string
  name: string
  color: string
  vkCount: number
  logCount: number
  vkNames: string[]
  logSnippets: string[]
}

const PREDEFINED_COLORS = [
  { name: 'Blue', value: '#3b82f6' },
  { name: 'Green', value: '#22c55e' },
  { name: 'Yellow', value: '#eab308' },
  { name: 'Purple', value: '#a855f7' },
  { name: 'Red', value: '#ef4444' },
  { name: 'Gray', value: '#71717a' },
  { name: 'Cyan', value: '#06b6d4' },
  { name: 'Orange', value: '#f97316' },
  { name: 'Pink', value: '#ec4899' },
  { name: 'Indigo', value: '#6366f1' },
]

const SEED_TAGS: TagDef[] = [
  { id: '1', name: 'production', color: '#3b82f6', vkCount: 5, logCount: 120, vkNames: ['prod-main', 'prod-backup', 'prod-ml', 'prod-analytics', 'prod-gateway'], logSnippets: ['GPT-4 completion 150ms', 'Claude request 89ms', 'Embedding call 45ms'] },
  { id: '2', name: 'staging', color: '#eab308', vkCount: 3, logCount: 45, vkNames: ['staging-main', 'staging-test', 'staging-qa'], logSnippets: ['Test completion 210ms', 'Debug request 180ms'] },
  { id: '3', name: 'team-a', color: '#22c55e', vkCount: 8, logCount: 300, vkNames: ['team-a-app', 'team-a-bot', 'team-a-service', 'team-a-worker', 'team-a-sandbox', 'team-a-prod', 'team-a-staging', 'team-a-dev'], logSnippets: ['Batch processing 1.2s', 'API call 67ms', 'Stream response 340ms'] },
  { id: '4', name: 'team-b', color: '#a855f7', vkCount: 4, logCount: 90, vkNames: ['team-b-main', 'team-b-research', 'team-b-demo', 'team-b-eval'], logSnippets: ['Evaluation run 2.3s', 'Prompt test 150ms'] },
  { id: '5', name: 'experimental', color: '#ef4444', vkCount: 2, logCount: 12, vkNames: ['exp-prototype', 'exp-try'], logSnippets: ['Model comparison 890ms'] },
  { id: '6', name: 'deprecated', color: '#71717a', vkCount: 1, logCount: 3, vkNames: ['legacy-old'], logSnippets: ['Deprecated call 200ms'] },
]

function ColorPicker({ value, onChange }: { value: string; onChange: (c: string) => void }) {
  const [open, setOpen] = useState(false)
  return (
    <div className="relative">
      <button type="button" onClick={() => setOpen(!open)}
        className="flex items-center gap-2 px-3 py-2 bg-zinc-950 border border-zinc-800 rounded-lg text-sm text-zinc-200 hover:border-zinc-700 transition-colors">
        <div className="w-4 h-4 rounded-full border border-zinc-700" style={{ backgroundColor: value }} />
        <Palette size={13} className="text-zinc-500" />
        <span className="text-xs text-zinc-500">Pick color</span>
      </button>
      {open && (
        <div className="absolute z-50 mt-1 p-2 bg-zinc-900 border border-zinc-800 rounded-xl shadow-xl grid grid-cols-5 gap-1.5 w-[200px]">
          {PREDEFINED_COLORS.map(c => (
            <button key={c.value} type="button" title={c.name}
              onClick={() => { onChange(c.value); setOpen(false) }}
              className={`w-8 h-8 rounded-lg border-2 transition-all hover:scale-110 ${value === c.value ? 'border-white scale-110' : 'border-transparent'}`}
              style={{ backgroundColor: c.value }}
            />
          ))}
        </div>
      )}
    </div>
  )
}

function TagModal({ initial, isEdit, onClose, onSave, saving }: {
  initial: { name: string; color: string }
  isEdit: boolean
  onClose: () => void
  onSave: (name: string, color: string) => void
  saving: boolean
}) {
  const [name, setName] = useState(initial.name)
  const [color, setColor] = useState(initial.color)
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-sm mx-4">
        <div className="flex items-center justify-between p-5 border-b border-zinc-800/50">
          <h2 className="text-lg font-semibold text-white">{isEdit ? 'Edit tag' : 'Create tag'}</h2>
          <button onClick={onClose} className="text-zinc-500 hover:text-white"><X size={18} /></button>
        </div>
        <div className="p-5 space-y-5">
          <div>
            <label className="block text-xs text-zinc-400 mb-1.5">Tag name *</label>
            <input type="text" value={name} onChange={e => setName(e.target.value)}
              onKeyDown={e => { if (e.key === 'Enter' && name.trim()) onSave(name.trim(), color) }}
              placeholder="production"
              autoFocus
              className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200 focus:outline-none focus:border-emerald-500/50" />
          </div>
          <div>
            <label className="block text-xs text-zinc-400 mb-1.5">Color</label>
            <ColorPicker value={color} onChange={setColor} />
          </div>
        </div>
        <div className="flex justify-end gap-3 px-5 py-4 border-t border-zinc-800/50">
          <button onClick={onClose} className="px-4 py-2 text-sm text-zinc-400 hover:text-white">Cancel</button>
          <button onClick={() => onSave(name.trim(), color)} disabled={saving || !name.trim()}
            className="px-4 py-2 text-sm bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-white rounded-lg flex items-center gap-2">
            {saving ? <RotateCcw size={14} className="animate-spin" /> : <Check size={14} />}
            {isEdit ? 'Update' : 'Create'}
          </button>
        </div>
      </div>
    </div>
  )
}

function DeleteTagModal({ tag, onClose, onConfirm, deleting }: {
  tag: TagDef; onClose: () => void; onConfirm: () => void; deleting: boolean
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-sm mx-4 p-6">
        <div className="flex items-center gap-3 mb-4">
          <div className="w-9 h-9 rounded-full bg-red-500/15 flex items-center justify-center"><AlertTriangle size={16} className="text-red-400" /></div>
          <div><div className="font-semibold text-white">Delete tag</div><div className="text-xs text-zinc-500">This action cannot be undone</div></div>
        </div>
        <p className="text-sm text-zinc-400 mb-2">
          Delete <span className="text-white font-medium">"{tag.name}"</span>?
        </p>
        <p className="text-xs text-zinc-500 mb-5">
          This tag will be removed from all {tag.vkCount} virtual keys and {tag.logCount} logs that currently use it.
        </p>
        <div className="flex justify-end gap-3">
          <button onClick={onClose} className="px-4 py-2 text-sm text-zinc-400 hover:text-white">Cancel</button>
          <button onClick={onConfirm} disabled={deleting}
            className="px-4 py-2 text-sm bg-red-600 hover:bg-red-500 disabled:opacity-50 text-white rounded-lg flex items-center gap-2">
            {deleting ? <RotateCcw size={13} className="animate-spin" /> : <Trash2 size={13} />}
            Delete
          </button>
        </div>
      </div>
    </div>
  )
}

function UsageDetailModal({ tag, onClose }: { tag: TagDef; onClose: () => void }) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-md mx-4 max-h-[80vh] overflow-y-auto">
        <div className="flex items-center justify-between p-5 border-b border-zinc-800/50">
          <div className="flex items-center gap-3">
            <div className="w-3 h-3 rounded-full" style={{ backgroundColor: tag.color }} />
            <h2 className="text-lg font-semibold text-white">"{tag.name}" usage</h2>
          </div>
          <button onClick={onClose} className="text-zinc-500 hover:text-white"><X size={18} /></button>
        </div>
        <div className="p-5 space-y-6">
          <div>
            <div className="flex items-center gap-2 text-xs text-zinc-400 mb-3">
              <KeyRound size={12} />
              <span className="font-medium uppercase tracking-wider">Virtual Keys ({tag.vkCount})</span>
            </div>
            {tag.vkNames.length === 0 ? (
              <div className="text-xs text-zinc-600 pl-5">No virtual keys with this tag</div>
            ) : (
              <div className="space-y-1.5">
                {tag.vkNames.map(name => (
                  <div key={name} className="flex items-center gap-2 px-3 py-2 bg-zinc-950/50 border border-zinc-800/50 rounded-lg">
                    <KeyRound size={11} className="text-emerald-400 shrink-0" />
                    <span className="text-sm text-zinc-300">{name}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
          <div>
            <div className="flex items-center gap-2 text-xs text-zinc-400 mb-3">
              <FileText size={12} />
              <span className="font-medium uppercase tracking-wider">Recent Logs ({tag.logCount})</span>
            </div>
            {tag.logSnippets.length === 0 ? (
              <div className="text-xs text-zinc-600 pl-5">No logs with this tag</div>
            ) : (
              <div className="space-y-1.5">
                {tag.logSnippets.map((log, i) => (
                  <div key={i} className="flex items-center gap-2 px-3 py-2 bg-zinc-950/50 border border-zinc-800/50 rounded-lg">
                    <FileText size={11} className="text-zinc-500 shrink-0" />
                    <span className="text-xs text-zinc-400 font-mono">{log}</span>
                  </div>
                ))}
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  )
}

function BulkTagModal({ tags, onClose }: { tags: TagDef[]; onClose: () => void }) {
  const [selectedTags, setSelectedTags] = useState<string[]>([])
  const [applied, setApplied] = useState(false)

  const toggleTag = (id: string) => {
    setSelectedTags(prev => prev.includes(id) ? prev.filter(t => t !== id) : [...prev, id])
  }

  const applyTags = () => {
    setApplied(true)
    setTimeout(() => onClose(), 1200)
  }

  if (applied) {
    return (
      <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
        <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-sm mx-4 p-6 text-center">
          <div className="w-10 h-10 rounded-full bg-emerald-500/15 flex items-center justify-center mx-auto mb-3">
            <Check size={18} className="text-emerald-400" />
          </div>
          <div className="font-semibold text-white mb-1">Tags applied</div>
          <div className="text-xs text-zinc-500">{selectedTags.length} tag(s) added to selected virtual keys</div>
        </div>
      </div>
    )
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-sm mx-4">
        <div className="flex items-center justify-between p-5 border-b border-zinc-800/50">
          <div className="flex items-center gap-2">
            <CheckSquare size={16} className="text-emerald-400" />
            <h2 className="text-lg font-semibold text-white">Bulk tag</h2>
          </div>
          <button onClick={onClose} className="text-zinc-500 hover:text-white"><X size={18} /></button>
        </div>
        <div className="p-5">
          <p className="text-xs text-zinc-400 mb-4">Select tags to add to multiple virtual keys at once.</p>
          <div className="space-y-2 max-h-48 overflow-y-auto">
            {tags.map(tag => (
              <button key={tag.id} onClick={() => toggleTag(tag.id)}
                className={`w-full flex items-center gap-3 px-3 py-2.5 rounded-lg border text-left transition-all ${
                  selectedTags.includes(tag.id)
                    ? 'bg-emerald-500/10 border-emerald-500/30 text-emerald-300'
                    : 'bg-zinc-950 border-zinc-800 text-zinc-400 hover:border-zinc-700'
                }`}>
                <div className="w-3 h-3 rounded-full shrink-0" style={{ backgroundColor: tag.color }} />
                <span className="text-sm">{tag.name}</span>
                {selectedTags.includes(tag.id) && <Check size={14} className="ml-auto text-emerald-400" />}
              </button>
            ))}
          </div>
        </div>
        <div className="flex justify-end gap-3 px-5 py-4 border-t border-zinc-800/50">
          <button onClick={onClose} className="px-4 py-2 text-sm text-zinc-400 hover:text-white">Cancel</button>
          <button onClick={applyTags} disabled={selectedTags.length === 0}
            className="px-4 py-2 text-sm bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-white rounded-lg flex items-center gap-2">
            <Check size={14} /> Apply to selected
          </button>
        </div>
      </div>
    </div>
  )
}

export default function TagManagement() {
  const [tags, setTags] = useState<TagDef[]>(SEED_TAGS)
  const [showCreate, setShowCreate] = useState(false)
  const [editing, setEditing] = useState<TagDef | null>(null)
  const [deleting, setDeleting] = useState<TagDef | null>(null)
  const [viewing, setViewing] = useState<TagDef | null>(null)
  const [showBulk, setShowBulk] = useState(false)
  const [search, setSearch] = useState('')

  const totalUses = useMemo(() => tags.reduce((sum, t) => sum + t.vkCount + t.logCount, 0), [tags])

  const filtered = useMemo(() => {
    if (!search) return tags
    const q = search.toLowerCase()
    return tags.filter(t => t.name.includes(q))
  }, [tags, search])

  const handleCreate = (name: string, color: string) => {
    const newTag: TagDef = {
      id: Date.now().toString(),
      name,
      color,
      vkCount: 0,
      logCount: 0,
      vkNames: [],
      logSnippets: [],
    }
    setTags(prev => [...prev, newTag])
    setShowCreate(false)
  }

  const handleUpdate = (name: string, color: string) => {
    if (!editing) return
    setTags(prev => prev.map(t => t.id === editing.id ? { ...t, name, color } : t))
    setEditing(null)
  }

  const handleDelete = () => {
    if (!deleting) return
    setTags(prev => prev.filter(t => t.id !== deleting.id))
    setDeleting(null)
  }

  return (
    <div className="p-6 space-y-6 overflow-y-auto h-full">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white">Tag Management</h1>
          <p className="text-sm text-zinc-400 mt-1">{tags.length} tags &middot; {totalUses} total uses across VKs and logs</p>
        </div>
        <div className="flex items-center gap-3">
          <button onClick={() => setShowBulk(true)}
            className="flex items-center gap-2 px-3 py-2 text-sm bg-zinc-950 hover:bg-zinc-900 border border-zinc-800 text-zinc-300 rounded-lg transition-colors">
            <CheckSquare size={14} /> Bulk tag
          </button>
          <button onClick={() => setShowCreate(true)}
            className="flex items-center gap-2 px-4 py-2 bg-emerald-600 hover:bg-emerald-500 active:scale-[0.98] text-white text-sm rounded-lg">
            <Plus size={15} /> Create tag
          </button>
        </div>
      </div>

      <div className="flex items-center gap-3">
        <div className="relative flex-1 max-w-xs">
          <input type="text" value={search} onChange={e => setSearch(e.target.value)}
            placeholder="Search tags…"
            className="w-full bg-zinc-950 border border-zinc-800 rounded-lg pl-3 pr-8 py-2 text-sm text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-emerald-500/50" />
          {search && (
            <button onClick={() => setSearch('')} className="absolute right-2.5 top-1/2 -translate-y-1/2 text-zinc-500 hover:text-zinc-300">
              <X size={14} />
            </button>
          )}
        </div>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
        {filtered.map(tag => (
          <div key={tag.id}
            className="bg-zinc-900/30 border border-zinc-800/50 rounded-xl p-4 transition-all hover:border-zinc-700/50 group">
            <div className="flex items-center justify-between mb-3">
              <div className="flex items-center gap-2.5">
                <div className="w-3 h-3 rounded-full shrink-0" style={{ backgroundColor: tag.color }} />
                <span className="font-medium text-white text-sm">{tag.name}</span>
              </div>
              <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-all">
                <button onClick={() => setViewing(tag)}
                  className="p-1.5 text-zinc-500 hover:text-blue-400 hover:bg-blue-400/10 rounded-lg" title="View usage">
                  <FileText size={13} />
                </button>
                <button onClick={() => setEditing(tag)}
                  className="p-1.5 text-zinc-500 hover:text-emerald-400 hover:bg-emerald-400/10 rounded-lg" title="Edit tag">
                  <Pencil size={13} />
                </button>
                <button onClick={() => setDeleting(tag)}
                  className="p-1.5 text-zinc-500 hover:text-red-400 hover:bg-red-400/10 rounded-lg" title="Delete tag">
                  <Trash2 size={13} />
                </button>
              </div>
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="bg-zinc-950/50 border border-zinc-800/50 rounded-lg px-3 py-2">
                <div className="flex items-center gap-1.5 text-[10px] text-zinc-500 uppercase tracking-wider mb-0.5">
                  <KeyRound size={10} /> VKs
                </div>
                <div className="text-lg font-semibold text-white">{tag.vkCount}</div>
              </div>
              <div className="bg-zinc-950/50 border border-zinc-800/50 rounded-lg px-3 py-2">
                <div className="flex items-center gap-1.5 text-[10px] text-zinc-500 uppercase tracking-wider mb-0.5">
                  <FileText size={10} /> Logs
                </div>
                <div className="text-lg font-semibold text-white">{tag.logCount}</div>
              </div>
            </div>
          </div>
        ))}
      </div>

      {filtered.length === 0 && (
        <div className="text-center py-16 text-zinc-600">
          {search ? 'No tags match your search' : 'No tags — create one to get started'}
        </div>
      )}

      {showCreate && (
        <TagModal initial={{ name: '', color: '#3b82f6' }} isEdit={false}
          onClose={() => setShowCreate(false)} onSave={handleCreate} saving={false} />
      )}
      {editing && (
        <TagModal initial={{ name: editing.name, color: editing.color }} isEdit={true}
          onClose={() => setEditing(null)} onSave={handleUpdate} saving={false} />
      )}
      {deleting && (
        <DeleteTagModal tag={deleting} onClose={() => setDeleting(null)} onConfirm={handleDelete} deleting={false} />
      )}
      {viewing && (
        <UsageDetailModal tag={viewing} onClose={() => setViewing(null)} />
      )}
      {showBulk && (
        <BulkTagModal tags={tags} onClose={() => setShowBulk(false)} />
      )}
    </div>
  )
}
