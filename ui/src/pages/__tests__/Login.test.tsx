import { describe, it, expect, vi, beforeEach } from 'vitest'
import { render, screen, fireEvent, waitFor } from '@testing-library/react'
import { MemoryRouter } from 'react-router-dom'
import Login from '../Login'

vi.mock('../../lib/api', () => ({
  authApi: {
    getConfig: vi.fn().mockResolvedValue({
      google_auth_enabled: false,
      google_client_id: null,
      google_redirect_uri: null,
    }),
    loginWithKey: vi.fn().mockResolvedValue({ token: 'tok' }),
  },
  getErrorMessage: (e: unknown) => String(e),
}))

describe('Login page', () => {
  beforeEach(() => {
    localStorage.clear()
  })

  it('renders accessible admin key input and submit button', async () => {
    render(
      <MemoryRouter>
        <Login />
      </MemoryRouter>,
    )

    const input = await screen.findByLabelText(/Admin Key/i)
    expect(input).toBeInTheDocument()
    expect(input).toHaveAttribute('type', 'password')

    const submit = screen.getByRole('button', { name: /Access with Admin Key/i })
    expect(submit).toBeInTheDocument()

    // Keyboard focus & input
    input.focus()
    expect(document.activeElement).toBe(input)
    fireEvent.change(input, { target: { value: 'sk-admin-test-123' } })
    expect((input as HTMLInputElement).value).toBe('sk-admin-test-123')

    await waitFor(() => {
      expect(screen.getByRole('heading', { name: /Welcome to Pylos/i })).toBeInTheDocument()
    })
  })
})
