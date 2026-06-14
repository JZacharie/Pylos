import { NavLink } from 'react-router-dom'
import {
  ChevronLeft,
  ChevronRight,
  ChevronDown,
  ChevronUp,
  FlaskConical,
  BookOpen,
  AlertCircle,
  BarChart2,
  Shield,
  Users,
  User,
  Landmark,
  Layers,
  CreditCard,
  FileBadge,
  Wrench,
  Search,
  Database,
  ShieldCheck,
  LogOut,
  KeyRound,
  Server,
  Bot,
  Cpu,
  ShieldAlert,
  Code2,
  LayoutGrid,
  HardDrive,
  Terminal,
  Tag,
  Puzzle,
  History,
  Sliders,
  Bell,
  Settings,
  DollarSign,
  Palette,
  ScrollText,
} from 'lucide-react'
import { useState } from 'react'
import { useQuery } from '@tanstack/react-query'
import { healthApi, authApi } from '../lib/api'

function isAdmin(): boolean {
  const userJson = typeof window !== 'undefined' ? localStorage.getItem('pylos_user') : null;
  if (!userJson) {
    // Fallback: if admin key is set, consider as admin
    return !!localStorage.getItem('pylos_admin_key');
  }
  try {
    const user = JSON.parse(userJson);
    return user.role === 'admin';
  } catch {
    return false;
  }
}

function isPlaygroup(): boolean {
  const userJson = typeof window !== 'undefined' ? localStorage.getItem('pylos_user') : null;
  if (!userJson) return false;
  try {
    const user = JSON.parse(userJson);
    return user.group === 'playgroup';
  } catch {
    return false;
  }
}

// Admin-only items hidden for non-admin users
const AI_GATEWAY_CORE = [
  { to: '/keys',       icon: KeyRound,        label: 'Virtual Keys' },
  { to: '/playground', icon: FlaskConical,    label: 'Playground' },
  { to: '/models',     icon: Server,          label: 'Models + Endpoints' },
  { to: '/agents',     icon: Bot,             label: 'Agents' },
  { to: '/mcp-servers',icon: Cpu,             label: 'MCP Servers' },
  { to: '/policies',   icon: FileBadge,       label: 'Policies' },
]

const AI_GATEWAY_ADMIN = [
  { to: '/guardrails', icon: Shield,          label: 'Guardrails' },
]

const NAV_TOOLS = [
  { to: '/tools/search', icon: Search, label: 'Search Tools' },
  { to: '/tools/vector-stores', icon: Database, label: 'Vector Stores' },
  { to: '/tools/policies', icon: ShieldCheck, label: 'Tool Policies' },
]

const OBSERVABILITY = [
  { to: '/analytics',  icon: BarChart2,       label: 'Usage' },
  { to: '/logs',       icon: ScrollText,      label: 'Logs' },
  { to: '/guardrails-monitor', icon: ShieldAlert, label: 'Guardrails Monitor' },
]

const ACCESS_CONTROL = [
  { to: '/teams',          icon: Users,          label: 'Teams' },
  { to: '/users',          icon: User,           label: 'Internal Users' },
  { to: '/organizations',  icon: Landmark,       label: 'Organizations' },
  { to: '/access-groups',  icon: Layers,         label: 'Access Groups' },
  { to: '/budgets',        icon: CreditCard,     label: 'Budgets' },
]

const DEVELOPER_TOOLS = [
  { to: '/api-reference',     icon: Code2,          label: 'API Reference' },
  { to: '/ai-hub',            icon: LayoutGrid,     label: 'AI Hub' },
  { to: '/learning-resources',icon: BookOpen,       label: 'Learning Resources' },
]

const EXPERIMENTAL = [
  { to: '/experimental/caching',        icon: HardDrive,  label: 'Caching' },
  { to: '/experimental/prompts',        icon: Terminal,   label: 'Prompts' },
  { to: '/experimental/api-playground', icon: FlaskConical,label: 'API Playground' },
  { to: '/experimental/tag-management', icon: Tag,        label: 'Tag Management' },
  { to: '/experimental/claude-plugins', icon: Puzzle,     label: 'Claude Code Plugins' },
  { to: '/experimental/old-usage',      icon: History,    label: 'Old Usage' },
]

const SETTINGS_SUB = [
  { to: '/settings/router',         icon: Sliders,    label: 'Router Settings' },
  { to: '/settings/logging-alerts', icon: Bell,       label: 'Logging & Alerts' },
  { to: '/settings/admin',          icon: Settings,   label: 'Admin Settings', hasDot: true },
  { to: '/settings/cost-tracking',  icon: DollarSign, label: 'Cost Tracking' },
  { to: '/settings/ui-theme',       icon: Palette,    label: 'UI Theme' },
]

