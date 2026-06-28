import { useState } from 'react'
import { Puzzle, Download, Trash2, Settings, Check, Plus, X, RotateCcw, AlertTriangle, Search } from 'lucide-react'

// ─── Types ────────────────────────────────────────────────────────────────────

interface PluginConfig {
  [key: string]: string
}

interface Plugin {
  id: string
  name: string
  description: string
  version: string
  author: string
  installed: boolean
  enabled: boolean
  config: PluginConfig
}

// ─── Seed Data ────────────────────────────────────────────────────────────────

const INITIAL_PLUGINS: Plugin[] = [
  {
    id: 'pylos-gateway',
    name: 'Pylos Gateway Plugin',
    description: 'Routes Claude Code requests through the Pylos gateway for centralized management and analytics.',
    version: '1.2.0',
    author: 'Pylos Team',
    installed: true,
    enabled: true,
    config: {
      gateway_url: 'https://gateway.pylos.dev',
      api_key: '',
      log_requests: 'true',
      max_retries: '3',
    },
  },
  {
    id: 'memory-bridge',
    name: 'Memory Bridge',
    description: 'Synchronizes Claude Code conversation memory across sessions and team members.',
    version: '0.9.0',
    author: 'Pylos Team',
    installed: true,
    enabled: true,
    config: {
      storage_backend: 'sqlite',
      sync_interval: '300',
      max_memory_mb: '512',
    },
  },
  {
    id: 'rag-connector',
    name: 'RAG Connector',
    description: 'Retrieval-augmented generation plugin for indexing and querying project documentation.',
    version: '1.0.0',
    author: 'Community',
    installed: false,
    enabled: false,
    config: {},
  },
  {
    id: 'cost-tracker',
    name: 'Cost Tracker',
    description: 'Tracks API usage costs per session with budget alerts and spending reports.',
    version: '0.8.0',
    author: 'Community',
    installed: false,
    enabled: false,
    config: {},
  },
  {
    id: 'guardrails-enforcer',
    name: 'Guardrails Enforcer',
    description: 'Applies safety guardrails and content filtering to Claude Code outputs in real time.',
    version: '1.1.0',
    author: 'Pylos Team',
    installed: false,
    enabled: false,
    config: {},
  },
]

// ─── ConfigModal ──────────────────────────────────────────────────────────────

