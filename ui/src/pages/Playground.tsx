import { useState, useRef, useEffect } from 'react'
import { useQuery } from '@tanstack/react-query'
import { useSearchParams } from 'react-router-dom'
import { api } from '../lib/api'
import { formatLatency, formatCost, providerColor } from '../lib/utils'
import {
  Send, StopCircle, Trash2, ChevronDown,
  Zap, Clock, Hash, Coins, CheckCircle, XCircle,
  SlidersHorizontal, X, Star, RotateCcw,
} from 'lucide-react'

// ─── Types ────────────────────────────────────────────────────────────────────

interface Model {
  id: string
  provider: string
  details?: { family?: string; parameter_size?: string }
  supports_streaming?: boolean
}

interface Message {
  role: 'system' | 'user' | 'assistant'
  content: string
}

interface RunResult {
  model: string
  provider: string
  content: string
  latency_ms: number
  tokens: { prompt: number; completion: number; total: number }
  cost_usd: number
  finish_reason: string
  error?: string
  streaming?: boolean
  actualProvider?: string
  actualModel?: string
}

// ─── API helpers ──────────────────────────────────────────────────────────────

async function fetchModels(): Promise<Model[]> {
  try {
    const r = await api.get('/v1/models')
    const data = r.data?.data
    return Array.isArray(data) ? data : []
  } catch {
    return []
  }
}

// ─── Composants ──────────────────────────────────────────────────────────────

function ModelSelect({
  models, value, onChange, disabled, favorites = [],
}: {
  models: Model[]
  value: string
  onChange: (v: string) => void
  disabled?: boolean
  favorites?: string[]
}) {
  const grouped: Record<string, Model[]> = {}
  if (Array.isArray(models)) {
    for (const m of models) {
      if (!grouped[m.provider]) grouped[m.provider] = []
      grouped[m.provider].push(m)
    }
  }

  return (
    <div className="relative">
      <select
        value={value}
        onChange={e => onChange(e.target.value)}
        disabled={disabled}
        className="w-full appearance-none bg-zinc-900 border border-zinc-800 rounded-lg
          px-3 py-2 text-sm text-zinc-100 pr-8
          focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20
          disabled:opacity-50 disabled:cursor-not-allowed"
      >
        <option value="">— Select a model —</option>
        {Object.entries(grouped).map(([provider, ms]) => (
          <optgroup key={provider} label={`▸ ${provider.toUpperCase()}`}>
            {ms.map(m => {
              const val = `${provider}::${m.id}`
              const isFav = favorites.includes(val)
              return (
                <option key={val} value={val}>
                  {isFav ? '★ ' : ''}{m.id}
                  {m.details?.parameter_size ? ` (${m.details.parameter_size})` : ''}
                </option>
              )
            })}
          </optgroup>
        ))}
      </select>
      <ChevronDown size={14} className="absolute right-2 top-1/2 -translate-y-1/2 text-zinc-500 pointer-events-none" />
    </div>
  )
}

function ChatMessage({ msg, isStreaming }: { msg: Message; isStreaming?: boolean }) {
  const isUser = msg.role === 'user'
  const isSystem = msg.role === 'system'

  if (isSystem) {
    return (
      <div className="rounded-lg border border-dashed border-zinc-700 bg-zinc-800/40 px-4 py-2 text-xs text-zinc-400">
        <span className="font-semibold text-zinc-500 mr-2">SYSTEM</span>
        {msg.content}
      </div>
    )
  }

  return (
    <div className={`flex gap-3 ${isUser ? 'flex-row-reverse' : 'flex-row'}`}>
      <div className={`w-7 h-7 rounded-full flex items-center justify-center flex-shrink-0 text-xs font-bold
        ${isUser ? 'bg-emerald-600 text-white' : 'bg-zinc-700 text-zinc-300'}`}>
        {isUser ? 'U' : 'AI'}
      </div>
      <div className={`max-w-[85%] sm:max-w-[80%] rounded-2xl px-4 py-3 text-sm leading-relaxed
        ${isUser
          ? 'bg-emerald-600 text-white rounded-tr-sm'
          : 'bg-zinc-800/50 border border-zinc-700/50 text-zinc-100 rounded-tl-sm'
        }`}>
        <pre className="whitespace-pre-wrap font-sans break-words text-xs sm:text-sm">
          {msg.content}
          {isStreaming && <span className="inline-block w-1.5 h-4 bg-zinc-400 ml-0.5 animate-pulse" />}
        </pre>
      </div>
    </div>
  )
}

