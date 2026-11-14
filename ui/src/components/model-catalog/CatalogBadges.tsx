import type { ReactNode } from 'react'

export function CapBadge({ ok, label }: { ok: boolean; label: string }) {
  return (
    <span className={`px-1.5 py-0.5 rounded text-[10px] font-medium ${
      ok ? 'bg-emerald-500/15 text-emerald-400 border border-emerald-500/25'
         : 'bg-zinc-800/60 text-zinc-600 border border-zinc-800'
    }`}>
      {label}
    </span>
  )
}

export const ProviderField = ({ label, children }: { label: string; children: ReactNode }) => (
  <div className="space-y-1.5">
    <label className="text-xs font-medium text-zinc-400">{label}</label>
    {children}
  </div>
)

export function ModelFormToggle({ checked, onToggle }: { checked: boolean; onToggle: () => void }) {
  return (
    <button
      type="button"
      onClick={onToggle}
      className={`w-9 h-5 rounded-full transition-colors relative flex-shrink-0 ${checked ? 'bg-emerald-600' : 'bg-zinc-700'}`}
    >
      <span className={`absolute top-0.5 left-0.5 w-4 h-4 rounded-full bg-white transition-transform ${checked ? 'translate-x-4' : 'translate-x-0'}`} />
    </button>
  )
}