function ConfigModal({
  plugin,
  onClose,
  onSave,
}: {
  plugin: Plugin
  onClose: () => void
  onSave: (id: string, config: PluginConfig) => void
}) {
  const [config, setConfig] = useState<PluginConfig>({ ...plugin.config })
  const [newKey, setNewKey] = useState('')

  const setEntry = (key: string, value: string) =>
    setConfig(c => ({ ...c, [key]: value }))

  const removeEntry = (key: string) =>
    setConfig(c => {
      const next = { ...c }
      delete next[key]
      return next
    })

  const addEntry = () => {
    if (newKey.trim() && !(newKey in config)) {
      setConfig(c => ({ ...c, [newKey.trim()]: '' }))
      setNewKey('')
    }
  }

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-lg mx-4 max-h-[90vh] overflow-y-auto">
        <div className="flex items-center justify-between p-5 border-b border-zinc-800/50">
          <div className="flex items-center gap-3">
            <div className="w-9 h-9 rounded-lg flex items-center justify-center bg-zinc-800/50">
              <Settings size={16} className="text-zinc-400" />
            </div>
            <div>
              <h2 className="text-lg font-semibold text-white">{plugin.name}</h2>
              <div className="text-xs text-zinc-500">Configuration</div>
            </div>
          </div>
          <button onClick={onClose} className="text-zinc-500 hover:text-white transition-colors">
            <X size={18} />
          </button>
        </div>

        <div className="p-5 space-y-4">
          {Object.entries(config).map(([key, value]) => (
            <div key={key} className="bg-zinc-950/50 border border-zinc-800/50 rounded-lg p-3 space-y-2">
              <div className="grid grid-cols-5 gap-2 items-end">
                <div className="col-span-2">
                  <label className="block text-xs text-zinc-500 mb-1">Key</label>
                  <input
                    value={key}
                    disabled
                    className="w-full bg-zinc-950 border border-zinc-800 rounded px-2 py-1.5 text-xs text-zinc-400
                      font-mono disabled:opacity-60"
                  />
                </div>
                <div className="col-span-2">
                  <label className="block text-xs text-zinc-500 mb-1">Value</label>
                  <input
                    value={value}
                    onChange={e => setEntry(key, e.target.value)}
                    className="w-full bg-zinc-950 border border-zinc-800 rounded px-2 py-1.5 text-xs text-zinc-200
                      font-mono focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
                  />
                </div>
                <div className="flex justify-end">
                  <button
                    onClick={() => removeEntry(key)}
                    className="p-1.5 text-zinc-500 hover:text-red-400 hover:bg-red-400/10 rounded-lg transition-colors"
                    title="Remove"
                  >
                    <Trash2 size={13} />
                  </button>
                </div>
              </div>
            </div>
          ))}

          <div className="flex items-center gap-2">
            <input
              value={newKey}
              onChange={e => setNewKey(e.target.value)}
              onKeyDown={e => e.key === 'Enter' && addEntry()}
              placeholder="new_key"
              className="flex-1 bg-zinc-950 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-200
                font-mono focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
            />
            <button
              onClick={addEntry}
              disabled={!newKey.trim() || newKey in config}
              className="px-3 py-2 text-sm bg-zinc-800 hover:bg-zinc-700 disabled:opacity-40
                disabled:cursor-not-allowed text-zinc-300 rounded-lg flex items-center gap-1.5 transition-colors"
            >
              <Plus size={14} />
              Add
            </button>
          </div>

          {Object.keys(config).length === 0 && (
            <div className="text-center py-6 text-zinc-600 text-sm">
              No configuration entries — add one above
            </div>
          )}
        </div>

        <div className="flex justify-end gap-3 px-5 py-4 border-t border-zinc-800/50">
          <button
            onClick={onClose}
            className="px-4 py-2 text-sm text-zinc-400 hover:text-white transition-colors"
          >
            Cancel
          </button>
          <button
            onClick={() => onSave(plugin.id, config)}
            className="px-4 py-2 text-sm bg-emerald-600 hover:bg-emerald-500 active:scale-[0.98]
              text-white rounded-lg flex items-center gap-2 transition-colors"
          >
            <Check size={14} />
            Save
          </button>
        </div>
      </div>
    </div>
  )
}

// ─── InstallConfirmModal ──────────────────────────────────────────────────────

function InstallConfirmModal({
  plugin,
  onClose,
  onConfirm,
  isInstalling,
}: {
  plugin: Plugin
  onClose: () => void
  onConfirm: () => void
  isInstalling: boolean
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-sm mx-4 p-6">
        <div className="flex items-center gap-3 mb-4">
          <div className="w-9 h-9 rounded-full bg-emerald-500/15 flex items-center justify-center">
            <Download size={16} className="text-emerald-400" />
          </div>
          <div>
            <div className="font-semibold text-white">Install plugin</div>
            <div className="text-xs text-zinc-500">v{plugin.version}</div>
          </div>
        </div>
        <p className="text-sm text-zinc-400 mb-5">
          Install <span className="text-white font-medium">{plugin.name}</span> by {plugin.author}?
        </p>
        <div className="flex justify-end gap-3">
          <button onClick={onClose} className="px-4 py-2 text-sm text-zinc-400 hover:text-white">Cancel</button>
          <button
            onClick={onConfirm}
            disabled={isInstalling}
            className="px-4 py-2 text-sm bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-white rounded-lg
              flex items-center gap-2 transition-colors active:scale-[0.98]"
          >
            {isInstalling ? <RotateCcw size={13} className="animate-spin" /> : <Download size={13} />}
            Install
          </button>
        </div>
      </div>
    </div>
  )
}

// ─── UninstallConfirmModal ────────────────────────────────────────────────────

