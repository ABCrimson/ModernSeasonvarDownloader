import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { clearMocks, mockIPC } from '@tauri-apps/api/mocks'
import { afterEach, expect, test } from 'vitest'
import { render } from 'vitest-browser-react'
import { AppVersion } from './app-version'

afterEach(() => clearMocks())

test('shows the version returned by the app_version command', async () => {
  mockIPC((cmd) => {
    if (cmd === 'app_version') return '9.9.9'
    throw new Error(`unmocked command ${cmd}`)
  })
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  const screen = await render(
    <QueryClientProvider client={qc}>
      <AppVersion />
    </QueryClientProvider>,
  )
  await expect.element(screen.getByText('v9.9.9')).toBeVisible()
})
