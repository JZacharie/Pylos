import { useState, useEffect } from 'react'
import {
  Send, Clock, Check, X, Copy, History, Code,
  ChevronDown,
} from 'lucide-react'

// ─── Types ────────────────────────────────────────────────────────────────────

interface Endpoint {
  id: string
  method: 'GET' | 'POST'
  path: string
  label: string
  defaultBody: string
}

interface Header {
  key: string
  value: string
  enabled: boolean
}

interface HistoryEntry {
  id: number
  endpoint: string
  method: string
  status: number | null
  statusText: string
  timeMs: number
  timestamp: number
  responseBody: string
}

// ─── Constants ────────────────────────────────────────────────────────────────

const ENDPOINTS: Endpoint[] = [
  {
    id: 'chat',
    method: 'POST',
    path: '/v1/chat/completions',
    label: 'Chat Completions',
    defaultBody: JSON.stringify(
      { model: 'gpt-4o-mini', messages: [{ role: 'user', content: 'Hello' }] },
      null,
      2,
    ),
  },
  {
    id: 'embeddings',
    method: 'POST',
    path: '/v1/embeddings',
    label: 'Embeddings',
    defaultBody: JSON.stringify(
      { model: 'text-embedding-3-small', input: 'Hello world' },
      null,
      2,
    ),
  },
  {
    id: 'models',
    method: 'GET',
    path: '/v1/models',
    label: 'List Models',
    defaultBody: '',
  },
]

const MAX_HISTORY = 10

// ─── Helpers ──────────────────────────────────────────────────────────────────

function methodColor(method: string) {
  switch (method) {
    case 'GET':  return 'bg-emerald-500/15 text-emerald-400 border-emerald-500/30'
    case 'POST': return 'bg-blue-500/15 text-blue-400 border-blue-500/30'
    default:     return 'bg-zinc-500/15 text-zinc-400 border-zinc-500/30'
  }
}

function statusColor(code: number) {
  if (code >= 200 && code < 300) return 'bg-emerald-500/15 text-emerald-400 border-emerald-500/30'
  if (code >= 300 && code < 400) return 'bg-blue-500/15 text-blue-400 border-blue-500/30'
  if (code >= 400 && code < 500) return 'bg-amber-500/15 text-amber-400 border-amber-500/30'
  return 'bg-red-500/15 text-red-400 border-red-500/30'
}

function formatMs(ms: number): string {
  if (ms < 1000) return `${ms.toFixed(0)}ms`
  return `${(ms / 1000).toFixed(2)}s`
}

function tryFormatJson(raw: string): string {
  try {
    return JSON.stringify(JSON.parse(raw), null, 2)
  } catch {
    return raw
  }
}

// ─── Component ────────────────────────────────────────────────────────────────

