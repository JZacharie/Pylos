import { NavLink, useLocation } from 'react-router-dom'
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

// AI Gateway items
const AI_GATEWAY = [
  { to: '/keys',         icon: KeyRound,        label: 'Virtual Keys' },
  { to: '/playground',   icon: FlaskConical,    label: 'Playground' },
  { to: '/providers',    icon: Server,          label: 'Providers' },
  { to: '/models',       icon: BookOpen,        label: 'Model Catalog' },
  { to: '/mcp-servers',  icon: Cpu,             label: 'MCP Servers' },
  { to: '/agents',       icon: Bot,             label: 'Agents' },
  { to: '/guardrails',   icon: Shield,          label: 'Guardrails' },
]

const NAV_TOOLS = [
  { to: '/tools/search',        icon: Search,      label: 'Search Tools' },
  { to: '/tools/vector-stores', icon: Database,    label: 'Vector Stores' },
  { to: '/tools/policies',      icon: ShieldCheck, label: 'Tool Policies' },
]

const OBSERVABILITY = [
  { to: '/analytics',              icon: BarChart2,   label: 'Analytics & Usage' },
  { to: '/logs',                   icon: ScrollText,  label: 'Logs' },
  { to: '/guardrails-monitor',     icon: ShieldAlert, label: 'Guardrails Monitor' },
  { to: '/settings/cost-tracking', icon: DollarSign,  label: 'Cost Tracking' },
]

const GOVERNANCE = [
  { to: '/organizations',  icon: Landmark,       label: 'Organizations' },
  { to: '/teams',          icon: Users,          label: 'Teams' },
  { to: '/users',          icon: User,           label: 'Internal Users' },
  { to: '/access-groups',  icon: Layers,         label: 'Access Groups' },
  { to: '/budgets',        icon: CreditCard,     label: 'Budgets' },
  { to: '/policies',       icon: FileBadge,      label: 'Routing Policies' },
]

const DEVELOPER_TOOLS = [
  { to: '/api-reference',               icon: Code2,          label: 'API Reference' },
  { to: '/experimental/api-playground', icon: Terminal,       label: 'API Tester' },
  { to: '/ai-hub',                      icon: LayoutGrid,     label: 'AI Hub' },
  { to: '/learning-resources',          icon: BookOpen,       label: 'Learning Resources' },
]

const SETTINGS_SUB = [
  { to: '/settings/router',         icon: Sliders,    label: 'Router & Fallbacks' },
  { to: '/settings/logging-alerts', icon: Bell,       label: 'Logging & Alerts' },
  { to: '/experimental/caching',    icon: HardDrive,  label: 'Prompt Caching' },
  { to: '/settings/admin',          icon: Settings,   label: 'Admin Settings', hasDot: true },
  { to: '/settings/ui-theme',       icon: Palette,    label: 'UI Theme' },
]

function SectionHeader({
  title,
  count,
  isExpanded,
  onToggle,
  isCollapsed,
  isOpen,
  topSpacing = true,
}: {
  title: string
  count?: number
  isExpanded: boolean
  onToggle: () => void
  isCollapsed: boolean
  isOpen?: boolean
  topSpacing?: boolean
}) {
  if (isCollapsed && !isOpen) {
    return <div className={`mx-3 ${topSpacing ? 'mt-4' : 'mt-2'} mb-2 border-t border-zinc-800/50`} />
  }

  return (
    <button
      type="button"
      onClick={onToggle}
      className={`flex items-center justify-between w-full px-3 ${
        topSpacing ? 'pt-4' : 'pt-2'
      } pb-1.5 text-xs font-semibold text-zinc-500 hover:text-zinc-300 uppercase tracking-wider transition-colors group cursor-pointer text-left select-none`}
      title={isExpanded ? `Collapse ${title}` : `Expand ${title}`}
    >
      <span className="flex items-center gap-1.5">
        <span>{title}</span>
        {count !== undefined && (
          <span className="text-[10px] font-normal text-zinc-600 group-hover:text-zinc-400 transition-colors">
            ({count})
          </span>
        )}
      </span>
      <span className="p-0.5 rounded text-zinc-600 group-hover:text-zinc-300 transition-colors">
        {isExpanded ? <ChevronUp size={13} /> : <ChevronDown size={13} />}
      </span>
    </button>
  )
}