export function Sidebar({ isOpen, onClose }: { isOpen?: boolean; onClose?: () => void }) {
  const [collapsed, setCollapsed] = useState(false)
  const [toolsExpanded, setToolsExpanded] = useState(true)
  const [experimentalExpanded, setExperimentalExpanded] = useState(true)
  const [settingsExpanded, setSettingsExpanded] = useState(true)

  const userJson = typeof window !== 'undefined' ? localStorage.getItem('pylos_user') : null;
  const user = userJson ? JSON.parse(userJson) : null;

  const { data: health, isError } = useQuery({
    queryKey: ['health'],
    queryFn: healthApi.check,
    refetchInterval: 30_000,
    retry: 1,
  })

  const isHealthy = !isError && health !== undefined

  return (
    <>
      {/* Backdrop overlay for mobile drawer */}
      {isOpen && (
        <div
          className="fixed inset-0 z-40 bg-black/60 backdrop-blur-sm md:hidden transition-opacity duration-300"
          onClick={onClose}
        />
      )}

      <aside
        className={`fixed inset-y-0 left-0 z-50 flex flex-col h-screen bg-zinc-950 border-r border-zinc-800/50 transition-all duration-300 md:duration-200
          md:static md:translate-x-0
          ${isOpen ? 'translate-x-0 shadow-2xl' : '-translate-x-full'}
          ${collapsed ? 'md:w-16' : 'md:w-56'} w-64`}
      >
        {/* Logo */}
        <div className="flex items-center justify-between px-4 py-5 border-b border-zinc-800/50">
          <div className="flex items-center gap-3">
            <div className="flex-shrink-0 w-8 h-8 overflow-hidden rounded-lg bg-zinc-800">
              <img src="/logo.png" alt="Pylos" className="w-full h-full object-contain" />
            </div>
            {(!collapsed || isOpen) && (
              <span className="font-bold text-lg text-white">Pylos</span>
            )}
          </div>
          {isOpen && (
            <button
              onClick={onClose}
              className="md:hidden text-zinc-400 hover:text-white p-1 rounded-lg hover:bg-zinc-800/50"
            >
              <ChevronLeft size={20} />
            </button>
          )}
        </div>

        {/* Nav */}
        <nav className="flex-1 px-2 py-4 space-y-1 overflow-y-auto">
          
          {/* AI GATEWAY SECTION */}
          {(!collapsed || isOpen) ? (
            <div className="px-3 pt-2 pb-2 text-xs font-semibold text-zinc-500 uppercase tracking-wider">
              AI Gateway
            </div>
          ) : (
            <div className="mx-3 mt-2 mb-2 border-t border-zinc-800/50" />
          )}

          {[...AI_GATEWAY_CORE, ...(isAdmin() ? AI_GATEWAY_ADMIN : [])].map(({ to, icon: Icon, label }) => (
            <NavLink
              key={to}
              to={to}
              onClick={onClose}
              className={({ isActive }) =>
                `flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-colors
                ${isActive
                  ? 'bg-zinc-800 text-white'
                  : 'text-zinc-400 hover:bg-zinc-800/50 hover:text-zinc-100'
                }`
              }
            >
              {({ isActive }) => (
                <>
                  {isActive && <div className="w-1 h-1 rounded-full bg-emerald-500 flex-shrink-0" />}
                  <Icon size={18} className="flex-shrink-0" />
                  {(!collapsed || isOpen) && <span>{label}</span>}
                </>
              )}
            </NavLink>
          ))}

          {/* Tools Dropdown */}
          <div className="flex flex-col mt-1">
            <button
              onClick={() => {
                if (collapsed) setCollapsed(false);
                setToolsExpanded(!toolsExpanded);
              }}
              className="flex items-center justify-between px-3 py-2 rounded-lg text-sm transition-colors text-zinc-400 hover:bg-zinc-800/50 hover:text-zinc-100 w-full"
            >
              <div className="flex items-center gap-3">
                <Wrench size={18} className="flex-shrink-0" />
                {(!collapsed || isOpen) && <span>Tools</span>}
              </div>
              {(!collapsed || isOpen) && (
                toolsExpanded ? <ChevronUp size={16} className="opacity-70" /> : <ChevronDown size={16} className="opacity-70" />
              )}
            </button>
            {toolsExpanded && (!collapsed || isOpen) && (
              <div className="flex flex-col mt-1 ml-4 space-y-1 border-l border-zinc-800/50 pl-2">
                {NAV_TOOLS.map(({ to, icon: Icon, label }) => (
                  <NavLink
                    key={to}
                    to={to}
                    onClick={onClose}
                    className={({ isActive }) =>
                      `flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-colors
                      ${isActive
                        ? 'bg-zinc-800 text-white'
                        : 'text-zinc-400 hover:bg-zinc-800/50 hover:text-zinc-100'
                      }`
                    }
                  >
                    {({ isActive }) => (
                      <>
                        {isActive && <div className="w-1 h-1 rounded-full bg-emerald-500 flex-shrink-0 absolute -ml-4" />}
                        <Icon size={16} className="flex-shrink-0" />
                        <span>{label}</span>
                      </>
                    )}
                  </NavLink>
                ))}
              </div>
            )}
          </div>

          {/* OBSERVABILITY SECTION */}
          {(!collapsed || isOpen) ? (
            <div className="px-3 pt-6 pb-2 text-xs font-semibold text-zinc-500 uppercase tracking-wider">
              Observability
            </div>
          ) : (
            <div className="mx-3 mt-6 mb-2 border-t border-zinc-800/50" />
          )}

          {OBSERVABILITY.map(({ to, icon: Icon, label }) => (
            <NavLink
              key={to}
              to={to}
              onClick={onClose}
              className={({ isActive }) =>
                `flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-colors
                ${isActive
                  ? 'bg-zinc-800 text-white'
                  : 'text-zinc-400 hover:bg-zinc-800/50 hover:text-zinc-100'
                }`
              }
            >
              {({ isActive }) => (
                <>
                  {isActive && <div className="w-1 h-1 rounded-full bg-emerald-500 flex-shrink-0" />}
                  <Icon size={18} className="flex-shrink-0" />
                  {(!collapsed || isOpen) && <span>{label}</span>}
                </>
              )}
            </NavLink>
          ))}

          {/* ACCESS CONTROL SECTION */}
          {isAdmin() && <>
            {(!collapsed || isOpen) ? (
              <div className="px-3 pt-6 pb-2 text-xs font-semibold text-zinc-500 uppercase tracking-wider">
                Access Control
              </div>
            ) : (
              <div className="mx-3 mt-6 mb-2 border-t border-zinc-800/50" />
            )}

            {ACCESS_CONTROL.map(({ to, icon: Icon, label }) => (
              <NavLink
                key={to}
                to={to}
                onClick={onClose}
                className={({ isActive }) =>
                  `flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-colors
                  ${isActive
                    ? 'bg-zinc-800 text-white'
                    : 'text-zinc-400 hover:bg-zinc-800/50 hover:text-zinc-100'
                  }`
                }
              >
                {({ isActive }) => (
                  <>
                    {isActive && <div className="w-1 h-1 rounded-full bg-emerald-500 flex-shrink-0" />}
                    <Icon size={18} className="flex-shrink-0" />
                    {(!collapsed || isOpen) && <span>{label}</span>}
                  </>
                )}
              </NavLink>
            ))}
          </>}

          {/* DEVELOPER TOOLS SECTION */}
          {(!collapsed || isOpen) ? (
            <div className="px-3 pt-6 pb-2 text-xs font-semibold text-zinc-500 uppercase tracking-wider">
              Developer Tools
            </div>
          ) : (
            <div className="mx-3 mt-6 mb-2 border-t border-zinc-800/50" />
          )}

          {DEVELOPER_TOOLS.map(({ to, icon: Icon, label }) => (
            <NavLink
              key={to}
              to={to}
              onClick={onClose}
              className={({ isActive }) =>
                `flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-colors
                ${isActive
                  ? 'bg-zinc-800 text-white'
                  : 'text-zinc-400 hover:bg-zinc-800/50 hover:text-zinc-100'
                }`
              }
            >
              {({ isActive }) => (
                <>
                  {isActive && <div className="w-1 h-1 rounded-full bg-emerald-500 flex-shrink-0" />}
                  <Icon size={18} className="flex-shrink-0" />
                  {(!collapsed || isOpen) && <span>{label}</span>}
                </>
              )}
            </NavLink>
          ))}

          {/* Experimental Dropdown */}
          <div className="flex flex-col mt-1">
            <button
              onClick={() => {
                if (collapsed) setCollapsed(false);
                setExperimentalExpanded(!experimentalExpanded);
              }}
              className="flex items-center justify-between px-3 py-2 rounded-lg text-sm transition-colors text-zinc-400 hover:bg-zinc-800/50 hover:text-zinc-100 w-full"
            >
              <div className="flex items-center gap-3">
                <FlaskConical size={18} className="flex-shrink-0" />
                {(!collapsed || isOpen) && <span>Experimental</span>}
              </div>
              {(!collapsed || isOpen) && (
                experimentalExpanded ? <ChevronUp size={16} className="opacity-70" /> : <ChevronDown size={16} className="opacity-70" />
              )}
            </button>
            {experimentalExpanded && (!collapsed || isOpen) && (
              <div className="flex flex-col mt-1 ml-4 space-y-1 border-l border-zinc-800/50 pl-2">
                {EXPERIMENTAL.map(({ to, icon: Icon, label }) => (
                  <NavLink
                    key={to}
                    to={to}
                    onClick={onClose}
                    className={({ isActive }) =>
                      `flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-colors
                      ${isActive
                        ? 'bg-zinc-800 text-white'
                        : 'text-zinc-400 hover:bg-zinc-800/50 hover:text-zinc-100'
                      }`
                    }
                  >
                    {({ isActive }) => (
                      <>
                        {isActive && <div className="w-1 h-1 rounded-full bg-emerald-500 flex-shrink-0 absolute -ml-4" />}
                        <Icon size={16} className="flex-shrink-0" />
                        <span>{label}</span>
                      </>
                    )}
                  </NavLink>
                ))}
              </div>
            )}
          </div>

          {/* SETTINGS SECTION */}
          {isAdmin() && <>
            {(!collapsed || isOpen) ? (
              <div className="px-3 pt-6 pb-2 text-xs font-semibold text-zinc-500 uppercase tracking-wider">
                Settings
              </div>
            ) : (
              <div className="mx-3 mt-6 mb-2 border-t border-zinc-800/50" />
            )}

            <div className="flex flex-col mt-1">
              <button
                onClick={() => {
                  if (collapsed) setCollapsed(false);
                  setSettingsExpanded(!settingsExpanded);
                }}
                className="flex items-center justify-between px-3 py-2 rounded-lg text-sm transition-colors text-zinc-400 hover:bg-zinc-800/50 hover:text-zinc-100 w-full"
              >
                <div className="flex items-center gap-3">
                  <Settings size={18} className="flex-shrink-0" />
                  {(!collapsed || isOpen) && (
                    <span className="flex items-center">
                      Settings
                      <span className="bg-blue-600 text-white text-[9px] px-1.5 py-0.5 rounded-full font-semibold ml-2">New</span>
                    </span>
                  )}
                </div>
                {(!collapsed || isOpen) && (
                  settingsExpanded ? <ChevronUp size={16} className="opacity-70" /> : <ChevronDown size={16} className="opacity-70" />
                )}
              </button>
              {settingsExpanded && (!collapsed || isOpen) && (
                <div className="flex flex-col mt-1 ml-4 space-y-1 border-l border-zinc-800/50 pl-2">
                  {SETTINGS_SUB.map(({ to, icon: Icon, label, hasDot }) => (
                    <NavLink
                      key={to}
                      to={to}
                      onClick={onClose}
                      className={({ isActive }) =>
                        `flex items-center gap-3 px-3 py-2 rounded-lg text-sm transition-colors
                        ${isActive
                          ? 'bg-zinc-800 text-white'
                          : 'text-zinc-400 hover:bg-zinc-800/50 hover:text-zinc-100'
                        }`
                      }
                    >
                      {({ isActive }) => (
                        <>
                          {isActive && <div className="w-1 h-1 rounded-full bg-emerald-500 flex-shrink-0 absolute -ml-4" />}
                          <Icon size={16} className="flex-shrink-0" />
                          <span className="flex-1 truncate">{label}</span>
                          {hasDot && <div className="w-1.5 h-1.5 rounded-full bg-blue-500 flex-shrink-0 ml-1 animate-pulse" />}
                        </>
                      )}
                    </NavLink>
                  ))}
                </div>
              )}
            </div>
          </>}

        </nav>

        {/* Status indicator */}
        <div className="px-4 py-3 border-t border-zinc-800/50">
          {(!collapsed || isOpen) && (
            <div className="flex items-center gap-2 text-xs">
              {isHealthy ? (
                <>
                  <div className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse" />
                  <span className="text-zinc-500">Gateway active</span>
                </>
              ) : (
                <>
                  <AlertCircle size={12} className="text-red-400" />
                  <span className="text-red-400">Gateway unreachable</span>
                </>
              )}
            </div>
          )}
          {collapsed && !isOpen && (
            <div className="flex justify-center">
              {isHealthy
                ? <div className="w-2 h-2 rounded-full bg-emerald-500 animate-pulse" />
                : <AlertCircle size={12} className="text-red-400" />
              }
            </div>
          )}
        </div>

        {/* Admin Key Configuration */}
        {isAdmin() && (
          <button
            onClick={() => {
              if (onClose) onClose();
              const currentKey = localStorage.getItem('pylos_admin_key') || '';
              const newKey = window.prompt("Configure Pylos Admin Key (PYLOS_ADMIN_KEY):", currentKey);
              if (newKey !== null) {
                localStorage.setItem('pylos_admin_key', newKey);
                window.location.reload();
              }
            }}
            className="flex items-center gap-3 px-4 py-3 border-t border-zinc-800/50
              text-zinc-500 hover:text-white hover:bg-zinc-800/50 transition-colors text-sm w-full text-left"
            title="Configure Admin Key"
          >
            <KeyRound size={16} className="flex-shrink-0" />
            {(!collapsed || isOpen) && <span>Admin Key</span>}
          </button>
        )}

        {/* User profile card */}
        {user ? (
          <div className="px-4 py-3 border-t border-zinc-800/50 flex flex-col gap-2">
            {isPlaygroup() && (!collapsed || isOpen) && (
              <div className="flex items-center gap-2 px-3 py-2 rounded-lg bg-amber-900/20 border border-amber-800/30 text-amber-400 text-[10px] font-semibold mb-1">
                <ShieldAlert size={12} />
                <span>Quarantine — Contact admin to activate access</span>
              </div>
            )}
            {(!collapsed || isOpen) ? (
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 rounded-full bg-zinc-800 border border-zinc-700 flex items-center justify-center font-bold text-indigo-400 text-sm">
                  {user.name ? user.name.substring(0, 2).toUpperCase() : 'U'}
                </div>
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-semibold text-white truncate">{user.name}</p>
                  <p className="text-xs text-zinc-500 truncate">{user.email}</p>
                  {user.group && <p className="text-[9px] text-zinc-600 mt-0.5">group: {user.group}</p>}
                </div>
              </div>
            ) : (
              <div className="flex justify-center">
                <div className="w-8 h-8 rounded-full bg-zinc-800 border border-zinc-700 flex items-center justify-center font-bold text-indigo-400 text-sm" title={`${user.name} (${user.group || 'default'})`}>
                  {user.name ? user.name.substring(0, 2).toUpperCase() : 'U'}
                </div>
              </div>
            )}
          </div>
        ) : (
          <div className="px-4 py-3 border-t border-zinc-800/50 flex flex-col gap-2">
            {(!collapsed || isOpen) ? (
              <div className="flex items-center gap-3">
                <div className="w-8 h-8 rounded-full bg-zinc-850 border border-zinc-800 flex items-center justify-center font-bold text-amber-500 text-sm">
                  AD
                </div>
                <div className="flex-1 min-w-0">
                  <p className="text-sm font-semibold text-white truncate">Administrator</p>
                  <p className="text-xs text-zinc-500 truncate">Static Admin Key</p>
                </div>
              </div>
            ) : (
              <div className="flex justify-center">
                <div className="w-8 h-8 rounded-full bg-zinc-850 border border-zinc-800 flex items-center justify-center font-bold text-amber-500 text-sm" title="Administrator">
                  AD
                </div>
              </div>
            )}
          </div>
        )}

        <button
          onClick={async () => {
            if (onClose) onClose();
            try {
              await authApi.logout();
            } catch (e) {
              console.error("Logout request failed:", e);
            } finally {
              localStorage.removeItem('pylos_admin_key');
              localStorage.removeItem('pylos_user');
              window.location.href = '/login';
            }
          }}
          className="flex items-center gap-3 px-4 py-3 border-t border-zinc-800/50
            text-zinc-500 hover:text-red-400 hover:bg-red-500/10 transition-colors text-sm w-full text-left"
          title="Logout"
        >
          <LogOut size={16} className="flex-shrink-0" />
          {(!collapsed || isOpen) && <span>Logout</span>}
        </button>

        {/* Collapse button */}
        <button
          onClick={() => setCollapsed(!collapsed)}
          className="hidden md:flex items-center justify-center py-3 border-t border-zinc-800/50
            text-zinc-500 hover:text-white hover:bg-zinc-800/50 transition-colors"
        >
          {collapsed
            ? <ChevronRight size={16} />
            : <ChevronLeft size={16} />
          }
        </button>
      </aside>
    </>
  )
}
