import { describe, it, expect, vi } from 'vitest'
import { render, screen, fireEvent } from '@testing-library/react'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import Providers from '../Providers'

vi.mock('../../lib/api', () => ({
  providersApi: {
    getAll: vi.fn().mockResolvedValue({
      total: 1,
      providers: [
        {
          name: 'openai',
          keys_count: 1,
          keys: [{ name: 'default', value: '***', models: ['gpt-4o'], weight: 1 }],
          network: {
            base_url: 'https://api.openai.com/v1',
            timeout_seconds: 30,
            max_retries: 3,
          },
        },
      ],
    }),
    listPresets: vi.fn().mockResolvedValue([]),
    create: vi.fn(),
    update: vi.fn(),
    remove: vi.fn(),
  },
  getErrorMessage: (e: unknown) => String(e),
}))

describe('Providers page', () => {
  it('renders provider list and opens add provider modal', async () => {
    const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
    render(
      <QueryClientProvider client={qc}>
        <Providers />
      </QueryClientProvider>,
    )

    expect(await screen.findByText('openai')).toBeInTheDocument()
    const addBtn = screen.getByRole('button', { name: /Add provider/i })
    expect(addBtn).toBeInTheDocument()

    fireEvent.click(addBtn)
    expect(await screen.findByText(/Add a provider/i)).toBeInTheDocument()
  })
})