function UninstallConfirmModal({
  plugin,
  onClose,
  onConfirm,
  isUninstalling,
}: {
  plugin: Plugin
  onClose: () => void
  onConfirm: () => void
  isUninstalling: boolean
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-zinc-900 border border-zinc-800 rounded-2xl shadow-xl w-full max-w-sm mx-4 p-6">
        <div className="flex items-center gap-3 mb-4">
          <div className="w-9 h-9 rounded-full bg-red-500/15 flex items-center justify-center">
            <AlertTriangle size={16} className="text-red-400" />
          </div>
          <div>
            <div className="font-semibold text-white">Uninstall plugin</div>
            <div className="text-xs text-zinc-500">This can be reinstalled later</div>
          </div>
        </div>
        <p className="text-sm text-zinc-400 mb-5">
          Remove <span className="text-white font-medium">{plugin.name}</span> from Claude Code?
          Its configuration will be lost.
        </p>
        <div className="flex justify-end gap-3">
          <button onClick={onClose} className="px-4 py-2 text-sm text-zinc-400 hover:text-white">Cancel</button>
          <button
            onClick={onConfirm}
            disabled={isUninstalling}
            className="px-4 py-2 text-sm bg-red-600 hover:bg-red-500 disabled:opacity-50 text-white rounded-lg
              flex items-center gap-2 transition-colors active:scale-[0.98]"
          >
            {isUninstalling ? <RotateCcw size={13} className="animate-spin" /> : <Trash2 size={13} />}
            Uninstall
          </button>
        </div>
      </div>
    </div>
  )
}

// ─── PluginCard ───────────────────────────────────────────────────────────────

function PluginCard({
  plugin,
  onInstall,
  onUninstall,
  onConfigure,
  onToggleEnabled,
}: {
  plugin: Plugin
  onInstall: () => void
  onUninstall: () => void
  onConfigure: () => void
  onToggleEnabled: () => void
}) {
  return (
    <div className="rounded-xl border border-zinc-800/50 bg-zinc-900/30 p-5 hover:border-zinc-700/50 transition-colors group">
      <div className="flex items-center gap-3 mb-3">
        <div className="w-9 h-9 rounded-lg flex items-center justify-center bg-zinc-800/50">
          <Puzzle size={16} className={plugin.installed ? 'text-emerald-400' : 'text-zinc-400'} />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="font-semibold text-white truncate">{plugin.name}</span>
            {plugin.installed && (
              <span className="inline-flex items-center gap-1 px-1.5 py-0.5 rounded text-[10px] font-medium bg-emerald-400/15 text-emerald-400">
                <Check size={10} />
                Installed
              </span>
            )}
          </div>
          <div className="text-xs text-zinc-500">
            v{plugin.version} &middot; {plugin.author}
          </div>
        </div>
      </div>

      <p className="text-sm text-zinc-400 mb-4 line-clamp-2">{plugin.description}</p>

      <div className="flex items-center gap-2 pt-3 border-t border-zinc-800/50">
        {plugin.installed ? (
          <>
            <button
              onClick={onToggleEnabled}
              className={`flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs transition-colors
                ${plugin.enabled
                  ? 'text-amber-400 bg-amber-400/10 hover:bg-amber-400/20'
                  : 'text-emerald-400 bg-emerald-400/10 hover:bg-emerald-400/20'
                }`}
              title={plugin.enabled ? 'Disable' : 'Enable'}
            >
              {plugin.enabled ? 'Enabled' : 'Disabled'}
            </button>
            <button
              onClick={onConfigure}
              className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs text-zinc-400
                bg-zinc-800/50 hover:bg-zinc-800 hover:text-white transition-colors"
            >
              <Settings size={12} />
              Configure
            </button>
            <div className="ml-auto">
              <button
                onClick={onUninstall}
                className="opacity-0 group-hover:opacity-100 p-1.5 text-zinc-500 hover:text-red-400
                  hover:bg-red-400/10 rounded-lg transition-all"
                title="Uninstall"
              >
                <Trash2 size={13} />
              </button>
            </div>
          </>
        ) : (
          <button
            onClick={onInstall}
            className="flex items-center gap-1.5 px-3 py-1.5 rounded-lg text-xs text-emerald-400
              bg-emerald-400/10 hover:bg-emerald-400/20 transition-colors"
          >
            <Download size={12} />
            Install
          </button>
        )}
      </div>
    </div>
  )
}

// ─── Main Page ────────────────────────────────────────────────────────────────

