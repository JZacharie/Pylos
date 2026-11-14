import { RotateCcw, Trash2, AlertTriangle } from 'lucide-react'

export function DeleteConfirm({
  modelId,
  onClose,
  onConfirm,
  isDeleting,
}: {
  modelId: string
  onClose: () => void
  onConfirm: () => void
  isDeleting: boolean
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm">
      <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-sm mx-4 p-6">
        <div className="flex items-center gap-3 mb-4">
          <div className="w-9 h-9 rounded-full bg-red-500/15 flex items-center justify-center">
            <AlertTriangle size={16} className="text-red-400" />
          </div>
          <div>
            <div className="font-semibold text-white">Remove from catalog</div>
            <div className="text-xs text-zinc-500">This removes only the catalog entry</div>
          </div>
        </div>
        <p className="text-sm text-zinc-400 mb-5">
          Remove <span className="text-white font-mono text-xs">{modelId}</span> from the model catalog?
        </p>
        <div className="flex justify-end gap-3">
          <button onClick={onClose} className="px-4 py-2 text-sm text-zinc-400 hover:text-white">Cancel</button>
          <button
            onClick={onConfirm}
            disabled={isDeleting}
            className="px-4 py-2 text-sm bg-red-600 hover:bg-red-500 disabled:opacity-50 text-white rounded-lg
              flex items-center gap-2 active:scale-[0.98]"
          >
            {isDeleting ? <RotateCcw size={13} className="animate-spin" /> : <Trash2 size={13} />}
            Remove
          </button>
        </div>
      </div>
    </div>
  )
}

// ─── LLM Credentials Tab — Re-use credentials from providers ─────────────────

