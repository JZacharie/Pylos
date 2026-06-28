import { useState, useRef, useEffect } from 'react'
import {
  FileText, Code, Key, Globe, Shield,
  Server, Zap, Copy, Check, ChevronRight,
} from 'lucide-react'

// ─── Types ────────────────────────────────────────────────────────────────────

interface Endpoint {
  method: 'GET' | 'POST' | 'PUT' | 'DELETE' | 'ANY'
  path: string
  title: string
  description: string
  headers?: { name: string; required: boolean; description: string }[]
  body?: string
  response: string
}

interface Section {
  id: string
  title: string
  icon: React.ReactNode
  endpoints: Endpoint[]
}

// ─── Data ─────────────────────────────────────────────────────────────────────

const SECTIONS: Section[] = [
  {
    id: 'inference',
    title: 'Inference Endpoints',
    icon: <Zap size={16} />,
    endpoints: [
      {
        method: 'POST',
        path: '/v1/chat/completions',
        title: 'Chat Completions',
        description: 'Create a chat completion. This is the primary endpoint for interacting with LLMs through Pylos. Compatible with the OpenAI Chat Completions API.',
        headers: [
          { name: 'Authorization', required: true, description: 'Bearer <virtual_key>' },
          { name: 'Content-Type', required: true, description: 'application/json' },
        ],
        body: `{
  "model": "gpt-4o",
  "messages": [
    { "role": "system", "content": "You are a helpful assistant." },
    { "role": "user", "content": "Hello!" }
  ],
  "temperature": 0.7,
  "max_tokens": 1024,
  "stream": false
}`,
        response: `{
  "id": "chatcmpl-abc123",
  "object": "chat.completion",
  "created": 1719600000,
  "model": "gpt-4o",
  "choices": [
    {
      "index": 0,
      "message": {
        "role": "assistant",
        "content": "Hello! How can I help you today?"
      },
      "finish_reason": "stop"
    }
  ],
  "usage": {
    "prompt_tokens": 25,
    "completion_tokens": 12,
    "total_tokens": 37
  }
}`,
      },
      {
        method: 'POST',
        path: '/v1/embeddings',
        title: 'Embeddings',
        description: 'Create embeddings for the given input. Useful for vector search, semantic similarity, and RAG pipelines.',
        headers: [
          { name: 'Authorization', required: true, description: 'Bearer <virtual_key>' },
          { name: 'Content-Type', required: true, description: 'application/json' },
        ],
        body: `{
  "model": "text-embedding-3-small",
  "input": "The quick brown fox jumps over the lazy dog.",
  "encoding_format": "float"
}`,
        response: `{
  "object": "list",
  "data": [
    {
      "object": "embedding",
      "embedding": [0.0023, -0.0091, 0.0156, ...],
      "index": 0
    }
  ],
  "model": "text-embedding-3-small",
  "usage": {
    "prompt_tokens": 12,
    "total_tokens": 12
  }
}`,
      },
      {
        method: 'POST',
        path: '/v1/images/generations',
        title: 'Image Generations',
        description: 'Generate images from a text prompt. Routes to providers like DALL-E, Stable Diffusion, or Flux.',
        headers: [
          { name: 'Authorization', required: true, description: 'Bearer <virtual_key>' },
          { name: 'Content-Type', required: true, description: 'application/json' },
        ],
        body: `{
  "model": "dall-e-3",
  "prompt": "A serene mountain landscape at sunset",
  "n": 1,
  "size": "1024x1024",
  "quality": "standard"
}`,
        response: `{
  "created": 1719600000,
  "data": [
    {
      "url": "https://oaidalleapiprodscus.blob.core.windows.net/...",
      "revised_prompt": "A serene mountain landscape at sunset..."
    }
  ]
}`,
      },
      {
        method: 'GET',
        path: '/v1/models',
        title: 'List Models',
        description: 'List all available models across your configured providers. Returns model names, capabilities, and provider info.',
        headers: [
          { name: 'Authorization', required: true, description: 'Bearer <virtual_key>' },
        ],
        response: `{
  "object": "list",
  "data": [
    {
      "id": "gpt-4o",
      "object": "model",
      "created": 1719600000,
      "owned_by": "openai",
      "provider": "openai",
      "capabilities": ["chat", "vision"]
    },
    {
      "id": "claude-sonnet-4-20250514",
      "object": "model",
      "created": 1719600000,
      "owned_by": "anthropic",
      "provider": "anthropic",
      "capabilities": ["chat", "vision"]
    }
  ]
}`,
      },
    ],
  },
  {
    id: 'management',
    title: 'Management Endpoints',
    icon: <Server size={16} />,
    endpoints: [
      {
        method: 'GET',
        path: '/providers',
        title: 'List Providers',
        description: 'List all configured LLM providers. Returns provider details including keys, network config, and status.',
        headers: [
          { name: 'Authorization', required: true, description: 'Bearer <admin_key>' },
        ],
        response: `{
  "total": 2,
  "providers": [
    {
      "name": "openai",
      "keys_count": 3,
      "network": {
        "base_url": null,
        "timeout_secs": 30,
        "max_retries": 3
      },
      "keys": [
        { "name": "primary", "models": ["gpt-4o"], "weight": 1.0, "value": "sk-...xyz" }
      ]
    }
  ]
}`,
      },
      {
        method: 'POST',
        path: '/providers',
        title: 'Create / Update Provider',
        description: 'Create a new provider or update an existing one. Include the provider name in the URL path.',
        headers: [
          { name: 'Authorization', required: true, description: 'Bearer <admin_key>' },
          { name: 'Content-Type', required: true, description: 'application/json' },
        ],
        body: `{
  "keys": [
    {
      "name": "primary",
      "value": "sk-...your-key",
      "models": ["gpt-4o", "gpt-4o-mini"],
      "weight": 1.0
    }
  ],
  "network": {
    "base_url": "https://api.openai.com/v1",
    "timeout_secs": 30,
    "max_retries": 3
  }
}`,
        response: `{
  "message": "Provider openai updated"
}`,
      },
      {
        method: 'GET',
        path: '/virtual-keys',
        title: 'List Virtual Keys',
        description: 'List all virtual keys used for client authentication. Virtual keys map to provider API keys with budget and rate limits.',
        headers: [
          { name: 'Authorization', required: true, description: 'Bearer <admin_key>' },
        ],
        response: `{
  "total": 2,
  "virtual_keys": [
    {
      "key_hash": "abc123",
      "key_name": "frontend-app",
      "budget_usd": 100.0,
      "budget_used_usd": 12.45,
      "rate_limit_rpm": 60,
      "models": ["gpt-4o", "gpt-4o-mini"],
      "is_active": true,
      "team_id": "team-001"
    }
  ]
}`,
      },
      {
        method: 'POST',
        path: '/virtual-keys',
        title: 'Create Virtual Key',
        description: 'Create a new virtual key for client authentication. Returns the key value once — store it securely.',
        headers: [
          { name: 'Authorization', required: true, description: 'Bearer <admin_key>' },
          { name: 'Content-Type', required: true, description: 'application/json' },
        ],
        body: `{
  "key_name": "frontend-app",
  "budget_usd": 100.0,
  "rate_limit_rpm": 60,
  "models": ["gpt-4o", "gpt-4o-mini"],
  "team_id": "team-001"
}`,
        response: `{
  "key": "sk-pylos-...generated-key",
  "key_name": "frontend-app",
  "message": "Virtual key created successfully"
}`,
      },
      {
        method: 'GET',
        path: '/api/logs/stats',
        title: 'Log Statistics',
        description: 'Get aggregated log statistics for a given time period. Used by the dashboard to render KPI cards.',
        headers: [
          { name: 'Authorization', required: true, description: 'Bearer <admin_key>' },
        ],
        response: `{
  "total_requests": 15420,
  "success_rate": 99.2,
  "average_latency_ms": 847,
  "total_tokens": 2340000,
  "total_cost_usd": 48.32,
  "total_prompt_tokens": 1560000,
  "total_completion_tokens": 780000,
  "total_compression_saved_bytes": 102400
}`,
      },
      {
        method: 'POST',
        path: '/config/reload',
        title: 'Reload Configuration',
        description: 'Hot-reload the Pylos gateway configuration. Useful after updating the config file without restarting the service.',
        headers: [
          { name: 'Authorization', required: true, description: 'Bearer <admin_key>' },
        ],
        response: `{
  "message": "Configuration reloaded successfully"
}`,
      },
    ],
  },
  {
    id: 'mcp',
    title: 'MCP Proxy',
    icon: <Globe size={16} />,
    endpoints: [
      {
        method: 'GET',
        path: '/mcp/{server_name}',
        title: 'MCP Server Info',
        description: 'Get information about an MCP (Model Context Protocol) server. Returns available tools and server metadata.',
        headers: [
          { name: 'Authorization', required: true, description: 'Bearer <virtual_key>' },
        ],
        response: `{
  "name": "my-mcp-server",
  "version": "1.0.0",
  "tools": [
    {
      "name": "get_weather",
      "description": "Get current weather for a location",
      "inputSchema": {
        "type": "object",
        "properties": {
          "location": { "type": "string" }
        }
      }
    }
  ]
}`,
      },
      {
        method: 'ANY',
        path: '/mcp/{server_name}/{path}',
        title: 'MCP Tool Call',
        description: 'Proxy any request to an MCP server. Supports all HTTP methods. The path is forwarded to the configured MCP server endpoint.',
        headers: [
          { name: 'Authorization', required: true, description: 'Bearer <virtual_key>' },
          { name: 'Content-Type', required: false, description: 'application/json' },
        ],
        body: `{
  "tool": "get_weather",
  "arguments": {
    "location": "San Francisco, CA"
  }
}`,
        response: `{
  "result": {
    "temperature": 72,
    "condition": "sunny",
    "humidity": 45
  }
}`,
      },
    ],
  },
]