export default function ApiPlayground() {
  const [selectedId, setSelectedId] = useState('chat')
  const [headers, setHeaders] = useState<Header[]>(() => {
    const stored = localStorage.getItem('pylos_pg_auth_key') || ''
    return [
      { key: 'Authorization', value: stored ? `Bearer ${stored}` : 'Bearer ', enabled: true },
      { key: 'Content-Type', value: 'application/json', enabled: true },
    ]
  })
  const [body, setBody] = useState(ENDPOINTS[0].defaultBody)
  const [loading, setLoading] = useState(false)
  const [response, setResponse] = useState<{
    status: number
    statusText: string
    body: string
    timeMs: number
  } | null>(null)
  const [history, setHistory] = useState<HistoryEntry[]>([])
  const [copied, setCopied] = useState<string | null>(null)

  const endpoint = ENDPOINTS.find(e => e.id === selectedId)!

  useEffect(() => {
    const ep = ENDPOINTS.find(e => e.id === selectedId)
    if (ep) setBody(ep.defaultBody)
    setResponse(null)
  }, [selectedId])

  function updateHeader(index: number, field: keyof Header, value: string | boolean) {
    setHeaders(prev => prev.map((h, i) => i === index ? { ...h, [field]: value } : h))
  }

  function addHeader() {
    setHeaders(prev => [...prev, { key: '', value: '', enabled: true }])
  }

  function removeHeader(index: number) {
    setHeaders(prev => prev.filter((_, i) => i !== index))
  }

  function copyToClipboard(text: string, id: string) {
    navigator.clipboard.writeText(text)
    setCopied(id)
    setTimeout(() => setCopied(null), 2000)
  }

  async function sendRequest() {
    if (loading) return

    setLoading(true)
    setResponse(null)

    const activeHeaders = headers.filter(h => h.enabled && h.key.trim())
    const headerObj: Record<string, string> = {}
    for (const h of activeHeaders) {
      headerObj[h.key.trim()] = h.value
    }

    const start = performance.now()

    try {
      const fetchInit: RequestInit = {
        method: endpoint.method,
        headers: headerObj,
      }

      if (endpoint.method === 'POST' && body.trim()) {
        fetchInit.body = body
      }

      const resp = await fetch(endpoint.path, fetchInit)
      const timeMs = performance.now() - start
      const text = await resp.text()

      setResponse({
        status: resp.status,
        statusText: resp.statusText,
        body: text,
        timeMs,
      })

      setHistory(prev => [
        {
          id: Date.now(),
          endpoint: endpoint.path,
          method: endpoint.method,
          status: resp.status,
          statusText: resp.statusText,
          timeMs,
          timestamp: Date.now(),
          responseBody: text,
        },
        ...prev,
      ].slice(0, MAX_HISTORY))
    } catch (err: unknown) {
      const timeMs = performance.now() - start
      const msg = err instanceof Error ? err.message : String(err)
      setResponse({
        status: 0,
        statusText: 'Network Error',
        body: msg,
        timeMs,
      })
      setHistory(prev => [
        {
          id: Date.now(),
          endpoint: endpoint.path,
          method: endpoint.method,
          status: null,
          statusText: 'Network Error',
          timeMs,
          timestamp: Date.now(),
          responseBody: msg,
        },
        ...prev,
      ].slice(0, MAX_HISTORY))
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="flex h-full overflow-hidden">
      {/* Main area */}
      <div className="flex-1 flex flex-col min-h-0 overflow-y-auto p-6 lg:p-8 space-y-6 max-w-5xl">
        {/* Header */}
        <div>
          <div className="flex items-center gap-3 mb-2">
            <div className="p-2 bg-blue-500/10 rounded-lg">
              <Code size={18} className="text-blue-400" />
            </div>
            <h1 className="text-2xl font-bold text-white">API Playground</h1>
          </div>
          <p className="text-sm text-zinc-400">
            Test Pylos API endpoints directly from the browser.
          </p>
        </div>

        {/* Endpoint selector */}
        <div className="rounded-xl border border-zinc-800/50 bg-zinc-900/30 p-4 space-y-3">
          <label className="text-xs font-semibold text-zinc-500 uppercase tracking-wide">Endpoint</label>
          <div className="relative">
            <select
              value={selectedId}
              onChange={e => setSelectedId(e.target.value)}
              className="w-full appearance-none bg-zinc-900 border border-zinc-800 rounded-lg
                px-3 py-2.5 text-sm text-zinc-100 pr-8
                focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
            >
              {ENDPOINTS.map(ep => (
                <option key={ep.id} value={ep.id}>
                  {ep.method} {ep.path} — {ep.label}
                </option>
              ))}
            </select>
            <ChevronDown size={14} className="absolute right-2 top-1/2 -translate-y-1/2 text-zinc-500 pointer-events-none" />
          </div>

          {/* Method badge + URL */}
          <div className="flex items-center gap-2">
            <span className={`px-2.5 py-1 rounded-md text-[11px] font-bold border ${methodColor(endpoint.method)}`}>
              {endpoint.method}
            </span>
            <code className="text-sm text-zinc-200 font-mono">{endpoint.path}</code>
          </div>
        </div>

        {/* Headers */}
        <div className="rounded-xl border border-zinc-800/50 bg-zinc-900/30 p-4 space-y-3">
          <div className="flex items-center justify-between">
            <label className="text-xs font-semibold text-zinc-500 uppercase tracking-wide">Headers</label>
            <button
              onClick={addHeader}
              className="text-xs text-emerald-400 hover:text-emerald-300 transition-colors"
            >
              + Add
            </button>
          </div>
          <div className="space-y-2">
            {headers.map((h, i) => (
              <div key={i} className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={h.enabled}
                  onChange={e => updateHeader(i, 'enabled', e.target.checked)}
                  className="accent-emerald-500"
                />
                <input
                  type="text"
                  value={h.key}
                  onChange={e => updateHeader(i, 'key', e.target.value)}
                  placeholder="Header name"
                  className="flex-1 bg-zinc-900 border border-zinc-800 rounded-lg px-3 py-1.5 text-xs font-mono
                    text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-emerald-500/50"
                />
                <input
                  type="text"
                  value={h.value}
                  onChange={e => updateHeader(i, 'value', e.target.value)}
                  placeholder="Value"
                  className="flex-[2] bg-zinc-900 border border-zinc-800 rounded-lg px-3 py-1.5 text-xs font-mono
                    text-zinc-200 placeholder-zinc-600 focus:outline-none focus:border-emerald-500/50"
                />
                <button
                  onClick={() => removeHeader(i)}
                  className="p-1 text-zinc-600 hover:text-red-400 transition-colors"
                >
                  <X size={14} />
                </button>
              </div>
            ))}
          </div>
        </div>

        {/* Request body */}
        {endpoint.method === 'POST' && (
          <div className="rounded-xl border border-zinc-800/50 bg-zinc-900/30 p-4 space-y-3">
            <label className="text-xs font-semibold text-zinc-500 uppercase tracking-wide">Request Body</label>
            <textarea
              value={body}
              onChange={e => setBody(e.target.value)}
              rows={10}
              spellCheck={false}
              className="w-full bg-zinc-950 border border-zinc-800 rounded-lg px-4 py-3 text-xs font-mono
                text-zinc-200 placeholder-zinc-600 resize-none
                focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20"
              placeholder='{ "model": "...", "messages": [...] }'
            />
          </div>
        )}

        {/* Send button */}
        <button
          onClick={sendRequest}
          disabled={loading}
          className="flex items-center justify-center gap-2 py-3 rounded-xl
            bg-emerald-600 hover:bg-emerald-500 text-white font-semibold text-sm
            transition-colors disabled:opacity-50 disabled:cursor-not-allowed active:scale-[0.99]"
        >
          {loading ? (
            <>
              <div className="w-4 h-4 border-2 border-white/30 border-t-white rounded-full animate-spin" />
              Sending…
            </>
          ) : (
            <>
              <Send size={16} />
              Send Request
            </>
          )}
        </button>

        {/* Response */}
        {response && (
          <div className="rounded-xl border border-zinc-800/50 bg-zinc-900/30 overflow-hidden space-y-0">
            {/* Response header */}
            <div className="flex items-center justify-between px-4 py-3 border-b border-zinc-800/50 bg-zinc-900/60">
              <div className="flex items-center gap-3">
                <span className="text-xs font-semibold text-zinc-500 uppercase tracking-wide">Response</span>
                <span className={`px-2 py-0.5 rounded text-[11px] font-bold border ${statusColor(response.status)}`}>
                  {response.status || 'ERR'} {response.statusText}
                </span>
                <span className="flex items-center gap-1 text-xs text-zinc-500">
                  <Clock size={11} />
                  {formatMs(response.timeMs)}
                </span>
              </div>
              <div className="flex items-center gap-1">
                <button
                  onClick={() => copyToClipboard(response.body, 'resp')}
                  className="p-1.5 rounded text-zinc-500 hover:text-zinc-300 transition-colors"
                  title="Copy response"
                >
                  {copied === 'resp' ? <Check size={13} className="text-emerald-400" /> : <Copy size={13} />}
                </button>
                <button
                  onClick={() => { setResponse(null) }}
                  className="p-1.5 rounded text-zinc-500 hover:text-zinc-300 transition-colors"
                  title="Clear response"
                >
                  <X size={13} />
                </button>
              </div>
            </div>
            {/* Response body */}
            <div className="p-4 overflow-x-auto max-h-[500px] overflow-y-auto">
              <pre className="text-xs leading-relaxed">
                <code className="text-zinc-300 font-mono whitespace-pre">{tryFormatJson(response.body)}</code>
              </pre>
            </div>
          </div>
        )}
      </div>

      {/* History sidebar */}
      <aside className="hidden lg:flex w-72 shrink-0 border-l border-zinc-800/50 bg-zinc-900/20 flex-col">
        <div className="p-4 border-b border-zinc-800/50">
          <div className="flex items-center justify-between">
            <h2 className="text-xs font-semibold text-zinc-500 uppercase tracking-wide flex items-center gap-1.5">
              <History size={12} />
              History
            </h2>
            {history.length > 0 && (
              <button
                onClick={() => setHistory([])}
                className="text-[11px] text-zinc-600 hover:text-red-400 transition-colors"
              >
                Clear
              </button>
            )}
          </div>
        </div>
        <div className="flex-1 overflow-y-auto p-2 space-y-1">
          {history.length === 0 && (
            <div className="text-xs text-zinc-600 text-center py-8">
              No requests yet
            </div>
          )}
          {history.map(entry => (
            <button
              key={entry.id}
              onClick={() => {
                setResponse({
                  status: entry.status ?? 0,
                  statusText: entry.statusText,
                  body: entry.responseBody,
                  timeMs: entry.timeMs,
                })
              }}
              className="w-full text-left rounded-lg border border-zinc-800/50 bg-zinc-900/40 p-2.5
                hover:border-zinc-700/50 hover:bg-zinc-800/40 transition-colors space-y-1"
            >
              <div className="flex items-center gap-2">
                <span className={`px-1.5 py-0.5 rounded text-[10px] font-bold border ${methodColor(entry.method)}`}>
                  {entry.method}
                </span>
                {entry.status ? (
                  <span className={`px-1.5 py-0.5 rounded text-[10px] font-bold border ${statusColor(entry.status)}`}>
                    {entry.status}
                  </span>
                ) : (
                  <span className="px-1.5 py-0.5 rounded text-[10px] font-bold border bg-red-500/15 text-red-400 border-red-500/30">
                    ERR
                  </span>
                )}
                <span className="text-[10px] text-zinc-600 ml-auto">{formatMs(entry.timeMs)}</span>
              </div>
              <div className="text-[11px] text-zinc-500 font-mono truncate">{entry.endpoint}</div>
              <div className="text-[10px] text-zinc-600">
                {new Date(entry.timestamp).toLocaleTimeString()}
              </div>
            </button>
          ))}
        </div>
      </aside>
    </div>
  )
}
