import { useState } from 'react'
import {
  Palette,
  Sun,
  Moon,
  Monitor,
  Type,
  PanelLeft,
  PanelRight,
  Zap,
  Save,
  RotateCcw,
  Check,
} from 'lucide-react'

const STORAGE_KEY = 'pylos_theme_settings'

type ThemeChoice = 'dark' | 'light' | 'system'
type AccentChoice = 'emerald' | 'blue' | 'purple' | 'orange' | 'red'
type FontChoice = 'small' | 'medium' | 'large'
type SidebarPos = 'left' | 'right'

interface ThemeSettings {
  theme: ThemeChoice
  accent: AccentChoice
  fontSize: FontChoice
  sidebarPosition: SidebarPos
  sidebarCollapsed: boolean
  compactMode: boolean
  animationsEnabled: boolean
}

const DEFAULTS: ThemeSettings = {
  theme: 'dark',
  accent: 'emerald',
  fontSize: 'medium',
  sidebarPosition: 'left',
  sidebarCollapsed: false,
  compactMode: false,
  animationsEnabled: true,
}

function loadSettings(): ThemeSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    if (!raw) return { ...DEFAULTS }
    return { ...DEFAULTS, ...JSON.parse(raw) }
  } catch {
    return { ...DEFAULTS }
  }
}

function saveSettingsToStorage(settings: ThemeSettings) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(settings))
}

const THEME_OPTIONS: { value: ThemeChoice; label: string; icon: typeof Moon }[] = [
  { value: 'dark', label: 'Dark', icon: Moon },
  { value: 'light', label: 'Light', icon: Sun },
  { value: 'system', label: 'System', icon: Monitor },
]

const ACCENT_COLORS: { value: AccentChoice; label: string; hex: string }[] = [
  { value: 'emerald', label: 'Emerald', hex: '#10b981' },
  { value: 'blue', label: 'Blue', hex: '#3b82f6' },
  { value: 'purple', label: 'Purple', hex: '#a855f7' },
  { value: 'orange', label: 'Orange', hex: '#f97316' },
  { value: 'red', label: 'Red', hex: '#ef4444' },
]

const FONT_OPTIONS: { value: FontChoice; label: string; size: string }[] = [
  { value: 'small', label: 'Small', size: '12px' },
  { value: 'medium', label: 'Medium', size: '14px' },
  { value: 'large', label: 'Large', size: '16px' },
]

const SIDEBAR_OPTIONS: { value: SidebarPos; label: string; icon: typeof PanelLeft }[] = [
  { value: 'left', label: 'Left', icon: PanelLeft },
  { value: 'right', label: 'Right', icon: PanelRight },
]

function Toggle({ enabled, onChange }: { enabled: boolean; onChange: () => void }) {
  return (
    <button
      onClick={onChange}
      className={`relative w-10 h-5 rounded-full transition-colors ${
        enabled ? 'bg-emerald-500' : 'bg-zinc-700'
      }`}
    >
      <div
        className={`absolute top-0.5 w-4 h-4 rounded-full bg-white transition-transform ${
          enabled ? 'translate-x-5' : 'translate-x-0.5'
        }`}
      />
    </button>
  )
}

function SectionCard({
  title,
  icon,
  children,
}: {
  title: string
  icon: React.ReactNode
  children: React.ReactNode
}) {
  return (
    <div className="rounded-xl border border-zinc-800/50 bg-zinc-900/30 p-5">
      <div className="flex items-center gap-2 mb-4">
        {icon}
        <h2 className="text-sm font-semibold text-zinc-300">{title}</h2>
      </div>
      <div className="space-y-4">{children}</div>
    </div>
  )
}