function ResultBadge({ result }: { result: RunResult }) {
  const color = providerColor(result.provider)
  const hasError = !!result.error
  const hasRouting = result.actualProvider && result.actualProvider !== result.provider

  return (
    <div className={`rounded-xl border p-4 space-y-3
      ${hasError ? 'border-red-800/50 bg-red-900/10' : 'border-zinc-800/50 bg-zinc-900/30'}`}>
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <div className="w-2 h-2 rounded-full" style={{ background: color }} />
          <span className="text-xs font-semibold text-zinc-300">{result.provider}</span>
          <span className="text-xs text-zinc-500 font-mono">{result.model}</span>
        </div>
        {hasError
          ? <XCircle size={14} className="text-red-400" />
          : <CheckCircle size={14} className="text-emerald-400" />
        }
      </div>

      {/* Routing info */}
      {hasRouting && (
        <div className="flex items-center gap-2 text-[11px] bg-amber-500/10 border border-amber-500/20 rounded-lg px-2.5 py-1.5">
          <Zap size={11} className="text-amber-400 flex-shrink-0" />
          <span className="text-amber-300 font-medium">Routed</span>
          <span className="text-zinc-500">to</span>
          <span className="text-zinc-300 font-semibold">{result.actualProvider}</span>
          {result.actualModel && result.actualModel !== result.model && (
            <>
              <span className="text-zinc-500">→</span>
              <span className="text-zinc-300 font-mono">{result.actualModel}</span>
            </>
          )}
        </div>
      )}

      {/* Metrics row */}
      {!hasError && (
        <div className="grid grid-cols-4 gap-2 text-xs">
          <Metric icon={<Clock size={10} />} label="Latency" value={formatLatency(result.latency_ms)} />
          <Metric icon={<Hash size={10} />} label="Tokens" value={result.tokens.total.toString()} />
          <Metric icon={<Coins size={10} />} label="Cost" value={formatCost(result.cost_usd)} />
          <Metric icon={<Zap size={10} />} label="Finish" value={result.finish_reason} />
        </div>
      )}

      {/* Error */}
      {hasError && (
        <div className="text-xs text-red-300 font-mono bg-red-950/30 rounded p-2 break-all">
          {result.error}
        </div>
      )}
    </div>
  )
}

function Metric({ icon, label, value }: { icon: React.ReactNode; label: string; value: string }) {
  return (
    <div className="bg-zinc-800/50 rounded-lg px-2 py-1.5 text-center">
      <div className="flex items-center justify-center gap-1 text-zinc-500 mb-0.5">
        {icon}
        <span>{label}</span>
      </div>
      <div className="font-semibold text-zinc-200">{value}</div>
    </div>
  )
}

// ─── Page principale ──────────────────────────────────────────────────────────

