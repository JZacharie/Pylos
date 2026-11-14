import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import VirtualKeys from '../VirtualKeys'

vi.mock('../../lib/api', () => ({
  virtualKeysApi: {
    getAll: vi.fn().mockResolvedValue({
      total: 1,
      virtual_keys: [
        {
          id: 'vk-1',
          name: 'team-alpha',
          value: 'sk-pylos-abc123456789',
          key_prefix: 'sk-pylos-abc',
          is_active: true,
          provider_configs: [],
          rate_limit: null,
          budget: null,
          created_at: 1700000000000,
          last_used_at: null,
        },
      ],
    }),
    create: vi.fn(),
    update: vi.fn(),
    remove: vi.fn(),
    rotate: vi.fn(),
    getBudget: vi.fn().mockResolvedValue(null),
  },
  providersApi: {
    getAll: vi.fn().mockResolvedValue({ total: 0, providers: [] }),
  },
  modelsApi: {
    getAll: vi.fn().mockResolvedValue({ data: [] }),
  },
  getErrorMessage: (e: unknown) => String(e),
}))

describe('VirtualKeys page', () => {
  it('renders virtual keys and supports search filtering', async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    render(
      <QueryClientProvider client={qc}>
        <VirtualKeys />
      </QueryClientProvider>,
    )

    expect(await screen.findByText('team-alpha')).toBeInTheDocument()

    const searchInput = screen.getByPlaceholderText(/Search/i)
    searchInput.focus()
    expect(document.activeElement).toBe(searchInput)

    fireEvent.change(searchInput, { target: { value: 'nonexistent-key' } })
    expect(screen.queryByText('team-alpha')).not.toBeInTheDocument()
  })
})