export function Sidebar({ isOpen, onClose }: { isOpen?: boolean; onClose?: () => void }) {
  const location = useLocation()
  const [collapsed, setCollapsed] = useState(false)
  const [expandedSections, setExpandedSections] = useState<Record<string, boolean>>(() => {
    try {
      const saved = localStorage.getItem('pylos_nav_sections')
      if (saved) return JSON.parse(saved)
    } catch {}
    return {
      aiGateway: true,
      tools: true,
      observability: true,
      accessControl: true,
      devTools: true,
      settings: true,
    }
  })

  const toggleSection = (section: string) => {
    if (collapsed) setCollapsed(false)
    setExpandedSections(prev => {
      const next = { ...prev, [section]: !prev[section] }
      try {
        localStorage.setItem('pylos_nav_sections', JSON.stringify(next))
      } catch {}
      return next
    })
  }

  // Auto-expand section if current route is inside it (adjusting state on route change)
  const [prevPath, setPrevPath] = useState(location.pathname)
  if (prevPath !== location.pathname) {
    setPrevPath(location.pathname)
    const p = location.pathname
    let changed = false
    const next = { ...expandedSections }
    if (!next.aiGateway && AI_GATEWAY.some(i => i.to === p || (i.to !== '/' && p.startsWith(i.to)))) {
      next.aiGateway = true
      changed = true
    }
    if (!next.tools && NAV_TOOLS.some(i => i.to === p || p.startsWith(i.to))) {
      next.tools = true
      changed = true
    }
    if (!next.observability && OBSERVABILITY.some(i => i.to === p || p.startsWith(i.to))) {
      next.observability = true
      changed = true
    }
    if (!next.accessControl && GOVERNANCE.some(i => i.to === p || p.startsWith(i.to))) {
      next.accessControl = true
      changed = true
    }
    if (!next.devTools && DEVELOPER_TOOLS.some(i => i.to === p || p.startsWith(i.to))) {
      next.devTools = true
      changed = true
    }
    if (!next.settings && SETTINGS_SUB.some(i => i.to === p || p.startsWith(i.to))) {
      next.settings = true
      changed = true
    }
    if (changed) {
      setExpandedSections(next)
      try {
        localStorage.setItem('pylos_nav_sections', JSON.stringify(next))
      } catch {}
    }
  }

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
          <SectionHeader
            title="AI Gateway"
            count={AI_GATEWAY.length}
            isExpanded={expandedSections.aiGateway}
            onToggle={() => toggleSection('aiGateway')}
            isCollapsed={collapsed}
            isOpen={isOpen}
            topSpacing={false}
          />

          {(expandedSections.aiGateway || (collapsed && !isOpen)) &&
            AI_GATEWAY.map(({ to, icon: Icon, label }) => (
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

          {/* TOOLS SECTION */}
          <SectionHeader
            title="Tools"
            count={NAV_TOOLS.length}
            isExpanded={expandedSections.tools}
            onToggle={() => toggleSection('tools')}
            isCollapsed={collapsed}
            isOpen={isOpen}
          />

          {(expandedSections.tools || (collapsed && !isOpen)) &&
            NAV_TOOLS.map(({ to, icon: Icon, label }) => (
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

          {/* OBSERVABILITY SECTION */}
          <SectionHeader
            title="Observability"
            count={OBSERVABILITY.length}
            isExpanded={expandedSections.observability}
            onToggle={() => toggleSection('observability')}
            isCollapsed={collapsed}
            isOpen={isOpen}
          />

          {(expandedSections.observability || (collapsed && !isOpen)) &&
            OBSERVABILITY.map(({ to, icon: Icon, label }) => (
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

          {/* ACCESS CONTROL & GOVERNANCE SECTION */}
          {isAdmin() && (
            <>
              <SectionHeader
                title="Access Control"
                count={GOVERNANCE.length}
                isExpanded={expandedSections.accessControl}
                onToggle={() => toggleSection('accessControl')}
                isCollapsed={collapsed}
                isOpen={isOpen}
              />

              {(expandedSections.accessControl || (collapsed && !isOpen)) &&
                GOVERNANCE.map(({ to, icon: Icon, label }) => (
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
            </>
          )}

          {/* DEVELOPER TOOLS SECTION */}
          <SectionHeader
            title="Developer Tools"
            count={DEVELOPER_TOOLS.length}
            isExpanded={expandedSections.devTools}
            onToggle={() => toggleSection('devTools')}
            isCollapsed={collapsed}
            isOpen={isOpen}
          />

          {(expandedSections.devTools || (collapsed && !isOpen)) &&
            DEVELOPER_TOOLS.map(({ to, icon: Icon, label }) => (
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

          {/* SETTINGS SECTION */}
          {isAdmin() && (
            <>
              <SectionHeader
                title="Settings"
                count={SETTINGS_SUB.length}
                isExpanded={expandedSections.settings}
                onToggle={() => toggleSection('settings')}
                isCollapsed={collapsed}
                isOpen={isOpen}
              />

              {(expandedSections.settings || (collapsed && !isOpen)) &&
                SETTINGS_SUB.map(({ to, icon: Icon, label, hasDot }) => (
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
                        {(!collapsed || isOpen) && (
                          <>
                            <span className="flex-1 truncate">{label}</span>
                            {hasDot && <div className="w-1.5 h-1.5 rounded-full bg-blue-500 flex-shrink-0 ml-1 animate-pulse" />}
                          </>
                        )}
                      </>
                    )}
                  </NavLink>
                ))}
            </>
          )}

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
        <button
          onClick={() => {
            if (onClose) onClose();
            const currentKey = localStorage.getItem('pylos_admin_key') || '';
            const isJwt = currentKey.startsWith('ey') || currentKey.includes('.');
            const newKey = window.prompt("Configure Pylos Admin Key (PYLOS_ADMIN_KEY):", isJwt ? '' : currentKey);
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
