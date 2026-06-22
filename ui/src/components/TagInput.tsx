import { useState, type KeyboardEvent } from 'react'
import { X } from 'lucide-react'

export default function TagInput({ tags, onChange }: { tags: string[]; onChange: (t: string[]) => void }) {
  const [input, setInput] = useState('')

  const addTag = () => {
    const val = input.trim()
    if (val && !tags.includes(val)) {
      onChange([...tags, val])
    }
    setInput('')
  }

  const handleKey = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Enter' || e.key === ',') {
      e.preventDefault()
      addTag()
    } else if (e.key === 'Tab' && input.trim()) {
      e.preventDefault()
      addTag()
    }
  }

  const removeTag = (tag: string) => onChange(tags.filter(t => t !== tag))

  return (
    <div className="flex flex-wrap items-center gap-1.5 bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 min-h-[38px] focus-within:border-emerald-500/50">
      {tags.map(tag => (
        <span key={tag} className="flex items-center gap-1 text-xs bg-zinc-800 text-zinc-200 px-2 py-0.5 rounded-full">
          {tag}
          <button type="button" onClick={() => removeTag(tag)} className="text-zinc-500 hover:text-white"><X size={11} /></button>
        </span>
      ))}
      <input
        type="text"
        value={input}
        onChange={e => setInput(e.target.value)}
        onKeyDown={handleKey}
        onBlur={addTag}
        placeholder={tags.length ? '' : 'Add tag (Enter)'}
        className="flex-1 min-w-[80px] bg-transparent text-sm text-zinc-200 placeholder-zinc-600 focus:outline-none"
      />
    </div>
  )
}
