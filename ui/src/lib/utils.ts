import { formatDistanceToNow, format } from 'date-fns'

export function formatTimestamp(ms: number): string {
  return format(new Date(ms), 'HH:mm:ss')
}

export function formatRelative(ms: number): string {
  return formatDistanceToNow(new Date(ms), { addSuffix: true })
}

export function formatDate(ms: number): string {
  return format(new Date(ms), 'yyyy-MM-dd HH:mm:ss')
}

export function formatLatency(ms: number): string {
  if (ms < 1000) return `${ms.toFixed(0)}ms`
  return `${(ms / 1000).toFixed(2)}s`
}

export function formatCost(usd: number): string {
  if (usd === 0) return '$0.00'
  if (usd < 0.001) return `$${(usd * 1000000).toFixed(2)}µ`
  if (usd < 0.01) return `$${(usd * 1000).toFixed(2)}m`
  return `$${usd.toFixed(4)}`
}

export function formatNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`
  return n.toString()
}

export function formatPercent(n: number): string {
  return `${n.toFixed(1)}%`
}

export const PROVIDER_COLORS: Record<string, string> = {
  antigravity: '#8b5cf6',
  agy: '#8b5cf6',
  openai: '#10a37f',
  anthropic: '#e06c00',
  openrouter: '#6366f1',
  mistral: '#ff6b6b',
  groq: '#f43f5e',
  deepseek: '#4b8bf4',
  ollama: '#f9f9f9',
  'ollama-jo3': '#f9f9f9',
  lemonade: '#e2b13c',
  'lemonade-jo3': '#e2b13c',
  'lemonade-optimus': '#d4941a',
  unknown: '#6b7280',
}

export function providerColor(provider: string): string {
  return PROVIDER_COLORS[provider.toLowerCase()] ?? PROVIDER_COLORS.unknown
}

export const STATUS_COLORS = {
  success: '#22c55e',
  error: '#ef4444',
}