export default function ClaudePlugins() {
  const [plugins, setPlugins] = useState<Plugin[]>(INITIAL_PLUGINS)
  const [search, setSearch] = useState('')
  const [tab, setTab] = useState<'installed' | 'available'>('installed')
  const [configuringPlugin, setConfiguringPlugin] = useState<Plugin | null>(null)
  const [installingPlugin, setInstallingPlugin] = useState<Plugin | null>(null)
  const [uninstallingPlugin, setUninstallingPlugin] = useState<Plugin | null>(null)

  const installed = plugins.filter(p => p.installed)
  const available = plugins.filter(p => !p.installed)

  const filteredInstalled = installed.filter(p =>
    p.name.toLowerCase().includes(search.toLowerCase()) ||
    p.description.toLowerCase().includes(search.toLowerCase())
  )

  const filteredAvailable = available.filter(p =>
    p.name.toLowerCase().includes(search.toLowerCase()) ||
    p.description.toLowerCase().includes(search.toLowerCase())
  )

  const displayPlugins = tab === 'installed' ? filteredInstalled : filteredAvailable

  const toggleEnabled = (id: string) =>
    setPlugins(ps =>
      ps.map(p => (p.id === id ? { ...p, enabled: !p.enabled } : p))
    )

  const handleInstall = (id: string) => {
    setPlugins(ps =>
      ps.map(p =>
        p.id === id
          ? { ...p, installed: true, enabled: true, config: {} }
          : p
      )
    )
    setInstallingPlugin(null)
  }

  const handleUninstall = (id: string) => {
    setPlugins(ps =>
      ps.map(p =>
        p.id === id
          ? { ...p, installed: false, enabled: false, config: {} }
          : p
      )
    )
    setUninstallingPlugin(null)
  }

  const handleSaveConfig = (id: string, config: PluginConfig) => {
    setPlugins(ps =>
      ps.map(p => (p.id === id ? { ...p, config } : p))
    )
    setConfiguringPlugin(null)
  }

  return (
    <div className="p-6 space-y-6">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white">Claude Code Plugins</h1>
          <p className="text-sm text-zinc-400 mt-1">
            {installed.length} installed &middot; {available.length} available
          </p>
        </div>
      </div>

      {/* Tabs & Search */}
      <div className="flex items-center gap-4">
        <div className="flex rounded-lg bg-zinc-900/50 border border-zinc-800/50 p-0.5">
          <button
            onClick={() => setTab('installed')}
            className={`px-4 py-1.5 text-sm rounded-md transition-colors ${
              tab === 'installed'
                ? 'bg-zinc-800 text-white'
                : 'text-zinc-500 hover:text-zinc-300'
            }`}
          >
            Installed ({installed.length})
          </button>
          <button
            onClick={() => setTab('available')}
            className={`px-4 py-1.5 text-sm rounded-md transition-colors ${
              tab === 'available'
                ? 'bg-zinc-800 text-white'
                : 'text-zinc-500 hover:text-zinc-300'
            }`}
          >
            Marketplace ({available.length})
          </button>
        </div>

        <div className="relative flex-1 max-w-sm">
          <Search size={14} className="absolute left-3 top-1/2 -translate-y-1/2 text-zinc-500" />
          <input
            value={search}
            onChange={e => setSearch(e.target.value)}
            placeholder="Search plugins…"
            className="w-full bg-zinc-900/50 border border-zinc-800/50 rounded-lg pl-9 pr-3 py-2 text-sm text-zinc-200
              placeholder:text-zinc-600 focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
          />
        </div>
      </div>

      {/* Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 xl:grid-cols-3 gap-4">
        {displayPlugins.map(p => (
          <PluginCard
            key={p.id}
            plugin={p}
            onInstall={() => setInstallingPlugin(p)}
            onUninstall={() => setUninstallingPlugin(p)}
            onConfigure={() => setConfiguringPlugin(p)}
            onToggleEnabled={() => toggleEnabled(p.id)}
          />
        ))}
        {displayPlugins.length === 0 && (
          <div className="col-span-full text-center py-16 text-zinc-600">
            {tab === 'installed'
              ? 'No installed plugins — browse the marketplace to add one'
              : 'No available plugins match your search'}
          </div>
        )}
      </div>

      {/* Config modal */}
      {configuringPlugin && (
        <ConfigModal
          plugin={configuringPlugin}
          onClose={() => setConfiguringPlugin(null)}
          onSave={handleSaveConfig}
        />
      )}

      {/* Install confirm */}
      {installingPlugin && (
        <InstallConfirmModal
          plugin={installingPlugin}
          onClose={() => setInstallingPlugin(null)}
          onConfirm={() => handleInstall(installingPlugin.id)}
          isInstalling={false}
        />
      )}

      {/* Uninstall confirm */}
      {uninstallingPlugin && (
        <UninstallConfirmModal
          plugin={uninstallingPlugin}
          onClose={() => setUninstallingPlugin(null)}
          onConfirm={() => handleUninstall(uninstallingPlugin.id)}
          isUninstalling={false}
        />
      )}
    </div>
  )
}