function PreviewSection({ settings }: { settings: ThemeSettings }) {
  const accentHex = ACCENT_COLORS.find(c => c.value === settings.accent)?.hex || '#10b981'
  const fontSize = FONT_OPTIONS.find(f => f.value === settings.fontSize)?.size || '14px'
  const isDark = settings.theme === 'dark' || (settings.theme === 'system' && true)

  const bgClass = isDark ? 'bg-zinc-950' : 'bg-white'
  const textClass = isDark ? 'text-white' : 'text-zinc-900'
  const mutedClass = isDark ? 'text-zinc-400' : 'text-zinc-500'
  const borderClass = isDark ? 'border-zinc-800' : 'border-zinc-200'
  const cardBg = isDark ? 'bg-zinc-900/30' : 'bg-zinc-50'

  return (
    <div className="rounded-xl border border-zinc-800/50 bg-zinc-900/30 p-5">
      <div className="flex items-center gap-2 mb-4">
        <Zap size={15} className="text-amber-400" />
        <h2 className="text-sm font-semibold text-zinc-300">Preview</h2>
      </div>

      <div
        className={`rounded-lg border ${borderClass} ${bgClass} p-4 space-y-3`}
        style={{ fontSize }}
      >
        <div className="flex gap-3">
          <div
            className={`w-16 h-24 rounded-md ${cardBg} border ${borderClass} flex flex-col items-center justify-center gap-1.5`}
          >
            <div className="w-6 h-1 rounded-full" style={{ backgroundColor: accentHex }} />
            <div className="w-8 h-1 rounded-full bg-zinc-600/40" />
            <div className="w-6 h-1 rounded-full bg-zinc-600/40" />
          </div>

          <div className="flex-1 space-y-2">
            <div className="flex items-center gap-2">
              <div
                className="w-3 h-3 rounded-full"
                style={{ backgroundColor: accentHex }}
              />
              <span className={`font-semibold ${textClass}`}>Dashboard</span>
            </div>

            <div className={`text-xs ${mutedClass}`}>
              {settings.compactMode ? 'Compact layout' : 'Standard layout'}
              {settings.sidebarCollapsed ? ' · Sidebar collapsed' : ''}
              {settings.theme === 'system' ? ' · System theme' : ` · ${settings.theme} theme`}
            </div>

            <div className="flex gap-2">
              <div
                className="px-3 py-1 rounded-md text-white text-xs font-medium"
                style={{ backgroundColor: accentHex }}
              >
                Primary
              </div>
              <div className={`px-3 py-1 rounded-md border text-xs ${borderClass} ${textClass}`}>
                Secondary
              </div>
            </div>

            <div className={`flex items-center gap-1.5 text-xs ${mutedClass}`}>
              <Zap size={11} style={{ color: accentHex }} />
              <span>Accent elements adapt to your color choice</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}

export default function UiTheme() {
  const [settings, setSettings] = useState<ThemeSettings>(loadSettings)
  const [saved, setSaved] = useState(false)
  const [hasChanges, setHasChanges] = useState(false)

  const update = <K extends keyof ThemeSettings>(key: K, value: ThemeSettings[K]) => {
    setSettings(s => ({ ...s, [key]: value }))
    setHasChanges(true)
    setSaved(false)
  }

  const handleSave = () => {
    saveSettingsToStorage(settings)
    setSaved(true)
    setHasChanges(false)
    setTimeout(() => setSaved(false), 2500)
  }

  const handleReset = () => {
    setSettings({ ...DEFAULTS })
    setSaved(false)
    setHasChanges(true)
  }

  return (
    <div className="p-6 space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-white flex items-center gap-3">
            <Palette size={24} className="text-emerald-400" />
            UI Theme Settings
          </h1>
          <p className="text-sm text-zinc-400 mt-1">Customize the dashboard appearance</p>
        </div>
        <div className="flex items-center gap-3">
          <button
            onClick={handleReset}
            className="flex items-center gap-2 px-3 py-2 text-sm text-zinc-400 hover:text-white
              border border-zinc-800 hover:border-zinc-700 rounded-lg transition-colors"
          >
            <RotateCcw size={14} />
            Reset
          </button>
          <button
            onClick={handleSave}
            disabled={!hasChanges && !saved}
            className={`flex items-center gap-2 px-4 py-2 text-sm rounded-lg font-medium transition-all ${
              saved
                ? 'bg-emerald-600 text-white'
                : hasChanges
                  ? 'bg-emerald-600 hover:bg-emerald-500 text-white'
                  : 'bg-zinc-800 text-zinc-500 cursor-not-allowed'
            }`}
          >
            {saved ? <Check size={14} /> : <Save size={14} />}
            {saved ? 'Saved' : 'Save'}
          </button>
        </div>
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
        {/* Theme */}
        <SectionCard title="Theme" icon={<Sun size={15} className="text-yellow-400" />}>
          <div className="flex gap-2">
            {THEME_OPTIONS.map(opt => {
              const Icon = opt.icon
              return (
                <button
                  key={opt.value}
                  onClick={() => update('theme', opt.value)}
                  className={`flex items-center gap-2 px-4 py-2.5 rounded-lg border text-sm transition-all ${
                    settings.theme === opt.value
                      ? 'bg-emerald-500/15 border-emerald-500/30 text-emerald-400'
                      : 'bg-zinc-950 border-zinc-800 text-zinc-400 hover:border-zinc-700 hover:text-zinc-300'
                  }`}
                >
                  <Icon size={14} />
                  {opt.label}
                </button>
              )
            })}
          </div>
        </SectionCard>

        {/* Accent Color */}
        <SectionCard title="Accent Color" icon={<Palette size={15} className="text-purple-400" />}>
          <div className="flex gap-3">
            {ACCENT_COLORS.map(c => (
              <button
                key={c.value}
                onClick={() => update('accent', c.value)}
                className={`group flex flex-col items-center gap-1.5`}
              >
                <div
                  className={`w-8 h-8 rounded-full border-2 transition-all flex items-center justify-center ${
                    settings.accent === c.value
                      ? 'border-white scale-110'
                      : 'border-zinc-800 hover:border-zinc-600'
                  }`}
                  style={{ backgroundColor: c.hex + '20' }}
                >
                  <div className="w-4 h-4 rounded-full" style={{ backgroundColor: c.hex }} />
                  {settings.accent === c.value && (
                    <Check size={10} className="text-white absolute" />
                  )}
                </div>
                <span className={`text-[10px] ${
                  settings.accent === c.value ? 'text-zinc-300 font-medium' : 'text-zinc-500'
                }`}>
                  {c.label}
                </span>
              </button>
            ))}
          </div>
        </SectionCard>

        {/* Font Size */}
        <SectionCard title="Font Size" icon={<Type size={15} className="text-blue-400" />}>
          <div className="flex gap-2">
            {FONT_OPTIONS.map(opt => (
              <button
                key={opt.value}
                onClick={() => update('fontSize', opt.value)}
                className={`flex items-center gap-2 px-4 py-2.5 rounded-lg border text-sm transition-all ${
                  settings.fontSize === opt.value
                    ? 'bg-emerald-500/15 border-emerald-500/30 text-emerald-400'
                    : 'bg-zinc-950 border-zinc-800 text-zinc-400 hover:border-zinc-700 hover:text-zinc-300'
                }`}
              >
                <span style={{ fontSize: opt.size }}>Aa</span>
                {opt.label}
              </button>
            ))}
          </div>
        </SectionCard>

        {/* Sidebar Position */}
        <SectionCard title="Sidebar Position" icon={<PanelLeft size={15} className="text-emerald-400" />}>
          <div className="flex gap-2">
            {SIDEBAR_OPTIONS.map(opt => {
              const Icon = opt.icon
              return (
                <button
                  key={opt.value}
                  onClick={() => update('sidebarPosition', opt.value)}
                  className={`flex items-center gap-2 px-4 py-2.5 rounded-lg border text-sm transition-all ${
                    settings.sidebarPosition === opt.value
                      ? 'bg-emerald-500/15 border-emerald-500/30 text-emerald-400'
                      : 'bg-zinc-950 border-zinc-800 text-zinc-400 hover:border-zinc-700 hover:text-zinc-300'
                  }`}
                >
                  <Icon size={14} />
                  {opt.label}
                </button>
              )
            })}
          </div>
        </SectionCard>

        {/* Sidebar Collapsed */}
        <SectionCard title="Sidebar Collapsed" icon={<PanelRight size={15} className="text-zinc-400" />}>
          <div className="flex items-center justify-between">
            <span className="text-sm text-zinc-300">Collapsed by default</span>
            <Toggle
              enabled={settings.sidebarCollapsed}
              onChange={() => update('sidebarCollapsed', !settings.sidebarCollapsed)}
            />
          </div>
        </SectionCard>

        {/* Compact Mode */}
        <SectionCard title="Compact Mode" icon={<Zap size={15} className="text-amber-400" />}>
          <div className="flex items-center justify-between">
            <span className="text-sm text-zinc-300">Enable compact mode</span>
            <Toggle
              enabled={settings.compactMode}
              onChange={() => update('compactMode', !settings.compactMode)}
            />
          </div>
        </SectionCard>

        {/* Animations */}
        <SectionCard title="Animations" icon={<Zap size={15} className="text-cyan-400" />}>
          <div className="flex items-center justify-between">
            <span className="text-sm text-zinc-300">Enable animations</span>
            <Toggle
              enabled={settings.animationsEnabled}
              onChange={() => update('animationsEnabled', !settings.animationsEnabled)}
            />
          </div>
        </SectionCard>
      </div>

      {/* Preview */}
      <PreviewSection settings={settings} />
    </div>
  )
}