// ─── Helpers ──────────────────────────────────────────────────────────────────

function methodColor(method: string) {
  switch (method) {
    case 'GET':    return 'bg-emerald-500/15 text-emerald-400 border-emerald-500/30'
    case 'POST':   return 'bg-blue-500/15 text-blue-400 border-blue-500/30'
    case 'PUT':    return 'bg-amber-500/15 text-amber-400 border-amber-500/30'
    case 'DELETE': return 'bg-red-500/15 text-red-400 border-red-500/30'
    case 'ANY':    return 'bg-purple-500/15 text-purple-400 border-purple-500/30'
    default:       return 'bg-zinc-500/15 text-zinc-400 border-zinc-500/30'
  }
}

// ─── CodeBlock ────────────────────────────────────────────────────────────────

function CodeBlock({ code, label }: { code: string; label: string }) {
  const [copied, setCopied] = useState(false)

  const handleCopy = () => {
    navigator.clipboard.writeText(code)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  return (
    <div className="rounded-xl border border-zinc-800/50 bg-zinc-950/80 overflow-hidden">
      <div className="flex items-center justify-between px-4 py-2 bg-zinc-900/60 border-b border-zinc-800/50">
        <span className="text-[11px] text-zinc-500 font-medium">{label}</span>
        <button
          onClick={handleCopy}
          className="text-zinc-500 hover:text-zinc-300 transition-colors p-1 rounded"
          title="Copy"
        >
          {copied ? <Check size={12} className="text-emerald-400" /> : <Copy size={12} />}
        </button>
      </div>
      <pre className="p-4 overflow-x-auto text-xs leading-relaxed">
        <code className="text-zinc-300 font-mono">{code}</code>
      </pre>
    </div>
  )
}

// ─── EndpointCard ─────────────────────────────────────────────────────────────

function EndpointCard({ endpoint }: { endpoint: Endpoint }) {
  const [expanded, setExpanded] = useState(false)

  return (
    <div className="rounded-xl border border-zinc-800/50 bg-zinc-900/20 overflow-hidden hover:border-zinc-700/50 transition-colors">
      <button
        onClick={() => setExpanded(!expanded)}
        className="w-full flex items-center gap-3 p-4 text-left"
      >
        <span className={`px-2.5 py-1 rounded-md text-[11px] font-bold border ${methodColor(endpoint.method)}`}>
          {endpoint.method}
        </span>
        <code className="text-sm text-zinc-200 font-mono flex-1 truncate">{endpoint.path}</code>
        <span className="text-xs text-zinc-500 hidden sm:block mr-2">{endpoint.title}</span>
        <ChevronRight
          size={16}
          className={`text-zinc-600 transition-transform duration-200 shrink-0 ${expanded ? 'rotate-90' : ''}`}
        />
      </button>

      {expanded && (
        <div className="px-4 pb-5 space-y-4 border-t border-zinc-800/50 pt-4">
          <p className="text-sm text-zinc-400 leading-relaxed">{endpoint.description}</p>

          {/* Headers */}
          {endpoint.headers && endpoint.headers.length > 0 && (
            <div>
              <h4 className="text-xs font-semibold text-zinc-300 mb-2">Headers</h4>
              <div className="rounded-lg border border-zinc-800/50 overflow-hidden">
                <table className="w-full text-xs">
                  <thead>
                    <tr className="bg-zinc-900/60">
                      <th className="text-left px-3 py-2 text-zinc-500 font-medium">Name</th>
                      <th className="text-left px-3 py-2 text-zinc-500 font-medium">Required</th>
                      <th className="text-left px-3 py-2 text-zinc-500 font-medium">Description</th>
                    </tr>
                  </thead>
                  <tbody>
                    {endpoint.headers.map((h, i) => (
                      <tr key={i} className="border-t border-zinc-800/50">
                        <td className="px-3 py-2 font-mono text-zinc-300">{h.name}</td>
                        <td className="px-3 py-2">
                          {h.required ? (
                            <span className="text-emerald-400 text-[11px] font-medium">Required</span>
                          ) : (
                            <span className="text-zinc-600 text-[11px]">Optional</span>
                          )}
                        </td>
                        <td className="px-3 py-2 text-zinc-500">{h.description}</td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}

          {/* Request Body */}
          {endpoint.body && (
            <CodeBlock code={endpoint.body} label="Request Body" />
          )}

          {/* Response */}
          <CodeBlock code={endpoint.response} label="Response" />
        </div>
      )}
    </div>
  )
}

// ─── Sidebar ──────────────────────────────────────────────────────────────────

function SidebarNav({
  activeSection,
  onNavigate,
}: {
  activeSection: string
  onNavigate: (id: string) => void
}) {
  const sections = [
    { id: 'auth', title: 'Authentication', icon: <Key size={14} /> },
    ...SECTIONS.map(s => ({ id: s.id, title: s.title, icon: s.icon })),
  ]

  return (
    <nav className="space-y-1">
      <div className="text-[11px] font-semibold text-zinc-600 uppercase tracking-wider px-3 mb-2">
        Documentation
      </div>
      {sections.map(s => (
        <button
          key={s.id}
          onClick={() => onNavigate(s.id)}
          className={`w-full flex items-center gap-2.5 px-3 py-2 rounded-lg text-xs font-medium transition-all ${
            activeSection === s.id
              ? 'bg-zinc-800/80 text-white'
              : 'text-zinc-500 hover:text-zinc-300 hover:bg-zinc-800/40'
          }`}
        >
          <span className={activeSection === s.id ? 'text-emerald-400' : 'text-zinc-600'}>
            {s.icon}
          </span>
          {s.title}
        </button>
      ))}
    </nav>
  )
}

// ─── Main ─────────────────────────────────────────────────────────────────────

export default function ApiReference() {
  const [activeSection, setActiveSection] = useState('auth')
  const contentRef = useRef<HTMLDivElement>(null)

  const scrollToSection = (id: string) => {
    setActiveSection(id)
    const el = document.getElementById(id)
    if (el) {
      el.scrollIntoView({ behavior: 'smooth', block: 'start' })
    }
  }

  useEffect(() => {
    const container = contentRef.current
    if (!container) return

    const handleScroll = () => {
      const sections = ['auth', ...SECTIONS.map(s => s.id)]
      for (const id of sections) {
        const el = document.getElementById(id)
        if (el) {
          const rect = el.getBoundingClientRect()
          if (rect.top <= 120) {
            setActiveSection(id)
          }
        }
      }
    }

    container.addEventListener('scroll', handleScroll)
    return () => container.removeEventListener('scroll', handleScroll)
  }, [])

  return (
    <div className="flex h-full">
      {/* Sidebar */}
      <aside className="hidden lg:block w-60 shrink-0 border-r border-zinc-800/50 bg-zinc-900/20 overflow-y-auto p-4">
        <SidebarNav activeSection={activeSection} onNavigate={scrollToSection} />
      </aside>

      {/* Content */}
      <div ref={contentRef} className="flex-1 overflow-y-auto p-6 lg:p-8 space-y-10 max-w-4xl">
        {/* Page header */}
        <div>
          <div className="flex items-center gap-3 mb-2">
            <div className="p-2 bg-blue-500/10 rounded-lg">
              <FileText size={18} className="text-blue-400" />
            </div>
            <h1 className="text-2xl font-bold text-white">API Reference</h1>
          </div>
          <p className="text-sm text-zinc-400 max-w-2xl">
            Complete documentation for the Pylos API. All inference endpoints are compatible with the OpenAI API spec.
            Management endpoints require an admin key.
          </p>
        </div>

        {/* Quick start */}
        <div className="rounded-xl border border-zinc-800/50 bg-zinc-900/30 p-5">
          <h2 className="text-sm font-semibold text-white mb-3 flex items-center gap-2">
            <Zap size={14} className="text-emerald-400" />
            Quick Start
          </h2>
          <div className="text-sm text-zinc-400 space-y-2">
            <p>Point your OpenAI-compatible client at the Pylos base URL:</p>
            <CodeBlock
              code={`curl -X POST https://your-gateway.com/v1/chat/completions \\
  -H "Authorization: Bearer sk-pylos-your-virtual-key" \\
  -H "Content-Type: application/json" \\
  -d '{"model":"gpt-4o","messages":[{"role":"user","content":"Hello!"}]}'`}
              label="cURL Example"
            />
          </div>
        </div>

        {/* Authentication */}
        <section id="auth" className="space-y-4">
          <div className="flex items-center gap-2">
            <Key size={18} className="text-emerald-400" />
            <h2 className="text-lg font-bold text-white">Authentication</h2>
          </div>

          <div className="space-y-4 text-sm text-zinc-400 leading-relaxed">
            <p>
              Pylos uses two types of keys for authentication:
            </p>

            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <div className="rounded-xl border border-zinc-800/50 bg-zinc-900/30 p-5 space-y-3">
                <div className="flex items-center gap-2">
                  <div className="w-8 h-8 rounded-lg bg-emerald-500/10 flex items-center justify-center">
                    <Key size={14} className="text-emerald-400" />
                  </div>
                  <h3 className="font-semibold text-white text-sm">Virtual Keys</h3>
                </div>
                <p className="text-xs text-zinc-500">
                  Used by <strong className="text-zinc-300">clients and applications</strong> to access inference endpoints.
                  Each key has configurable budget limits, rate limits, and model access controls.
                </p>
                <div className="bg-zinc-950/80 rounded-lg p-3 font-mono text-xs text-emerald-400">
                  Authorization: Bearer sk-pylos-...
                </div>
              </div>

              <div className="rounded-xl border border-zinc-800/50 bg-zinc-900/30 p-5 space-y-3">
                <div className="flex items-center gap-2">
                  <div className="w-8 h-8 rounded-lg bg-amber-500/10 flex items-center justify-center">
                    <Shield size={14} className="text-amber-400" />
                  </div>
                  <h3 className="font-semibold text-white text-sm">Admin Keys</h3>
                </div>
                <p className="text-xs text-zinc-500">
                  Used for <strong className="text-zinc-300">management endpoints</strong> (providers, keys, config).
                  Provide full access to gateway administration. Store securely and rotate regularly.
                </p>
                <div className="bg-zinc-950/80 rounded-lg p-3 font-mono text-xs text-amber-400">
                  Authorization: Bearer sk-admin-...
                </div>
              </div>
            </div>

            <div className="rounded-xl border border-blue-500/20 bg-blue-500/5 p-4">
              <div className="flex items-start gap-2">
                <Code size={14} className="text-blue-400 mt-0.5 shrink-0" />
                <div className="text-xs text-zinc-400">
                  <strong className="text-blue-400">Tip:</strong> You can also pass the key as the{' '}
                  <code className="text-zinc-300 bg-zinc-800/50 px-1.5 py-0.5 rounded">X-API-Key</code>{' '}
                  header. The{' '}
                  <code className="text-zinc-300 bg-zinc-800/50 px-1.5 py-0.5 rounded">Authorization</code>{' '}
                  Bearer scheme is recommended for OpenAI SDK compatibility.
                </div>
              </div>
            </div>
          </div>
        </section>

        {/* Endpoint sections */}
        {SECTIONS.map(section => (
          <section key={section.id} id={section.id} className="space-y-4">
            <div className="flex items-center gap-2">
              <span className="text-emerald-400">{section.icon}</span>
              <h2 className="text-lg font-bold text-white">{section.title}</h2>
              <span className="text-xs text-zinc-600 ml-1">
                {section.endpoints.length} endpoint{section.endpoints.length !== 1 ? 's' : ''}
              </span>
            </div>

            <div className="space-y-3">
              {section.endpoints.map((ep, i) => (
                <EndpointCard key={i} endpoint={ep} />
              ))}
            </div>
          </section>
        ))}

        {/* Footer */}
        <div className="border-t border-zinc-800/50 pt-6 pb-8 text-center">
          <p className="text-xs text-zinc-600">
            Pylos API Reference &middot; All inference endpoints are OpenAI-compatible
          </p>
        </div>
      </div>
    </div>
  )
}