export default function Playground() {
  const [searchParams] = useSearchParams()
  const modelParam = searchParams.get('model')
  const [selectedModel, setSelectedModel] = useState(() => {
    return localStorage.getItem('pylos-playground-last-model') || ''
  })
  const [favorites, setFavorites] = useState<string[]>(() => {
    try {
      const saved = localStorage.getItem('pylos-playground-favorites')
      return saved ? JSON.parse(saved) : []
    } catch {
      return []
    }
  })
  const [systemPrompt, setSystemPrompt] = useState(() => {
    return localStorage.getItem('pylos-playground-system-prompt') ?? 'You are a helpful assistant.'
  })
  const [input, setInput] = useState('')
  const [messages, setMessages] = useState<Message[]>(() => {
    try {
      const saved = localStorage.getItem('pylos-playground-messages')
      return saved ? JSON.parse(saved) : []
    } catch {
      return []
    }
  })
  const [streaming, setStreaming] = useState(() => {
    const saved = localStorage.getItem('pylos-playground-streaming')
    return saved !== null ? saved === 'true' : true
  })
  const [temperature, setTemperature] = useState(() => {
    const saved = localStorage.getItem('pylos-playground-temperature')
    return saved !== null ? parseFloat(saved) : 0.7
  })
  const [maxTokens, setMaxTokens] = useState(() => {
    const saved = localStorage.getItem('pylos-playground-max-tokens')
    return saved !== null ? parseInt(saved, 10) : 512
  })
  const [cavemanMode, setCavemanMode] = useState<'off' | 'lite' | 'full' | 'ultra' | 'wenyan'>(() => {
    return (localStorage.getItem('pylos-playground-caveman-mode') as any) || 'off'
  })
  const [cavemanCompress, setCavemanCompress] = useState(() => {
    return localStorage.getItem('pylos-playground-caveman-compress') === 'true'
  })
  const [forceProviderModel, setForceProviderModel] = useState(() => {
    return localStorage.getItem('pylos-playground-force-provider-model') === 'true'
  })
  const [bypassGuardrails, setBypassGuardrails] = useState(() => {
    return localStorage.getItem('pylos-playground-bypass-guardrails') === 'true'
  })
  const [isRunning, setIsRunning] = useState(false)
  const [lastResult, setLastResult] = useState<RunResult | null>(null)
  const [streamingContent, setStreamingContent] = useState('')
  const [showConfig, setShowConfig] = useState(false)
  const abortRef = useRef<AbortController | null>(null)
  const bottomRef = useRef<HTMLDivElement>(null)


  useEffect(() => {
    if (selectedModel) {
      localStorage.setItem('pylos-playground-last-model', selectedModel)
    }
  }, [selectedModel])

  useEffect(() => {
    localStorage.setItem('pylos-playground-favorites', JSON.stringify(favorites))
  }, [favorites])

  useEffect(() => {
    localStorage.setItem('pylos-playground-messages', JSON.stringify(messages))
  }, [messages])

  useEffect(() => {
    localStorage.setItem('pylos-playground-system-prompt', systemPrompt)
  }, [systemPrompt])

  useEffect(() => {
    localStorage.setItem('pylos-playground-streaming', String(streaming))
  }, [streaming])

  useEffect(() => {
    localStorage.setItem('pylos-playground-temperature', String(temperature))
  }, [temperature])

  useEffect(() => {
    localStorage.setItem('pylos-playground-max-tokens', String(maxTokens))
  }, [maxTokens])

  useEffect(() => {
    localStorage.setItem('pylos-playground-caveman-mode', cavemanMode)
  }, [cavemanMode])

  useEffect(() => {
    localStorage.setItem('pylos-playground-caveman-compress', String(cavemanCompress))
  }, [cavemanCompress])

  useEffect(() => {
    localStorage.setItem('pylos-playground-force-provider-model', String(forceProviderModel))
  }, [forceProviderModel])

  useEffect(() => {
    localStorage.setItem('pylos-playground-bypass-guardrails', String(bypassGuardrails))
  }, [bypassGuardrails])
  const toggleFavorite = (modelStr: string) => {
    setFavorites(prev =>
      prev.includes(modelStr)
        ? prev.filter(m => m !== modelStr)
        : [...prev, modelStr]
    )
  }
  const { data: modelsData, isLoading: modelsLoading } = useQuery({
    queryKey: ['models'],
    queryFn: fetchModels,
    staleTime: 60_000,
  })

  const models = modelsData ?? []

  // Sync streaming flag with selected model capabilities
  useEffect(() => {
    if (!models || !selectedModel) return
    const [provider, ...rest] = selectedModel.split('::')
    const modelId = rest.join('::')
    const mdl = models.find(m => m.provider === provider && m.id === modelId)
    if (mdl && typeof mdl.supports_streaming === 'boolean') {
      if (streaming !== mdl.supports_streaming) {
        setStreaming(mdl.supports_streaming)
      }
    }
  }, [selectedModel, models])

  useEffect(() => {
    if (!models.length) return

    if (modelParam) {
      const matched = models.find(
        m => `${m.provider}::${m.id}` === modelParam || `${m.provider}/${m.id}` === modelParam || m.id === modelParam
      )
      if (matched) {
        setSelectedModel(`${matched.provider}::${matched.id}`)
        return
      }
    }

    const savedModel = localStorage.getItem('pylos-playground-last-model')
    if (savedModel) {
      const matched = models.find(m => `${m.provider}::${m.id}` === savedModel)
      if (matched) {
        setSelectedModel(savedModel)
        return
      }
    }

    if (!selectedModel) {
      const ollama = models.find(m => m.provider === 'ollama-jo3' && m.id === 'llama3.1:8b')
        ?? models.find(m => m.provider === 'ollama-jo3')
        ?? models[0]
      if (ollama) setSelectedModel(`${ollama.provider}::${ollama.id}`)
    }
  }, [models, modelParam])

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: 'smooth' })
  }, [messages, streamingContent])

  function parseSelected() {
    const [provider, ...rest] = selectedModel.split('::')
    return { provider, model: rest.join('::') }
  }

  async function run() {
    if (!input.trim() || !selectedModel || isRunning) return

    const { provider, model } = parseSelected()
    const userMsg: Message = { role: 'user', content: input.trim() }
    const newMessages = [...messages, userMsg]
    setMessages(newMessages)
    setInput('')
    setIsRunning(true)
    setStreamingContent('')
    setLastResult(null)

    const start = performance.now()
    abortRef.current = new AbortController()

    const payload = {
      model,
      messages: [
        ...(systemPrompt ? [{ role: 'system', content: systemPrompt }] : []),
        ...newMessages,
      ],
      temperature,
      max_tokens: maxTokens,
      stream: streaming,
    }

    try {
      const headers: Record<string, string> = {
        'Content-Type': 'application/json',
        'Authorization': 'Bearer sk-pylos-admin',
        'X-Pylos-Source': 'playground',
      }
      if (cavemanMode !== 'off') {
        headers['x-caveman-mode'] = cavemanMode
      }
      if (cavemanCompress) {
        headers['x-caveman-compress'] = 'true'
      }
      if (forceProviderModel) {
        headers['x-pylos-force-provider'] = provider
        headers['x-pylos-force-model'] = model
      }
      if (bypassGuardrails) {
        headers['x-bypass-guardrails'] = 'true'
      }

      if (streaming) {
        const resp = await fetch('/v1/chat/completions', {
          method: 'POST',
          headers,
          body: JSON.stringify(payload),
          signal: abortRef.current.signal,
        })

        if (!resp.ok) {
          let errorMsg: string
          try {
            const err = await resp.clone().json()
            errorMsg = err.error?.message ?? `HTTP ${resp.status}`
          } catch {
            const text = await resp.text()
            errorMsg = `HTTP ${resp.status}: ${text.slice(0, 300)}`
          }
          throw new Error(errorMsg)
        }

        const actualProvider = resp.headers.get('X-Pylos-Provider') ?? undefined
        const actualModel = resp.headers.get('X-Pylos-Model') ?? undefined

        const reader = resp.body!.getReader()
        const decoder = new TextDecoder()
        let fullContent = ''
        let finishReason = 'stop'
        let firstTokenTimeout: ReturnType<typeof setTimeout> | undefined
        const firstTokenPromise = new Promise<void>((_, reject) => {
          firstTokenTimeout = setTimeout(() => {
            reject(new Error('Timeout waiting for first token'))
          }, 60000)
        })

        while (true) {
          const readPromise = reader.read()
          const { done, value } = await Promise.race([
            readPromise.then(r => ({ ...r, _race: 'read' as const })),
            firstTokenPromise.then(() => { throw new Error('Timeout waiting for first token') }),
          ]).catch(err => {
            clearTimeout(firstTokenTimeout)
            if (fullContent.trim()) {
              return { done: true as const, value: undefined, _race: 'read' as const }
            }
            throw err
          }) as { done: boolean; value?: Uint8Array; _race: string }
          if (done) break
          clearTimeout(firstTokenTimeout)

          const chunk = decoder.decode(value, { stream: true })
          for (const line of chunk.split('\n')) {
            const data = line.replace(/^data: /, '').trim()
            if (!data || data === '[DONE]') continue
            let streamError: string | null = null
            try {
              const parsed = JSON.parse(data)
              if (parsed.error) {
                streamError = typeof parsed.error === 'object'
                  ? (parsed.error.message ?? JSON.stringify(parsed.error))
                  : parsed.error
              } else {
                const delta = parsed.choices?.[0]?.delta?.content ?? ''
                finishReason = parsed.choices?.[0]?.finish_reason ?? finishReason
                if (delta) {
                  fullContent += delta
                  setStreamingContent(fullContent)
                }
              }
            } catch { /* ignore parse errors */ }
            if (streamError) {
              clearTimeout(firstTokenTimeout)
              throw new Error(streamError)
            }
          }
        }

        clearTimeout(firstTokenTimeout)

        // Validate that we received some content
        if (!fullContent.trim()) {
          throw new Error('Empty response from model')
        }

        const latency = performance.now() - start
        const assistantMsg: Message = { role: 'assistant', content: fullContent }
        setMessages(prev => [...prev, assistantMsg])
        setStreamingContent('')

        const completionTokens = Math.max(1, Math.floor(fullContent.length / 4))
        const promptText = newMessages.map(m => m.content).join(' ')
        const promptTokens = Math.max(1, Math.floor(promptText.length / 4))

        setLastResult({
          model,
          provider,
          content: fullContent,
          latency_ms: latency,
          tokens: { prompt: promptTokens, completion: completionTokens, total: promptTokens + completionTokens },
          cost_usd: 0,
          finish_reason: finishReason,
          streaming: true,
          actualProvider,
          actualModel,
        })
      } else {
        const resp = await fetch('/v1/chat/completions', {
          method: 'POST',
          headers,
          body: JSON.stringify(payload),
          signal: abortRef.current.signal,
        })

        if (!resp.ok) {
          let errorMsg: string
          try {
            const err = await resp.clone().json()
            errorMsg = err.error?.message ?? `HTTP ${resp.status}`
          } catch {
            const text = await resp.text()
            errorMsg = `HTTP ${resp.status}: ${text.slice(0, 300)}`
          }
          throw new Error(errorMsg)
        }

        const latency = performance.now() - start
        const data = await resp.json()
        const choice = data.choices?.[0]
        const usage = data.usage ?? {}
        const content = choice?.message?.content ?? ''

        if (!content.trim()) {
          throw new Error('Empty response from model')
        }

        const actualProvider = resp.headers.get('X-Pylos-Provider') ?? undefined
        const actualModel = resp.headers.get('X-Pylos-Model') ?? undefined

        const assistantMsg: Message = { role: 'assistant', content }
        setMessages(prev => [...prev, assistantMsg])
        setLastResult({
          model,
          provider,
          content,
          latency_ms: latency,
          tokens: {
            prompt: usage.prompt_tokens ?? 0,
            completion: usage.completion_tokens ?? 0,
            total: usage.total_tokens ?? 0,
          },
          cost_usd: 0,
          finish_reason: choice?.finish_reason ?? 'stop',
          actualProvider,
          actualModel,
        })
      }
    } catch (err: unknown) {
      if ((err as Error)?.name === 'AbortError') return
      const msg = err instanceof Error ? err.message : String(err)
      setMessages(prev => [
        ...prev,
        { role: 'assistant', content: `Error: ${msg}` },
      ])
      setLastResult({
        model,
        provider,
        content: '',
        latency_ms: performance.now() - start,
        tokens: { prompt: 0, completion: 0, total: 0 },
        cost_usd: 0,
        finish_reason: 'error',
        error: msg,
      })
    } finally {
      setIsRunning(false)
      setStreamingContent('')
    }
  }

  function stop() {
    abortRef.current?.abort()
    setIsRunning(false)
    setStreamingContent('')
  }

  function clear() {
    setMessages([])
    setLastResult(null)
    setStreamingContent('')
  }

  function reset() {
    setMessages([])
    setLastResult(null)
    setStreamingContent('')
    setSystemPrompt('You are a helpful assistant.')
    setTemperature(0.7)
    setMaxTokens(512)
    setStreaming(true)
    setCavemanMode('off')
    setCavemanCompress(false)
    setForceProviderModel(false)

    localStorage.removeItem('pylos-playground-messages')
    localStorage.removeItem('pylos-playground-system-prompt')
    localStorage.removeItem('pylos-playground-streaming')
    localStorage.removeItem('pylos-playground-temperature')
    localStorage.removeItem('pylos-playground-max-tokens')
    localStorage.removeItem('pylos-playground-caveman-mode')
    localStorage.removeItem('pylos-playground-caveman-compress')
    localStorage.removeItem('pylos-playground-force-provider-model')
  }

  const { provider: currentProvider } = selectedModel ? parseSelected() : { provider: '' }

  return (
    <div className="flex h-full overflow-hidden relative">

      {/* Backdrop overlay for config drawer on mobile */}
      {showConfig && (
        <div
          className="fixed inset-0 z-30 bg-black/60 backdrop-blur-sm md:hidden"
          onClick={() => setShowConfig(false)}
        />
      )}

      {/* ── Config sidebar ─────────────────────────────────────── */}
      <aside
        className={`fixed inset-y-0 right-0 z-40 w-64 bg-zinc-950 border-l border-zinc-800/80 flex flex-col overflow-y-auto transition-transform duration-300 md:duration-200
          md:static md:translate-x-0
          ${showConfig ? 'translate-x-0' : 'translate-x-full'}
        `}
      >
        <div className="p-4 space-y-4">
          <div className="flex items-center justify-between">
            <h2 className="text-xs font-semibold text-zinc-500 uppercase tracking-wide">Configuration</h2>
            <button
              onClick={() => setShowConfig(false)}
              className="md:hidden text-zinc-400 hover:text-white p-1 rounded-lg hover:bg-zinc-800/50"
            >
              <X size={16} />
            </button>
          </div>

          {/* Model selector */}
          <div className="space-y-1.5">
            <label className="text-xs text-zinc-400">Model</label>
            <div className="flex gap-2">
              <div className="flex-1 min-w-0">
                {modelsLoading
                  ? <div className="h-9 bg-zinc-800 rounded-lg animate-pulse" />
                  : <ModelSelect
                      models={models}
                      value={selectedModel}
                      onChange={setSelectedModel}
                      disabled={isRunning}
                      favorites={favorites}
                    />
                }
              </div>
              {selectedModel && (
                <button
                  onClick={() => toggleFavorite(selectedModel)}
                  disabled={isRunning}
                  className={`p-2 rounded-lg border flex items-center justify-center flex-shrink-0 transition-all active:scale-95
                    ${favorites.includes(selectedModel)
                      ? 'bg-amber-500/10 border-amber-500/50 text-amber-400 hover:bg-amber-500/20'
                      : 'bg-zinc-900 border-zinc-800 text-zinc-400 hover:text-zinc-200 hover:border-zinc-700'
                    }`}
                  title={favorites.includes(selectedModel) ? 'Remove from favorites' : 'Add to favorites'}
                >
                  <Star size={16} fill={favorites.includes(selectedModel) ? 'currentColor' : 'none'} />
                </button>
              )}
            </div>

            {/* Quick Switch Favorites */}
            {favorites.length > 0 && (
              <div className="pt-1 space-y-1">
                <div className="text-[10px] font-semibold text-zinc-500 uppercase tracking-wider flex items-center gap-1">
                  <Star size={10} fill="currentColor" className="text-amber-500" />
                  <span>Favorite Models</span>
                </div>
                <div className="flex flex-wrap gap-1">
                  {favorites.map(fav => {
                    const [prov, ...restId] = fav.split('::')
                    const name = restId.join('::')
                    const isSelected = selectedModel === fav
                    return (
                      <button
                        key={fav}
                        onClick={() => setSelectedModel(fav)}
                        disabled={isRunning}
                        title={`${name} (${prov})`}
                        className={`text-xs px-2 py-1 rounded-md transition-all text-left truncate max-w-full flex items-center gap-1.5 border active:scale-[0.97]
                          ${isSelected
                            ? 'bg-emerald-600/10 border-emerald-500/40 text-emerald-400 font-medium'
                            : 'bg-zinc-900/40 border-zinc-800/80 text-zinc-400 hover:text-zinc-200 hover:bg-zinc-800/60'
                          }`}
                      >
                        <div className="w-1.5 h-1.5 rounded-full flex-shrink-0" style={{ background: providerColor(prov) }} />
                        <span className="truncate">{name}</span>
                      </button>
                    )
                  })}
                </div>
              </div>
            )}

            {currentProvider && (
              <div className="flex items-center gap-1.5 text-xs text-zinc-500 pt-1">
                <div className="w-1.5 h-1.5 rounded-full" style={{ background: providerColor(currentProvider) }} />
                {currentProvider}
              </div>
            )}
          </div>

          <div className="border-t border-zinc-800/50" />

          {/* System prompt */}
          <div className="space-y-1.5">
            <label className="text-xs text-zinc-400">System Prompt</label>
            <textarea
              value={systemPrompt}
              onChange={e => setSystemPrompt(e.target.value)}
              rows={3}
              disabled={isRunning}
              className="w-full bg-zinc-900 border border-zinc-800 rounded-lg px-3 py-2 text-sm
                text-zinc-200 resize-none focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20
                disabled:opacity-50"
            />
          </div>

          <div className="border-t border-zinc-800/50" />

          {/* Temperature */}
          <div className="space-y-1.5">
            <div className="flex justify-between">
              <label className="text-xs text-zinc-400">Temperature</label>
              <span className="text-xs text-zinc-300 font-mono">{temperature.toFixed(1)}</span>
            </div>
            <input
              type="range" min="0" max="2" step="0.1"
              value={temperature}
              onChange={e => setTemperature(parseFloat(e.target.value))}
              disabled={isRunning}
              className="w-full accent-emerald-500"
            />
          </div>

          {/* Max tokens */}
          <div className="space-y-1.5">
            <div className="flex justify-between">
              <label className="text-xs text-zinc-400">Max Tokens</label>
              <span className="text-xs text-zinc-300 font-mono">{maxTokens}</span>
            </div>
            <input
              type="range" min="64" max="4096" step="64"
              value={maxTokens}
              onChange={e => setMaxTokens(parseInt(e.target.value))}
              disabled={isRunning}
              className="w-full accent-emerald-500"
            />
          </div>

          {/* Streaming toggle */}
          <div className="flex items-center justify-between">
            <label className="text-xs text-zinc-400">Streaming</label>
            <button
              onClick={() => setStreaming(!streaming)}
              disabled={isRunning}
              className={`relative w-10 h-5 rounded-full transition-colors
                ${streaming ? 'bg-emerald-600' : 'bg-zinc-700'}`}
            >
              <div className={`absolute top-0.5 w-4 h-4 bg-white rounded-full shadow transition-transform
                ${streaming ? 'translate-x-5' : 'translate-x-0.5'}`} />
            </button>
          </div>

          <div className="border-t border-zinc-800/50" />

          {/* Caveman Mode */}
          <div className="space-y-1.5">
            <label className="text-xs text-zinc-400">Caveman Output Mode</label>
            <select
              value={cavemanMode}
              onChange={e => setCavemanMode(e.target.value as any)}
              disabled={isRunning}
              className="w-full bg-zinc-900 border border-zinc-800 rounded-lg px-3 py-2 text-sm text-zinc-100 focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20 disabled:opacity-50"
            >
              <option value="off">Off (Normal)</option>
              <option value="lite">Lite (Direct)</option>
              <option value="full">Full (Classic)</option>
              <option value="ultra">Ultra (Terse)</option>
              <option value="wenyan">Wenyan (Classical)</option>
            </select>
          </div>

          {/* Input Shrinking */}
          <div className="flex items-center justify-between">
            <div className="flex flex-col">
              <span className="text-xs text-zinc-400">Input Shrinking</span>
              <span className="text-[10px] text-zinc-500">Strip prompt fillers</span>
            </div>
            <button
              onClick={() => setCavemanCompress(!cavemanCompress)}
              disabled={isRunning}
              className={`relative w-10 h-5 rounded-full transition-colors
                ${cavemanCompress ? 'bg-emerald-600' : 'bg-zinc-700'}`}
            >
              <div className={`absolute top-0.5 w-4 h-4 bg-white rounded-full shadow transition-transform
                ${cavemanCompress ? 'translate-x-5' : 'translate-x-0.5'}`} />
            </button>
          </div>

          {/* Force Provider/Model */}
          <div className="flex items-center justify-between">
            <div className="flex flex-col">
              <span className="text-xs text-zinc-400">Force Provider/Model</span>
              <span className="text-[10px] text-zinc-500">Bypass routing rules</span>
            </div>
            <button
              onClick={() => setForceProviderModel(!forceProviderModel)}
              disabled={isRunning}
              className={`relative w-10 h-5 rounded-full transition-colors
                ${forceProviderModel ? 'bg-emerald-600' : 'bg-zinc-700'}`}
            >
              <div className={`absolute top-0.5 w-4 h-4 bg-white rounded-full shadow transition-transform
                ${forceProviderModel ? 'translate-x-5' : 'translate-x-0.5'}`} />
            </button>
          </div>

          {/* Bypass Guardrails */}
          <div className="flex items-center justify-between">
            <div className="flex flex-col">
              <span className="text-xs text-zinc-400">Bypass Guardrails</span>
              <span className="text-[10px] text-zinc-500">Disable PII/secret filters</span>
            </div>
            <button
              onClick={() => setBypassGuardrails(!bypassGuardrails)}
              disabled={isRunning}
              className={`relative w-10 h-5 rounded-full transition-colors
                ${bypassGuardrails ? 'bg-emerald-600' : 'bg-zinc-700'}`}
            >
              <div className={`absolute top-0.5 w-4 h-4 bg-white rounded-full shadow transition-transform
                ${bypassGuardrails ? 'translate-x-5' : 'translate-x-0.5'}`} />
            </button>
          </div>

          <div className="border-t border-zinc-800/50" />

          {/* Action buttons */}
          <div className="flex gap-2">
            <button
              onClick={clear}
              disabled={isRunning || messages.length === 0}
              className="flex-1 flex items-center justify-center gap-1.5 py-2 rounded-lg
                border border-zinc-800 text-zinc-400 text-xs hover:border-red-700/50 hover:text-red-400
                transition-colors disabled:opacity-40"
              title="Clear chat history only"
            >
              <Trash2 size={13} />
              Clear
            </button>
            <button
              onClick={reset}
              disabled={isRunning}
              className="flex-1 flex items-center justify-center gap-1.5 py-2 rounded-lg
                border border-zinc-800 text-zinc-400 text-xs hover:border-amber-700/50 hover:text-amber-400
                transition-colors disabled:opacity-40"
              title="Reset chat and configuration to defaults"
            >
              <RotateCcw size={13} />
              Reset
            </button>
          </div>
        </div>

        {/* Result badge */}
        {lastResult && (
          <div className="mt-auto p-4 border-t border-zinc-800/50">
            <h3 className="text-xs font-semibold text-zinc-500 uppercase tracking-wide mb-2">Last run</h3>
            <ResultBadge result={lastResult} />
          </div>
        )}
      </aside>

      {/* ── Conversation panel ────────────────────────────────── */}
      <div className="flex-1 flex flex-col min-h-0 bg-zinc-950">
        {/* Mobile Header Toolbar */}
        <div className="flex md:hidden items-center justify-between border-b border-zinc-800/50 px-4 py-2 bg-zinc-900/50">
          <span className="text-xs font-semibold text-zinc-400 uppercase tracking-wider">Playground</span>
          <div className="flex items-center gap-2">
            <button
              onClick={() => setShowConfig(true)}
              className="p-1.5 rounded-lg text-zinc-400 hover:text-white hover:bg-zinc-800 transition-colors"
              title="Configuration"
            >
              <SlidersHorizontal size={18} />
            </button>
            <button
              onClick={clear}
              disabled={isRunning || messages.length === 0}
              className="p-1.5 rounded-lg text-zinc-400 hover:text-red-400 hover:bg-zinc-800 transition-colors disabled:opacity-40"
              title="Clear conversation"
            >
              <Trash2 size={18} />
            </button>
            <button
              onClick={reset}
              disabled={isRunning}
              className="p-1.5 rounded-lg text-zinc-400 hover:text-amber-400 hover:bg-zinc-800 transition-colors disabled:opacity-40"
              title="Reset conversation and config"
            >
              <RotateCcw size={18} />
            </button>
          </div>
        </div>

        {/* Messages */}
        <div className="flex-1 overflow-y-auto p-4 sm:p-5 space-y-4">
          {messages.length === 0 && !isRunning && (
            <div className="h-full flex flex-col items-center justify-center text-zinc-600 gap-3">
              <div className="w-12 h-12 rounded-2xl bg-zinc-900 border border-zinc-800/50 flex items-center justify-center">
                <Zap size={20} className="text-emerald-500" />
              </div>
                <div className="text-center">
                <div className="font-medium text-zinc-400">Pylos Playground</div>
                <div className="text-sm mt-1">
                  Select a model and send a message
                </div>
              </div>
              {models.length > 0 && (
                <div className="text-xs text-zinc-700 mt-2">
                  {models.filter(m => m.provider === 'ollama-jo3').length} local Ollama models available
                </div>
              )}
            </div>
          )}

          {messages.map((msg, i) => (
            <ChatMessage
              key={i}
              msg={msg}
              isStreaming={isRunning && i === messages.length - 1 && msg.role === 'assistant'}
            />
          ))}

          {/* Streaming assistant bubble */}
          {isRunning && streamingContent && (
            <ChatMessage
              msg={{ role: 'assistant', content: streamingContent }}
              isStreaming
            />
          )}

          {/* Thinking indicator */}
          {isRunning && !streamingContent && (
            <div className="flex gap-3">
              <div className="w-7 h-7 rounded-full bg-zinc-700 flex items-center justify-center text-xs font-bold text-zinc-300">
                AI
              </div>
              <div className="bg-zinc-800/50 border border-zinc-700/50 rounded-2xl rounded-tl-sm px-4 py-3">
                <div className="flex gap-1.5">
                  {[0, 1, 2].map(i => (
                    <div key={i}
                      className="w-2 h-2 rounded-full bg-zinc-500 animate-bounce"
                      style={{ animationDelay: `${i * 0.15}s` }}
                    />
                  ))}
                </div>
              </div>
            </div>
          )}

          <div ref={bottomRef} />
        </div>

        {/* Input */}
        <div className="border-t border-zinc-800/50 p-4 bg-zinc-950">
          <div className="flex gap-3 items-end">
            <textarea
              value={input}
              onChange={e => setInput(e.target.value)}
              onKeyDown={e => {
                if (e.key === 'Enter' && !e.shiftKey) {
                  e.preventDefault()
                  run()
                }
              }}
              placeholder="Type a message… (Enter to send, Shift+Enter for newline)"
              rows={2}
              disabled={isRunning || !selectedModel}
              className="flex-1 bg-zinc-900 border border-zinc-800 rounded-xl px-4 py-3 text-sm
                text-zinc-100 placeholder-zinc-600 resize-none
                focus:outline-none focus:border-emerald-500/50 focus:ring-1 focus:ring-emerald-500/20
                disabled:opacity-50 disabled:cursor-not-allowed"
            />
            {isRunning ? (
              <button
                onClick={stop}
                className="p-3 rounded-xl bg-red-600 hover:bg-red-500 text-white transition-colors flex-shrink-0 active:scale-[0.98]"
              >
                <StopCircle size={18} />
              </button>
            ) : (
              <button
                onClick={run}
                disabled={!input.trim() || !selectedModel}
                className="p-3 rounded-xl bg-emerald-600 hover:bg-emerald-500 text-white transition-colors flex-shrink-0
                  disabled:opacity-40 disabled:cursor-not-allowed active:scale-[0.98]"
              >
                <Send size={18} />
              </button>
            )}
          </div>
          {selectedModel && (
            <div className="text-xs text-zinc-600 mt-2 ml-1">
              {parseSelected().model} via {parseSelected().provider}
              {streaming ? ' · streaming' : ' · blocking'}
              · temp {temperature} · max {maxTokens} tokens
            </div>
          )}
        </div>
      </div>
    </div>
  )
}
